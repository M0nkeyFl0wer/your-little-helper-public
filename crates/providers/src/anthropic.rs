use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: i32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

pub struct AnthropicClient {
    http: Client,
    key: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(model: &str) -> Result<Self> {
        let key = env::var("ANTHROPIC_API_KEY").map_err(|_| anyhow!("ANTHROPIC_API_KEY not set"))?;
        Ok(Self { http: Client::new(), key, model: model.to_string() })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = "https://api.anthropic.com/v1/messages";

        // Anthropic doesn't support system messages in the same array, so filter them out
        // and handle system prompt separately if needed
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .into_iter()
            .filter(|m| m.role != "system")
            .map(|m| AnthropicMessage { role: m.role, content: m.content })
            .collect();

        let req = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: anthropic_messages,
        };

        let resp = self.http
            .post(url)
            .header("x-api-key", &self.key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("anthropic error: {}", resp.status()));
        }

        let body: AnthropicResponse = resp.json().await?;
        let text = body
            .content
            .get(0)
            .map(|c| c.text.clone())
            .unwrap_or_default();
        Ok(text)
    }
}
