use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::{ChatMessage, StreamChunk};
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
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
}

/// Streaming response: each line is one of these JSON objects.
#[derive(Debug, Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaMessage>,
    #[serde(default)]
    done: bool,
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
            http: SHARED_HTTP.clone(),
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

    pub async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: UnboundedSender<StreamChunk>,
    ) -> Result<()> {
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
            stream: true,
        };
        let resp = self.http.post(url).json(&req).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("ollama error: {}", resp.status()));
        }

        // Ollama streams line-delimited JSON
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| anyhow!("stream read error: {}", e))?;
            buf.push_str(&String::from_utf8_lossy(&bytes));

            // Process complete lines
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf = buf[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<OllamaStreamChunk>(&line) {
                    Ok(chunk_data) => {
                        if let Some(msg) = &chunk_data.message {
                            if !msg.content.is_empty() {
                                let _ = tx.send(StreamChunk::Text(msg.content.clone()));
                            }
                        }
                        if chunk_data.done {
                            let _ = tx.send(StreamChunk::Done { stop_reason: None });
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamChunk::Error(format!(
                            "Failed to parse Ollama stream: {}",
                            e
                        )));
                        return Ok(());
                    }
                }
            }
        }

        let _ = tx.send(StreamChunk::Done { stop_reason: None });
        Ok(())
    }
}
