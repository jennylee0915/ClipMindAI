// src-tauri/src/bin/test_ollama.rs
// Execution: cargo run --bin test_ollama

use reqwest;
use serde_json::json;
use tokio;

#[tokio::main]
async fn main() {
    println!("Testing Ollama connection...");
    
    // 1. Test basic connection
    match test_connection().await {
        Ok(true) => println!("Ollama connection successful"),
        Ok(false) => println!("Ollama service not responding"),
        Err(e) => println!("Connection error: {}", e),
    }
    
    // 2. Test model list
    match list_models().await {
        Ok(models) => println!("Available models: {:?}", models),
        Err(e) => println!("Unable to fetch model list: {}", e),
    }
    
    // 3. Test intent prediction
    println!("\nTesting intent prediction...");
    test_intent_prediction().await;
}

async fn test_connection() -> Result<bool, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(response.status().is_success())
}

async fn list_models() -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    
    if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
        let model_names: Vec<String> = models
            .iter()
            .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
            .map(|s| s.to_string())
            .collect();
        Ok(model_names)
    } else {
        Ok(vec![])
    }
}

async fn test_intent_prediction() {
    let test_cases = vec![
        ("https://github.com/microsoft/vscode", "URL"),
        ("Please translate this text for me", "Chinese text"),
        ("def hello():\n    print('Hello')", "Code"),
        ("No.7, Sec. 5, Xinyi Rd., Xinyi Dist., Taipei City", "Address"),
        ("2024-12-25 14:30", "DateTime"),
    ];
    
    let client = reqwest::Client::new();
    
    for (content, content_type) in test_cases {
        println!("\n📋 Test content: {} ({})", 
            if content.len() > 30 { &content[..30] } else { content },
            content_type
        );
        
        let prompt = format!(
            r#"Analyze this content and guess what the user most likely wants to do (maximum 2 suggestions): Content: {} Please answer with short phrases, separate multiple suggestions with |."#,
            content
        );
        
        let request = json!({
            "model": "llama3.2:1b",
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.7,
                "max_tokens": 50
            }
        });
        
        match client
            .post("http://localhost:11434/api/generate")
            .json(&request)
            .send()
            .await
        {
            Ok(response) => {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    if let Some(ai_response) = json.get("response").and_then(|r| r.as_str()) {
                        println!(" AI suggestion: {}", ai_response.trim());
                    }
                }
            }
            Err(e) => println!(" Error: {}", e),
        }
    }
}
