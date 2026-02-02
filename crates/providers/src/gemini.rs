use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use shared::settings::ProviderAuth;
use std::env;
use std::time::Duration;

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
}

impl GeminiClient {
    pub fn new(model: &str) -> Result<Self> {
        let key = env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY not set"))?;
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(45)).build()?,
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
            env::var("GEMINI_API_KEY")
                .map_err(|_| anyhow!("No Gemini authentication configured"))?
        };

        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(45)).build()?,
            auth_token,
            model: model.to_string(),
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.auth_token
        );

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
            raw_contents.pop();
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
        let resp = self.http.post(url).json(&req).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let body = body.trim();
            if body.is_empty() {
                return Err(anyhow!("gemini error: {}", status));
            }
            let body = if body.len() > 800 {
                format!("{}...", &body[..800])
            } else {
                body.to_string()
            };
            return Err(anyhow!("gemini error: {}\n{}", status, body));
        }
        let body: GeminiResponse = resp.json().await?;
        let text = body
            .candidates
            .get(0)
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.get(0))
            .map(|p| p.text.clone())
            .unwrap_or_default();
        Ok(text)
    }
}
