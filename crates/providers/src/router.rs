use anyhow::{anyhow, Result};
use shared::agent_api::ChatMessage;
use shared::settings::ModelProvider;
use crate::gemini::GeminiClient;
use crate::ollama::OllamaClient;
use crate::openai::OpenAIClient;
use crate::anthropic::AnthropicClient;

pub struct ProviderRouter {
    config: ModelProvider,
}

impl ProviderRouter {
    pub fn new(config: ModelProvider) -> Self {
        Self { config }
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let mut last_error = None;

        // Try providers in order of preference
        for provider in &self.config.provider_preference {
            let result = match provider.as_str() {
                "local" => {
                    let client = OllamaClient::new(self.config.local_model.clone());
                    client.generate(messages.clone()).await
                }
                "openai" => {
                    let client = OpenAIClient::new(&self.config.openai_model)?;
                    client.generate(messages.clone()).await
                }
                "anthropic" => {
                    let client = AnthropicClient::new(&self.config.anthropic_model)?;
                    client.generate(messages.clone()).await
                }
                "gemini" => {
                    let client = GeminiClient::new(&self.config.gemini_model)?;
                    client.generate(messages.clone()).await
                }
                _ => {
                    last_error = Some(anyhow!("Unknown provider: {}", provider));
                    continue;
                }
            };

            match result {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("No providers configured")))
    }
}
