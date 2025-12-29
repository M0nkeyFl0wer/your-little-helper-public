use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use shared::settings::ProviderAuth;
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

pub struct OpenAIClient {
    http: Client,
    auth_token: String,
    model: String,
}

impl OpenAIClient {
    pub fn new(model: &str) -> Result<Self> {
        let key = env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))?;
        Ok(Self { http: Client::new(), auth_token: key, model: model.to_string() })
    }

    pub fn from_auth(model: &str, auth: &ProviderAuth) -> Result<Self> {
        let auth_token = if let Some(api_key) = &auth.api_key {
            api_key.clone()
        } else if let Some(oauth) = &auth.oauth {
            oauth.access_token.clone()
        } else {
            // Try environment variable as fallback
            env::var("OPENAI_API_KEY").map_err(|_| anyhow!("No OpenAI authentication configured"))?
        };

        Ok(Self {
            http: Client::new(),
            auth_token,
            model: model.to_string(),
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = "https://api.openai.com/v1/chat/completions";
        let openai_messages: Vec<OpenAIMessage> = messages
            .into_iter()
            .map(|m| OpenAIMessage { role: m.role, content: m.content })
            .collect();
        let req = OpenAIRequest { model: self.model.clone(), messages: openai_messages };
        let resp = self.http
            .post(url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("openai error: {}", resp.status()));
        }
        let body: OpenAIResponse = resp.json().await?;
        let text = body
            .choices
            .get(0)
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        Ok(text)
    }
}
