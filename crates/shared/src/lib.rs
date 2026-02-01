pub mod events;
pub mod preview_types;
pub mod skill;
pub mod version;

pub mod settings {
    use serde::{Deserialize, Serialize};

    fn default_true() -> bool {
        true
    }

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
        pub local_model: String,              // e.g., "llama3.2:3b" for Ollama
        pub provider_preference: Vec<String>, // e.g., ["local", "openai", "anthropic", "gemini"]
        pub openai_model: String,             // e.g., "gpt-4o-mini"
        pub anthropic_model: String,          // e.g., "claude-3-5-sonnet-20241022"
        pub gemini_model: String,             // e.g., "gemini-1.5-flash"

        // Authentication (either API key or OAuth)
        pub openai_auth: ProviderAuth,
        pub anthropic_auth: ProviderAuth,
        pub gemini_auth: ProviderAuth,
    }

    /// User profile for personalization
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct UserProfile {
        pub name: String,
        pub mascot_image_path: Option<String>, // Path to pet/mascot image
        pub dark_mode: bool,
        pub onboarding_complete: bool,
        /// Whether user granted terminal command execution permission
        #[serde(default)]
        pub terminal_permission_granted: bool,
    }

    /// Slack integration settings
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct SlackSettings {
        /// Incoming webhook URL for notifications
        pub webhook_url: Option<String>,
        /// Default channel (optional)
        pub default_channel: Option<String>,
        /// Enable Slack notifications
        pub enabled: bool,
    }

    /// Build-mode settings (spec-kit integration)
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct BuildSettings {
        /// Optional path to spec-assistant.js
        pub spec_kit_path: Option<String>,
        /// Default folder for new projects
        pub default_project_folder: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AppSettings {
        pub allowed_dirs: Vec<String>,
        pub model: ModelProvider,
        pub enable_internet_research: bool,
        pub max_results: usize,
        pub user_profile: UserProfile,
        #[serde(default)]
        pub slack: SlackSettings,
        #[serde(default)]
        pub build: BuildSettings,
        #[serde(default = "default_true")]
        pub enable_campaign_context: bool,
        #[serde(default = "default_true")]
        pub enable_persona_context: bool,
        #[serde(default)]
        pub share_system_summary: bool,
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
                    provider_preference: vec!["anthropic".into(), "openai".into(), "gemini".into(), "local".into()], // Try cloud providers first, fall back to local
                    openai_model: "gpt-4o-mini".into(),
                    anthropic_model: "claude-3-5-sonnet-20241022".into(),
                    gemini_model: "gemini-1.5-flash".into(),
                    openai_auth: ProviderAuth::default(),
                    anthropic_auth: ProviderAuth::default(),
                    gemini_auth: ProviderAuth::default(),
                },
                enable_internet_research: false,
                max_results: 200,
                user_profile: UserProfile::default(),
                slack: SlackSettings::default(),
                build: BuildSettings::default(),
                enable_campaign_context: true,
                enable_persona_context: true,
                share_system_summary: false,
            }
        }
    }
}

pub mod agent_api {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChatMessage {
        pub role: String, // "system" | "user" | "assistant"
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
