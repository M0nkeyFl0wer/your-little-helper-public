use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use std::env;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
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
            http: Client::builder().timeout(Duration::from_secs(120)).build().unwrap_or_else(|_| Client::new()),
            base,
            model,
        }
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let conversation: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: m.role,
                content: m.content,
            })
            .collect();
        let url = format!("{}/api/chat", self.base);
        let req = OllamaChatRequest {
            model: &self.model,
            messages: conversation,
            stream: false,
        };
        let resp = self.http.post(url).json(&req).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("ollama error: {}", resp.status()));
        }
        let body: OllamaChatResponse = resp.json().await?;
        Ok(body.message.content)
    }
}
