pub mod settings {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OAuthCredentials {
        pub access_token: String,
        pub refresh_token: Option<String>,
        pub expires_at: Option<i64>, // Unix timestamp
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProviderAuth {
        pub api_key: Option<String>,
        pub oauth: Option<OAuthCredentials>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ModelProvider {
        pub local_model: String,      // e.g., "llama3.2:3b" for Ollama
        pub provider_preference: Vec<String>, // e.g., ["local", "openai", "anthropic", "gemini"]
        pub openai_model: String,     // e.g., "gpt-4o-mini"
        pub anthropic_model: String,  // e.g., "claude-3-5-sonnet-20241022"
        pub gemini_model: String,     // e.g., "gemini-1.5-flash"

        // Authentication (either API key or OAuth)
        pub openai_auth: ProviderAuth,
        pub anthropic_auth: ProviderAuth,
        pub gemini_auth: ProviderAuth,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AppSettings {
        pub allowed_dirs: Vec<String>,
        pub model: ModelProvider,
        pub enable_internet_research: bool,
        pub max_results: usize,
    }

    impl Default for ProviderAuth {
        fn default() -> Self {
            Self {
                api_key: None,
                oauth: None,
            }
        }
    }

    impl Default for AppSettings {
        fn default() -> Self {
            Self {
                allowed_dirs: vec![],
                model: ModelProvider {
                    local_model: "llama3.2:3b".into(),
                    provider_preference: vec!["local".into()], // Default to local-only for privacy
                    openai_model: "gpt-4o-mini".into(),
                    anthropic_model: "claude-3-5-sonnet-20241022".into(),
                    gemini_model: "gemini-1.5-flash".into(),
                    openai_auth: ProviderAuth::default(),
                    anthropic_auth: ProviderAuth::default(),
                    gemini_auth: ProviderAuth::default(),
                },
                enable_internet_research: false,
                max_results: 200,
            }
        }
    }
}

pub mod agent_api {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChatMessage {
        pub role: String,   // "system" | "user" | "assistant"
        pub content: String,
    }
}

pub mod search_types {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SearchQuery {
        pub text: String,
        pub extensions: Option<Vec<String>>, // e.g., ["pdf","md"]
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SearchResult {
        pub path: String,
        pub file_name: String,
        pub size_bytes: u64,
        pub modified: Option<i64>, // unix timestamp
        pub score: f32,
    }
}
