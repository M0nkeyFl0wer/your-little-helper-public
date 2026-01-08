use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaResponse {
    response: String,
}

pub struct OllamaClient {
    http: Client,
    base: String,
    model: String,
}

impl OllamaClient {
    pub fn new(model: String) -> Self {
        let base =
            env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
        Self {
            http: Client::new(),
            base,
            model,
        }
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let prompt = messages
            .into_iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        let url = format!("{}/api/generate", self.base);
        let req = OllamaRequest {
            model: &self.model,
            prompt,
            stream: false,
        };
        let resp = self.http.post(url).json(&req).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("ollama error: {}", resp.status()));
        }
        let body: OllamaResponse = resp.json().await?;
        Ok(body.response)
    }
}
