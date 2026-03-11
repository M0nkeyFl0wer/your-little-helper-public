//! Provider router -- tries LLM providers in preference order with automatic fallback.
//!
//! The router is the single entry point the rest of the app uses to call an LLM.
//! It iterates through `provider_preference` (e.g., `["openai", "anthropic", "local"]`),
//! attempting each until one succeeds. On fallback, the metadata records which provider
//! failed and why, so the UI can surface a non-blocking notice to the user.

use crate::anthropic::AnthropicClient;
use crate::gemini::GeminiClient;
use crate::ollama::OllamaClient;
use crate::openai::OpenAIClient;
use anyhow::{anyhow, Result};
use shared::agent_api::ChatMessage;
use shared::settings::ModelProvider;
use std::time::Instant;

/// Metadata about a completed generation: which provider answered, how long
/// it took, and whether a fallback occurred.
#[derive(Debug, Clone)]
pub struct GenerationMeta {
    pub provider: String,
    pub model: String,
    pub duration_ms: u64,
    /// If we had to fall back away from the configured primary provider, this captures
    /// which provider failed and why (best-effort string).
    pub fallback_from: Option<String>,
    pub fallback_error: Option<String>,
}

/// The full response from a generation call, pairing the LLM output with
/// provider metadata for diagnostics and UI display.
#[derive(Debug, Clone)]
pub struct GenerationResponse {
    pub text: String,
    pub meta: GenerationMeta,
}

/// Routes LLM requests to the best available provider.
///
/// Created once from [`ModelProvider`] config and reused for the app lifetime.
/// Each call to [`generate`](ProviderRouter::generate) clones messages so the
/// router can retry on a different provider without consuming the input.
pub struct ProviderRouter {
    config: ModelProvider,
}

impl ProviderRouter {
    pub fn new(config: ModelProvider) -> Self {
        Self { config }
    }

    /// Convenience wrapper that discards metadata and returns just the text.
    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        Ok(self.generate_with_meta(messages).await?.text)
    }

    /// Generate a response, returning both the text and provider metadata.
    ///
    /// Walks `provider_preference` in order. Each provider is instantiated
    /// fresh (they are lightweight -- they share a `LazyLock` HTTP client pool).
    /// If the primary provider fails, the error is captured in `GenerationMeta`
    /// so the UI can show "answered by X (Y was unavailable)".
    pub async fn generate_with_meta(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<GenerationResponse> {
        let mut last_error = None;
        let primary = self.config.provider_preference.first().cloned();
        let mut attempt_errors: Vec<(String, String)> = Vec::new();

        // Try providers in order of preference, falling back on failure
        for provider in self.config.provider_preference.iter() {
            let attempt_start = Instant::now();
            let result = match provider.as_str() {
                "local" => {
                    let client = OllamaClient::new(self.config.local_model.clone());
                    client.generate(messages.clone()).await
                }
                "openai" => match OpenAIClient::from_auth(
                    &self.config.openai_model,
                    &self.config.openai_auth,
                    self.config.openai_base_url.as_deref(),
                ) {
                    Ok(client) => client.generate(messages.clone()).await,
                    Err(e) => Err(e),
                },
                "anthropic" => match AnthropicClient::from_auth(
                    &self.config.anthropic_model,
                    &self.config.anthropic_auth,
                ) {
                    Ok(client) => client.generate(messages.clone()).await,
                    Err(e) => Err(e),
                },
                "gemini" => match GeminiClient::from_auth(
                    &self.config.gemini_model,
                    &self.config.gemini_auth,
                ) {
                    Ok(client) => client.generate(messages.clone()).await,
                    Err(e) => Err(e),
                },
                _ => {
                    last_error = Some(anyhow!("Unknown provider: {}", provider));
                    continue;
                }
            };

            match result {
                Ok(text) => {
                    let duration_ms = attempt_start.elapsed().as_millis() as u64;
                    let model = match provider.as_str() {
                        "local" => self.config.local_model.clone(),
                        "openai" => self.config.openai_model.clone(),
                        "anthropic" => self.config.anthropic_model.clone(),
                        "gemini" => self.config.gemini_model.clone(),
                        _ => String::new(),
                    };

                    let (fallback_from, fallback_error) =
                        match (&primary, attempt_errors.is_empty()) {
                            (Some(p), false) if p != provider => {
                                // Prefer the primary provider error if present; otherwise use the last error.
                                let primary_err = attempt_errors
                                    .iter()
                                    .find(|(prov, _)| prov == p)
                                    .map(|(_, e)| e.clone());
                                let err = primary_err
                                    .or_else(|| attempt_errors.last().map(|(_, e)| e.clone()));
                                (Some(p.clone()), err)
                            }
                            _ => (None, None),
                        };
                    return Ok(GenerationResponse {
                        text,
                        meta: GenerationMeta {
                            provider: provider.to_string(),
                            model,
                            duration_ms,
                            fallback_from,
                            fallback_error,
                        },
                    });
                }
                Err(e) => {
                    attempt_errors.push((provider.to_string(), e.to_string()));
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("No providers configured")))
    }
}
