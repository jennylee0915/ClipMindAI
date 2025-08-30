// src-tauri/src/analyzer/genie_engine.rs

use crate::clipboard::types::{
    AiAnalysis, AiActionSuggestion, AiActionType, 
    BasicContentType, RuleAnalysis
};
use std::collections::HashMap;
use std::process::Command;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use std::path::PathBuf;

pub struct GenieEngine {
    genie_bundle_path: PathBuf,
    model_name: String,
    timeout_ms: u64,
}

impl GenieEngine {
    pub fn new() -> Self {
        //let genie_bundle_path = PathBuf::from("./genie_bundle");
        let genie_bundle_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("genie_bundle");

        
        Self {
            genie_bundle_path,
            model_name: "phi-3.5-mini".to_string(),
            timeout_ms: 100000,
        }
    }

    /// 使用 Genie CLI 進行推理
    pub async fn predict_intent(
        &self,
        content: &str,
        basic_type: &BasicContentType,
    ) -> Result<Vec<AiActionSuggestion>, String> {
        info!("使用 Genie 開始 AI 意圖預測，內容類型: {:?}", basic_type);
        
        let prompt = self.build_intelligent_prompt(content, basic_type);
        
        // 調用 Genie
        let response = match self.call_genie(&prompt).await {
            Ok(resp) => {
                info!("Genie 回應成功: {}", &resp[..100.min(resp.len())]);
                resp
            },
            Err(e) => {
                warn!("Genie 調用失敗: {}", e);
                return Ok(self.get_fallback_suggestions(basic_type));
            }
        };
        
        let suggestions = self.parse_ai_response(&response, basic_type);
        
        info!("AI 預測完成: {} 個建議", suggestions.len());
        Ok(suggestions)
    }

    pub async fn process_ai_task(
        &self,
        content: &str,
        task_type: &str,
        _parameters: Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        info!("Process AI task: {}", task_type);
        
        let prompt = match task_type {
            "translate" => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nTranslate the following text to Traditional Chinese. Only return the translation:\n\n{}\n<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    content
                )
            },
            "summarize" | "summarize_webpage" => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nSummarize the following content in Traditional Chinese (max 100 characters):\n\n{}\n<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    content
                )
            },
            "explain_code" => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nYou are a code expert.Explain this code in simple Traditional Chinese:\n\n{}\n<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    content
                )
            },
            "optimize_code" => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nYou are a code optimization expert.Analyze and suggest optimizations for this code:\n\n{}\n<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    content
                )
            },
            "add_comments" => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nYou are a documentation expert.Add helpful comments to this code:\n\n{}\n<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    content
                )
            },
            "extract_keywords" => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nYou are a text analysis expert.Extract keywords from the following content:\n\n{}\n<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    content
                )
            },
            _ => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nAnalyze the following content:\n\n{}\n<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    content
                )
            }
        };
        
        let response = self.call_genie(&prompt).await?;
        Ok(response)
    }

    // Test Genie Connect 
    pub async fn test_connection(&self) -> Result<bool, String> {
        // Check if genie-t2t-run.exe exists
        let genie_exe = self.genie_bundle_path.join("genie-t2t-run.exe");
        
        if !genie_exe.exists() {
            return Err(format!("Genie executable not found at: {:?}", genie_exe));
        }
        
        // Test a simple question first
        let test_prompt = "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nWhat is France's capital?<|eot_id|><|start_header_id|>assistant<|end_header_id|>";
        
        match self.call_genie(&test_prompt).await {
            Ok(_) => {
                info!("Genie 連接測試成功");
                Ok(true)
            },
            Err(e) => {
                error!("Genie 連接測試失敗: {}", e);
                Err(e)
            }
        }
    }


    async fn call_genie(&self, prompt: &str) -> Result<String, String> {
        let genie_exe = self.genie_bundle_path.join("genie-t2t-run.exe");
        let config_file = self.genie_bundle_path.join("genie_config.json");
        
        info!("調用 Genie");
        
        // Use tokio's spawn_blocking to execute synchronous commands
        let genie_exe_str = genie_exe.to_string_lossy().to_string();
        let config_file_str = config_file.to_string_lossy().to_string();
        let prompt_owned = prompt.to_owned();
        let timeout_ms = self.timeout_ms;
        
        let result = tokio::task::spawn_blocking(move || {
            // Execute Genie on Windows
            let output = Command::new(genie_exe_str)
                .arg("-c")
                .arg(config_file_str)
                .arg("-p")
                .arg(prompt_owned)
                .current_dir("./genie_bundle") 
                .output()
                .map_err(|e| format!("Failed to execute Genie: {}", e))?;
            
            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Genie error: {}", error_msg));
            }
            
            let response = String::from_utf8_lossy(&output.stdout);
            
            
            // Parse Genie output to extract AI response
            // Genie output format: [BEGIN]: <response> [END]
            if let Some(start) = response.find("[BEGIN]:") {
                if let Some(end) = response.find("[END]") {
                    let ai_response = response[start + 8..end].trim().to_string();
                    info!("Genie response (parsed): {}", ai_response);
                    return Ok(ai_response);
                } else {
                    Err("Genie response missing [END] marker".to_string())
                }
            } else {
                Err("Genie response missing [BEGIN] marker".to_string())
            }
            
            // If parsing fails, return raw response
            //info!("Genie return：{}", response);
            //Ok(response.to_string())
        }).await;
        
        match result {
            Ok(Ok(response)) => {
                info!("Genie success");
                Ok(response)
            },
            Ok(Err(e)) => Err(e),
            Err(e) => Err(format!("Genie failed: {}", e)),
        }
    }

    /// setting intelligent prompt
    fn build_intelligent_prompt(&self, content: &str, basic_type: &BasicContentType) -> String {
        let truncated_content = if content.len() > 300 {
            format!("{}...", &content[..300])
        } else {
            content.to_string()
        };
        
        match basic_type {
            BasicContentType::Url => {
                format!(
                    "<|system|>\nYou are a helpful assistant. Be helpful but brief.<|end|>\n<|user|>Analyze this URL: {}\n\nSuggest 2-3 appropriate actions from:\n1. Summarize webpage\n2. Search related\n3. Save bookmark\n4. Translate page\n\nAnswer with action numbers only, separated by commas.<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    truncated_content
                )
            },
            BasicContentType::Code => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nYou are a code expert.Analyze this code: {}\n\nSuggest 2-3 actions from:\n1. Explain code\n2. Optimize code\n3. Find bugs\n4. Format code\n5. Search docs\n6. Add comments\n\nAnswer with action numbers only, separated by commas.<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    truncated_content
                )
            },
            BasicContentType::PlainText => {
                let language = if self.is_english(&truncated_content) { "English" } else { "Traditional Chinese" };
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nAnalyze this {} text: {}\n\nSuggest 2-3 actions from:\n1.English Translate to Tradtional Chinese or Chinese Translate to English\n2. Summarize\n3. Extract keywords\n4. Analyze sentiment\n5. Search related\n6. Rewrite\n\nAnswer with action numbers only, separated by commas.<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    language, truncated_content
                )
            },
            _ => {
                format!(
                    "<|begin_of_text|><|start_header_id|>user<|end_header_id|>\n\nAnalyze: {}\n\nSuggest actions: 1. Search 2. Save note\n\nAnswer with numbers.<|eot_id|><|start_header_id|>assistant<|end_header_id|>",
                    truncated_content
                )
            }
        }
    }

    /// Parsing AI response
    fn parse_ai_response(&self, response: &str, basic_type: &BasicContentType) -> Vec<AiActionSuggestion> {
        let mut suggestions = Vec::new();
        let response_lower = response.to_lowercase();
        
        info!("Parsing AI response: {}", &response[..100.min(response.len())]);
        
        // Parse numbers from response
        let numbers: Vec<&str> = response.split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        
        match basic_type {
            BasicContentType::Url => {
                for num in &numbers {
                    match num.trim() {
                        "1" => suggestions.push(self.create_suggestion("ai_summarize_webpage", "AI Summarize", "📖", 0.85)),
                        "2" => suggestions.push(self.create_suggestion("search_related", "Search Related", "🔍", 0.8)),
                        "3" => suggestions.push(self.create_suggestion("save_bookmark", "Save Bookmark", "⭐", 0.75)),
                        "4" => suggestions.push(self.create_suggestion("ai_translate_webpage", "AI Translate", "🌐", 0.8)),
                        _ => {}
                    }
                }
            },
            BasicContentType::Code => {
                for num in &numbers {
                    match num.trim() {
                        "1" => suggestions.push(self.create_suggestion("ai_explain_code", "AI Explain Code", "💡", 0.95)),
                        "2" => suggestions.push(self.create_suggestion("ai_optimize_code", "AI Optimize", "⚡", 0.85)),
                        "3" => suggestions.push(self.create_suggestion("find_bugs", "Find Bugs", "🐛", 0.8)),
                        "4" => suggestions.push(self.create_suggestion("format_code", "Format Code", "✨", 0.75)),
                        "5" => suggestions.push(self.create_suggestion("search_docs", "Search Docs", "📚", 0.7)),
                        "6" => suggestions.push(self.create_suggestion("ai_add_comments", "AI Add Comments", "📝", 0.75)),
                        _ => {}
                    }
                }
            },
            BasicContentType::PlainText => {
                for num in &numbers {
                    match num.trim() {
                        "1" => suggestions.push(self.create_suggestion("ai_translate", "AI Translate", "🌐", 0.9)),
                        "2" => suggestions.push(self.create_suggestion("ai_summarize", "AI Summarize", "📋", 0.85)),
                        "3" => suggestions.push(self.create_suggestion("ai_extract_keywords", "AI Keywords", "🔑", 0.75)),
                        "4" => suggestions.push(self.create_suggestion("ai_sentiment", "AI Sentiment", "😊", 0.7)),
                        "5" => suggestions.push(self.create_suggestion("search_related", "Search Related", "🔍", 0.7)),
                        "6" => suggestions.push(self.create_suggestion("ai_rewrite", "AI Rewrite", "✏️", 0.75)),
                        _ => {}
                    }
                }
            },
            _ => {}
        }
        
        // If no valid suggestions, use fallback
        if suggestions.is_empty() {
            suggestions = self.get_fallback_suggestions(basic_type);
        }
        
        suggestions
    }

    /// Suggestion creation helper
    fn create_suggestion(&self, id: &str, label: &str, icon: &str, confidence: f32) -> AiActionSuggestion {
        AiActionSuggestion {
            action_id: id.to_string(),
            label: label.to_string(),
            icon: icon.to_string(),
            action_type: if id.starts_with("ai_") { 
                AiActionType::AiProcessing 
            } else { 
                AiActionType::SystemAction 
            },
            confidence,
            reason: Some(format!("Genie AI suggested: {}", label)),
            parameters: None,
        }
    }

    /// Fallback suggestions
    fn get_fallback_suggestions(&self, basic_type: &BasicContentType) -> Vec<AiActionSuggestion> {
        match basic_type {
            BasicContentType::Url => vec![
                self.create_suggestion("ai_summarize_webpage", "AI Summarize", "📋", 0.7),
            ],
            BasicContentType::Code => vec![
                self.create_suggestion("ai_explain_code", "AI Explain Code", "💡", 0.8),
            ],
            BasicContentType::PlainText => vec![
                self.create_suggestion("ai_translate", "AI Translate", "🌐", 0.8),
                self.create_suggestion("ai_summarize", "AI Summarize", "📋", 0.7),
            ],
            _ => vec![]
        }
    }

    /// Detect if text is primarily English
    fn is_english(&self, text: &str) -> bool {
        let english_chars = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();
        
        if total_chars == 0 { return false; }
        
        (english_chars as f32 / total_chars as f32) > 0.7
    }

    /// Full analysis function
    pub async fn analyze(
        &self,
        content: &str,
        basic_type: &BasicContentType,
        _rule_analysis: Option<&RuleAnalysis>,
    ) -> Result<AiAnalysis, String> {
        let intent_predictions = self.predict_intent(content, basic_type).await?;
        
        let confidence = if !intent_predictions.is_empty() {
            intent_predictions.iter()
                .map(|p| p.confidence)
                .sum::<f32>() / intent_predictions.len() as f32
        } else {
            0.0
        };
        
        Ok(AiAnalysis {
            intent_predictions,
            summary: None,
            confidence,
            raw_response: None,
        })
    }
}