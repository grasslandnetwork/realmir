use crate::error::{CliptionsError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// OpenAI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
    pub temperature: f64,
    pub daily_spending_limit_usd: f64,
    pub max_tokens: u32,
    pub project_id: String,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            daily_spending_limit_usd: 5.0,
            max_tokens: 4000,
            project_id: String::new(),
        }
    }
}

/// Browser use configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserUseConfig {
    pub max_steps: u32,
    pub use_vision: bool,
    pub timeout_seconds: u32,
}

impl Default for BrowserUseConfig {
    fn default() -> Self {
        Self {
            max_steps: 25,
            use_vision: true,
            timeout_seconds: 300,
        }
    }
}

/// Cost tracking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTrackingConfig {
    pub enabled: bool,
    pub sync_frequency_hours: u32,
    pub alert_threshold_percent: f64,
}

impl Default for CostTrackingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sync_frequency_hours: 1,
            alert_threshold_percent: 80.0,
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliptionsConfig {
    pub openai: OpenAIConfig,
    pub browser_use: BrowserUseConfig,
    pub cost_tracking: CostTrackingConfig,
}

impl Default for CliptionsConfig {
    fn default() -> Self {
        Self {
            openai: OpenAIConfig::default(),
            browser_use: BrowserUseConfig::default(),
            cost_tracking: CostTrackingConfig::default(),
        }
    }
}

/// Cost tracking data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCosts {
    pub date: String,
    pub total_cost_usd: f64,
    pub breakdown: HashMap<String, f64>,
}

/// Usage tracking data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_requests: u64,
    pub breakdown: HashMap<String, UsageBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageBreakdown {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u64,
}

/// Configuration manager
#[derive(Debug)]
pub struct ConfigManager {
    config: CliptionsConfig,
    config_path: PathBuf,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self> {
        let config_path = PathBuf::from("config/llm.yaml");
        let config = Self::load_config(&config_path)?;
        
        Ok(Self {
            config,
            config_path,
        })
    }

    /// Create configuration manager with custom path
    pub fn with_path<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = Self::load_config(&config_path)?;
        
        Ok(Self {
            config,
            config_path,
        })
    }

    /// Load configuration from file
    fn load_config(config_path: &Path) -> Result<CliptionsConfig> {
        if !config_path.exists() {
            return Err(CliptionsError::ConfigError(
                format!("Configuration file not found: {}", config_path.display())
            ));
        }

        let content = fs::read_to_string(config_path)
            .map_err(|e| CliptionsError::ConfigError(
                format!("Failed to read config file: {}", e)
            ))?;

        let mut config: CliptionsConfig = serde_yaml::from_str(&content)
            .map_err(|e| CliptionsError::ConfigError(
                format!("Failed to parse config file: {}", e)
            ))?;

        // Override with environment variables if present
        Self::apply_env_overrides(&mut config);

        // Validate configuration
        Self::validate_config(&config)?;

        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(config: &mut CliptionsConfig) {
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            config.openai.api_key = api_key;
        }

        if let Ok(project_id) = env::var("OPENAI_PROJECT_ID") {
            config.openai.project_id = project_id;
        }

        if let Ok(limit) = env::var("OPENAI_DAILY_SPENDING_LIMIT") {
            if let Ok(limit_value) = limit.parse::<f64>() {
                config.openai.daily_spending_limit_usd = limit_value;
            }
        }
    }

    /// Validate configuration
    fn validate_config(config: &CliptionsConfig) -> Result<()> {
        if config.openai.api_key.is_empty() {
            return Err(CliptionsError::ConfigError(
                "OpenAI API key is required".to_string()
            ));
        }

        if config.openai.project_id.is_empty() {
            return Err(CliptionsError::ConfigError(
                "OpenAI project ID is required".to_string()
            ));
        }

        if config.openai.daily_spending_limit_usd <= 0.0 {
            return Err(CliptionsError::ConfigError(
                "Daily spending limit must be positive".to_string()
            ));
        }

        if config.openai.temperature < 0.0 || config.openai.temperature > 2.0 {
            return Err(CliptionsError::ConfigError(
                "Temperature must be between 0.0 and 2.0".to_string()
            ));
        }

        if config.cost_tracking.alert_threshold_percent < 0.0 || config.cost_tracking.alert_threshold_percent > 100.0 {
            return Err(CliptionsError::ConfigError(
                "Alert threshold percent must be between 0 and 100".to_string()
            ));
        }

        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &CliptionsConfig {
        &self.config
    }

    /// Get OpenAI configuration
    pub fn get_openai_config(&self) -> &OpenAIConfig {
        &self.config.openai
    }

    /// Get browser use configuration
    pub fn get_browser_config(&self) -> &BrowserUseConfig {
        &self.config.browser_use
    }

    /// Get cost tracking configuration
    pub fn get_cost_tracking_config(&self) -> &CostTrackingConfig {
        &self.config.cost_tracking
    }

    /// Update daily spending limit
    pub fn set_daily_spending_limit(&mut self, limit: f64) -> Result<()> {
        if limit <= 0.0 {
            return Err(CliptionsError::ConfigError(
                "Daily spending limit must be positive".to_string()
            ));
        }
        
        self.config.openai.daily_spending_limit_usd = limit;
        Ok(())
    }

    /// Save configuration to file
    pub fn save_config(&self) -> Result<()> {
        let content = serde_yaml::to_string(&self.config)
            .map_err(|e| CliptionsError::ConfigError(
                format!("Failed to serialize config: {}", e)
            ))?;

        fs::write(&self.config_path, content)
            .map_err(|e| CliptionsError::ConfigError(
                format!("Failed to write config file: {}", e)
            ))?;

        Ok(())
    }

    /// Check if spending is under the daily limit
    pub fn check_spending_limit(&self, current_spending: f64) -> Result<bool> {
        let limit = self.config.openai.daily_spending_limit_usd;
        Ok(current_spending < limit)
    }

    /// Check if spending is approaching the alert threshold
    pub fn check_alert_threshold(&self, current_spending: f64) -> bool {
        let limit = self.config.openai.daily_spending_limit_usd;
        let threshold = limit * (self.config.cost_tracking.alert_threshold_percent / 100.0);
        current_spending >= threshold
    }

    /// Get remaining budget for the day
    pub fn get_remaining_budget(&self, current_spending: f64) -> f64 {
        let limit = self.config.openai.daily_spending_limit_usd;
        (limit - current_spending).max(0.0)
    }
}

/// Simple cost tracker for project-specific spending
pub struct CostTracker {
    project_id: String,
    config: ConfigManager,
}

impl CostTracker {
    /// Create a new cost tracker
    pub fn new(project_id: String) -> Result<Self> {
        let config = ConfigManager::new()?;
        Ok(Self {
            project_id,
            config,
        })
    }

    /// Create cost tracker with custom config
    pub fn with_config(project_id: String, config: ConfigManager) -> Self {
        Self {
            project_id,
            config,
        }
    }

    /// Check if execution is allowed based on spending limits
    pub fn check_execution_allowed(&self, current_spending: f64) -> Result<bool> {
        self.config.check_spending_limit(current_spending)
    }

    /// Get spending status information
    pub fn get_spending_status(&self, current_spending: f64) -> SpendingStatus {
        let limit = self.config.get_openai_config().daily_spending_limit_usd;
        let remaining = self.config.get_remaining_budget(current_spending);
        let alert_triggered = self.config.check_alert_threshold(current_spending);
        let over_limit = current_spending >= limit;

        SpendingStatus {
            project_id: self.project_id.clone(),
            current_spending,
            daily_limit: limit,
            remaining_budget: remaining,
            alert_triggered,
            over_limit,
        }
    }

    /// Get project ID
    pub fn get_project_id(&self) -> &str {
        &self.project_id
    }
}

/// Spending status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingStatus {
    pub project_id: String,
    pub current_spending: f64,
    pub daily_limit: f64,
    pub remaining_budget: f64,
    pub alert_triggered: bool,
    pub over_limit: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config(dir: &TempDir) -> PathBuf {
        let config_path = dir.path().join("test_config.yaml");
        let config_content = r#"
openai:
  api_key: "test-api-key"
  model: "gpt-4o"
  temperature: 0.1
  daily_spending_limit_usd: 10.0
  max_tokens: 4000
  project_id: "test-project-id"

browser_use:
  max_steps: 25
  use_vision: true
  timeout_seconds: 300

cost_tracking:
  enabled: true
  sync_frequency_hours: 1
  alert_threshold_percent: 80
"#;
        fs::write(&config_path, config_content).unwrap();
        config_path
    }

    #[test]
    fn test_load_config_includes_api_key() {
        // Ensure clean environment
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("OPENAI_PROJECT_ID");
        env::remove_var("OPENAI_DAILY_SPENDING_LIMIT");
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let config = config_manager.get_config();
        
        assert_eq!(config.openai.api_key, "test-api-key");
        assert_eq!(config.openai.project_id, "test-project-id");
        assert_eq!(config.openai.daily_spending_limit_usd, 10.0);
    }

    #[test]
    fn test_missing_api_key_in_config() {
        // Ensure clean environment
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("OPENAI_PROJECT_ID");
        env::remove_var("OPENAI_DAILY_SPENDING_LIMIT");
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid_config.yaml");
        let config_content = r#"
openai:
  api_key: ""
  model: "gpt-4o"
  project_id: "test-project"
  daily_spending_limit_usd: 10.0
  max_tokens: 4000
  temperature: 0.1

browser_use:
  max_steps: 25
  use_vision: true
  timeout_seconds: 300

cost_tracking:
  enabled: true
  sync_frequency_hours: 1
  alert_threshold_percent: 80
"#;
        fs::write(&config_path, config_content).unwrap();
        
        let result = ConfigManager::with_path(&config_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("OpenAI API key is required"));
    }

    #[test]
    fn test_daily_spending_limit_config_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let config = config_manager.get_openai_config();
        
        assert_eq!(config.daily_spending_limit_usd, 10.0);
    }

    #[test]
    fn test_spending_limit_check_under_limit() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let under_limit = config_manager.check_spending_limit(5.0).unwrap();
        
        assert!(under_limit);
    }

    #[test]
    fn test_spending_limit_check_over_limit() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let over_limit = config_manager.check_spending_limit(15.0).unwrap();
        
        assert!(!over_limit);
    }

    #[test]
    fn test_spending_limit_check_no_data() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let under_limit = config_manager.check_spending_limit(0.0).unwrap();
        
        assert!(under_limit);
    }

    #[test]
    fn test_project_specific_spending_limit_check() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let cost_tracker = CostTracker::with_config("test-proj".to_string(), config_manager);
        
        let status = cost_tracker.get_spending_status(8.0);
        assert_eq!(status.project_id, "test-proj");
        assert_eq!(status.current_spending, 8.0);
        assert_eq!(status.daily_limit, 10.0);
        assert_eq!(status.remaining_budget, 2.0);
        assert!(status.alert_triggered); // 8.0 > 80% of 10.0
        assert!(!status.over_limit);
    }

    #[test]
    fn test_cost_tracking_during_execution() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let cost_tracker = CostTracker::with_config("test-proj".to_string(), config_manager);
        
        // Under limit
        assert!(cost_tracker.check_execution_allowed(5.0).unwrap());
        
        // Over limit
        assert!(!cost_tracker.check_execution_allowed(15.0).unwrap());
    }

    #[test]
    fn test_config_validation() {
        // Ensure clean environment
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("OPENAI_PROJECT_ID");
        env::remove_var("OPENAI_DAILY_SPENDING_LIMIT");
        
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid_config.yaml");
        
        // Test invalid temperature
        let config_content = r#"
openai:
  api_key: "test-key"
  project_id: "test-proj"
  temperature: 3.0
  model: "gpt-4o"
  daily_spending_limit_usd: 10.0
  max_tokens: 4000

browser_use:
  max_steps: 25
  use_vision: true
  timeout_seconds: 300

cost_tracking:
  enabled: true
  sync_frequency_hours: 1
  alert_threshold_percent: 80
"#;
        fs::write(&config_path, config_content).unwrap();
        
        let result = ConfigManager::with_path(&config_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Temperature must be between"));
    }

    #[test]
    fn test_alert_threshold() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        
        // Below threshold (80% of $10 = $8)
        assert!(!config_manager.check_alert_threshold(7.0));
        
        // At threshold
        assert!(config_manager.check_alert_threshold(8.0));
        
        // Above threshold
        assert!(config_manager.check_alert_threshold(9.0));
    }

    #[test]
    fn test_remaining_budget() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        
        // Normal case
        assert_eq!(config_manager.get_remaining_budget(3.0), 7.0);
        
        // Over budget should return 0
        assert_eq!(config_manager.get_remaining_budget(15.0), 0.0);
        
        // Exactly at limit
        assert_eq!(config_manager.get_remaining_budget(10.0), 0.0);
    }

    #[test]
    fn test_env_override() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config(&temp_dir);
        
        // Save the current environment variable if it exists
        let original_api_key = env::var("OPENAI_API_KEY").ok();
        
        // Set our test environment variable
        env::set_var("OPENAI_API_KEY", "env-override-key");
        
        let config_manager = ConfigManager::with_path(&config_path).unwrap();
        let config = config_manager.get_config();
        
        // Should use environment variable value
        assert_eq!(config.openai.api_key, "env-override-key");
        
        // Restore the original environment variable
        match original_api_key {
            Some(key) => env::set_var("OPENAI_API_KEY", key),
            None => env::remove_var("OPENAI_API_KEY"),
        }
    }
}