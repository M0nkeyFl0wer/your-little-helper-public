//! HTTP client for generating text embeddings via Ollama's `/api/embeddings` endpoint.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;

static SHARED_HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(1)
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

pub struct EmbeddingClient {
    http: Client,
    base_url: String,
    model: String,
}

impl EmbeddingClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            http: SHARED_HTTP.clone(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    /// Default client using Ollama with nomic-embed-text.
    pub fn default_ollama() -> Self {
        Self::new("http://localhost:11434", "nomic-embed-text")
    }

    /// Check if the embedding service is available.
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.http.get(&url).send().await.is_ok()
    }

    /// Embed a single text string.
    pub async fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let req = EmbedRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };
        let resp = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| anyhow!("embedding request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("embedding error {}: {}", status, &body[..body.len().min(300)]));
        }

        let body: EmbedResponse = resp.json().await?;
        if body.embedding.is_empty() {
            return Err(anyhow!("empty embedding returned"));
        }
        Ok(body.embedding)
    }

    /// Embed multiple texts, returning one embedding vector per input.
    pub async fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        // Ollama doesn't support batch embeddings in a single request,
        // so we send them sequentially (could parallelize with join_all if needed)
        for text in texts {
            results.push(self.embed_single(text).await?);
        }
        Ok(results)
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }
}
