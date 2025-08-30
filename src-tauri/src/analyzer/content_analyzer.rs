// src-tauri/src/analyzer/content_analyzer.rs
use crate::clipboard::types::{
    BasicContentType, CompleteAnalysis, RuleAnalysis, AiAnalysis, 
    MergedActionSuggestion, ClipboardError
};
use crate::analyzer::rule_engine::RuleEngine;
use crate::analyzer::ai_engine::AiEngine;
use std::time::Instant;
use log::{info, warn};

pub struct ContentAnalyzer {
    rule_engine: RuleEngine,
    pub ai_engine: AiEngine,
    ai_timeout_ms: u64,
}

impl ContentAnalyzer {
    pub fn new() -> Self {
        Self {
            rule_engine: RuleEngine::new(),
            ai_engine: AiEngine::new(),
            ai_timeout_ms: 10000,
        }
    }

    
    pub async fn analyze_content(
        &self,
        content: &str,
        content_type: BasicContentType,
    ) -> Result<CompleteAnalysis, ClipboardError> {
        let start_time = Instant::now();
        
        info!("Start analyzing content, type: {:?}, length: {}", content_type, content.len());
        
        // Phase 1: Rule engine analysis (fast, must complete)
        let rule_analysis = self.rule_engine.analyze(content, content_type.clone());
        info!("Rule analysis completed, confidence: {:.2}", rule_analysis.confidence);
        
        // Phase 2: AI intent prediction (optional, with timeout)
        let ai_analysis = if rule_analysis.needs_ai_analysis {
            info!("Triggering AI intent prediction...");
            match self.predict_ai_intent(content, &content_type).await {
                Ok(analysis) => {
                    info!("AI analysis completed, predicted {} actions", analysis.intent_predictions.len());
                    Some(analysis)
                },
                Err(e) => {
                    warn!("AI analysis failed, continue using rule analysis: {}", e);
                    None
                }
            }
        } else {
            info!("Skipping AI analysis (rule engine already provides sufficient suggestions)");
            None
        };
        
        // Phase 3: Merge suggestions
        let merged_actions = self.merge_suggestions(&rule_analysis, &ai_analysis);
        
        let processing_time = start_time.elapsed().as_millis() as u64;
        info!("Content analysis completed, total time: {}ms", processing_time);
        
        Ok(CompleteAnalysis {
            rule_analysis,
            ai_analysis,
            merged_actions,
            processing_time_ms: processing_time,
        })
    }

    // Public method for processing AI tasks
    pub async fn process_ai_task(
        &self,
        content: &str,
        task_type: &str,
        parameters: Option<std::collections::HashMap<String, String>>,
    ) -> Result<String, String> {
        self.ai_engine.process_ai_task(content, task_type, parameters).await
    }

    /// AI intent prediction (with timeout)
    async fn predict_ai_intent(
        &self,
        content: &str,
        content_type: &BasicContentType,
    ) -> Result<AiAnalysis, ClipboardError> {
        // Wrap AI call with tokio::time::timeout
        let ai_future = self.ai_engine.predict_intent(content, content_type);
        
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(self.ai_timeout_ms),
            ai_future
        ).await {
            Ok(Ok(predictions)) => {
                let confidence = if !predictions.is_empty() {
                    predictions.iter().map(|p| p.confidence).sum::<f32>() / predictions.len() as f32
                } else {
                    0.0
                };
                
                Ok(AiAnalysis {
                    intent_predictions: predictions,
                    summary: None,
                    confidence,
                    raw_response: None,
                })
            },
            Ok(Err(e)) => Err(ClipboardError::AiProcessingError(e)),
            Err(_) => Err(ClipboardError::AnalysisTimeout),
        }
    }

    /// Merge rule and AI suggestions
    fn merge_suggestions(
        &self,
        rule_analysis: &RuleAnalysis,
        ai_analysis: &Option<AiAnalysis>,
    ) -> Vec<MergedActionSuggestion> {
        let mut merged = Vec::new();
        let mut hotkey_counter = 1;

        // 1. Add high-confidence rule suggestions
        for action in &rule_analysis.suggested_actions {
            if action.confidence >= 0.8 {
                merged.push(MergedActionSuggestion {
                    id: action.id.clone(),
                    label: action.label.clone(),
                    icon: action.icon.clone(),
                    action_type: "rule".to_string(),
                    hotkey: hotkey_counter.to_string(),
                    confidence: action.confidence,
                    source: "rule_engine".to_string(),
                    reason: Some("Based on rule matching".to_string()),
                    estimated_time: action.estimated_time,
                    parameters: action.parameters.clone(),
                });
                hotkey_counter += 1;
            }
        }

        merged
    }

    /// Test AI engine connection
    pub async fn test_ai_connection(&self) -> bool {
        match self.ai_engine.test_connection().await {
            Ok(connected) => {
                if connected {
                    info!("AI engine connection successful");
                } else {
                    warn!("AI engine connection failed");
                }
                connected
            },
            Err(e) => {
                warn!("Unable to test AI engine connection: {}", e);
                false
            }
        }
    }
}
