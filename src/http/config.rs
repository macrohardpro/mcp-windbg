use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),
    
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// HTTP server port
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// Maximum upload size in bytes (default: 500MB)
    #[serde(default = "default_max_upload_size")]
    pub max_upload_size: usize,
    
    /// Maximum concurrent analysis sessions (default: 5)
    #[serde(default = "default_max_concurrent_sessions")]
    pub max_concurrent_sessions: usize,
    
    /// Cleanup task interval in seconds (default: 3600 = 1 hour)
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_secs: u64,
    
    /// Session time-to-live in seconds (default: 86400 = 24 hours)
    #[serde(default = "default_session_ttl")]
    pub session_ttl_secs: u64,

    /// Maximum stored session directories (default: 50)
    #[serde(default = "default_max_stored_sessions")]
    pub max_stored_sessions: usize,

    /// Path configuration
    #[serde(default)]
    pub paths: PathConfig,
    
    /// LLM API configuration
    pub llm: LlmConfig,
    
    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

/// Path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    /// Path to mcp-windbg-rs executable
    #[serde(default = "default_mcp_server_path")]
    pub mcp_server: PathBuf,
    
    /// Path to Python interpreter
    #[serde(default = "default_python_path")]
    pub python: PathBuf,
    
    /// Path to CDB.exe
    #[serde(default = "default_cdb_path")]
    pub cdb: PathBuf,
    
    /// Workspace root directory for temporary files
    #[serde(default = "default_workspace_root")]
    pub workspace_root: PathBuf,
}

/// LLM API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// API key for LLM service
    pub api_key: String,
    
    /// API base URL
    pub api_base: String,
    
    /// Model name
    pub model: String,
    
    /// Maximum tool-calling turns (default: 30)
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    
    /// Analysis timeout in seconds (default: 300 = 5 minutes)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting (default: true)
    #[serde(default = "default_rate_limit_enabled")]
    pub enabled: bool,
    
    /// Maximum uploads per minute per IP (default: 3)
    #[serde(default = "default_max_uploads_per_minute")]
    pub max_uploads_per_minute: usize,
}

// Default value functions
fn default_port() -> u16 {
    8080
}

fn default_max_upload_size() -> usize {
    500 * 1024 * 1024 // 500MB
}

fn default_max_concurrent_sessions() -> usize {
    5
}

fn default_cleanup_interval() -> u64 {
    3600 // 1 hour
}

fn default_session_ttl() -> u64 {
    86400 // 24 hours
}

fn default_max_stored_sessions() -> usize {
    50
}

fn default_mcp_server_path() -> PathBuf {
    PathBuf::from("mcp-windbg-rs.exe")
}

fn default_python_path() -> PathBuf {
    PathBuf::from("python")
}

fn default_cdb_path() -> PathBuf {
    PathBuf::from("cdb.exe")
}

fn default_workspace_root() -> PathBuf {
    std::env::temp_dir().join("web-dump-debugger")
}

fn default_max_turns() -> u32 {
    30
}

fn default_timeout() -> u64 {
    300 // 5 minutes
}

fn default_rate_limit_enabled() -> bool {
    true
}

fn default_max_uploads_per_minute() -> usize {
    3
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            mcp_server: default_mcp_server_path(),
            python: default_python_path(),
            cdb: default_cdb_path(),
            workspace_root: default_workspace_root(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: default_rate_limit_enabled(),
            max_uploads_per_minute: default_max_uploads_per_minute(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
    
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        let api_key = std::env::var("API_KEY")
            .map_err(|_| ConfigError::ValidationError("API_KEY environment variable not set".to_string()))?;
        let api_base = std::env::var("API_BASE")
            .map_err(|_| ConfigError::ValidationError("API_BASE environment variable not set".to_string()))?;
        let model = std::env::var("MODEL")
            .map_err(|_| ConfigError::ValidationError("MODEL environment variable not set".to_string()))?;
        
        let port = std::env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_port);
        
        let max_upload_size = std::env::var("MAX_UPLOAD_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_max_upload_size);
        
        let max_concurrent_sessions = std::env::var("MAX_CONCURRENT_SESSIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_max_concurrent_sessions);
        
        let cleanup_interval_secs = std::env::var("CLEANUP_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_cleanup_interval);
        
        let session_ttl_secs = std::env::var("SESSION_TTL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_session_ttl);

        let max_stored_sessions = std::env::var("MAX_STORED_SESSIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_max_stored_sessions);

        let max_turns = std::env::var("MAX_TURNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_max_turns);
        
        let timeout_secs = std::env::var("TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_timeout);
        
        let mcp_server = std::env::var("MCP_SERVER_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_mcp_server_path());
        
        let python = std::env::var("PYTHON_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_python_path());
        
        let cdb = std::env::var("CDB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_cdb_path());
        
        let workspace_root = std::env::var("WORKSPACE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_workspace_root());
        
        let rate_limit_enabled = std::env::var("RATE_LIMIT_ENABLED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_rate_limit_enabled);
        
        let max_uploads_per_minute = std::env::var("MAX_UPLOADS_PER_MINUTE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_max_uploads_per_minute);
        
        let config = ServerConfig {
            port,
            max_upload_size,
            max_concurrent_sessions,
            cleanup_interval_secs,
            session_ttl_secs,
            max_stored_sessions,
            paths: PathConfig {
                mcp_server,
                python,
                cdb,
                workspace_root,
            },
            llm: LlmConfig {
                api_key,
                api_base,
                model,
                max_turns,
                timeout_secs,
            },
            rate_limit: RateLimitConfig {
                enabled: rate_limit_enabled,
                max_uploads_per_minute,
            },
        };
        
        config.validate()?;
        Ok(config)
    }
    
    /// Validate configuration
    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::ValidationError("Port cannot be 0".to_string()));
        }
        
        if self.max_upload_size == 0 {
            return Err(ConfigError::ValidationError("Max upload size cannot be 0".to_string()));
        }
        
        if self.max_concurrent_sessions == 0 {
            return Err(ConfigError::ValidationError("Max concurrent sessions cannot be 0".to_string()));
        }
        
        if self.llm.api_key.is_empty() {
            return Err(ConfigError::ValidationError("LLM API key cannot be empty".to_string()));
        }
        
        if self.llm.api_base.is_empty() {
            return Err(ConfigError::ValidationError("LLM API base URL cannot be empty".to_string()));
        }
        
        if self.llm.model.is_empty() {
            return Err(ConfigError::ValidationError("LLM model cannot be empty".to_string()));
        }
        
        Ok(())
    }
    
    /// Get cleanup interval as Duration
    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_secs(self.cleanup_interval_secs)
    }
    
    /// Get session TTL as Duration
    pub fn session_ttl(&self) -> Duration {
        Duration::from_secs(self.session_ttl_secs)
    }
    
    /// Get analysis timeout as Duration
    pub fn analysis_timeout(&self) -> Duration {
        Duration::from_secs(self.llm.timeout_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = ServerConfig {
            port: default_port(),
            max_upload_size: default_max_upload_size(),
            max_concurrent_sessions: default_max_concurrent_sessions(),
            cleanup_interval_secs: default_cleanup_interval(),
            session_ttl_secs: default_session_ttl(),
            max_stored_sessions: default_max_stored_sessions(),
            paths: PathConfig::default(),
            llm: LlmConfig {
                api_key: "test-key".to_string(),
                api_base: "https://api.example.com".to_string(),
                model: "test-model".to_string(),
                max_turns: default_max_turns(),
                timeout_secs: default_timeout(),
            },
            rate_limit: RateLimitConfig::default(),
        };
        
        assert_eq!(config.port, 8080);
        assert_eq!(config.max_upload_size, 500 * 1024 * 1024);
        assert_eq!(config.max_concurrent_sessions, 5);
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_validation_errors() {
        let mut config = ServerConfig {
            port: 0,
            max_upload_size: default_max_upload_size(),
            max_concurrent_sessions: default_max_concurrent_sessions(),
            cleanup_interval_secs: default_cleanup_interval(),
            session_ttl_secs: default_session_ttl(),
            max_stored_sessions: default_max_stored_sessions(),
            paths: PathConfig::default(),
            llm: LlmConfig {
                api_key: "test-key".to_string(),
                api_base: "https://api.example.com".to_string(),
                model: "test-model".to_string(),
                max_turns: default_max_turns(),
                timeout_secs: default_timeout(),
            },
            rate_limit: RateLimitConfig::default(),
        };
        
        assert!(config.validate().is_err());
        
        config.port = 8080;
        config.llm.api_key = String::new();
        assert!(config.validate().is_err());
    }
}
