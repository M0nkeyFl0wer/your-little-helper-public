use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use shared::settings::ProviderAuth;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;

static SHARED_HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(2)
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
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
    auth_token: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(model: &str) -> Result<Self> {
        let key =
            env::var("ANTHROPIC_API_KEY").map_err(|_| anyhow!("ANTHROPIC_API_KEY not set"))?;
        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token: key,
            model: model.to_string(),
        })
    }

    pub fn from_auth(model: &str, auth: &ProviderAuth) -> Result<Self> {
        let auth_token = if let Some(api_key) = &auth.api_key {
            api_key.clone()
        } else if let Some(oauth) = &auth.oauth {
            oauth.access_token.clone()
        } else {
            // Try environment variable as fallback
            env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow!("No Anthropic authentication configured"))?
        };

        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token,
            model: model.to_string(),
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = "https://api.anthropic.com/v1/messages";

        let mut system_prompt = String::new();
        let mut anthropic_messages: Vec<AnthropicMessage> = Vec::new();
        for m in messages {
            if m.role == "system" {
                if !system_prompt.is_empty() {
                    system_prompt.push_str("\n\n");
                }
                system_prompt.push_str(&m.content);
            } else {
                anthropic_messages.push(AnthropicMessage {
                    role: m.role,
                    content: m.content,
                });
            }
        }

        let system = if system_prompt.trim().is_empty() {
            None
        } else {
            Some(system_prompt)
        };

        let req = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system,
            messages: anthropic_messages,
        };

        let resp = self
            .http
            .post(url)
            .header("x-api-key", &self.auth_token)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let detail: String = body.chars().take(800).collect();
            if detail.trim().is_empty() {
                return Err(anyhow!("anthropic error: {}", status));
            }
            return Err(anyhow!("anthropic error: {}\n{}", status, detail));
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
