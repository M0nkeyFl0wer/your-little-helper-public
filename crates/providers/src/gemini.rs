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

/// Shared HTTP client — keeps TCP/TLS connections alive across requests.
static SHARED_HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(2)
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiCandidatePart {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiCandidateContent {
    parts: Vec<GeminiCandidatePart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiCandidateContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

pub struct GeminiClient {
    http: Client,
    auth_token: String,
    model: String,
    use_oauth: bool,
}

impl GeminiClient {
    pub fn new(model: &str) -> Result<Self> {
        let key = env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY not set"))?;
        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token: key,
            model: model.to_string(),
            use_oauth: false,
        })
    }

    pub fn from_auth(model: &str, auth: &ProviderAuth) -> Result<Self> {
        let (auth_token, use_oauth) = if let Some(api_key) = &auth.api_key {
            (api_key.clone(), false)
        } else if let Some(oauth) = &auth.oauth {
            (oauth.access_token.clone(), true)
        } else {
            // Try environment variable as fallback
            let key = env::var("GEMINI_API_KEY")
                .map_err(|_| anyhow!("No Gemini authentication configured"))?;
            (key, false)
        };

        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token,
            model: model.to_string(),
            use_oauth,
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        // OAuth tokens go in Authorization header; API keys go in URL query
        let url = if self.use_oauth {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                self.model
            )
        } else {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                self.model, self.auth_token
            )
        };

        // Gemini has strict requirements:
        //   1. contents must start with role "user"
        //   2. contents must end with role "user"
        //   3. roles must alternate (user, model, user, model, ...)
        //   4. system messages go in the separate system_instruction field
        //
        // We merge consecutive same-role messages and ensure alternation.

        let mut system_parts: Vec<GeminiPart> = Vec::new();
        let mut raw_contents: Vec<GeminiContent> = Vec::new();

        for m in &messages {
            if m.role == "system" {
                system_parts.push(GeminiPart {
                    text: m.content.clone(),
                });
            } else {
                let role = match m.role.as_str() {
                    "assistant" => "model",
                    "user" => "user",
                    _ => "user",
                };
                // Merge consecutive messages with the same role
                if let Some(last) = raw_contents.last_mut() {
                    if last.role == role {
                        last.parts.push(GeminiPart {
                            text: m.content.clone(),
                        });
                        continue;
                    }
                }
                raw_contents.push(GeminiContent {
                    role: role.to_string(),
                    parts: vec![GeminiPart {
                        text: m.content.clone(),
                    }],
                });
            }
        }

        // Ensure contents starts with "user"
        if raw_contents.first().map(|c| c.role.as_str()) == Some("model") {
            raw_contents.remove(0);
        }
        // Ensure contents ends with "user" (Gemini requires this)
        if raw_contents.last().map(|c| c.role.as_str()) == Some("model") {
            raw_contents.push(GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart {
                    text: "Continue.".to_string(),
                }],
            });
        }
        // If empty after trimming, nothing to send
        if raw_contents.is_empty() {
            return Err(anyhow!("No user messages to send to Gemini"));
        }

        let system_instruction = if system_parts.is_empty() {
            None
        } else {
            Some(GeminiSystemInstruction {
                parts: system_parts,
            })
        };

        let req = GeminiRequest {
            contents: raw_contents,
            system_instruction,
        };
        // Retry loop for transient errors (429 rate-limit, 503 overloaded)
        let mut last_status = reqwest::StatusCode::OK;
        let mut last_body = String::new();
        for attempt in 0..4u32 {
            if attempt > 0 {
                let delay = Duration::from_millis(1000 * 2u64.pow(attempt - 1)); // 1s, 2s, 4s
                tokio::time::sleep(delay).await;
            }
            let mut request = self.http.post(&url).json(&req);
            if self.use_oauth {
                request = request.header("Authorization", format!("Bearer {}", self.auth_token));
            }
            let resp = request.send().await?;
            if resp.status().is_success() {
                let body: GeminiResponse = resp.json().await?;
                let text = body
                    .candidates
                    .first()
                    .and_then(|c| c.content.as_ref())
                    .and_then(|c| c.parts.first())
                    .map(|p| p.text.clone())
                    .unwrap_or_default();
                return Ok(text);
            }
            last_status = resp.status();
            last_body = resp.text().await.unwrap_or_default();
            // Only retry on 429 (rate limit) or 503 (overloaded)
            if last_status != reqwest::StatusCode::TOO_MANY_REQUESTS
                && last_status != reqwest::StatusCode::SERVICE_UNAVAILABLE
            {
                break;
            }
        }

        let body = last_body.trim().to_string();
        if body.is_empty() {
            return Err(anyhow!("gemini error: {}", last_status));
        }
        let body = if body.len() > 800 {
            format!("{}...", &body.chars().take(800).collect::<String>())
        } else {
            body
        };
        Err(anyhow!("gemini error: {}\n{}", last_status, body))
    }

    /// Helper to build normalized contents + system_instruction from ChatMessages.
    fn build_gemini_request(messages: &[ChatMessage]) -> Result<GeminiRequest> {
        let mut system_parts: Vec<GeminiPart> = Vec::new();
        let mut raw_contents: Vec<GeminiContent> = Vec::new();

        for m in messages {
            if m.role == "system" {
                system_parts.push(GeminiPart {
                    text: m.content.clone(),
                });
            } else {
                let role = match m.role.as_str() {
                    "assistant" => "model",
                    "user" => "user",
                    _ => "user",
                };
                if let Some(last) = raw_contents.last_mut() {
                    if last.role == role {
                        last.parts.push(GeminiPart {
                            text: m.content.clone(),
                        });
                        continue;
                    }
                }
                raw_contents.push(GeminiContent {
                    role: role.to_string(),
                    parts: vec![GeminiPart {
                        text: m.content.clone(),
                    }],
                });
            }
        }

        if raw_contents.first().map(|c| c.role.as_str()) == Some("model") {
            raw_contents.remove(0);
        }
        if raw_contents.last().map(|c| c.role.as_str()) == Some("model") {
            raw_contents.push(GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart {
                    text: "Continue.".to_string(),
                }],
            });
        }
        if raw_contents.is_empty() {
            return Err(anyhow!("No user messages to send to Gemini"));
        }

        let system_instruction = if system_parts.is_empty() {
            None
        } else {
            Some(GeminiSystemInstruction {
                parts: system_parts,
            })
        };

        Ok(GeminiRequest {
            contents: raw_contents,
            system_instruction,
        })
    }

    pub async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: UnboundedSender<StreamChunk>,
    ) -> Result<()> {
        let url = if self.use_oauth {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
                self.model
            )
        } else {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
                self.model, self.auth_token
            )
        };

        let req = Self::build_gemini_request(&messages)?;

        let mut request = self.http.post(&url).json(&req);
        if self.use_oauth {
            request = request.header("Authorization", format!("Bearer {}", self.auth_token));
        }
        let resp = request.send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let detail: String = body.chars().take(800).collect();
            if detail.trim().is_empty() {
                return Err(anyhow!("gemini error: {}", status));
            }
            return Err(anyhow!("gemini error: {}\n{}", status, detail));
        }

        let mut parser = crate::sse::SseParser::new();
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| anyhow!("stream read error: {}", e))?;
            for event in parser.feed(&bytes) {
                match serde_json::from_str::<GeminiResponse>(&event.data) {
                    Ok(resp) => {
                        if let Some(text) = resp
                            .candidates
                            .first()
                            .and_then(|c| c.content.as_ref())
                            .and_then(|c| c.parts.first())
                            .map(|p| &p.text)
                        {
                            if !text.is_empty() {
                                let _ = tx.send(StreamChunk::Text(text.clone()));
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        let _ = tx.send(StreamChunk::Done { stop_reason: None });
        Ok(())
    }
}
