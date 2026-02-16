use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::{ChatMessage, StreamChunk};
use shared::settings::ProviderAuth;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
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

/// Streaming delta types for SSE parsing.
#[derive(Debug, Deserialize)]
struct OpenAIStreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamResponse {
    choices: Vec<OpenAIStreamChoice>,
}

pub struct OpenAIClient {
    http: Client,
    auth_token: String,
    model: String,
    base_url: String,
}

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

impl OpenAIClient {
    pub fn new(model: &str) -> Result<Self> {
        let key = env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))?;
        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token: key,
            model: model.to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
        })
    }

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
            stream: None,
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
            .get(0)
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        Ok(text)
    }

    pub async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: UnboundedSender<StreamChunk>,
    ) -> Result<()> {
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
            stream: Some(true),
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

        let mut parser = crate::sse::SseParser::new();
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| anyhow!("stream read error: {}", e))?;
            for event in parser.feed(&bytes) {
                if event.data == "[DONE]" {
                    let _ = tx.send(StreamChunk::Done { stop_reason: None });
                    return Ok(());
                }
                match serde_json::from_str::<OpenAIStreamResponse>(&event.data) {
                    Ok(resp) => {
                        if let Some(choice) = resp.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                if !content.is_empty() {
                                    let _ = tx.send(StreamChunk::Text(content.clone()));
                                }
                            }
                            if choice.finish_reason.is_some() {
                                let _ = tx.send(StreamChunk::Done {
                                    stop_reason: choice.finish_reason.clone(),
                                });
                                return Ok(());
                            }
                        }
                    }
                    Err(_) => {
                        // Skip unparseable SSE lines (e.g. comments)
                    }
                }
            }
        }

        let _ = tx.send(StreamChunk::Done { stop_reason: None });
        Ok(())
    }
}
