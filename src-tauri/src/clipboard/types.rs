// src-tauri/src/clipboard/types.rs
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BasicContentType {
    Url,
    Email,
    Phone,
    Financial,
    DateTime,
    Code,
    Address,
    PlainText,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    PlainText,
    Url,
    Email,
    Code,
    Phone,
    Address,
    Financial,
    DateTime,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEvent {
    pub content: String,
    pub content_type: BasicContentType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_app: Option<String>,
    pub content_hash: String,
    pub content_length: usize,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAnalysis {
    pub confidence: f32,
    pub metadata: HashMap<String, String>,
    pub suggested_actions: Vec<ActionSuggestion>,
    pub needs_ai_analysis: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAnalysis {
    pub intent_predictions: Vec<AiActionSuggestion>,
    pub summary: Option<String>,
    pub confidence: f32,
    pub raw_response: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteAnalysis {
    pub rule_analysis: RuleAnalysis,
    pub ai_analysis: Option<AiAnalysis>,
    pub merged_actions: Vec<MergedActionSuggestion>,
    pub processing_time_ms: u64,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiActionSuggestion {
    pub action_id: String,
    pub label: String,
    pub icon: String,
    pub action_type: AiActionType,
    pub confidence: f32,
    pub reason: Option<String>,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiActionType {
    SystemAction,
    AiProcessing,
    HybridAction,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentPredictionRequest {
    pub content: String,
    pub content_type: BasicContentType,
    pub context: Option<UserContext>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub recent_actions: Vec<String>,
    pub time_of_day: String,
    pub app_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSuggestion {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub action_type: ActionType,
    pub hotkey: String,
    pub confidence: f32,
    pub estimated_time: Option<u64>,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    Immediate,
    AiDelayed,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedActionSuggestion {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub action_type: String, // "rule", "ai", "hybrid"
    pub hotkey: String,
    pub confidence: f32,
    pub source: String, // "rule_engine", "ai_engine", "merged"
    pub reason: Option<String>,
    pub estimated_time: Option<u64>,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("Failed to access clipboard: {0}")]
    AccessError(String),
    #[error("Content parsing error: {0}")]
    ParsingError(String),
    #[error("Analysis timeout")]
    AnalysisTimeout,
    #[error("AI processing error: {0}")]
    AiProcessingError(String),
}

impl ClipboardEvent {
    pub fn new(content: String, content_type: BasicContentType, source_app: Option<String>) -> Self {
        Self {
            content: content.clone(),
            content_type,
            timestamp: chrono::Utc::now(),
            source_app,
            content_hash: Self::calculate_hash(&content),
            content_length: content.len(),
        }
    }
    
    fn calculate_hash(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

impl ActionSuggestion {
    pub fn immediate(id: &str, label: &str, icon: &str, hotkey: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            icon: icon.to_string(),
            action_type: ActionType::Immediate,
            hotkey: hotkey.to_string(),
            confidence: 1.0,
            estimated_time: None,
            parameters: None,
        }
    }
    
    pub fn ai_delayed(id: &str, label: &str, icon: &str, hotkey: &str, estimated_time: u64) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            icon: icon.to_string(),
            action_type: ActionType::AiDelayed,
            hotkey: hotkey.to_string(),
            confidence: 1.0,
            estimated_time: Some(estimated_time),
            parameters: None,
        }
    }
}