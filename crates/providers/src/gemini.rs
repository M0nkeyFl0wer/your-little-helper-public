use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::ChatMessage;
use shared::settings::ProviderAuth;
use std::env;

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
    system_instruction: Option<GeminiContent>,
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
            http: Client::new(),
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
            http: Client::new(),
            auth_token,
            model: model.to_string(),
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.auth_token
        );
        let mut system_instruction = None;
        let mut contents: Vec<GeminiContent> = Vec::new();
        for m in messages {
            if m.role == "system" {
                let part = GeminiPart { text: m.content };
                system_instruction = Some(GeminiContent {
                    role: "system".to_string(),
                    parts: vec![part],
                });
            } else {
                contents.push(GeminiContent {
                    role: m.role,
                    parts: vec![GeminiPart { text: m.content }],
                });
            }
        }
        let req = GeminiRequest {
            contents,
            system_instruction,
        };
        let resp = self.http.post(url).json(&req).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("gemini error: {}", resp.status()));
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
