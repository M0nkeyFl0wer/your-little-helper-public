//! OpenAI-compatible chat completions client.
//!
//! Also used for any provider exposing an OpenAI-compatible API (Kimi/Moonshot,
//! OpenRouter, Together, etc.) by setting `openai_base_url` in settings.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use shared::settings::ProviderAuth;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;

/// Shared HTTP client -- keeps TCP/TLS connections alive across requests.
static SHARED_HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(2)
        .build()
        .expect("failed to build HTTP client")
});

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
    /// Base URL for the API. Defaults to `https://api.openai.com` but can be
    /// overridden for OpenAI-compatible third-party endpoints.
    base_url: String,
}

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

impl OpenAIClient {
    /// Create a client using the `OPENAI_API_KEY` environment variable.
    pub fn new(model: &str) -> Result<Self> {
        let key = env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))?;
        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token: key,
            model: model.to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
        })
    }

    /// Create a client from persisted auth config, with optional custom base URL.
    ///
    /// Auth priority: API key > OAuth token > `OPENAI_API_KEY` env var.
    pub fn from_auth(model: &str, auth: &ProviderAuth, base_url: Option<&str>) -> Result<Self> {
        let auth_token = if let Some(api_key) = &auth.api_key {
            api_key.clone()
        } else if let Some(oauth) = &auth.oauth {
            oauth.access_token.clone()
        } else {
            // Try environment variable as fallback
            env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow!("No OpenAI authentication configured"))?
        };

        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token,
            model: model.to_string(),
            base_url: base_url
                .unwrap_or(DEFAULT_BASE_URL)
                .trim_end_matches('/')
                .to_string(),
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let openai_messages: Vec<OpenAIMessage> = messages
            .into_iter()
            .map(|m| OpenAIMessage {
                role: m.role,
                content: m.content,
            })
            .collect();
        let req = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
        };
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let detail: String = body.chars().take(800).collect();
            if detail.trim().is_empty() {
                return Err(anyhow!("openai error: {}", status));
            }
            return Err(anyhow!("openai error: {}\n{}", status, detail));
        }
        let body: OpenAIResponse = resp.json().await?;
        let text = body
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        Ok(text)
    }
}
