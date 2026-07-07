//! OpenAI-compatible embeddings client + config.
//!
//! Mirrors the pattern of `crate::llm::client::OpenAIClient` but for the
//! `/embeddings` endpoint: takes a batch of texts, returns one vector per text.
//! Reads `RPGEN_EMBEDDING_*` env vars (already in `.env`) and reuses the
//! `OPENAI_API_KEY`/`OPENAI_MAX_CONCURRENT` knobs for auth/concurrency.

use std::env;
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use super::{Embedder, EmbeddingError};

/// Configuration for the embeddings endpoint.
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// Base URL, e.g. `http://srv-dgx-spark-01.local:8994/v1`.
    pub endpoint: String,
    /// Model name, e.g. `Qwen3-Embedding-8B-f16.gguf`.
    pub model: String,
    /// Vector dimension the model produces (e.g. 4096).
    pub dimension: usize,
    /// Bearer token if the endpoint requires auth (the local Qwen3 server does not).
    pub api_key: Option<String>,
    /// Max concurrent embedding HTTP requests.
    pub max_concurrent: usize,
    /// Texts per embedding request (the endpoint accepts an array).
    pub batch_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8994/v1".to_string(),
            model: "Qwen3-Embedding-8B-f16.gguf".to_string(),
            dimension: 4096,
            api_key: None,
            max_concurrent: 4,
            batch_size: 64,
        }
    }
}

impl EmbeddingConfig {
    /// Read from `RPGEN_EMBEDDING_*` env vars, falling back to defaults.
    ///
    /// Reuses `OPENAI_API_KEY` for auth and `OPENAI_MAX_CONCURRENT` for the
    /// concurrency cap (mirroring the LLM client's conventions).
    pub fn from_env() -> Self {
        Self {
            endpoint: env::var("RPGEN_EMBEDDING_ENDPOINT").unwrap_or_else(|_| {
                "http://localhost:8994/v1".to_string()
            }),
            model: env::var("RPGEN_EMBEDDING_MODEL").unwrap_or_else(|_| {
                "Qwen3-Embedding-8B-f16.gguf".to_string()
            }),
            dimension: env::var("RPGEN_EMBEDDING_DIMENSION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4096),
            api_key: env::var("OPENAI_API_KEY").ok(),
            max_concurrent: env::var("OPENAI_MAX_CONCURRENT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            batch_size: env::var("RPGEN_EMBEDDING_BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(64),
        }
    }
}

// --- OpenAI-compatible /embeddings wire types ---

#[derive(Debug, Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

/// HTTP client that calls an OpenAI-compatible `/embeddings` endpoint.
pub struct EmbeddingClient {
    client: Client,
    config: EmbeddingConfig,
    semaphore: Arc<Semaphore>,
}

impl std::fmt::Debug for EmbeddingClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingClient")
            .field("endpoint", &self.config.endpoint)
            .field("model", &self.config.model)
            .field("dimension", &self.config.dimension)
            .field("max_concurrent", &self.config.max_concurrent)
            .field("batch_size", &self.config.batch_size)
            .finish_non_exhaustive()
    }
}

impl EmbeddingClient {
    /// Construct from a config. Allocates a `reqwest::Client` and a semaphore
    /// sized to `config.max_concurrent`.
    pub fn new(config: EmbeddingConfig) -> Result<Self, EmbeddingError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());
        let max_conc = config.max_concurrent.max(1);
        let semaphore = Arc::new(Semaphore::new(max_conc));
        Ok(Self {
            client,
            config,
            semaphore,
        })
    }

    /// Borrow the config (read-only).
    pub fn config(&self) -> &EmbeddingConfig {
        &self.config
    }
}

#[async_trait]
impl Embedder for EmbeddingClient {
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| EmbeddingError::Api("semaphore closed".to_string()))?;

        let url = format!("{}/embeddings", self.config.endpoint.trim_end_matches('/'));
        let req = EmbeddingRequest {
            model: &self.config.model,
            input: texts,
        };

        let mut builder = self.client.post(&url).json(&req);
        if let Some(ref key) = self.config.api_key {
            builder = builder.header("Authorization", format!("Bearer {key}"));
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            let body_preview = if body.len() > 500 {
                format!("{}...", &body[..500])
            } else {
                body.clone()
            };
            return Err(EmbeddingError::Api(format!(
                "{status} from {url}: {body_preview}"
            )));
        }

        let parsed: EmbeddingResponse = serde_json::from_str(&body)?;
        if parsed.data.len() != texts.len() {
            return Err(EmbeddingError::Api(format!(
                "expected {} embeddings, got {}",
                texts.len(),
                parsed.data.len()
            )));
        }
        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }

    fn dimension(&self) -> usize {
        self.config.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_defaults_and_overrides() {
        // Defaults (no env vars set).
        env::remove_var("RPGEN_EMBEDDING_ENDPOINT");
        env::remove_var("RPGEN_EMBEDDING_MODEL");
        env::remove_var("RPGEN_EMBEDDING_DIMENSION");
        env::remove_var("RPGEN_EMBEDDING_BATCH_SIZE");
        let cfg = EmbeddingConfig::from_env();
        assert_eq!(cfg.dimension, 4096);
        assert!(cfg.model.contains("Qwen3"));

        // Overrides (env vars set).
        env::set_var("RPGEN_EMBEDDING_ENDPOINT", "http://test:1234/v1");
        env::set_var("RPGEN_EMBEDDING_MODEL", "test-model");
        env::set_var("RPGEN_EMBEDDING_DIMENSION", "768");
        env::set_var("RPGEN_EMBEDDING_BATCH_SIZE", "32");
        let cfg = EmbeddingConfig::from_env();
        assert_eq!(cfg.endpoint, "http://test:1234/v1");
        assert_eq!(cfg.model, "test-model");
        assert_eq!(cfg.dimension, 768);
        assert_eq!(cfg.batch_size, 32);

        env::remove_var("RPGEN_EMBEDDING_ENDPOINT");
        env::remove_var("RPGEN_EMBEDDING_MODEL");
        env::remove_var("RPGEN_EMBEDDING_DIMENSION");
        env::remove_var("RPGEN_EMBEDDING_BATCH_SIZE");
    }

    #[test]
    fn client_constructs() {
        let cfg = EmbeddingConfig::default();
        let client = EmbeddingClient::new(cfg).unwrap();
        assert_eq!(client.config().dimension, 4096);
    }
}
