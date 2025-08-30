// src-tauri/src/clipboard/content_detector.rs
use super::types::{BasicContentType, ClipboardEvent};

#[derive(Clone)]
pub struct ContentDetector;

impl ContentDetector {
    pub fn new() -> Self {
        Self
    }
    
    pub fn detect(&self, content: &str) -> BasicContentType {
        let content = content.trim();
        
        if self.is_url(content) {
            return BasicContentType::Url;
        }
        
        if self.is_email(content) {
            return BasicContentType::Email;
        }
        
        if self.is_financial(content) {
            return BasicContentType::Financial;
        }
        
        if self.is_datetime(content) {
            return BasicContentType::DateTime;
        }

        if self.is_phone(content) {
            return BasicContentType::Phone;
        }

        if self.is_code(content) {
            return BasicContentType::Code;
        }

        if self.is_address(content) {
            return BasicContentType::Address;
        }
        
        BasicContentType::PlainText
    }
    
    pub fn create_event(&self, content: String, source_app: Option<String>) -> ClipboardEvent {
        let content_type = self.detect(&content);
        ClipboardEvent::new(content, content_type, source_app)
    }
    
    fn is_url(&self, content: &str) -> bool {
        lazy_static::lazy_static! {
            static ref URL_REGEX: regex::Regex = regex::Regex::new(
                r"^(https?://|ftp://)?([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})(:[0-9]+)?(/.*)?$"
            ).unwrap();
        }
        URL_REGEX.is_match(content)
    }
    
    fn is_email(&self, content: &str) -> bool {
        lazy_static::lazy_static! {
            static ref EMAIL_REGEX: regex::Regex = regex::Regex::new(
                r"^[^\s@]+@[^\s@]+\.[^\s@]+$"
            ).unwrap();
        }
        EMAIL_REGEX.is_match(content)
    }

    fn is_phone(&self, content: &str) -> bool {
        let digits = content.chars().filter(|c| c.is_ascii_digit()).count();
        if digits < 7 || digits > 15 {
            return false;
        }

        if content.contains(':') {
            return false;
        }

        let has_phone_chars = content.chars().all(|c| c.is_ascii_digit() || c == '+' || c == '-' || c == ' ' || c == '(' || c == ')');
        has_phone_chars
    }
    
    fn is_financial(&self, content: &str) -> bool {
        lazy_static::lazy_static! {
            static ref SYMBOL_REGEX: regex::Regex = regex::Regex::new(
                r"(?ix) ^\s*(?:\p{Sc})\s*\d{1,3}(?:[,\d]{0,12})(?:[.]\d{1,2})?\s*$"
            ).unwrap();

            static ref CODE_REGEX: regex::Regex = regex::Regex::new(
                r"(?ix) ^\s*(?:USD|EUR|JPY|TWD|NTD|NT\$?)\s*\d{1,3}(?:[,\d]{0,12})(?:[.]\d{1,2})?\s*$|
                ^\s*\d{1,3}(?:[,\d]{0,12})(?:[.]\d{1,2})?\s*(?:USD|EUR|JPY|TWD|NTD|NT)\s*$"
            ).unwrap();
        }

        SYMBOL_REGEX.is_match(content) || CODE_REGEX.is_match(content)
    }
    
    fn is_datetime(&self, content: &str) -> bool {
        lazy_static::lazy_static! {
            static ref DATE_REGEX: regex::Regex = regex::Regex::new(
                r"(?x)
                ^(?:
                # YYYY-MM-DD or YYYY/MM/DD
                (?:19|20)\d{2}[-/](?:0[1-9]|1[0-2])[-/](?:0[1-9]|[12]\d|3[01])
                |
                # DD-MM-YYYY or DD/MM/YYYY
                (?:0[1-9]|[12]\d|3[01])[-/](?:0[1-9]|1[0-2])[-/](?:19|20)\d{2}
                |
                # MM-DD-YYYY or MM/DD/YYYY
                (?:0[1-9]|1[0-2])[-/](?:0[1-9]|[12]\d|3[01])[-/](?:19|20)\d{2}
                |
                # HH:MM(:SS)?
                (?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d)?
                )$"
            ).unwrap();
        }
        DATE_REGEX.is_match(content)
    }
    
    fn is_code(&self, content: &str) -> bool {
        let code_keywords = [
            "def ", "function ", "class ", "import ", "#include", "console.log",
            "println", "System.out", "cout <<", "<?php", "#!/", "<script>", 
            "public class", "private ", "void "
        ];
        let sql_keywords = ["select ", "from ", "where "];

        let lower = content.to_lowercase();
        let has_keywords = code_keywords.iter().any(|&kw| lower.contains(kw));
        let has_sql = sql_keywords.iter().any(|&kw| lower.contains(kw));
        let multiple_lines = content.lines().count() >= 5;
        let total_chars = content.chars().count().max(1);
        let paren_count = content.chars().filter(|&c| c == '(' || c == ')').count();
        let paren_ratio = paren_count as f64 / total_chars as f64;

        has_keywords || has_sql || (multiple_lines && (paren_ratio > 0.02))
    }
    
    fn is_address(&self, content: &str) -> bool {
        let address_keywords = [
            "街", "路", "巷", "弄", "號", "樓", "室", "市", "縣", "段",
        ];

        let count = address_keywords
            .iter()
            .filter(|kw| content.contains(*kw))
            .count();

        count >= 2
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_detection() {
        let detector = ContentDetector::new();
        
        assert_eq!(detector.detect("https://github.com/microsoft/vscode"), BasicContentType::Url);
        assert_eq!(detector.detect("http://example.com"), BasicContentType::Url);
        assert_eq!(detector.detect("ftp://files.example.com"), BasicContentType::Url);
        assert_eq!(detector.detect("not a url"), BasicContentType::PlainText);
    }

    #[test]
    fn test_email_detection() {
        let detector = ContentDetector::new();
        
        assert_eq!(detector.detect("test@example.com"), BasicContentType::Email);
        assert_eq!(detector.detect("user.name@domain.org"), BasicContentType::Email);
        assert_eq!(detector.detect("not an email"), BasicContentType::PlainText);
        assert_eq!(detector.detect("@incomplete"), BasicContentType::PlainText);
    }

    #[test]
    fn test_phone_detection() {
        let detector = ContentDetector::new();
        
        assert_eq!(detector.detect("+886912345678"), BasicContentType::Phone);
        assert_eq!(detector.detect("0912-345-678"), BasicContentType::Phone);
        assert_eq!(detector.detect("(02) 1234-5678"), BasicContentType::Phone);
        assert_eq!(detector.detect("123"), BasicContentType::PlainText); // 太短
    }

    #[test]
    fn test_code_detection() {
        let detector = ContentDetector::new();
        
        assert_eq!(detector.detect("def hello():\n    print('Hello')"), BasicContentType::Code);
        assert_eq!(detector.detect("function test() { return 42; }"), BasicContentType::Code);
        assert_eq!(detector.detect("#include <stdio.h>"), BasicContentType::Code);
        assert_eq!(detector.detect("SELECT * FROM users"), BasicContentType::Code);
        assert_eq!(detector.detect("just text"), BasicContentType::PlainText);
    }

    #[test]
    fn test_financial_detection() {
        let detector = ContentDetector::new();
        
        assert_eq!(detector.detect("$100"), BasicContentType::Financial);
        assert_eq!(detector.detect("NT$1000"), BasicContentType::Financial);
        assert_eq!(detector.detect("€50"), BasicContentType::Financial);
        assert_eq!(detector.detect("100 USD"), BasicContentType::Financial);
        assert_eq!(detector.detect("no money here"), BasicContentType::PlainText);
    }

    #[test]
    fn test_datetime_detection() {
        let detector = ContentDetector::new();
        
        assert_eq!(detector.detect("2024-01-15"), BasicContentType::DateTime);
        assert_eq!(detector.detect("01/15/2024"), BasicContentType::DateTime);
        assert_eq!(detector.detect("14:30"), BasicContentType::DateTime);
        assert_eq!(detector.detect("not a date"), BasicContentType::PlainText);
    }

    #[test]
    fn test_address_detection() {
        let detector = ContentDetector::new();
        
        assert_eq!(detector.detect("台北市信義區信義路五段7號"), BasicContentType::Address);
        assert_eq!(detector.detect("123 Main Street, New York"), BasicContentType::Address);
        assert_eq!(detector.detect("short st"), BasicContentType::PlainText); // 太短
    }

    #[test]
    fn test_create_event() {
        let detector = ContentDetector::new();
        
        let event = detector.create_event("https://example.com".to_string(), Some("browser".to_string()));
        
        assert_eq!(event.content_type, BasicContentType::Url);
        assert_eq!(event.content, "https://example.com");
        assert_eq!(event.source_app, Some("browser".to_string()));
        assert!(event.content_length > 0);
        assert!(!event.content_hash.is_empty());
    }
}