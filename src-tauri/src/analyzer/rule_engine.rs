//src-tauri/src/analyzer/rule_engine.rs
use crate::clipboard::types::{BasicContentType, RuleAnalysis, ActionSuggestion};
use std::collections::HashMap;

pub struct RuleEngine;

impl RuleEngine {
    pub fn new() -> Self {
        Self
    }
    
    pub fn analyze(&self, content: &str, basic_type: BasicContentType) -> RuleAnalysis {
        match basic_type {
            BasicContentType::Url => self.analyze_url(content),
            BasicContentType::Email => self.analyze_email(content),
            BasicContentType::Phone => self.analyze_phone(content),
            BasicContentType::Financial => self.analyze_financial(content),
            BasicContentType::Code => self.analyze_code(content),
            BasicContentType::Address => self.analyze_address(content),
            BasicContentType::DateTime => self.analyze_datetime(content),
            
            // AI extension point: PlainText is fully handled by AI
            BasicContentType::PlainText => RuleAnalysis {
                confidence: 0.1,
                metadata: HashMap::new(),
                suggested_actions: vec![
                    // Temporary skip: in the future, this will be AI-generated smart suggestions
                    ActionSuggestion::immediate("save_text", "Save Text", "💾", "1"),
                ],
                needs_ai_analysis: true,  // Mark that AI is needed
            },
        }
    }
    
    fn analyze_url(&self, content: &str) -> RuleAnalysis {
        let domain = self.extract_domain(content);
        
        let mut actions = vec![
            ActionSuggestion::immediate("open_browser", "Open Link", "🌐", "1"),
            ActionSuggestion::immediate("save_bookmark", "Save Bookmark", "⭐", "2"),
        ];
        
        // AI extension point: in the future, AI can enhance this with smart suggestions
        // Example: AI analyzes webpage content and provides personalized suggestions
        
        let mut metadata = HashMap::new();
        metadata.insert("domain".to_string(), domain);
        
        RuleAnalysis {
            confidence: 0.95,
            metadata,
            suggested_actions: actions,
            needs_ai_analysis: true,  // URL can also be enhanced by AI
        }
    }
    
    fn analyze_email(&self, content: &str) -> RuleAnalysis {
        let actions = vec![
            ActionSuggestion::immediate("compose_email", "Compose Email", "✉️", "1"),
            ActionSuggestion::immediate("save_contact", "Save Contact", "👤", "2"),
        ];
        
        RuleAnalysis {
            confidence: 0.90,
            metadata: HashMap::new(),
            suggested_actions: actions,
            needs_ai_analysis: false,  // Email does not need AI enhancement
        }
    }
    
    fn analyze_phone(&self, content: &str) -> RuleAnalysis {
        let actions = vec![
            ActionSuggestion::immediate("call_phone", "Call Phone", "📞", "1"),
            ActionSuggestion::immediate("save_contact", "Save Contact", "👤", "2"),
            ActionSuggestion::immediate("send_sms", "Send SMS", "💬", "3"),
        ];

        
        RuleAnalysis {
            confidence: 0.85,
            metadata: HashMap::new(),
            suggested_actions: actions,
            needs_ai_analysis: false,
        }
    }
    
    fn analyze_financial(&self, content: &str) -> RuleAnalysis {
        let currency = if content.contains("NT$") { "TWD" } 
                      else if content.contains("$") { "USD" }
                      else if content.contains("€") { "EUR" }
                      else { "Unknown" };
        
        let actions = vec![
            ActionSuggestion::immediate("save_expense", "Record Expense", "💰", "1"),
            ActionSuggestion::immediate("currency_convert", "Currency Conversion", "🔄", "2"),
        ];
        
        let mut metadata = HashMap::new();
        metadata.insert("currency".to_string(), currency.to_string());
        
        RuleAnalysis {
            confidence: 0.88,
            metadata,
            suggested_actions: actions,
            needs_ai_analysis: false,
        }
    }
    
    fn analyze_code(&self, content: &str) -> RuleAnalysis {
        
        let mut actions = vec![
            ActionSuggestion::immediate("open_vscode", "Open in VSCode", "💻", "1"),
            ActionSuggestion::immediate("format_code", "Format Code", "✨", "2"),
        ];
        
        let metadata = HashMap::new();
        
        RuleAnalysis {
            confidence: 0.80,
            metadata,
            suggested_actions: actions,
            needs_ai_analysis: true,  // Code can be enhanced by AI
        }
    }
    
    fn analyze_address(&self, content: &str) -> RuleAnalysis {
        let actions = vec![
            ActionSuggestion::immediate("open_maps", "Open in Maps", "🗺️", "1"),
            ActionSuggestion::immediate("save_location", "Save Location", "📍", "2"),
        ];
        
        RuleAnalysis {
            confidence: 0.75,
            metadata: HashMap::new(),
            suggested_actions: actions,
            needs_ai_analysis: false,
        }
    }
    
    fn analyze_datetime(&self, content: &str) -> RuleAnalysis {
        let actions = vec![
            ActionSuggestion::immediate("add_calendar", "Add to Calendar", "📅", "1"),
            ActionSuggestion::immediate("set_reminder", "Set Reminder", "⏰", "2"),
        ];
        
        RuleAnalysis {
            confidence: 0.80,
            metadata: HashMap::new(),
            suggested_actions: actions,
            needs_ai_analysis: false,
        }
    }
    
    fn extract_domain(&self, url: &str) -> String {
        url.split('/')
            .nth(2)
            .unwrap_or("unknown")
            .replace("www.", "")
    }
}
