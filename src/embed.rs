use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

// ── Trait ──

pub enum Embedder {
    OpenAI(OpenAIEmbedder),
    None,
}

impl Embedder {
    pub fn from_config(provider: &str, model: &str, dimensions: usize) -> Self {
        match provider {
            "openai" => {
                if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                    Embedder::OpenAI(OpenAIEmbedder {
                        api_key: key,
                        model: model.to_string(),
                        dimensions,
                    })
                } else {
                    eprintln!("Warning: OPENAI_API_KEY not set, semantic search disabled");
                    Embedder::None
                }
            }
            _ => Embedder::None,
        }
    }

    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        match self {
            Embedder::OpenAI(e) => e.embed_batch(texts).await,
            Embedder::None => bail!("No embedding provider configured"),
        }
    }

    pub async fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        match self {
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
            Embedder::OpenAI(e) => e.dimensions,
            Embedder::None => 0,
        }
    }
}

// ── OpenAI ──

#[allow(dead_code)]
pub struct OpenAIEmbedder {
    api_key: String,
    model: String,
    dimensions: usize,
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

        // Process in batches of 100
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
