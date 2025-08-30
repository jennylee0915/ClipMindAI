// src-tauri/src/lib.rs 
pub mod clipboard;
pub mod actions;
pub mod analyzer;

use clipboard::monitor::{ClipboardMonitor, ClipboardChange};
use clipboard::content_detector::ContentDetector;
use std::sync::{Arc, Mutex};
use log::{info, warn, error};
use serde::{Serialize, Deserialize};
use std::collections::VecDeque;
use tokio::sync::broadcast;
use tauri::{AppHandle, Manager};

// Import from modules
use actions::popup::{show_popup_window, run_action, close_popup, resize_popup_to_content};
use analyzer::content_analyzer::ContentAnalyzer;
use clipboard::types::CompleteAnalysis;

// Structures needed for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    pub content: String,
    pub content_type: String,
    pub timestamp: String,
    pub content_length: usize,
    pub content_preview: String,
}

// Global state
static mut CLIPBOARD_HISTORY: Option<Arc<Mutex<VecDeque<ClipboardItem>>>> = None;
static mut CLIPBOARD_MONITOR: Option<Arc<Mutex<ClipboardMonitor>>> = None;
static mut IS_RUNNING: Option<Arc<Mutex<bool>>> = None;
static mut CONTENT_ANALYZER: Option<Arc<ContentAnalyzer>> = None;

const MAX_HISTORY_SIZE: usize = 100;

// Safe string truncate function
fn safe_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    
    if end == 0 {
        return String::new();
    }
    
    format!("{}...", &s[..end])
}

// Initialize global state
fn init_global_state() {
    unsafe {
        if CLIPBOARD_HISTORY.is_none() {
            CLIPBOARD_HISTORY = Some(Arc::new(Mutex::new(VecDeque::new())));
        }
        if IS_RUNNING.is_none() {
            IS_RUNNING = Some(Arc::new(Mutex::new(false)));
        }
        if CONTENT_ANALYZER.is_none() {
            CONTENT_ANALYZER = Some(Arc::new(ContentAnalyzer::new()));
        }
    }
}

// Frontend API commands
#[tauri::command]
async fn start_clipboard_monitoring(app: AppHandle) -> Result<String, String> {
    init_global_state();
    
    unsafe {
        // Check if it is already running
        if let Some(ref is_running) = IS_RUNNING {
            let running = is_running.lock().unwrap();
            if *running {
                return Ok("Clipboard monitoring is already running".to_string());
            }
        }
        
        // Test AI engine connection
        if let Some(ref analyzer) = CONTENT_ANALYZER {
            let ai_connected = analyzer.test_ai_connection().await;
            if ai_connected {
                info!("AI engine is ready");
            } else {
                warn!("AI engine connection failed, only rule engine will be used");
            }
        }
        
        // Create a new monitor
        let mut monitor = ClipboardMonitor::new(None)
            .map_err(|e| format!("Failed to create monitor: {}", e))?;
        
        // Start monitoring
        let event_receiver = monitor.start_monitoring().await
            .map_err(|e| format!("Failed to start monitoring: {}", e))?;
        
        info!("Monitor started");
        
        // Save monitor
        CLIPBOARD_MONITOR = Some(Arc::new(Mutex::new(monitor)));
        
        // Set running state
        if let Some(ref is_running) = IS_RUNNING {
            let mut running = is_running.lock().unwrap();
            *running = true;
        }
        
        // Start event processing task
        start_event_processing(event_receiver, app).await;
        
        Ok("Monitoring with AI enabled...".to_string())
    }
}

#[tauri::command]
async fn stop_clipboard_monitoring() -> Result<String, String> {
    unsafe {
        // Set stopped state
        if let Some(ref is_running) = IS_RUNNING {
            let mut running = is_running.lock().unwrap();
            *running = false;
        }
        
        // Stop monitor
        if let Some(ref monitor_arc) = CLIPBOARD_MONITOR {
            let mut monitor = monitor_arc.lock().unwrap();
            
            if let Err(e) = monitor.stop_monitoring_sync() {
                return Err(format!("Failed to stop monitoring: {}", e));
            }
        }
        
        // Clean up state
        CLIPBOARD_MONITOR = None;
        
        Ok("Stop monitoring...".to_string())
    }
}

// Start event processing
async fn start_event_processing(mut event_receiver: broadcast::Receiver<ClipboardChange>, app: AppHandle) {
    unsafe {
        if let Some(ref is_running_arc) = IS_RUNNING {
            let is_running_arc = Arc::clone(is_running_arc);
            
            tokio::spawn(async move {
                info!("Clipboard event processor started (AI enhanced)");
                
                loop {
                    // Check if it should stop
                    {
                        let running = is_running_arc.lock().unwrap();
                        if !*running {
                            info!("Received stop signal, ending event processing");
                            break;
                        }
                    }
                    
                    // Receive event
                    match event_receiver.recv().await {
                        Ok(change) => {
                            handle_clipboard_change_with_ai(change, app.clone()).await;
                        },
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Event channel closed");
                            break;
                        },
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            info!("Event processing delayed, skipped {} events", count);
                            continue;
                        }
                    }
                }
                
                info!("Clipboard event processor stopped");
            });
        }
    }
}

// Handle clipboard change (AI enhanced - using original popup)
async fn handle_clipboard_change_with_ai(change: ClipboardChange, app: AppHandle) {
    let content_preview = safe_truncate(&change.event.content, 50);
    
    info!(
        "Clipboard change - delay: {}ms, type: {:?}, length: {}, preview: '{}'",
        change.source_detection_time_ms,
        change.event.content_type,
        change.event.content_length,
        content_preview
    );
    
    // Add to history
    let clipboard_item = ClipboardItem {
        id: uuid::Uuid::new_v4().to_string(),
        content: change.event.content.clone(),
        content_type: format!("{:?}", change.event.content_type),
        timestamp: change.event.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
        content_length: change.event.content_length,
        content_preview: if change.event.content.len() > 100 {
            safe_truncate(&change.event.content, 100)
        } else {
            change.event.content.clone()
        },
    };
    
    unsafe {
        if let Some(ref history) = CLIPBOARD_HISTORY {
            let mut history_guard = history.lock().unwrap();
            history_guard.push_front(clipboard_item);
            
            if history_guard.len() > MAX_HISTORY_SIZE {
                history_guard.pop_back();
            }
        }
    }
    
    
    // Use show_popup_window but pass AI analysis result
    if let Err(e) = show_popup_window(
        app,
        change.event.content,
        format!("{:?}", change.event.content_type)
    ).await {
        warn!("Failed to show popup: {}", e);
    }
}


#[tauri::command]
async fn get_ai_suggestions(
    content: String,
    content_type: String,
) -> Result<Vec<serde_json::Value>, String> {
    info!("Start real AI analysis, type: {}", content_type);
    
    init_global_state();
    
    unsafe {
        if let Some(ref analyzer) = CONTENT_ANALYZER {
            // Parse content type
            let parsed_type = match content_type.as_str() {
                "Url" => clipboard::types::BasicContentType::Url,
                "Email" => clipboard::types::BasicContentType::Email,
                "Phone" => clipboard::types::BasicContentType::Phone,
                "Financial" => clipboard::types::BasicContentType::Financial,
                "Code" => clipboard::types::BasicContentType::Code,
                "Address" => clipboard::types::BasicContentType::Address,
                "DateTime" => clipboard::types::BasicContentType::DateTime,
                _ => clipboard::types::BasicContentType::PlainText,
            };
            
            // Directly call ai_engine's predict_intent method
            match analyzer.ai_engine.predict_intent(&content, &parsed_type).await {
                Ok(predictions) => {
                    // Convert to frontend format
                    let ai_suggestions: Vec<serde_json::Value> = predictions
                        .iter()
                        .enumerate()
                        .map(|(index, action)| serde_json::json!({
                            "id": action.action_id,
                            "label": action.label,
                            "icon": action.icon,
                            "hotkey": (index + 4).to_string(), // start from 4th
                            "source": "ai",
                            "reason": action.reason,
                            "confidence": action.confidence
                        }))
                        .collect();
                    
                    info!("Real AI suggestions generated: {} suggestions", ai_suggestions.len());
                    Ok(ai_suggestions)
                },
                Err(e) => {
                    warn!("AI analysis failed: {}", e);
                    
                    // Provide smart fallback suggestions
                    let fallback_suggestions = match content_type.as_str() {
                        "Url" => vec![
                            serde_json::json!({
                                "id": "ai_summarize_webpage",
                                "label": "AI Summarize Webpage",
                                "icon": "📖",
                                "hotkey": "4",
                                "source": "ai",
                                "reason": "Fallback smart suggestion",
                                "confidence": 0.6
                            })
                        ],
                        "Code" => vec![
                            serde_json::json!({
                                "id": "ai_explain_code",
                                "label": "AI Explain Code", 
                                "icon": "💡",
                                "hotkey": "4",
                                "source": "ai",
                                "reason": "Fallback smart suggestion",
                                "confidence": 0.7
                            })
                        ],
                        _ => vec![
                            serde_json::json!({
                                "id": "ai_translate",
                                "label": "AI Translate",
                                "icon": "📝", 
                                "hotkey": "4",
                                "source": "ai",
                                "reason": "Fallback smart suggestion",
                                "confidence": 0.6
                            })
                        ]
                    };
                    
                    info!("Using fallback smart suggestions: {}", fallback_suggestions.len());
                    Ok(fallback_suggestions)
                }
            }
        } else {
            Err("AI engine not initialized".to_string())
        }
    }
}


#[tauri::command]
async fn process_ai_task(
    task_type: String,
    content: String,
    parameters: Option<std::collections::HashMap<String, String>>
) -> Result<String, String> {
    info!("Processing AI task: {}", task_type);
    
    unsafe {
        if let Some(ref analyzer) = CONTENT_ANALYZER {
            match analyzer.process_ai_task(&content, &task_type, parameters).await {
                Ok(result) => {
                    info!("AI task completed: {}", task_type);
                    Ok(result)
                },
                Err(e) => {
                    error!("AI task failed: {}", e);
                    Err(e)
                }
            }
        } else {
            Err("AI engine not initialized".to_string())
        }
    }
}

// Get clipboard history
#[tauri::command]
async fn get_clipboard_history() -> Result<Vec<ClipboardItem>, String> {
    init_global_state();
    
    unsafe {
        if let Some(ref history) = CLIPBOARD_HISTORY {
            let history_guard = history.lock().unwrap();
            Ok(history_guard.iter().cloned().collect())
        } else {
            Ok(vec![])
        }
    }
}

// Clear history
#[tauri::command]
async fn clear_clipboard_history() -> Result<String, String> {
    unsafe {
        if let Some(ref history) = CLIPBOARD_HISTORY {
            let mut history_guard = history.lock().unwrap();
            history_guard.clear();
            Ok("Clipboard history cleared".to_string())
        } else {
            Ok("No history to clear".to_string())
        }
    }
}

// Copy specific item to clipboard
#[tauri::command]
async fn copy_item_to_clipboard(content: String) -> Result<String, String> {
    use arboard::Clipboard;
    
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(&content).map_err(|e| e.to_string())?;
    
    Ok("Copied to clipboard".to_string())
}

// Test command
#[tauri::command]
async fn test_clipboard_detection(content: String) -> Result<String, String> {
    let detector = ContentDetector::new();
    let content_type = detector.detect(&content);
    
    let type_name = match content_type {
        clipboard::types::BasicContentType::Url => "🌐 URL",
        clipboard::types::BasicContentType::Email => "✉️ Email",
        clipboard::types::BasicContentType::Phone => "📞 Phone",
        clipboard::types::BasicContentType::Financial => "💰 Financial",
        clipboard::types::BasicContentType::DateTime => "📅 Date",
        clipboard::types::BasicContentType::Code => "💻 Code",
        clipboard::types::BasicContentType::Address => "🏠 Address",
        clipboard::types::BasicContentType::PlainText => "📝 PlainText",
    };
    
    Ok(format!("Content type: {} | Content length: {} characters", type_name, content.len()))
}

// Test AI connection command
#[tauri::command]
async fn test_ai_connection() -> Result<String, String> {
    init_global_state();
    
    unsafe {
        if let Some(ref analyzer) = CONTENT_ANALYZER {
            let connected = analyzer.test_ai_connection().await;
            if connected {
                Ok("AI engine connection successful".to_string())
            } else {
                Err("AI engine connection failed".to_string())
            }
        } else {
            Err("AI engine not initialized".to_string())
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("ClipMind AI Enhanced application starting...");
    
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // Original clipboard commands
            start_clipboard_monitoring,
            stop_clipboard_monitoring,
            get_clipboard_history,
            clear_clipboard_history,
            copy_item_to_clipboard,
            test_clipboard_detection,
            
            // Original popup commands
            show_popup_window,
            run_action,
            close_popup,
            resize_popup_to_content,
            
            // AI enhanced commands
            get_ai_suggestions,
            process_ai_task,
            test_ai_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}