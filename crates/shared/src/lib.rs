//! Shared types, settings, and serialization used across all Little Helper crates.
//!
//! This crate is the "common vocabulary" for the project. It defines:
//! - [`settings`] -- Application configuration persisted to disk (providers, auth, user profile).
//! - [`agent_api`] -- Chat message types used between the UI and LLM providers.
//! - [`search_types`] -- Query/result types for the fuzzy file finder.
//! - [`preview_types`] -- Rich preview content shown in the companion panel.
//! - [`skill`] -- Skill system: traits, permissions, execution lifecycle.
//! - [`events`] -- Audit log and real-time skill execution events.
//! - [`version`] -- User-friendly version tracking types (hides git internals).

pub mod events;
pub mod preview_types;
pub mod skill;
pub mod version;

/// Application settings persisted as JSON on disk.
///
/// Settings are loaded once at startup and can be edited through the UI.
/// The [`AppSettings`] struct is the root; it owns sub-structs for each
/// configurable subsystem (model providers, user profile, Slack, etc.).
pub mod settings {
    use serde::{Deserialize, Serialize};

    fn default_true() -> bool {
        true
    }

    /// OAuth 2.0 credentials obtained through the browser-based flow.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OAuthCredentials {
        pub access_token: String,
        pub refresh_token: Option<String>,
        pub expires_at: Option<i64>, // Unix timestamp
    }

    /// Authentication for a single LLM provider.
    ///
    /// Supports two auth strategies: a plain API key (simplest) or OAuth
    /// credentials from the browser flow. The provider clients try API key
    /// first, then OAuth, then fall back to environment variables.
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProviderAuth {
        pub api_key: Option<String>,
        pub oauth: Option<OAuthCredentials>,
    }

    /// Configuration for all LLM providers and model selection.
    ///
    /// Each cloud provider has two model slots: a primary (higher quality) and
    /// a fast variant (cheaper/faster for routine tasks). The `provider_preference`
    /// list controls fallback order -- the router tries each in sequence until
    /// one succeeds.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ModelProvider {
        pub local_model: String,              // e.g., "llama3.2:3b" for Ollama
        pub provider_preference: Vec<String>, // e.g., ["local", "openai", "anthropic", "gemini"]
        pub openai_model: String,             // e.g., "gpt-4o-mini"
        pub anthropic_model: String,          // e.g., "claude-sonnet-4-20250514"
        pub gemini_model: String,             // e.g., "gemini-2.5-flash"

        #[serde(default = "default_fast_openai")]
        pub openai_fast_model: String,
        #[serde(default = "default_fast_anthropic")]
        pub anthropic_fast_model: String,
        #[serde(default = "default_fast_gemini")]
        pub gemini_fast_model: String,

        /// Custom base URL for OpenAI-compatible APIs (Kimi, OpenRouter, Together, etc.)
        /// When set, the "openai" provider routes to this URL instead of api.openai.com.
        #[serde(default)]
        pub openai_base_url: Option<String>,

        // Authentication (either API key or OAuth)
        pub openai_auth: ProviderAuth,
        pub anthropic_auth: ProviderAuth,
        pub gemini_auth: ProviderAuth,
    }

    fn default_fast_openai() -> String {
        "gpt-4o-mini".to_string()
    }
    fn default_fast_anthropic() -> String {
        "claude-3-haiku-20240307".to_string()
    }
    fn default_fast_gemini() -> String {
        "gemini-1.5-flash".to_string()
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

    /// Root configuration for the entire application.
    ///
    /// Serialized to/from `settings.json` in the app data directory.
    /// New fields should always have `#[serde(default)]` so that older
    /// config files deserialize without errors.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AppSettings {
        /// Directories the file finder is allowed to search.
        pub allowed_dirs: Vec<String>,
        /// Extra directories scanned for RAG context injection.
        #[serde(default)]
        pub external_context_dirs: Vec<String>,
        pub model: ModelProvider,
        pub enable_internet_research: bool,
        /// Maximum file search results returned to the UI.
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

    impl Default for AppSettings {
        fn default() -> Self {
            Self {
                allowed_dirs: vec![],
                external_context_dirs: vec![],
                model: ModelProvider {
                    local_model: "llama3.2:3b".into(),
                    // Default to cloud providers for better results; fall back to local
                    provider_preference: vec![
                        "openai".into(), // Moonshot/Kimi via OpenAI compatibility
                        "anthropic".into(),
                        "gemini".into(),
                        "local".into(),
                    ],
                    openai_model: "kimi-k2-5".into(),
                    anthropic_model: "claude-sonnet-4-20250514".into(),
                    gemini_model: "gemini-2.0-flash".to_string(),
                    openai_fast_model: default_fast_openai(),
                    anthropic_fast_model: default_fast_anthropic(),
                    gemini_fast_model: default_fast_gemini(),
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

/// Chat message types shared between the UI layer and LLM provider clients.
pub mod agent_api {
    use serde::{Deserialize, Serialize};

    /// A single message in an LLM conversation.
    ///
    /// Role follows the OpenAI/Anthropic convention: "system", "user", or
    /// "assistant". Provider clients translate to API-specific formats
    /// (e.g., Anthropic's separate `system` field, Gemini's "model" role).
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChatMessage {
        pub role: String, // "system" | "user" | "assistant"
        pub content: String,
    }
}

/// Types for the fuzzy file search subsystem.
pub mod search_types {
    use serde::{Deserialize, Serialize};

    /// A user-initiated file search with optional extension filter.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SearchQuery {
        pub text: String,
        pub extensions: Option<Vec<String>>, // e.g., ["pdf","md"]
    }

    /// A single file matching a search query, ranked by relevance score.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SearchResult {
        pub path: String,
        pub file_name: String,
        pub size_bytes: u64,
        pub modified: Option<i64>, // unix timestamp
        /// Relevance score (0.0-1.0) combining substring position and length ratio.
        pub score: f32,
    }
}
