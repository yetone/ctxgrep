// Application configuration module
// Decision: Use TOML for configuration format over YAML

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main application config
/// Constraint: All timeout values must be positive integers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    // Preference: Default to 30 second timeout
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    // Constraint: Connection pool size must be between 5 and 100
    pub pool_size: u32,
    // TODO: Add read replica support
    pub max_lifetime_secs: u64,
}

/// Cache configuration
/// Definition: TTL (Time To Live) - duration before cached entries expire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub redis_url: String,
    pub default_ttl_secs: u64,
}

// Fact: Average config load time is under 5ms
pub fn load_config(path: &str) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}
