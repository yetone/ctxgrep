use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Embedder ──

pub enum Embedder {
    Local(Box<LocalEmbedder>),
    OpenAI(OpenAIEmbedder),
    None,
}

impl Embedder {
    pub fn from_config(provider: &str, model: &str, dimensions: usize) -> Self {
        match provider {
            "local" => {
                let cache_dir = crate::config::ctxgrep_dir().join("cache");
                match LocalEmbedder::new(model, Some(&cache_dir.to_string_lossy())) {
                    Ok(e) => Embedder::Local(Box::new(e)),
                    Err(e) => {
                        eprintln!("Warning: failed to initialize local embedder: {e}");
                        eprintln!("Semantic search disabled. Try: ctxgrep doctor");
                        Embedder::None
                    }
                }
            }
            "openai" => {
                if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                    Embedder::OpenAI(OpenAIEmbedder {
                        api_key: key,
                        model: model.to_string(),
                        _dimensions: dimensions,
                    })
                } else {
                    eprintln!("Warning: OPENAI_API_KEY not set, semantic search disabled");
                    Embedder::None
                }
            }
            "none" => Embedder::None,
            _ => {
                eprintln!(
                    "Warning: unknown embedding provider '{provider}', semantic search disabled"
                );
                Embedder::None
            }
        }
    }

    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        match self {
            Embedder::Local(e) => e.embed_batch(texts),
            Embedder::OpenAI(e) => e.embed_batch(texts).await,
            Embedder::None => bail!("No embedding provider configured"),
        }
    }

    pub async fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        match self {
            Embedder::Local(e) => e.embed_single(text),
            Embedder::OpenAI(e) => e.embed_single(text).await,
            Embedder::None => bail!("No embedding provider configured"),
        }
    }

    pub fn is_available(&self) -> bool {
        !matches!(self, Embedder::None)
    }

    #[allow(dead_code)]
    pub fn dimensions(&self) -> usize {
        match self {
            Embedder::Local(e) => e.dimensions,
            Embedder::OpenAI(_) => 1536,
            Embedder::None => 0,
        }
    }

    #[allow(dead_code)]
    pub fn provider_name(&self) -> &str {
        match self {
            Embedder::Local(_) => "local",
            Embedder::OpenAI(_) => "openai",
            Embedder::None => "none",
        }
    }
}

// ── Local Embedder (fastembed) ──

pub struct LocalEmbedder {
    model: std::sync::Mutex<fastembed::TextEmbedding>,
    dimensions: usize,
}

struct ModelMeta {
    repo: &'static str,
    model_file: &'static str,
    pooling: fastembed::Pooling,
}

const TOKENIZER_FILES: &[&str] = &[
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

fn get_model_meta(model_name: &str) -> ModelMeta {
    match model_name {
        "all-minilm-l6-v2" | "AllMiniLML6V2" => ModelMeta {
            repo: "Qdrant/all-MiniLM-L6-v2-onnx",
            model_file: "model.onnx",
            pooling: fastembed::Pooling::Mean,
        },
        "bge-small-en-v1.5" | "BGESmallENV15" => ModelMeta {
            repo: "Xenova/bge-small-en-v1.5",
            model_file: "onnx/model.onnx",
            pooling: fastembed::Pooling::Cls,
        },
        "multilingual-e5-small" => ModelMeta {
            repo: "intfloat/multilingual-e5-small",
            model_file: "onnx/model.onnx",
            pooling: fastembed::Pooling::Mean,
        },
        "multilingual-e5-base" => ModelMeta {
            repo: "intfloat/multilingual-e5-base",
            model_file: "onnx/model.onnx",
            pooling: fastembed::Pooling::Mean,
        },
        "multilingual-e5-large" => ModelMeta {
            repo: "Qdrant/multilingual-e5-large-onnx",
            model_file: "model.onnx",
            pooling: fastembed::Pooling::Mean,
        },
        _ => ModelMeta {
            repo: "Qdrant/all-MiniLM-L6-v2-onnx",
            model_file: "model.onnx",
            pooling: fastembed::Pooling::Mean,
        },
    }
}

fn model_cache_dir(cache_dir: &Path, repo: &str) -> PathBuf {
    cache_dir.join("models").join(repo.replace('/', "--"))
}

fn all_files_cached(dir: &Path, meta: &ModelMeta) -> bool {
    if !dir
        .join(
            meta.model_file
                .rsplit('/')
                .next()
                .unwrap_or(meta.model_file),
        )
        .exists()
    {
        return false;
    }
    for f in TOKENIZER_FILES {
        if !dir.join(f).exists() {
            return false;
        }
    }
    true
}

fn download_model_files(cache_dir: &Path, meta: &ModelMeta) -> Result<()> {
    let dir = model_cache_dir(cache_dir, meta.repo);
    std::fs::create_dir_all(&dir)?;

    let endpoint =
        std::env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".to_string());

    let mirror_endpoints = if endpoint == "https://huggingface.co" {
        vec![
            "https://huggingface.co".to_string(),
            "https://hf-mirror.com".to_string(),
        ]
    } else {
        vec![endpoint]
    };

    let mut files_to_download: Vec<&str> = Vec::new();
    files_to_download.push(meta.model_file);
    for f in TOKENIZER_FILES {
        files_to_download.push(f);
    }

    for file in &files_to_download {
        let local_name = file.rsplit('/').next().unwrap_or(file);
        let local_path = dir.join(local_name);
        if local_path.exists() {
            continue;
        }

        let mut downloaded = false;
        for ep in &mirror_endpoints {
            let url = format!("{}/{}/resolve/main/{}", ep, meta.repo, file);
            eprintln!("Downloading {} ...", url);

            match download_file_blocking(&url, &local_path) {
                Ok(_) => {
                    downloaded = true;
                    break;
                }
                Err(e) => {
                    eprintln!("  Failed from {}: {}", ep, e);
                    let _ = std::fs::remove_file(&local_path);
                }
            }
        }

        if !downloaded {
            bail!("Failed to download {} from any endpoint", file);
        }
    }

    Ok(())
}

fn download_file_blocking(url: &str, dest: &Path) -> Result<()> {
    let url = url.to_string();
    let dest = dest.to_path_buf();

    // Run in a separate thread to avoid conflicts with the tokio runtime
    let handle = std::thread::spawn(move || -> Result<()> {
        let resp = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()?
            .get(&url)
            .header("User-Agent", "ctxgrep/0.1")
            .send()?;

        if !resp.status().is_success() {
            bail!("HTTP {}: {}", resp.status(), url);
        }

        let bytes = resp.bytes()?;
        std::fs::write(&dest, &bytes)?;
        eprintln!(
            "  Saved {} ({:.1} MB)",
            dest.display(),
            bytes.len() as f64 / 1_048_576.0
        );
        Ok(())
    });

    handle
        .join()
        .map_err(|_| anyhow::anyhow!("Download thread panicked"))?
}

fn load_from_cache(cache_dir: &Path, meta: &ModelMeta) -> Result<fastembed::TextEmbedding> {
    use fastembed::{
        InitOptionsUserDefined, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel,
    };

    let dir = model_cache_dir(cache_dir, meta.repo);
    let model_local = meta
        .model_file
        .rsplit('/')
        .next()
        .unwrap_or(meta.model_file);

    let onnx_file = std::fs::read(dir.join(model_local))?;
    let tokenizer_files = TokenizerFiles {
        tokenizer_file: std::fs::read(dir.join("tokenizer.json"))?,
        config_file: std::fs::read(dir.join("config.json"))?,
        special_tokens_map_file: std::fs::read(dir.join("special_tokens_map.json"))?,
        tokenizer_config_file: std::fs::read(dir.join("tokenizer_config.json"))?,
    };

    let user_model = UserDefinedEmbeddingModel::new(onnx_file, tokenizer_files)
        .with_pooling(meta.pooling.clone());

    let te = TextEmbedding::try_new_from_user_defined(user_model, InitOptionsUserDefined::new())?;
    Ok(te)
}

impl LocalEmbedder {
    pub fn new(model_name: &str, cache_dir: Option<&str>) -> Result<Self> {
        use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

        let meta = get_model_meta(model_name);

        let (model_enum, dims) = match model_name {
            "all-minilm-l6-v2" | "AllMiniLML6V2" => (EmbeddingModel::AllMiniLML6V2, 384),
            "bge-small-en-v1.5" | "BGESmallENV15" => (EmbeddingModel::BGESmallENV15, 384),
            "multilingual-e5-small" => (EmbeddingModel::MultilingualE5Small, 384),
            "multilingual-e5-base" => (EmbeddingModel::MultilingualE5Base, 768),
            "multilingual-e5-large" => (EmbeddingModel::MultilingualE5Large, 1024),
            _ => (EmbeddingModel::AllMiniLML6V2, 384),
        };

        let cache_path = cache_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::config::ctxgrep_dir().join("cache"));

        // 1. Check if we already have files cached locally (custom download)
        let custom_dir = model_cache_dir(&cache_path, meta.repo);
        if all_files_cached(&custom_dir, &meta) {
            eprintln!("Loading model from local cache: {}", custom_dir.display());
            match load_from_cache(&cache_path, &meta) {
                Ok(model) => {
                    return Ok(Self {
                        model: std::sync::Mutex::new(model),
                        dimensions: dims,
                    });
                }
                Err(e) => {
                    eprintln!("Warning: cached model load failed: {e}, trying other methods...");
                }
            }
        }

        // 2. Try standard hf-hub download via fastembed
        let mut opts = TextInitOptions::new(model_enum).with_show_download_progress(true);
        if let Some(dir) = cache_dir {
            opts = opts.with_cache_dir(dir.into());
        }

        match TextEmbedding::try_new(opts) {
            Ok(model) => {
                return Ok(Self {
                    model: std::sync::Mutex::new(model),
                    dimensions: dims,
                });
            }
            Err(e) => {
                eprintln!("Standard model download failed: {e}");
                eprintln!("Trying mirror download...");
            }
        }

        // 3. Fallback: download from mirror and load as user-defined
        download_model_files(&cache_path, &meta)?;
        let model = load_from_cache(&cache_path, &meta)?;
        Ok(Self {
            model: std::sync::Mutex::new(model),
            dimensions: dims,
        })
    }

    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut model = self
            .model
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {e}"))?;
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = model.embed(refs, None)?;
        Ok(embeddings)
    }

    pub fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let mut model = self
            .model
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {e}"))?;
        let results = model.embed(vec![text], None)?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty embedding response"))
    }
}

// ── OpenAI Embedder ──

#[allow(dead_code)]
pub struct OpenAIEmbedder {
    api_key: String,
    model: String,
    _dimensions: usize,
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
    #[allow(dead_code)]
    index: usize,
}

impl OpenAIEmbedder {
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let client = reqwest::Client::new();
        let mut all_embeddings = Vec::new();

        for batch in texts.chunks(100) {
            let req = EmbedRequest {
                model: self.model.clone(),
                input: batch.to_vec(),
            };

            let resp = client
                .post("https://api.openai.com/v1/embeddings")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&req)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                bail!("OpenAI API error {status}: {body}");
            }

            let data: EmbedResponse = resp.json().await?;
            let mut sorted = data.data;
            sorted.sort_by_key(|d| d.index);
            for d in sorted {
                all_embeddings.push(d.embedding);
            }
        }

        Ok(all_embeddings)
    }

    async fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_batch(&[text.to_string()]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty embedding response"))
    }
}

// ── Vector math ──

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}
