use anyhow::Result;
use shared::agent_api::ChatMessage;
use shared::settings::AppSettings;

pub struct AgentHost {
    pub settings: AppSettings,
}

impl AgentHost {
    pub fn new(settings: AppSettings) -> Self {
        Self { settings }
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        use providers::router::ProviderRouter;
        let router = ProviderRouter::new(self.settings.model.clone());
        router.generate(messages).await
    }
}
