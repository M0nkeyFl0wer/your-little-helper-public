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
        pub anthropic_model: String,          // e.g., "claude-sonnet-4-20250514"
        pub gemini_model: String,             // e.g., "gemini-2.5-flash"

        /// Custom base URL for OpenAI-compatible APIs (Kimi, OpenRouter, Together, etc.)
        /// When set, the "openai" provider routes to this URL instead of api.openai.com.
        #[serde(default)]
        pub openai_base_url: Option<String>,

        // Authentication (either API key or OAuth)
        pub openai_auth: ProviderAuth,
        pub anthropic_auth: ProviderAuth,
        pub gemini_auth: ProviderAuth,
    }

    /// User profile for personalization
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UserProfile {
        pub name: String,
        pub mascot_image_path: Option<String>, // Path to pet/mascot image
        pub dark_mode: bool,
        pub onboarding_complete: bool,
        /// Whether user granted terminal command execution permission
        #[serde(default)]
        pub terminal_permission_granted: bool,
    }

    impl Default for UserProfile {
        fn default() -> Self {
            Self {
                name: String::new(),
                mascot_image_path: None,
                dark_mode: false,
                onboarding_complete: false,
                // For early testers, default ON so they experience the “superpowers” flow.
                terminal_permission_granted: true,
            }
        }
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
        /// Brave Search API key (free tier: 2000 queries/month)
        #[serde(default)]
        pub brave_search_api_key: Option<String>,
    }

    impl ProviderAuth {
        /// Returns true if either an API key or OAuth token is configured.
        pub fn has_auth(&self) -> bool {
            self.api_key.is_some() || self.oauth.is_some()
        }
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
                    // Default to cloud providers for better results; fall back to local
                    provider_preference: vec![
                        "openai".into(),  // Moonshot/Kimi via OpenAI compatibility
                        "anthropic".into(),
                        "gemini".into(),
                        "local".into(),
                    ],
                    openai_model: "kimi-k2-5".into(),
                    anthropic_model: "claude-sonnet-4-20250514".into(),
                    gemini_model: "gemini-2.5-flash".into(),
                    openai_base_url: None,
                    openai_auth: ProviderAuth::default(),
                    anthropic_auth: ProviderAuth::default(),
                    gemini_auth: ProviderAuth::default(),
                },
                // For early testers: start enabled; user can turn off anytime.
                enable_internet_research: true,
                max_results: 200,
                user_profile: UserProfile::default(),
                slack: SlackSettings::default(),
                build: BuildSettings::default(),
                // Public/test builds should not load private/work context by default.
                enable_campaign_context: false,
                enable_persona_context: false,
                // For early testers: start enabled; user can turn off anytime.
                share_system_summary: true,
                brave_search_api_key: None,
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

    /// Chunk types emitted during streaming AI generation.
    #[derive(Debug, Clone)]
    pub enum StreamChunk {
        /// Incremental text delta from the model.
        Text(String),
        /// A native tool_use block has started (Anthropic only).
        ToolUseStart { id: String, name: String },
        /// Partial JSON input for the current tool_use block.
        ToolInputDelta(String),
        /// A complete tool call ready for execution.
        ToolUseComplete {
            id: String,
            name: String,
            input: serde_json::Value,
        },
        /// Stream finished.
        Done { stop_reason: Option<String> },
        /// Error encountered mid-stream.
        Error(String),
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
