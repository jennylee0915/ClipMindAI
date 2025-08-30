// src-tauri/src/analyzer/ai_engine.rs
use crate::clipboard::types::{
    AiAnalysis, AiActionSuggestion, AiActionType,
    BasicContentType, RuleAnalysis
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use log::{info, warn};
use std::env;
use std::fs;                 // NEW: 讀檔
use serde_yaml;              // NEW: 解析 YAML

#[derive(Debug, Serialize)]
struct ChatMessageReq {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessageReq>,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResp {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResp,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

/* ==================== YAML 設定 ==================== */
#[derive(Debug, Deserialize, Clone)]
struct AiConfig {
    ollama_url: Option<String>,
    timeout_ms: Option<u64>,
    api_key: Option<String>,
    models: Option<HashMap<String, String>>, // key: 行為名稱 / "default"
}

#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    ai: Option<AiConfig>,
}
/* ================================================== */

pub struct AiEngine {
    client: Client,
    ollama_url: String,
    model_name: String,                 // 仍保留 default model，避免破壞既有呼叫
    timeout_ms: u64,
    api_key: Option<String>,
    models: HashMap<String, String>,    // NEW: 行為 → 模型 對照表
}

impl AiEngine {
    /* 讀取 YAML 設定（路徑優先序：環境變數 CLIP_AI_CONFIG → ./config.yaml） */
    fn load_config() -> Option<AppConfig> {
        let path = env::var("CLIP_AI_CONFIG").unwrap_or_else(|_| "../config.yaml".to_string());
        match fs::read_to_string(&path) {
            Ok(s) => match serde_yaml::from_str::<AppConfig>(&s) {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    warn!("Failed to parse YAML config ({}): {}", path, e);
                    None
                }
            },
            Err(_) => {
                warn!("Config file not found at: {}", path);
                None
            }
        }
    }

    pub fn new() -> Self {
        let default_url = "http://127.0.0.1/v1.0".to_string();
        let default_model = ".bot/Llama 3.2 3B @NPU".to_string();
        let default_timeout = 100000;
        let default_api_key = env::var("KUWA_API_KEY").ok();

        // 嘗試讀 config.yaml
        let loaded = Self::load_config();

        // 展開 ai 區段
        let ai_cfg = loaded.as_ref().and_then(|c| c.ai.clone());

        let ollama_url = ai_cfg.as_ref()
            .and_then(|a| a.ollama_url.clone())
            .unwrap_or(default_url);

        let timeout_ms = ai_cfg.as_ref()
            .and_then(|a| a.timeout_ms)
            .unwrap_or(default_timeout);

        let api_key = ai_cfg.as_ref()
            .and_then(|a| a.api_key.clone())
            .or(default_api_key);

        let mut models = ai_cfg.as_ref()
            .and_then(|a| a.models.clone())
            .unwrap_or_else(HashMap::new);

        if !models.contains_key("default") {
            models.insert("default".to_string(), default_model.clone());
        }

        let model_name = models.get("default").cloned().unwrap_or(default_model);

        Self {
            client: Client::new(),
            ollama_url,
            model_name,
            timeout_ms,
            api_key,
            models,
        }
    }

    fn pick_model(&self, task_type: &str) -> String {
        self.models
            .get(task_type)
            .cloned()
            .or_else(|| self.models.get("default").cloned())
            .unwrap_or_else(|| self.model_name.clone())
    }

    pub async fn predict_intent(
        &self,
        content: &str,
        basic_type: &BasicContentType,
    ) -> Result<Vec<AiActionSuggestion>, String> {
        info!("Starting AI intent prediction, content type: {:?}", basic_type);

        let prompt = self.build_intelligent_prompt(content, basic_type);

        // 意圖預測走 default 模型
        let model = self.pick_model("default");

        let response = match self.call_ollama(&prompt, 100000, &model).await {
            Ok(resp) => {
                info!("Chat API response success: {}", &resp[..100.min(resp.len())]);
                resp
            },
            Err(e) => {
                warn!("AI call failed: {}", e);
                return Ok(self.get_fallback_suggestions(basic_type));
            }
        };

        let suggestions = self.parse_ai_response(&response, basic_type);

        info!("AI prediction completed: {} suggestions", suggestions.len());
        Ok(suggestions)
    }

    // Execute specific AI task (deep processing)
    pub async fn process_ai_task(
        &self,
        content: &str,
        task_type: &str,
        _parameters: Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        info!("Executing AI task: {}", task_type);

        let prompt = match task_type {
            "translate" => {
                format!(
                    "Translate the following content into Traditional Chinese, return only the translation:\n\n{}",
                    content
                )
            },
            "summarize" | "summarize_webpage" => {
                format!(
                    "Summarize the following content concisely in english (no more than 100 characters):\n\n{}",
                    content
                )
            },
            "explain_code" => {
                format!(
                    "Explain the functionality of this code snippet in english (no more than 100 characters):\n\n{}",
                    content
                )
            },
            "optimize_code" => {
                format!(
                    "Analyze this code and provide optimization suggestions in english:\n\n{}",
                    content
                )
            },
            "add_comments" => {
                format!(
                    "Add comments to this code snippet in english:\n\n{}",
                    content
                )
            },
            "extract_keywords" => {
                format!(
                    "Extract keywords and important information from the following content in english:\n\n{}",
                    content
                )
            },
            _ => {
                format!("Analyze the following content in in english:\n\n{}", content)
            }
        };

        // 這裡依任務類型挑選模型
        let model = self.pick_model(task_type);

        let response = self.call_ollama(&prompt, 100000, &model).await?; // Allow more time for complex tasks
        Ok(response)
    }

    // Test AI engine connection
    pub async fn test_connection(&self) -> Result<bool, String> {
        // Test chat/completions with a minimal request
        let url = format!("{}/chat/completions", self.ollama_url);
        let req = ChatRequest {
            model: self.model_name.clone(),
            messages: vec![ChatMessageReq {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
        };

        let mut builder = self.client
            .post(&url)
            .json(&req)
            .timeout(Duration::from_millis(self.timeout_ms));

        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }

        match builder.send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => Err(format!("Unable to connect to Chat API: {}", e)),
        }
    }

    /// Call the Chat API
    async fn call_ollama(&self, prompt: &str, timeout_ms: u64, model: &str) -> Result<String, String> {
        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessageReq {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let url = format!("{}/chat/completions", self.ollama_url);

        info!("Sending request to Chat API with model `{}`; timeout: {}ms", model, timeout_ms);

        let mut builder = self.client
            .post(&url)
            .json(&request)
            .timeout(Duration::from_millis(timeout_ms));

        // Optional Bearer Token
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }

        let response = builder
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    warn!("Chat API request timed out: {}ms", timeout_ms);
                    format!("Request timed out ({}ms)", timeout_ms)
                } else if e.is_connect() {
                    warn!("Unable to connect to Chat API");
                    "Unable to connect to Chat API".to_string()
                } else {
                    warn!("Chat API request failed: {}", e);
                    format!("Request failed: {}", e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("Chat API error {}: {}", status, body);
            return Err(format!("Chat API error {}: {}", status, body));
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let text = chat_response
            .choices
            .get(0)
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        if text.is_empty() {
            warn!("Chat API returned empty content");
            return Err("Empty response".to_string());
        }

        info!("Chat API responded successfully: {}", &text[..50.min(text.len())]);
        Ok(text)
    }

    /// Build intelligent prompt
    fn build_intelligent_prompt(&self, content: &str, basic_type: &BasicContentType) -> String {
        let truncated_content = if content.len() > 300 {
            format!("{}...", &content[..300])
        } else {
            content.to_string()
        };

        match basic_type {
            BasicContentType::Url => {
                format!(
                    "Analyze the URL: {}\n\nChoose 2-3 of the most appropriate next actions from the following:\n1. Open in browser\n2. Summarize webpage\n3. Search related info\n4. Save bookmark\n5. Translate webpage\n\nAnswer format: Action2,Action3,Action5,, DO NOT answer anything esle, DO NOT explain",
                    truncated_content
                )
            },
            BasicContentType::Code => {
                format!(
                    "Analyze the code: {}\n\nChoose 2-3 of the most appropriate next actions from the following:\n1. Explain code functionality\n2. Optimization suggestions\n3. Find errors\n4. Format code\n5. Search documentation\n6. Add comments\n\nAnswer format: Action1,Action2,Action5, DO NOT answer anything esle, DO NOT explain",
                    truncated_content
                )
            },
            BasicContentType::PlainText => {
                let language = if self.is_english(&truncated_content) { "English" } else { "Traditional Chinese" };
                format!(
                    "Analyze {} text: {}\n\nChoose 2-3 of the most appropriate actions from the following:\n1. Translate\n2. Summarize\n3. Extract keywords\n4. Sentiment analysis\n5. Search related\n6. Rewrite/improve\n\nAnswer format: Action1,Action2,Action3, DO NOT answer anything esle, DO NOT explain",
                    language, truncated_content
                )
            },
            _ => {
                format!(
                    "Analyze content: {}\n\nSuggested actions:\n1. Search related info\n2. Save as note\n\nAnswer format: Action1,Action2, DO NOT answer anything esle, DO NOT explain",
                    truncated_content
                )
            }
        }
    }

    // Parse AI response
    fn parse_ai_response(&self, response: &str, basic_type: &BasicContentType) -> Vec<AiActionSuggestion> {
        let mut suggestions = Vec::new();
        let response_lower = response.to_lowercase();

        info!("Parsing AI response: {}", &response[..100.min(response.len())]);

        match basic_type {
            BasicContentType::Url => {
                if response_lower.contains("summarize") || response_lower.contains("abstract") || response_lower.contains("2") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_summarize_webpage".to_string(),
                        label: "AI Summarize Webpage".to_string(),
                        icon: "📖".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.9,
                        reason: Some("AI suggested summarizing webpage".to_string()),
                        parameters: None,
                    });
                }
                if response_lower.contains("translate") || response_lower.contains("5") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_translate_webpage".to_string(),
                        label: "AI Translate".to_string(),
                        icon: "🌐".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.8,
                        reason: Some("AI suggested translating webpage".to_string()),
                        parameters: None,
                    });
                }
            },
            BasicContentType::Code => {
                if response_lower.contains("explain") || response_lower.contains("1") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_explain_code".to_string(),
                        label: "AI Explain Code".to_string(),
                        icon: "💡".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.95,
                        reason: Some("AI suggested explaining code functionality".to_string()),
                        parameters: None,
                    });
                }
                if response_lower.contains("optimize") || response_lower.contains("2") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_optimize_code".to_string(),
                        label: "AI Optimize Code".to_string(),
                        icon: "⚡".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.8,
                        reason: Some("AI suggested code optimization".to_string()),
                        parameters: None,
                    });
                }
                if response_lower.contains("comment") || response_lower.contains("6") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_add_comments".to_string(),
                        label: "AI Add Comments".to_string(),
                        icon: "📝".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.7,
                        reason: Some("AI suggested adding code comments".to_string()),
                        parameters: None,
                    });
                }
            },
            BasicContentType::PlainText => {
                if response_lower.contains("translate") || response_lower.contains("1") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_translate".to_string(),
                        label: "AI Translate".to_string(),
                        icon: "📋".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.82,
                        reason: Some("AI suggested translating this text".to_string()),
                        parameters: None,
                    });
                }
                if response_lower.contains("summarize") || response_lower.contains("abstract") || response_lower.contains("2") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_summarize".to_string(),
                        label: "AI Summarize".to_string(),
                        icon: "📋".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.8,
                        reason: Some("AI suggested generating a summary".to_string()),
                        parameters: None,
                    });
                }
                if response_lower.contains("keyword") || response_lower.contains("3") {
                    suggestions.push(AiActionSuggestion {
                        action_id: "ai_extract_keywords".to_string(),
                        label: "AI Extract Keywords".to_string(),
                        icon: "🔑".to_string(),
                        action_type: AiActionType::AiProcessing,
                        confidence: 0.7,
                        reason: Some("AI suggested extracting key information".to_string()),
                        parameters: None,
                    });
                }
            },
            _ => {}
        }

        if suggestions.is_empty() {
            suggestions = self.get_fallback_suggestions(basic_type);
        }

        suggestions
    }

    /// Fallback suggestions
    fn get_fallback_suggestions(&self, basic_type: &BasicContentType) -> Vec<AiActionSuggestion> {
        match basic_type {
            BasicContentType::Url => vec![
                AiActionSuggestion {
                    action_id: "ai_summarize_webpage".to_string(),
                    label: "AI Summarize Webpage".to_string(),
                    icon: "📋".to_string(),
                    action_type: AiActionType::AiProcessing,
                    confidence: 0.7,
                    reason: Some("Fallback suggestion: summarize webpage".to_string()),
                    parameters: None,
                }
            ],
            BasicContentType::Code => vec![
                AiActionSuggestion {
                    action_id: "ai_explain_code".to_string(),
                    label: "AI Explain Code".to_string(),
                    icon: "📋".to_string(),
                    action_type: AiActionType::AiProcessing,
                    confidence: 0.8,
                    reason: Some("Fallback suggestion: explain code functionality".to_string()),
                    parameters: None,
                }
            ],
            BasicContentType::PlainText => vec![
                AiActionSuggestion {
                    action_id: "ai_translate".to_string(),
                    label: "AI Translate".to_string(),
                    icon: "📋".to_string(),
                    action_type: AiActionType::AiProcessing,
                    confidence: 0.8,
                    reason: Some("Fallback suggestion: translate text".to_string()),
                    parameters: None,
                },
                AiActionSuggestion {
                    action_id: "ai_summarize".to_string(),
                    label: "AI Summarize".to_string(),
                    icon: "📋".to_string(),
                    action_type: AiActionType::AiProcessing,
                    confidence: 0.7,
                    reason: Some("Fallback suggestion: generate summary".to_string()),
                    parameters: None,
                }
            ],
            _ => vec![]
        }
    }

    // Detect if text is English
    fn is_english(&self, text: &str) -> bool {
        let english_chars = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();

        if total_chars == 0 { return false; }

        (english_chars as f32 / total_chars as f32) > 0.7
    }

    /// Full analysis method (for compatibility)
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
