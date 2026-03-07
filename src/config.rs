use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_index")]
    pub index: IndexConfig,
    #[serde(default = "default_embedding")]
    pub embedding: EmbeddingConfig,
    #[serde(default = "default_retrieval")]
    pub retrieval: RetrievalConfig,
    #[serde(default = "default_memory")]
    pub memory: MemoryConfig,
    #[serde(default = "default_pack")]
    pub pack: PackConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_true")]
    pub follow_gitignore: bool,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: String,
    #[serde(default = "default_extensions")]
    pub default_extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default = "default_035")]
    pub semantic_weight: f64,
    #[serde(default = "default_035")]
    pub lexical_weight: f64,
    #[serde(default = "default_015")]
    pub recency_weight: f64,
    #[serde(default = "default_010")]
    pub importance_weight: f64,
    #[serde(default = "default_005")]
    pub scope_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_extractor")]
    pub extractor: String,
    #[serde(default = "default_min_importance")]
    pub min_importance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackConfig {
    #[serde(default = "default_budget")]
    pub default_budget: usize,
    #[serde(default = "default_true")]
    pub include_sources: bool,
    #[serde(default = "default_true")]
    pub include_snippets: bool,
}

// ── Defaults ──

fn default_db_path() -> String {
    "~/.ctxgrep/index.db".into()
}
fn default_true() -> bool {
    true
}
fn default_max_file_size() -> String {
    "5MB".into()
}
fn default_extensions() -> Vec<String> {
    [
        "md", "txt", "org", "rst", "jsonl", "json", "py", "ts", "js", "go", "rs", "lua",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
fn default_provider() -> String {
    "local".into()
}
fn default_model() -> String {
    "all-minilm-l6-v2".into()
}
fn default_dimensions() -> usize {
    384
}
fn default_mode() -> String {
    "hybrid".into()
}
fn default_top_k() -> usize {
    10
}
fn default_035() -> f64 {
    0.35
}
fn default_015() -> f64 {
    0.15
}
fn default_010() -> f64 {
    0.10
}
fn default_005() -> f64 {
    0.05
}
fn default_extractor() -> String {
    "heuristic".into()
}
fn default_min_importance() -> f64 {
    0.60
}
fn default_budget() -> usize {
    4000
}

fn default_index() -> IndexConfig {
    IndexConfig {
        db_path: default_db_path(),
        follow_gitignore: true,
        max_file_size: default_max_file_size(),
        default_extensions: default_extensions(),
    }
}
fn default_embedding() -> EmbeddingConfig {
    EmbeddingConfig {
        provider: default_provider(),
        model: default_model(),
        dimensions: default_dimensions(),
    }
}
fn default_retrieval() -> RetrievalConfig {
    RetrievalConfig {
        default_mode: default_mode(),
        top_k: default_top_k(),
        semantic_weight: 0.35,
        lexical_weight: 0.35,
        recency_weight: 0.15,
        importance_weight: 0.10,
        scope_weight: 0.05,
    }
}
fn default_memory() -> MemoryConfig {
    MemoryConfig {
        enabled: true,
        extractor: default_extractor(),
        min_importance: default_min_importance(),
    }
}
fn default_pack() -> PackConfig {
    PackConfig {
        default_budget: default_budget(),
        include_sources: true,
        include_snippets: true,
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            index: default_index(),
            embedding: default_embedding(),
            retrieval: default_retrieval(),
            memory: default_memory(),
            pack: default_pack(),
        }
    }
}

// ── Helpers ──

pub fn ctxgrep_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ctxgrep")
}

pub fn config_path() -> PathBuf {
    ctxgrep_dir().join("config.toml")
}

pub fn resolve_db_path(config: &Config) -> PathBuf {
    let raw = &config.index.db_path;
    if let Some(stripped) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(stripped)
    } else {
        PathBuf::from(raw)
    }
}

pub fn parse_max_file_size(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    if let Some(n) = s.strip_suffix("MB") {
        n.trim().parse::<u64>().unwrap_or(5) * 1024 * 1024
    } else if let Some(n) = s.strip_suffix("KB") {
        n.trim().parse::<u64>().unwrap_or(5120) * 1024
    } else if let Some(n) = s.strip_suffix("GB") {
        n.trim().parse::<u64>().unwrap_or(1) * 1024 * 1024 * 1024
    } else {
        s.parse::<u64>().unwrap_or(5 * 1024 * 1024)
    }
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

pub fn ensure_dirs() -> Result<()> {
    let dir = ctxgrep_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(dir.join("cache"))?;
    Ok(())
}
