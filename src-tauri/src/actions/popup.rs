// src-tauri/src/actions/popup.rs
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder, Manager, Emitter};
use serde_json::json;

#[tauri::command]
pub async fn show_popup_window(
    app: AppHandle,
    content: String,
    content_type: String
) -> Result<(), String> {
    let content_chars = content.chars().count();
    let content_lines = content.lines().count().max(1);

    let action_count = match content_type.as_str() {
        "Code" => 5,
        "Url" => 4,
        "PlainText" => if content_chars > 100 { 6 } else { 4 },
        _ => 3,
    };

    let fixed_width = 350.0;

    // Enhanced window size calculation to accommodate AI suggestions
    let drag_handle_height = 30.0;  // Drag area
    let header_height = 50.0;       // Header section
    let content_height = (content_lines as f64 * 20.0).clamp(60.0, 120.0); // Content area
    let suggestions_title_height = 25.0; // Title section
    
    // Calculate actions height with buffer for AI suggestions
    let estimated_total_actions = action_count + 3; // Basic + up to 3 AI suggestions
    let actions_height = estimated_total_actions as f64 * 45.0; 
    
    let footer_height = 20.0;       // Smaller footer
    let padding = 15.0;             // Overall padding
    
    let dynamic_height = drag_handle_height + header_height + content_height + 
                        suggestions_title_height + actions_height + footer_height + padding;

    // Force close existing popup
    if let Some(existing) = app.get_webview_window("popup") {
        println!("Closing existing popup window");
        let _ = existing.destroy();
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }

    match WebviewWindowBuilder::new(
        &app,
        "popup",
        WebviewUrl::App("popup.html".into())
    )
    .inner_size(fixed_width, dynamic_height)
    .resizable(false)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .position(900.0, 100.0) // Position on right side instead of center
    .title("ClipMind Popup")
    .visible(true)
    .initialization_script(&format!(r#"
        console.log('Popup initialization script executing');
        window.addEventListener('beforeunload', function() {{
            console.log('Popup window about to close');
        }});
        function onReady() {{
            document.documentElement.style.background = 'transparent';
            document.body.style.background = 'transparent';
            document.body.style.margin = '0';
            document.body.style.padding = '0';
            window.clipboardData = {{
                content: `{}`,
                contentType: `{}`,
                actionCount: {},
                windowSize: {{ width: {}, height: {} }}
            }};
            window.dispatchEvent(new CustomEvent('tauriReady'));
        }}
        if (document.readyState === 'loading') {{
            document.addEventListener('DOMContentLoaded', onReady);
        }} else {{
            setTimeout(onReady, 50);
        }}
    "#,
        content.replace('`', r#"\`"#),
        content_type,
        action_count,
        fixed_width,
        dynamic_height
    ))
    .build()
    {
        Ok(_) => {
            println!("Popup created, size: {:.0}x{:.0}", fixed_width, dynamic_height);
            Ok(())
        },
        Err(e) => Err(format!("Failed to create popup: {}", e))
    }
}

#[tauri::command]
pub async fn close_popup(app: AppHandle) -> Result<(), String> {
    println!("Executing close popup command");
    
    if let Some(popup) = app.get_webview_window("popup") {
        println!("Destroying popup window");
        popup.destroy().map_err(|e| {
            println!("Failed to destroy window: {}", e);
            e.to_string()
        })?;
        println!("Popup window has been closed");
    } else {
        println!("Popup window not found");
    }
    
    Ok(())
}

// Dynamic window resize command
#[tauri::command]
pub async fn resize_popup_to_content(
    app: AppHandle,
    content_height: f64,
    action_count: i32
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("popup") {
        // Use the same calculation as show_popup_window for consistency
        let drag_handle_height = 30.0;
        let header_height = 50.0;
        let suggestions_title_height = 25.0;
        let actions_height = action_count as f64 * 45.0;
        let footer_height = 20.0;
        let padding = 15.0;
        
        let new_height = drag_handle_height + header_height + content_height + 
                        suggestions_title_height + actions_height + footer_height + padding;
        
        if let Ok(current_size) = window.inner_size() {
            let new_width = current_size.width as f64;
            
            window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: new_width as u32,
                height: new_height as u32,
            })).map_err(|e| e.to_string())?;
            
            println!("Window resized to: {}x{}", new_width, new_height);
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn run_action(action_id: String, content: Option<String>) -> Result<String, String> {
    println!("Executing action: {} with content: {:?}", action_id, content);
    
    match action_id.as_str() {
        "search" => {
            if let Some(content) = content {
                let encoded_content = content
                    .replace(" ", "+")
                    .replace("&", "%26")
                    .replace("=", "%3D")
                    .replace("?", "%3F");
                
                let search_url = format!("https://www.google.com/search?q={}", encoded_content);
                
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", &search_url])
                        .spawn();
                }
                
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open")
                        .arg(&search_url)
                        .spawn();
                }
                
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&search_url)
                        .spawn();
                }
                
                Ok(format!("Open Google search: {}", content))
            } else {
                Err("no search context".to_string())
            }
        },
        "translate" => {
            Ok("Translation feature triggered".to_string())
        },
        "summarize" => {
            Ok("Summarization feature triggered".to_string())
        },
        "open_browser" => {
            if let Some(content) = content {
                let url = if content.starts_with("http://") || content.starts_with("https://") {
                    content
                } else {
                    format!("http://{}", content)
                };
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &url])
                        .spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open")
                        .arg(&url)
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&url)
                        .spawn();
                }
                Ok(format!("Opened in browser: {}", url))
            } else {
                Err("No URL provided".to_string())
            }
        },
        "open_vscode" => {
            if let Some(content) = content {
                use std::fs;
                use std::io::Write;
                let file_path = "clipmind_temp.txt";
                if let Ok(mut file) = fs::File::create(file_path) {
                    let _ = file.write_all(content.as_bytes());
                }

                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "code", file_path])
                        .spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open")
                        .args(["-a", "Visual Studio Code", file_path])
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("code")
                        .arg(file_path)
                        .spawn();
                }
                Ok(format!("Use VSCode to open: {}", file_path))
            } else {
                Err("no context".to_string())
            }
        },
        "compose_email" => {
            if let Some(content) = content {
                #[cfg(target_os = "windows")]
                {
                    let mailto = format!("mailto:{}", content);
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", &mailto])
                        .spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let mailto = format!("mailto:{}", content);
                    let _ = std::process::Command::new("open")
                        .arg(&mailto)
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let mailto = format!("mailto:{}", content);
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&mailto)
                        .spawn();
                }
                Ok(format!("Write email to: {}", content))
            } else {
                Err("no email address".to_string())
            }
        },
        "open_maps" => {
            if let Some(content) = content {
                let url = format!("https://www.google.com/maps/search/{}", urlencoding::encode(&content));
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &url])
                        .spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open")
                        .arg(&url)
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&url)
                        .spawn();
                }
                Ok(format!("open google map: {}", content))
            } else {
                Err("no address".to_string())
            }
        },
        "save_text" => {
            if let Some(content) = content {
                use std::fs;
                let file_path = "clipmind_saved_text.txt";
                if let Ok(_) = fs::write(file_path, &content) {
                    Ok(format!("save file: {}", file_path))
                } else {
                    Err("save failed".to_string())
                }
            } else {
                Err("no context".to_string())
            }
        },
        _ => {
            println!("Unimplemented action: {}", action_id);
            Ok(format!("Action '{}' triggered but not yet implemented", action_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search() {
        let result = run_action("search".to_string(), Some("rust".to_string())).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Google search"));
    }

    #[tokio::test]
    async fn test_open_browser() {
        let result = run_action("open_browser".to_string(), Some("example.com".to_string())).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("browser"));
    }

    #[tokio::test]
    async fn test_open_vscode() {
        let result = run_action("open_vscode".to_string(), Some("test content".to_string())).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("VSCode"));
    }

    #[tokio::test]
    async fn test_save_text() {
        let result = run_action("save_text".to_string(), Some("test".to_string())).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("save file"));
    }
}