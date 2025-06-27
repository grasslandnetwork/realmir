use crate::error::{CliptionsError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use regex::Regex;

/// Tweet ID extracted from URLs
pub type TweetId = String;

/// Represents announcement data for social media
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementData {
    pub round_id: String,
    pub target_time: String,
    pub hashtags: Vec<String>,
    pub message: String,
    pub prize_pool: Option<f64>,
}

/// Represents a social media task execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub task_name: String,
    pub parameters: HashMap<String, String>,
    pub timeout_seconds: u32,
}

/// Social media task interface
pub trait SocialTask {
    /// Execute the social media task
    fn execute(&self, context: &TaskContext) -> Result<String>;
    
    /// Get task name
    fn get_name(&self) -> &str;
    
    /// Validate task parameters
    fn validate_parameters(&self, params: &HashMap<String, String>) -> Result<()>;
}

/// URL parser for social media platforms
pub struct UrlParser {
    twitter_regex: Regex,
}

impl UrlParser {
    /// Create a new URL parser
    pub fn new() -> Result<Self> {
        let twitter_regex = Regex::new(r"https?://(?:www\.)?(?:twitter\.com|x\.com)/[^/]+/status/(\d+)")
            .map_err(|e| CliptionsError::ValidationError(format!("Invalid regex: {}", e)))?;
        
        Ok(Self {
            twitter_regex,
        })
    }

    /// Extract tweet ID from Twitter/X URL
    pub fn extract_tweet_id(&self, url: &str) -> Result<TweetId> {
        if let Some(captures) = self.twitter_regex.captures(url) {
            if let Some(tweet_id) = captures.get(1) {
                return Ok(tweet_id.as_str().to_string());
            }
        }
        
        Err(CliptionsError::ValidationError(
            format!("Invalid Twitter URL: {}", url)
        ))
    }

    /// Validate URL format
    pub fn validate_url(&self, url: &str) -> Result<()> {
        Url::parse(url)
            .map_err(|e| CliptionsError::ValidationError(format!("Invalid URL: {}", e)))?;
        Ok(())
    }

    /// Extract domain from URL
    pub fn extract_domain(&self, url: &str) -> Result<String> {
        let parsed = Url::parse(url)
            .map_err(|e| CliptionsError::ValidationError(format!("Invalid URL: {}", e)))?;
        
        Ok(parsed.domain().unwrap_or("unknown").to_string())
    }
}

impl Default for UrlParser {
    fn default() -> Self {
        Self::new().expect("Failed to create URL parser")
    }
}

/// Hashtag manager for social media posts
pub struct HashtagManager {
    standard_hashtags: Vec<String>,
}

impl HashtagManager {
    /// Create a new hashtag manager
    pub fn new() -> Self {
        Self {
            standard_hashtags: vec![
                "#Cliptions".to_string(),
                "#PredictionMarket".to_string(),
                "#CLIP".to_string(),
            ],
        }
    }

    /// Create hashtag manager with custom default hashtags
    pub fn with_defaults(hashtags: Vec<String>) -> Self {
        Self {
            standard_hashtags: hashtags,
        }
    }

    /// Generate hashtags for a round
    pub fn generate_hashtags(&self, round_id: &str, custom_hashtags: Option<Vec<String>>) -> Vec<String> {
        let mut hashtags = self.standard_hashtags.clone();
        
        // Add round-specific hashtag
        hashtags.push(format!("#{}", round_id));
        
        // Add custom hashtags if provided
        if let Some(custom) = custom_hashtags {
            hashtags.extend(custom);
        }
        
        hashtags
    }

    /// Format hashtags for social media
    pub fn format_hashtags(&self, hashtags: &[String]) -> String {
        hashtags.join(" ")
    }

    /// Extract hashtags from text
    pub fn extract_hashtags(&self, text: &str) -> Vec<String> {
        let hashtag_regex = Regex::new(r"#\w+").unwrap();
        hashtag_regex
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    /// Validate hashtag format
    pub fn validate_hashtag(&self, hashtag: &str) -> bool {
        let hashtag_regex = Regex::new(r"^#\w+$").unwrap();
        hashtag_regex.is_match(hashtag)
    }
}

impl Default for HashtagManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Announcement formatter for different types of round announcements
pub struct AnnouncementFormatter {
    hashtag_manager: HashtagManager,
}

impl AnnouncementFormatter {
    /// Create a new announcement formatter
    pub fn new() -> Self {
        Self {
            hashtag_manager: HashtagManager::new(),
        }
    }

    /// Create announcement formatter with custom hashtag manager
    pub fn with_hashtag_manager(hashtag_manager: HashtagManager) -> Self {
        Self {
            hashtag_manager,
        }
    }

    /// Create a standard round announcement
    pub fn create_standard_announcement(&self, data: &AnnouncementData) -> String {
        let hashtags = self.hashtag_manager.generate_hashtags(&data.round_id, None);
        let hashtag_string = self.hashtag_manager.format_hashtags(&hashtags);
        
        let prize_info = if let Some(prize) = data.prize_pool {
            format!(" Prize pool: {} TAO.", prize)
        } else {
            String::new()
        };
        
        format!(
            "🎯 {} is now live! Target will be revealed at {}.{} Submit your predictions below! {}",
            data.round_id,
            data.target_time,
            prize_info,
            hashtag_string
        )
    }

    /// Create a custom announcement with provided message
    pub fn create_custom_announcement(&self, data: &AnnouncementData) -> String {
        let hashtags = self.hashtag_manager.generate_hashtags(&data.round_id, Some(data.hashtags.clone()));
        let hashtag_string = self.hashtag_manager.format_hashtags(&hashtags);
        
        format!("{} {}", data.message, hashtag_string)
    }

    /// Format announcement based on type
    pub fn format_announcement(&self, data: &AnnouncementData, use_custom: bool) -> String {
        if use_custom && !data.message.is_empty() {
            self.create_custom_announcement(data)
        } else {
            self.create_standard_announcement(data)
        }
    }
}

impl Default for AnnouncementFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock social media task for testing
pub struct MockSocialTask {
    name: String,
    should_succeed: bool,
}

impl MockSocialTask {
    /// Create a new mock task that succeeds
    pub fn new(name: String) -> Self {
        Self {
            name,
            should_succeed: true,
        }
    }

    /// Create a mock task that fails
    pub fn new_failing(name: String) -> Self {
        Self {
            name,
            should_succeed: false,
        }
    }
}

impl SocialTask for MockSocialTask {
    fn execute(&self, context: &TaskContext) -> Result<String> {
        if self.should_succeed {
            Ok(format!("Mock task '{}' executed successfully with context: {:?}", self.name, context))
        } else {
            Err(CliptionsError::ValidationError(
                format!("Mock task '{}' failed", self.name)
            ))
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn validate_parameters(&self, _params: &HashMap<String, String>) -> Result<()> {
        Ok(())
    }
}

/// Social media workflow manager
pub struct SocialWorkflow {
    tasks: Vec<Box<dyn SocialTask>>,
    url_parser: UrlParser,
    announcement_formatter: AnnouncementFormatter,
}

impl SocialWorkflow {
    /// Create a new social workflow
    pub fn new() -> Result<Self> {
        Ok(Self {
            tasks: Vec::new(),
            url_parser: UrlParser::new()?,
            announcement_formatter: AnnouncementFormatter::new(),
        })
    }

    /// Add a task to the workflow
    pub fn add_task(&mut self, task: Box<dyn SocialTask>) {
        self.tasks.push(task);
    }

    /// Execute all tasks in the workflow
    pub fn execute_workflow(&self, contexts: &[TaskContext]) -> Result<Vec<String>> {
        let mut results = Vec::new();
        
        for (task, context) in self.tasks.iter().zip(contexts.iter()) {
            // Validate parameters first
            task.validate_parameters(&context.parameters)?;
            
            // Execute the task
            let result = task.execute(context)?;
            results.push(result);
        }
        
        Ok(results)
    }

    /// Get URL parser
    pub fn get_url_parser(&self) -> &UrlParser {
        &self.url_parser
    }

    /// Get announcement formatter
    pub fn get_announcement_formatter(&self) -> &AnnouncementFormatter {
        &self.announcement_formatter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tweet_id_from_url() {
        let parser = UrlParser::new().unwrap();
        
        // Test Twitter URL
        let twitter_url = "https://twitter.com/cliptions_testnet/status/1234567890";
        let tweet_id = parser.extract_tweet_id(twitter_url).unwrap();
        assert_eq!(tweet_id, "1234567890");
        
        // Test X URL
        let x_url = "https://x.com/cliptions_testnet/status/9876543210";
        let tweet_id = parser.extract_tweet_id(x_url).unwrap();
        assert_eq!(tweet_id, "9876543210");
        
        // Test invalid URL
        let invalid_url = "https://example.com/not-a-tweet";
        assert!(parser.extract_tweet_id(invalid_url).is_err());
    }

    #[test]
    fn test_validate_url() {
        let parser = UrlParser::new().unwrap();
        
        assert!(parser.validate_url("https://twitter.com/test").is_ok());
        assert!(parser.validate_url("http://example.com").is_ok());
        assert!(parser.validate_url("not-a-url").is_err());
    }

    #[test]
    fn test_extract_domain() {
        let parser = UrlParser::new().unwrap();
        
        let domain = parser.extract_domain("https://twitter.com/test").unwrap();
        assert_eq!(domain, "twitter.com");
        
        let domain = parser.extract_domain("https://x.com/test").unwrap();
        assert_eq!(domain, "x.com");
    }

    #[test]
    fn test_generate_hashtags() {
        let hashtag_manager = HashtagManager::new();
        
        let hashtags = hashtag_manager.generate_hashtags("round1", None);
        assert!(hashtags.contains(&"#Cliptions".to_string()));
        assert!(hashtags.contains(&"#round1".to_string()));
        
        let custom_hashtags = vec!["#custom".to_string()];
        let hashtags = hashtag_manager.generate_hashtags("round2", Some(custom_hashtags));
        assert!(hashtags.contains(&"#custom".to_string()));
    }

    #[test]
    fn test_custom_hashtags() {
        let custom_defaults = vec!["#CustomTag".to_string()];
        let hashtag_manager = HashtagManager::with_defaults(custom_defaults);
        
        let hashtags = hashtag_manager.generate_hashtags("round1", None);
        assert!(hashtags.contains(&"#CustomTag".to_string()));
        assert!(hashtags.contains(&"#round1".to_string()));
    }

    #[test]
    fn test_format_hashtags() {
        let hashtag_manager = HashtagManager::new();
        let hashtags = vec!["#tag1".to_string(), "#tag2".to_string()];
        
        let formatted = hashtag_manager.format_hashtags(&hashtags);
        assert_eq!(formatted, "#tag1 #tag2");
    }

    #[test]
    fn test_extract_hashtags() {
        let hashtag_manager = HashtagManager::new();
        let text = "This is a tweet with #hashtag1 and #hashtag2";
        
        let hashtags = hashtag_manager.extract_hashtags(text);
        assert_eq!(hashtags, vec!["#hashtag1", "#hashtag2"]);
    }

    #[test]
    fn test_validate_hashtag() {
        let hashtag_manager = HashtagManager::new();
        
        assert!(hashtag_manager.validate_hashtag("#validhashtag"));
        assert!(hashtag_manager.validate_hashtag("#valid123"));
        assert!(!hashtag_manager.validate_hashtag("invalid"));
        assert!(!hashtag_manager.validate_hashtag("#invalid-tag"));
    }

    #[test]
    fn test_create_standard_round_announcement() {
        let formatter = AnnouncementFormatter::new();
        let data = AnnouncementData {
            round_id: "round1".to_string(),
            target_time: "2024-01-01 12:00:00".to_string(),
            hashtags: vec![],
            message: "".to_string(),
            prize_pool: Some(100.0),
        };
        
        let announcement = formatter.create_standard_announcement(&data);
        assert!(announcement.contains("round1 is now live"));
        assert!(announcement.contains("2024-01-01 12:00:00"));
        assert!(announcement.contains("Prize pool: 100 TAO"));
        assert!(announcement.contains("#Cliptions"));
        assert!(announcement.contains("#round1"));
    }

    #[test]
    fn test_create_custom_round_announcement() {
        let formatter = AnnouncementFormatter::new();
        let data = AnnouncementData {
            round_id: "round2".to_string(),
            target_time: "2024-01-01 12:00:00".to_string(),
            hashtags: vec!["#custom".to_string()],
            message: "Custom announcement message".to_string(),
            prize_pool: None,
        };
        
        let announcement = formatter.create_custom_announcement(&data);
        assert!(announcement.contains("Custom announcement message"));
        assert!(announcement.contains("#custom"));
        assert!(announcement.contains("#round2"));
    }

    #[test]
    fn test_full_announcement_flow() {
        let formatter = AnnouncementFormatter::new();
        let data = AnnouncementData {
            round_id: "round3".to_string(),
            target_time: "2024-01-01 12:00:00".to_string(),
            hashtags: vec![],
            message: "".to_string(),
            prize_pool: Some(50.0),
        };
        
        // Test standard announcement
        let standard = formatter.format_announcement(&data, false);
        assert!(standard.contains("round3 is now live"));
        
        // Test custom announcement (should fallback to standard when message is empty)
        let custom = formatter.format_announcement(&data, true);
        assert!(custom.contains("round3 is now live"));
    }

    #[test]
    fn test_social_task_execute_success() {
        let task = MockSocialTask::new("test_task".to_string());
        let context = TaskContext {
            task_name: "test_task".to_string(),
            parameters: HashMap::new(),
            timeout_seconds: 30,
        };
        
        let result = task.execute(&context);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("executed successfully"));
    }

    #[test]
    fn test_social_task_execute_with_kwargs() {
        let task = MockSocialTask::new("test_task".to_string());
        let mut parameters = HashMap::new();
        parameters.insert("param1".to_string(), "value1".to_string());
        
        let context = TaskContext {
            task_name: "test_task".to_string(),
            parameters,
            timeout_seconds: 30,
        };
        
        let result = task.execute(&context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_social_task_failure() {
        let task = MockSocialTask::new_failing("failing_task".to_string());
        let context = TaskContext {
            task_name: "failing_task".to_string(),
            parameters: HashMap::new(),
            timeout_seconds: 30,
        };
        
        let result = task.execute(&context);
        assert!(result.is_err());
    }

    #[test]
    fn test_social_workflow() {
        let mut workflow = SocialWorkflow::new().unwrap();
        
        let task1 = Box::new(MockSocialTask::new("task1".to_string()));
        let task2 = Box::new(MockSocialTask::new("task2".to_string()));
        
        workflow.add_task(task1);
        workflow.add_task(task2);
        
        let contexts = vec![
            TaskContext {
                task_name: "task1".to_string(),
                parameters: HashMap::new(),
                timeout_seconds: 30,
            },
            TaskContext {
                task_name: "task2".to_string(),
                parameters: HashMap::new(),
                timeout_seconds: 30,
            },
        ];
        
        let results = workflow.execute_workflow(&contexts).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("task1"));
        assert!(results[1].contains("task2"));
    }

    #[test]
    fn test_announcement_data_validation() {
        let data = AnnouncementData {
            round_id: "round1".to_string(),
            target_time: "2024-01-01 12:00:00".to_string(),
            hashtags: vec!["#test".to_string()],
            message: "Test message".to_string(),
            prize_pool: Some(100.0),
        };
        
        assert_eq!(data.round_id, "round1");
        assert_eq!(data.target_time, "2024-01-01 12:00:00");
        assert_eq!(data.hashtags, vec!["#test"]);
        assert_eq!(data.message, "Test message");
        assert_eq!(data.prize_pool, Some(100.0));
    }
}