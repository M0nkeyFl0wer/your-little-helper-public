//! Core types for the Little Helper app
//!
//! This module contains all the main type definitions used throughout the app,
//! including result types, screen states, chat types, and the main AppState.

use agent_host::{classify_command, AgentHost, CommandResult, DangerLevel};

#[cfg(not(windows))]
use agent_host::execute_with_sudo;
use eframe::egui;
use services::web_preview::WebPreviewService;
use shared::agent_api::{ChatMessage as ApiChatMessage, StreamChunk};
use shared::preview_types::{parse_preview_tags, strip_preview_tags, PreviewContent};
use shared::settings::AppSettings;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;

use futures::future::AbortHandle;
use sysinfo::System;

use crate::context::{
    get_campaign_summary, load_campaign_context, load_ddd_workflow, load_personas,
};
use crate::set_primary_provider_preference;
use crate::state::run_ai_generation;
use crate::utils::{
    clean_ai_response, is_path_in_allowed_dirs, run_user_command,
    validate_command_against_allowed,
};

/// Result from background AI generation
#[derive(Debug)]
pub struct AiResult {
    pub response: String,
    pub preview_file: Option<PathBuf>,
    pub error: Option<String>,
    /// Commands that were executed (for transparency)
    pub executed_commands: Vec<(String, String, bool)>, // (command, output, success)
    pub pending_commands: Vec<String>,
}

/// Result from background command execution
#[derive(Debug)]
pub struct CommandExecResult {
    pub command: String,
    pub output: Result<CommandResult, String>,
}

/// Result from background web preview fetch
#[derive(Debug)]
pub struct WebPreviewResult {
    pub url: String,
    pub title: Option<String>,
    pub screenshot: Option<PathBuf>,
    pub og_image: Option<String>,
    pub snippet: Option<String>,
}

/// Result from a background OAuth flow.
pub struct OAuthResult {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub error: Option<String>,
}

/// Result from the background Ollama setup thread.
pub struct OllamaSetupResult {
    pub status: crate::ollama_manager::OllamaStatus,
    pub ollama_up: bool,
    pub recommended_model: String,
    pub recommended_desc: String,
}

/// Current app screen
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppScreen {
    Onboarding,
    Chat,
}

/// Chat mode - determines agent behavior and available skills
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ChatMode {
    /// Find files and content
    Find,
    /// Tech support - diagnose and fix problems
    Fix,
    /// Deep research with citations
    Research,
    /// Work with data and files
    Data,
    /// Content creation with personas
    Content,
    /// Build projects with spec-kit workflows
    Build,
}

impl ChatMode {
    /// Get the mode name as a string for the agent system
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMode::Find => "find",
            ChatMode::Fix => "fix",
            ChatMode::Research => "research",
            ChatMode::Data => "data",
            ChatMode::Content => "content",
            ChatMode::Build => "build",
        }
    }
}

/// A chat message
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    /// Optional low-level details (e.g. provider errors). Kept out of the main message UI.
    pub details: Option<String>,
    #[allow(dead_code)] // Will be used for chat history display
    pub timestamp: String,
}

/// Active viewer in the preview panel
#[derive(Clone, Debug)]
pub enum ActiveViewer {
    /// Default preview panel content (mode intro, files, etc)
    Panel,
    /// Matrix rain animation while processing
    Matrix,
    /// Easter egg!
    RickRoll,
    /// (command, output) for showing command results
    CommandOutput(String, String),
}

/// Main application state
pub struct AppState {
    pub settings: AppSettings,
    pub current_screen: AppScreen,
    pub current_mode: ChatMode,
    /// For detecting mode changes
    pub previous_mode: Option<ChatMode>,

    /// First-time intro for Spec (Build) tab
    pub spec_intro_shown: bool,
    /// First-time intro for Fix (Doc) tab — auto-offer health scan
    pub fix_intro_shown: bool,
    /// Pulsing hint on mode picker — dismissed after first mode switch
    pub show_mode_picker_hint: bool,
    /// Current input text
    pub input_text: String,
    /// Preserve input per mode
    pub mode_input_drafts: std::collections::HashMap<ChatMode, String>,
    /// Per-mode chat threads
    pub mode_chat_histories: std::collections::HashMap<ChatMode, Vec<ChatMessage>>,

    /// Modes with unseen assistant replies
    pub unread_modes: HashSet<ChatMode>,
    /// Unified thread history across all modes
    pub thread_history: crate::thread_history::ThreadHistory,
    /// Current thread ID (for continuing conversations)
    pub current_thread_id: Option<String>,
    /// Whether to show thread history view
    pub show_thread_history: bool,
    /// Thread history search query
    pub thread_search_query: String,
    /// Mode filter for thread history panel (None = all modes)
    pub thread_history_mode_filter: Option<ChatMode>,
    /// Whether the AI is currently thinking/processing (per mode)
    pub is_thinking: std::collections::HashMap<ChatMode, bool>,
    /// What the agent is currently doing (per mode)
    pub thinking_status: std::collections::HashMap<ChatMode, String>,
    /// Which mode currently has an active AI request
    pub thinking_mode: Option<ChatMode>,
    /// When an AI request started (per mode)
    pub thinking_started_at: HashMap<ChatMode, std::time::Instant>,
    /// Whether we've shown a slow-response hint (per mode)
    pub slow_response_hint_shown: HashMap<ChatMode, bool>,
    /// Whether to show attention near the model indicator
    pub show_model_hint: bool,
    /// When the model hint started
    pub model_hint_started_at: Option<std::time::Instant>,
    /// Available for future agentic features
    #[allow(dead_code)]
    pub agent_host: agent_host::AgentHost,
    /// Context manager for documents and personas
    pub context_manager: agent_host::context_manager::ContextManager,
    /// Skill registry for available tools
    pub skill_registry: agent_host::skills::SkillRegistry,

    // Preview panel (new interactive preview companion)
    pub preview_panel: crate::preview_panel::PreviewPanel,

    // Legacy preview panel (for file viewers)
    pub show_preview: bool,
    pub active_viewer: ActiveViewer,
    /// File to auto-open after response
    pub pending_preview: Option<PathBuf>,

    /// Per-mode preview state: saves (ActiveViewer, PreviewContent) when switching modes
    pub mode_preview_state: HashMap<ChatMode, (ActiveViewer, Option<shared::preview_types::PreviewContent>)>,

    // Re-focus the chat input after AI replies
    pub refocus_input: bool,

    // Onboarding
    pub onboarding_name: String,

    // Pending command approvals
    pub pending_commands: Vec<String>,

    // Sudo password handling
    pub password_dialog: crate::modals::PasswordDialog,
    /// Command waiting for sudo password
    pub pending_sudo_command: Option<String>,

    // Background command execution channel
    pub command_result_rx: Option<Receiver<CommandExecResult>>,

    // Background mascot texture
    pub mascot_texture: Option<egui::TextureHandle>,
    pub mascot_loaded: bool,

    // Async AI response channel
    pub ai_result_rx: Option<Receiver<AiResult>>,

    // Abort handles for in-flight AI work (per mode)
    pub ai_abort_handles: HashMap<ChatMode, AbortHandle>,

    // Web preview service and async fetch channel
    pub web_preview_service: Arc<WebPreviewService>,
    pub web_preview_rx: Option<Receiver<WebPreviewResult>>,

    pub show_settings_dialog: bool,
    pub new_allowed_dir: String,
    pub settings_status: Option<String>,
    pub settings_status_is_error: bool,

    // API key input fields (temporary storage for settings dialog)
    pub openai_api_key_input: String,
    pub anthropic_api_key_input: String,
    pub gemini_api_key_input: String,

    // Build mode inputs and status
    pub spec_kit_path_input: String,
    pub build_folder_input: String,
    pub build_project_name_input: String,
    pub build_status: Option<String>,
    pub build_status_is_error: bool,

    // Session usage (approx)
    pub session_input_tokens_est: u64,
    pub session_output_tokens_est: u64,
    pub last_prompt_tokens_est: u32,
    pub last_response_tokens_est: u32,

    // Settings stats cache
    pub settings_perf_last_update: Option<Instant>,
    pub settings_cpu_percent: f32,
    pub settings_mem_mb: u64,

    // CPU/memory nudge
    pub cpu_high_since: Option<Instant>,
    pub cpu_nudge_dismissed: bool,

    // Background Ollama setup channel (fires once at startup)
    pub ollama_setup_rx: Option<Receiver<OllamaSetupResult>>,

    // Live status updates from the AI pipeline (e.g. "Searching…", "Running command…")
    pub ai_status_rx: Option<Receiver<String>>,

    // Streaming AI response channel — UI polls this per frame
    pub ai_stream_rx: Option<Receiver<StreamChunk>>,
    /// Accumulated streaming text per mode (displayed as partial assistant message)
    pub streaming_partial: HashMap<ChatMode, String>,

    // Background OAuth flow channel
    pub oauth_result_rx: Option<Receiver<OAuthResult>>,
    /// True while an OAuth browser flow is in progress
    pub oauth_in_progress: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let (mut settings, _) = crate::utils::load_settings_or_default();
        crate::utils::ensure_allowed_dirs(&mut settings);

        // Apply preloaded user info if available (bespoke builds)
        if crate::secrets::should_skip_onboarding() {
            settings.user_profile.onboarding_complete = true;
            settings.user_profile.terminal_permission_granted = true;
            if !crate::secrets::PRELOAD_USER_NAME.is_empty() {
                settings.user_profile.name = crate::secrets::PRELOAD_USER_NAME.to_string();
            }
        }

        // ── Start Ollama in the background (don't block the UI) ──
        // Quick check: is Ollama already listening? (fast, non-blocking)
        let ollama_already_up = crate::ollama_manager::ollama_reachable();

        // Kick off Ollama startup + model pull in a background thread.
        // Results arrive via the ollama_setup_rx channel.
        let (ollama_tx, ollama_rx) = std::sync::mpsc::channel::<OllamaSetupResult>();
        {
            let settings_clone = settings.clone();
            std::thread::spawn(move || {
                use crate::ollama_manager::{self, OllamaStatus};

                let status = ollama_manager::ensure_ollama_running();
                let ollama_up = matches!(
                    status,
                    OllamaStatus::AlreadyRunning | OllamaStatus::Started
                );

                // Pick model based on RAM
                let (recommended, recommended_desc) = ollama_manager::recommended_model();
                let mut model_to_use = settings_clone.model.local_model.clone();
                if ollama_up && model_to_use == "llama3.2:3b" {
                    model_to_use = recommended.to_string();
                }

                // Pull model if needed
                if ollama_up {
                    if let Some(binary) = ollama_manager::ollama_binary() {
                        if !ollama_manager::model_available(&binary, &model_to_use) {
                            let _ = ollama_manager::pull_model(&binary, &model_to_use);
                        }
                    }
                }

                let _ = ollama_tx.send(OllamaSetupResult {
                    status,
                    ollama_up,
                    recommended_model: recommended.to_string(),
                    recommended_desc: recommended_desc.to_string(),
                });
            });
        }

        // If Ollama was already running, apply provider fallback immediately.
        // Otherwise, defer until the background thread reports back.
        if ollama_already_up {
            let primary_provider = settings
                .model
                .provider_preference
                .first()
                .map(|s| s.as_str())
                .unwrap_or("local");
            let missing_key = match primary_provider {
                "openai" => !settings.model.openai_auth.has_auth(),
                "anthropic" => !settings.model.anthropic_auth.has_auth(),
                "gemini" => !settings.model.gemini_auth.has_auth(),
                _ => false,
            };
            if primary_provider != "local" && missing_key {
                settings.model.provider_preference = vec![
                    "local".to_string(),
                    "anthropic".to_string(),
                    "openai".to_string(),
                    "gemini".to_string(),
                ];
                crate::utils::save_settings(&settings);
            }
        } else if !ollama_already_up {
            // Ollama not running yet — if local is preferred, try cloud fallback
            let primary_provider = settings
                .model
                .provider_preference
                .first()
                .map(|s| s.as_str())
                .unwrap_or("local");
            if primary_provider == "local" {
                let fallback = if settings.model.anthropic_auth.has_auth() {
                    Some("anthropic")
                } else if settings.model.openai_auth.has_auth() {
                    Some("openai")
                } else if settings.model.gemini_auth.has_auth() {
                    Some("gemini")
                } else {
                    None
                };
                if let Some(provider) = fallback {
                    set_primary_provider_preference(
                        &mut settings.model.provider_preference,
                        provider,
                    );
                    crate::utils::save_settings(&settings);
                }
            }
        }

        let user_name = if settings.user_profile.name.is_empty() {
            "friend".to_string()
        } else {
            settings.user_profile.name.clone()
        };

        let welcome_msg = ChatMessage {
            role: "assistant".to_string(),
            content: format!(
                "Hi {}! I'm your Little Helper. What would you like me to help you with today?\n\n\
                You can ask me to find files, fix problems, do deep research, work with data, or create content.",
                user_name
            ),
            details: None,
            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
        };

        // Initialize preview panel with mode intro
        let mut preview_panel = crate::preview_panel::PreviewPanel::new();
        preview_panel.show_mode_intro("find");

        let find_history = vec![welcome_msg.clone()];
        // Fix mode gets its own Doc intro on first switch — no generic welcome needed
        let fix_history = Vec::new();

        // Show onboarding for first-run users
        let initial_screen = if settings.user_profile.onboarding_complete {
            AppScreen::Chat
        } else {
            AppScreen::Onboarding
        };

        Self {
            settings: settings.clone(),
            current_screen: initial_screen,
            current_mode: ChatMode::Find,
            previous_mode: None,
            spec_intro_shown: false,
            fix_intro_shown: false,
            // Show mode picker hint for first-time users (not for returning users)
            show_mode_picker_hint: !settings.user_profile.onboarding_complete,
            input_text: String::new(),
            mode_input_drafts: HashMap::new(),
            mode_chat_histories: {
                let mut h = HashMap::new();
                h.insert(ChatMode::Find, find_history);
                h.insert(ChatMode::Fix, fix_history);
                h.insert(ChatMode::Research, vec![ChatMessage {
                    role: "assistant".to_string(),
                    content: format!(
                        "Hi {}! I'm Scholar — your research assistant.\n\n\
                        I can search the web, dig into topics, cross-reference sources, and put together reports.\n\n\
                        Try asking me to research something, or say **\"write a report on...\"** and I'll create a document for you.",
                        user_name
                    ),
                    details: None,
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                }]);
                h.insert(ChatMode::Data, Vec::new());
                h.insert(ChatMode::Content, Vec::new());
                h.insert(ChatMode::Build, Vec::new());
                h
            },
            unread_modes: HashSet::new(),
            thread_history: crate::thread_history::ThreadHistory::load_from_disk(),
            current_thread_id: None,
            show_thread_history: false,
            thread_search_query: String::new(),
            thread_history_mode_filter: None,
            is_thinking: {
                let mut m = std::collections::HashMap::new();
                m.insert(ChatMode::Find, false);
                m.insert(ChatMode::Fix, false);
                m.insert(ChatMode::Research, false);
                m.insert(ChatMode::Data, false);
                m.insert(ChatMode::Content, false);
                m.insert(ChatMode::Build, false);
                m
            },
            thinking_status: {
                let mut m = std::collections::HashMap::new();
                m.insert(ChatMode::Find, String::new());
                m.insert(ChatMode::Fix, String::new());
                m.insert(ChatMode::Research, String::new());
                m.insert(ChatMode::Data, String::new());
                m.insert(ChatMode::Content, String::new());
                m.insert(ChatMode::Build, String::new());
                m
            },
            thinking_mode: None,
            thinking_started_at: HashMap::new(),
            slow_response_hint_shown: HashMap::new(),
            show_model_hint: false,
            model_hint_started_at: None,
            agent_host: AgentHost::new(settings.clone()),
            context_manager: agent_host::context_manager::ContextManager::new(
                agent_host::context_manager::ContextManager::default_dir()
            ).unwrap_or_else(|_| {
                agent_host::context_manager::ContextManager::new(
                    std::path::PathBuf::from("./context")
                ).expect("Failed to create context manager")
            }),
            skill_registry: agent_host::skills::init_empty_registry(),
            preview_panel,
            show_preview: true,
            active_viewer: ActiveViewer::Panel,
            pending_preview: None,
            mode_preview_state: HashMap::new(),
            refocus_input: false,
            onboarding_name: String::new(),
            pending_commands: Vec::new(),
            password_dialog: crate::modals::PasswordDialog::new("sudo_password"),
            pending_sudo_command: None,
            command_result_rx: None,
            mascot_texture: None,
            mascot_loaded: false,
            ai_result_rx: None,
            ai_abort_handles: HashMap::new(),
            web_preview_service: Arc::new(WebPreviewService::new()),
            web_preview_rx: None,
            show_settings_dialog: false,
            new_allowed_dir: String::new(),
            settings_status: None,
            settings_status_is_error: false,
            openai_api_key_input: String::new(),
            anthropic_api_key_input: String::new(),
            gemini_api_key_input: String::new(),
            spec_kit_path_input: settings
                .build
                .spec_kit_path
                .clone()
                .unwrap_or_default(),
            build_folder_input: settings
                .build
                .default_project_folder
                .clone()
                .or_else(|| dirs::home_dir().map(|h| h.to_string_lossy().to_string()))
                .unwrap_or_default(),
            build_project_name_input: String::new(),
            build_status: None,
            build_status_is_error: false,

            session_input_tokens_est: 0,
            session_output_tokens_est: 0,
            last_prompt_tokens_est: 0,
            last_response_tokens_est: 0,
            settings_perf_last_update: None,
            settings_cpu_percent: 0.0,
            settings_mem_mb: 0,

            cpu_high_since: None,
            cpu_nudge_dismissed: false,
            ollama_setup_rx: Some(ollama_rx),
            ai_status_rx: None,
            ai_stream_rx: None,
            streaming_partial: HashMap::new(),
            oauth_result_rx: None,
            oauth_in_progress: false,
        }
    }
}

impl AppState {
    /// Poll for background Ollama setup completion. Call once per frame.
    /// When the setup thread finishes, applies provider fallback and posts
    /// a status message into the chat history.
    pub fn poll_ollama_setup(&mut self) {
        let result = match &self.ollama_setup_rx {
            Some(rx) => rx.try_recv().ok(),
            None => return,
        };
        let Some(result) = result else { return };
        // Consume the channel — only fires once
        self.ollama_setup_rx = None;

        use crate::ollama_manager::OllamaStatus;

        // Apply recommended model if still on default
        if result.ollama_up && self.settings.model.local_model == "llama3.2:3b" {
            self.settings.model.local_model = result.recommended_model.clone();
            crate::utils::save_settings(&self.settings);
        }

        // Provider fallback based on final Ollama state
        if result.ollama_up {
            let primary = self.settings.model.provider_preference
                .first().map(|s| s.as_str()).unwrap_or("local");
            let missing_key = match primary {
                "openai" => !self.settings.model.openai_auth.has_auth(),
                "anthropic" => !self.settings.model.anthropic_auth.has_auth(),
                "gemini" => !self.settings.model.gemini_auth.has_auth(),
                _ => false,
            };
            if primary != "local" && missing_key {
                self.settings.model.provider_preference = vec![
                    "local".to_string(), "anthropic".to_string(),
                    "openai".to_string(), "gemini".to_string(),
                ];
                crate::utils::save_settings(&self.settings);
            }
        }

        // Post a status message
        let has_any_cloud_key = self.settings.model.openai_auth.has_auth()
            || self.settings.model.anthropic_auth.has_auth()
            || self.settings.model.gemini_auth.has_auth();

        let gpu = crate::ollama_manager::has_gpu_acceleration();

        let msg = match &result.status {
            OllamaStatus::Started if !gpu => Some(format!(
                "Local AI started — using {}.\n\n\
                Your computer doesn't have a GPU that speeds up AI, so the \
                local model will be pretty slow. It still works and keeps \
                everything private!\n\n\
                **Recommendation:** For a much better experience, open \
                **Settings** and switch to a cloud provider like Gemini \
                (free tier available) or paste an API key for OpenAI/Anthropic.",
                result.recommended_desc
            )),
            OllamaStatus::Started => Some(format!(
                "Local AI started automatically. Using {} for this session.",
                result.recommended_desc
            )),
            OllamaStatus::AlreadyRunning => None, // already running, no fuss
            OllamaStatus::NotFound | OllamaStatus::StartFailed(_)
                if !has_any_cloud_key =>
            {
                Some(
                    "Heads up: I couldn't start the local AI engine, and no cloud API key is set.\n\n\
                    To get started, open **Settings** (gear icon) and paste a Gemini, OpenAI, or Anthropic API key."
                        .to_string()
                )
            }
            OllamaStatus::NotFound | OllamaStatus::StartFailed(_) => {
                Some(
                    "Local AI isn't available, so I switched to your cloud provider. You can change this in Settings."
                        .to_string()
                )
            }
        };

        if let Some(content) = msg {
            let chat_msg = ChatMessage {
                role: "assistant".to_string(),
                content,
                details: None,
                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
            };
            // Post to Find and Fix histories (the default visible tabs)
            if let Some(h) = self.mode_chat_histories.get_mut(&ChatMode::Find) {
                h.push(chat_msg.clone());
            }
            if let Some(h) = self.mode_chat_histories.get_mut(&ChatMode::Fix) {
                h.push(chat_msg);
            }
        }
    }

    /// Poll for background OAuth flow completion. Call once per frame.
    pub fn poll_oauth_result(&mut self) {
        let result = match &self.oauth_result_rx {
            Some(rx) => rx.try_recv().ok(),
            None => return,
        };
        let Some(result) = result else { return };
        self.oauth_result_rx = None;
        self.oauth_in_progress = false;

        if let Some(err) = &result.error {
            self.settings_status = Some(format!("Sign-in failed: {}", err));
            self.settings_status_is_error = true;
            return;
        }

        let oauth_creds = shared::settings::OAuthCredentials {
            access_token: result.access_token,
            refresh_token: result.refresh_token,
            expires_at: Some(chrono::Utc::now().timestamp() + 3600), // ~1 hour
        };

        match result.provider.as_str() {
            "gemini" => {
                self.settings.model.gemini_auth.oauth = Some(oauth_creds);
            }
            _ => {}
        }

        crate::utils::save_settings(&self.settings);
        self.settings_status = Some("Signed in with Google!".to_string());
        self.settings_status_is_error = false;
    }

    fn ollama_reachable() -> bool {
        crate::ollama_manager::ollama_reachable()
    }

    fn provider_has_api_key(&self, provider: &str) -> bool {
        match provider {
            "openai" => self.settings.model.openai_auth.has_auth(),
            "anthropic" => self.settings.model.anthropic_auth.has_auth(),
            "gemini" => self.settings.model.gemini_auth.has_auth(),
            _ => true,
        }
    }

    fn estimate_tokens(text: &str) -> u32 {
        // Rough heuristic: ~4 chars per token for English.
        (text.chars().count() as u32).saturating_div(4).max(1)
    }

    pub fn model_context_hint_tokens(&self) -> u32 {
        // Very rough context limits for display only.
        // We use a smaller "comfort" window for stable performance.
        let provider = self
            .settings
            .model
            .provider_preference
            .first()
            .map(|s| s.as_str())
            .unwrap_or("local");

        let model = match provider {
            "openai" => self.settings.model.openai_model.as_str(),
            "anthropic" => self.settings.model.anthropic_model.as_str(),
            "gemini" => self.settings.model.gemini_model.as_str(),
            _ => self.settings.model.local_model.as_str(),
        };
        let m = model.to_lowercase();

        if provider == "gemini" && m.contains("1.5") {
            1_000_000
        } else if provider == "anthropic" {
            200_000
        } else if provider == "openai" {
            128_000
        } else {
            8_192
        }
    }

    fn build_api_messages_with_budget(&self, system_prompt: String) -> (Vec<ApiChatMessage>, u32, usize) {
        // Budget: keep prompts small and fast even on cloud models.
        const COMFORT_TOTAL_TOKENS: u32 = 8_000;
        const RESERVED_FOR_REPLY: u32 = 2_000;
        let budget = COMFORT_TOTAL_TOKENS.saturating_sub(RESERVED_FOR_REPLY);

        let mut msgs: Vec<ApiChatMessage> = vec![ApiChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        }];

        let mut used = Self::estimate_tokens(&msgs[0].content);

        // Add recent messages from newest backwards until we hit budget.
        let history = self.chat_history();
        let mut kept_rev: Vec<ApiChatMessage> = Vec::new();
        let mut dropped = 0usize;

        for msg in history.iter().rev() {
            let t = Self::estimate_tokens(&msg.content);
            if used.saturating_add(t) > budget {
                // Stop here — don't skip and include older messages
                dropped = history.len() - kept_rev.len();
                break;
            }
            used = used.saturating_add(t);
            kept_rev.push(ApiChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        kept_rev.reverse();
        msgs.extend(kept_rev);

        (msgs, used, dropped)
    }

    pub fn update_settings_perf(&mut self) {
        let now = Instant::now();
        if self
            .settings_perf_last_update
            .map(|t| now.duration_since(t) < std::time::Duration::from_secs(1))
            .unwrap_or(false)
        {
            return;
        }
        self.settings_perf_last_update = Some(now);

        let mut sys = System::new();
        sys.refresh_processes();

        if let Ok(pid) = sysinfo::get_current_pid() {
            if let Some(proc_) = sys.process(pid) {
                // cpu_usage is a % of a single core (sysinfo semantics)
                self.settings_cpu_percent = proc_.cpu_usage();
                // sysinfo returns bytes
                self.settings_mem_mb = (proc_.memory() / (1024 * 1024)) as u64;
            }
        }
    }
    pub fn is_path_permitted(&self, path: &Path) -> bool {
        is_path_in_allowed_dirs(path, &self.settings.allowed_dirs)
    }

    pub fn spec_kit_path(&self) -> PathBuf {
        if let Some(path) = &self.settings.build.spec_kit_path {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return crate::utils::expand_user_path(trimmed);
            }
        }

        dirs::home_dir()
            .map(|h| h.join("Projects/spec-kit-assistant/archive/legacy-node/spec-assistant.js"))
            .unwrap_or_default()
    }

    pub fn resolve_build_folder(&self) -> Result<PathBuf, String> {
        let raw = self.build_folder_input.trim();
        if raw.is_empty() {
            return Err("Please choose a folder first.".to_string());
        }

        let path = crate::utils::expand_user_path(raw);
        if !path.exists() || !path.is_dir() {
            return Err("That folder doesn't exist yet. Pick an existing folder.".to_string());
        }

        if !self.is_path_permitted(&path) {
            return Err("That folder is outside your allowed folders. Add it in Settings.".to_string());
        }

        Ok(path)
    }

    fn shell_quote(arg: &str) -> String {
        if arg.contains(' ') || arg.contains('"') {
            format!("\"{}\"", arg.replace('"', "\\\""))
        } else {
            arg.to_string()
        }
    }

    pub fn run_spec_kit_command(&mut self, args: Vec<String>) {
        let spec_kit_path = self.spec_kit_path();
        if !spec_kit_path.exists() {
            self.build_status = Some(
                "Spec isn’t set up yet. In the Spec tab, click ‘Find Spec Kit Assistant…’.".to_string(),
            );
            self.build_status_is_error = true;
            return;
        }

        let folder = match self.resolve_build_folder() {
            Ok(path) => path,
            Err(err) => {
                self.build_status = Some(err);
                self.build_status_is_error = true;
                return;
            }
        };

        let mut cmd_parts = vec![
            "node".to_string(),
            Self::shell_quote(&spec_kit_path.to_string_lossy()),
        ];
        for arg in args {
            cmd_parts.push(Self::shell_quote(&arg));
        }

        let command = format!(
            "cd {} && {}",
            Self::shell_quote(&folder.to_string_lossy()),
            cmd_parts.join(" ")
        );

        let (tx, rx) = channel::<CommandExecResult>();
        self.command_result_rx = Some(rx);
        self.thinking_mode = Some(self.current_mode);
        self.is_thinking.insert(self.current_mode, true);
        self.thinking_status
            .insert(self.current_mode, "Spec is building...".to_string());
        self.build_status = Some("Running Spec Kit...".to_string());
        self.build_status_is_error = false;

        // Keep the Spec logo visible while running.
        if self.current_mode == ChatMode::Build {
            self.show_preview = true;
            self.active_viewer = ActiveViewer::Panel;
            self.preview_panel.show_mode_intro("build");
        }

        std::thread::spawn(move || {
            let output = run_user_command(&command);
            let _ = tx.send(CommandExecResult { command, output });
        });
    }

    /// Get chat history for current mode
    pub fn chat_history(&self) -> &Vec<ChatMessage> {
        self.mode_chat_histories.get(&self.current_mode).unwrap()
    }

    /// Get mutable chat history for current mode
    pub fn chat_history_mut(&mut self) -> &mut Vec<ChatMessage> {
        self.mode_chat_histories
            .get_mut(&self.current_mode)
            .unwrap()
    }

    /// Push a message to current mode's chat history
    pub fn push_chat(&mut self, msg: ChatMessage) {
        self.mode_chat_histories
            .get_mut(&self.current_mode)
            .unwrap()
            .push(msg);
    }

    pub fn push_chat_to(&mut self, mode: ChatMode, msg: ChatMessage) {
        if mode != self.current_mode && msg.role == "assistant" {
            self.unread_modes.insert(mode);
        }
        if let Some(history) = self.mode_chat_histories.get_mut(&mode) {
            history.push(msg);
        }
    }

    /// Sync the current chat into thread_history and save to disk.
    /// Call after adding a user or assistant message.
    pub fn sync_thread_history(&mut self, mode: ChatMode) {
        let history = match self.mode_chat_histories.get(&mode) {
            Some(h) => h,
            None => return,
        };

        // Skip if only welcome messages (no user messages yet)
        let has_user_msg = history.iter().any(|m| m.role == "user");
        if !has_user_msg {
            return;
        }

        // Get or create thread ID
        let thread_id = self.current_thread_id.clone().unwrap_or_else(|| {
            let id = format!(
                "{}-{}",
                chrono::Utc::now().format("%Y%m%d-%H%M%S"),
                std::process::id() % 10000
            );
            self.current_thread_id = Some(id.clone());
            id
        });

        // Find first user message for title generation
        let first_user_msg = history
            .iter()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("New conversation");

        // Build or update thread
        if let Some(thread) = self.thread_history.get_thread_mut(&thread_id) {
            // Update existing thread with latest message
            if let Some(last) = history.last() {
                thread.add_message(&last.content);
            }
            // Sync full message list
            thread.messages = history
                .iter()
                .map(|m| crate::thread_history::SimpleMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect();
        } else {
            // Create new thread
            let mut thread = crate::thread_history::Thread::new(
                thread_id.clone(),
                mode,
                first_user_msg,
            );
            thread.message_count = history.len();
            thread.messages = history
                .iter()
                .map(|m| crate::thread_history::SimpleMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect();
            if let Some(last) = history.last() {
                thread.last_message_preview =
                    if last.content.len() > 80 {
                        format!("{}...", &last.content[..80])
                    } else {
                        last.content.clone()
                    };
            }
            self.thread_history.upsert_thread(thread);
        }

        self.thread_history.save_to_disk();
    }

    /// Load a thread from history back into the active chat.
    /// Switches mode if needed and closes the history panel.
    pub fn load_thread(&mut self, thread_id: &str) {
        let (mode, messages) = {
            let thread = match self.thread_history.get_thread(thread_id) {
                Some(t) => t,
                None => return,
            };
            let msgs: Vec<ChatMessage> = thread
                .messages
                .iter()
                .map(|m| ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    details: None,
                    timestamp: String::new(),
                })
                .collect();
            (thread.mode, msgs)
        };

        if !messages.is_empty() {
            self.mode_chat_histories.insert(mode, messages);
        }
        self.current_mode = mode;
        self.current_thread_id = Some(thread_id.to_string());
        self.show_thread_history = false;
        self.preview_panel.show_mode_intro(mode.as_str());
    }

    /// Check for completed AI responses (called each frame)
    /// Poll for live status updates from the AI pipeline. Call once per frame.
    pub fn poll_ai_status(&mut self) {
        if let Some(rx) = &self.ai_status_rx {
            // Drain all pending status updates (use the latest one)
            let mut latest: Option<String> = None;
            while let Ok(status) = rx.try_recv() {
                latest = Some(status);
            }
            if let Some(status) = latest {
                if let Some(mode) = self.thinking_mode {
                    self.thinking_status.insert(mode, status);
                }
            }
        }
    }

    /// Poll for streaming text chunks from the AI pipeline. Call once per frame.
    /// Drains all available chunks and appends text to `streaming_partial[mode]`.
    pub fn poll_ai_stream(&mut self) {
        if let Some(rx) = &self.ai_stream_rx {
            let mode = self.thinking_mode.unwrap_or(self.current_mode);
            loop {
                match rx.try_recv() {
                    Ok(chunk) => match chunk {
                        StreamChunk::Text(t) => {
                            self.streaming_partial
                                .entry(mode)
                                .or_default()
                                .push_str(&t);
                        }
                        StreamChunk::Done { stop_reason } => {
                            // If this is an iteration reset (between agent loop turns),
                            // clear the partial text so the next iteration starts fresh.
                            if stop_reason.as_deref() == Some("iteration_reset") {
                                self.streaming_partial.remove(&mode);
                                // Don't break — keep draining for the next iteration's chunks
                                continue;
                            }
                            // Otherwise it's the real end — stop polling
                            break;
                        }
                        StreamChunk::Error(_) => {
                            break;
                        }
                        // ToolUseStart/ToolInputDelta/ToolUseComplete are handled inside
                        // run_ai_generation; they don't reach the UI stream channel as text.
                        _ => {}
                    },
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break;
                    }
                }
            }
        }
    }

    pub fn poll_ai_response(&mut self) {
        if let Some(rx) = &self.ai_result_rx {
            // Non-blocking check for result
            if let Ok(result) = rx.try_recv() {
                let response_mode = self.thinking_mode;

                // Clear thinking state for the mode that was processing
                if let Some(mode) = self.thinking_mode {
                    self.is_thinking.insert(mode, false);
                    self.thinking_status.insert(mode, String::new());
                    self.ai_abort_handles.remove(&mode);
                    self.thinking_started_at.remove(&mode);
                    self.slow_response_hint_shown.remove(&mode);
                }
                self.thinking_mode = None;
                self.ai_status_rx = None;
                self.ai_stream_rx = None;
                // Clear streaming partial for the mode that finished
                if let Some(mode) = response_mode {
                    self.streaming_partial.remove(&mode);
                }
                self.show_model_hint = false;
                self.model_hint_started_at = None;
                self.ai_result_rx = None;
                self.refocus_input = true;

                // Return to welcome view (unless Rick Roll is showing)
                if matches!(self.active_viewer, ActiveViewer::Matrix) {
                    self.active_viewer = ActiveViewer::Panel;
                }

                if let Some(error) = result.error {
                    self.pending_commands.clear();

                    // Friendlier, actionable error messaging (and pop Settings for key/config issues)
                    let lower = error.to_lowercase();
                    let mut open_settings = false;
                    let (error_content, details) = if lower.contains("no gemini authentication")
                        || lower.contains("gemini_api_key")
                        || lower.contains("gemini error")
                        || lower.contains("no openai authentication")
                        || lower.contains("openai") && lower.contains("api key")
                        || lower.contains("no anthropic authentication")
                        || lower.contains("anthropic") && lower.contains("api key")
                        || lower.contains("401")
                        || lower.contains("403")
                        || lower.contains("unauthorized")
                        || lower.contains("forbidden")
                    {
                        open_settings = true;
                        let provider = if lower.contains("gemini") {
                            "Gemini"
                        } else if lower.contains("anthropic") {
                            "Anthropic"
                        } else if lower.contains("openai") {
                            "OpenAI"
                        } else {
                            "your provider"
                        };
                        (
                            format!(
                                "I couldn’t connect to {}.\n\n\
What to do next:\n\
- I opened Settings so you can paste/check your API key\n\
- Make sure the key is valid and the right API is enabled\n\
- If you’d rather not use cloud keys, switch to Local (Ollama)",
                                provider
                            ),
                            Some(error.clone()),
                        )
                    } else if lower.contains("connection refused")
                        || lower.contains("error sending request")
                            && lower.contains("11434")
                        || lower.contains("ollama error")
                        || lower.contains("tcp connect error")
                            && lower.contains("11434")
                    {
                        open_settings = true;
                        (
                            "I can't reach Ollama (the local AI engine).\n\n\
What to do next:\n\
- If Ollama is installed, make sure it's running\n\
- If it's not installed, grab it from https://ollama.com\n\
- Or switch to a cloud provider in Settings and paste an API key"
                                .to_string(),
                            Some(error.clone()),
                        )
                    } else {
                        (
                            "I hit an error while processing that. Try again; if it keeps happening, switch models or open Settings.".to_string(),
                            Some(error.clone()),
                        )
                    };

                    if open_settings {
                        self.show_settings_dialog = true;
                        // Make sure the user sees the UI while fixing this.
                        self.show_preview = true;
                        if matches!(self.active_viewer, ActiveViewer::Matrix) {
                            self.active_viewer = ActiveViewer::Panel;
                        }
                    }
                    let error_msg = ChatMessage {
                        role: "assistant".to_string(),
                        content: error_content,
                        details,
                        timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                    };
                    if let Some(mode) = response_mode {
                        self.push_chat_to(mode, error_msg);
                    } else {
                        self.push_chat(error_msg);
                    }
                } else {
                    let mode = response_mode.unwrap_or(self.current_mode);

                    // Store file to preview
                    self.pending_preview = result.preview_file;
                    self.pending_commands = result.pending_commands.clone();

                    // Show executed commands in preview panel (not in chat — keep it clean)
                    // Also extract real file paths from command output for clickable buttons
                    let mut found_files: Vec<String> = Vec::new();
                    if !result.executed_commands.is_empty() {
                        if let Some((cmd, output, _)) = result.executed_commands.last() {
                            self.active_viewer =
                                ActiveViewer::CommandOutput(cmd.clone(), output.clone());
                        }
                        self.show_preview = true;

                        // Extract real file paths from all command outputs
                        for (_cmd, output, success) in &result.executed_commands {
                            if *success {
                                for line in output.lines() {
                                    let trimmed = line.trim();
                                    if trimmed.starts_with('/') && std::path::Path::new(trimmed).exists() {
                                        found_files.push(trimmed.to_string());
                                    }
                                }
                            }
                        }
                    }

                    // Parse for preview tags (<preview type="..." ...>)
                    for tag in parse_preview_tags(&result.response) {
                        if let Some(content) = tag.to_content() {
                            match &content {
                                PreviewContent::Web { url, .. } => {
                                    // Fetch web preview metadata in background
                                    self.fetch_web_preview(url.clone(), Some(tag.caption.clone()));
                                }
                                PreviewContent::File { path, .. } => {
                                    if self.is_path_permitted(path) {
                                        self.pending_preview = Some(path.clone());
                                    }
                                }
                                _ => {
                                    self.preview_panel.show_content(content);
                                }
                            }
                        }
                    }

                    // If nothing explicit was previewed, try to proactively preview a referenced image/pdf.
                    if self.pending_preview.is_none() {
                        if let Some(path) = crate::utils::extract_previewable_file(
                            &result.response,
                            &self.settings.allowed_dirs,
                        ) {
                            if self.is_path_permitted(&path) {
                                self.pending_preview = Some(path);
                            }
                        }
                    }

                    self.last_response_tokens_est = Self::estimate_tokens(&result.response);
                    self.session_output_tokens_est = self
                        .session_output_tokens_est
                        .saturating_add(self.last_response_tokens_est as u64);

                    // Clean up response - remove action tags (both old and new style)
                    let clean_response = clean_ai_response(&result.response);
                    // Also strip new-style preview tags
                    let mut clean_response = strip_preview_tags(&clean_response);

                    // Append real file paths found from commands so they become clickable
                    if !found_files.is_empty() && !found_files.iter().any(|f| clean_response.contains(f)) {
                        clean_response.push_str("\n\nFiles found:\n");
                        for f in &found_files {
                            clean_response.push_str(&format!("  {}\n", f));
                        }
                    }

                    let assistant_msg = ChatMessage {
                        role: "assistant".to_string(),
                        content: if clean_response.trim().is_empty() {
                            result.response.clone()
                        } else {
                            clean_response
                        },
                        details: None,
                        timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                    };
                    self.push_chat_to(mode, assistant_msg);

                    // Sync thread history after AI response
                    self.sync_thread_history(mode);

                    if !self.pending_commands.is_empty() {
                        self.push_chat_to(mode, ChatMessage {
                            role: "assistant".to_string(),
                            content: "I'd like to do something that needs your OK first — check the buttons below.".to_string(),
                            details: None,
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                    }
                }
            }
        }
    }

    pub fn poll_command_result(&mut self) {
        if let Some(rx) = &self.command_result_rx {
            if let Ok(result) = rx.try_recv() {
                self.command_result_rx = None;
                // Clear thinking state for the mode that was processing
                let active_mode = self.thinking_mode;
                if let Some(mode) = active_mode {
                    self.is_thinking.insert(mode, false);
                    self.thinking_status.insert(mode, String::new());
                }
                self.thinking_mode = None;

                match result.output {
                    Ok(cmd_result) => {
                        if active_mode == Some(ChatMode::Build) {
                            // Keep the Spec logo in the preview panel.
                            self.active_viewer = ActiveViewer::Panel;
                            self.preview_panel.show_mode_intro("build");

                            self.push_chat(ChatMessage {
                                role: "assistant".to_string(),
                                content: if cmd_result.success {
                                    "Spec finished. (Open Details to see the full output.)".to_string()
                                } else {
                                    "Spec hit an error. (Open Details to see the full output.)".to_string()
                                },
                                details: Some(cmd_result.output.clone()),
                                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                            });
                        } else {
                            self.active_viewer = ActiveViewer::CommandOutput(
                                result.command.clone(),
                                cmd_result.output.clone(),
                            );
                            self.push_chat(ChatMessage {
                                role: "assistant".to_string(),
                                content: format!(
                                    "Command `{}` completed.\n\n```\n{}\n```",
                                    result.command, cmd_result.output
                                ),
                                details: None,
                                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                            });
                        }
                        if active_mode == Some(ChatMode::Build) {
                            self.build_status = Some(if cmd_result.success {
                                "Spec Kit finished successfully".to_string()
                            } else {
                                "Spec Kit reported an error".to_string()
                            });
                            self.build_status_is_error = !cmd_result.success;
                        }
                    }
                    Err(err) => {
                        self.push_chat(ChatMessage {
                            role: "assistant".to_string(),
                            content: format!("Command `{}` failed to run: {}", result.command, err),
                            details: None,
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                        if active_mode == Some(ChatMode::Build) {
                            self.build_status = Some("Spec Kit command failed".to_string());
                            self.build_status_is_error = true;
                            self.active_viewer = ActiveViewer::Panel;
                            self.preview_panel.show_mode_intro("build");
                        }
                    }
                }
            }
        }
    }

    /// Poll for web preview fetch results
    pub fn poll_web_preview(&mut self) {
        if let Some(rx) = &self.web_preview_rx {
            if let Ok(result) = rx.try_recv() {
                self.web_preview_rx = None;

                // Update the preview panel with fetched metadata
                self.preview_panel.show_content(PreviewContent::Web {
                    url: result.url,
                    title: result.title,
                    screenshot: result.screenshot,
                    og_image: result.og_image,
                    snippet: result.snippet,
                });
            }
        }
    }

    /// Fetch web preview metadata in background
    pub fn fetch_web_preview(&mut self, url: String, snippet: Option<String>) {
        let (tx, rx) = channel::<WebPreviewResult>();
        self.web_preview_rx = Some(rx);

        // Show loading state immediately with URL and snippet
        self.preview_panel.show_content(PreviewContent::Web {
            url: url.clone(),
            title: Some("Loading...".to_string()),
            screenshot: None,
            og_image: None,
            snippet: snippet.clone(),
        });

        let service = Arc::clone(&self.web_preview_service);

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => {
                    let _ = tx.send(WebPreviewResult {
                        url: url.clone(),
                        title: None,
                        screenshot: None,
                        og_image: None,
                        snippet,
                    });
                    return;
                }
            };

            let preview = rt.block_on(service.get_preview(&url));

            let result = match preview {
                Ok(p) => WebPreviewResult {
                    url: p.url,
                    title: p.title,
                    screenshot: p.screenshot_path,
                    og_image: p.og_image,
                    snippet: p.snippet,
                },
                Err(_) => WebPreviewResult {
                    url,
                    title: None,
                    screenshot: None,
                    og_image: None,
                    snippet,
                },
            };

            let _ = tx.send(result);
        });
    }

    pub fn approve_command(&mut self, command: String) {
        self.pending_commands.retain(|c| c != &command);
        if let Err(reason) = validate_command_against_allowed(&command, &self.settings.allowed_dirs)
        {
            self.push_chat(ChatMessage {
                role: "assistant".to_string(),
                content: format!("Command `{}` blocked: {}", command, reason),
                details: None,
                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
            });
            self.command_result_rx = None;
            // Clear thinking for current mode
            self.is_thinking.insert(self.current_mode, false);
            self.thinking_status.insert(self.current_mode, String::new());
            return;
        }

        // Check if command needs sudo
        let danger_level = classify_command(&command);
        eprintln!("DEBUG: Command '{}' classified as {:?}", command, danger_level);
        if danger_level == DangerLevel::NeedsSudo {
            eprintln!("DEBUG: Opening password dialog for sudo command");
            #[cfg(windows)]
            {
                self.push_chat(ChatMessage {
                    role: "assistant".to_string(),
                    content: "This command needs admin privileges, but privileged execution isn’t supported on Windows yet.".to_string(),
                    details: None,
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                });
                self.command_result_rx = None;
                self.is_thinking.insert(self.current_mode, false);
                self.thinking_status.insert(self.current_mode, String::new());
                return;
            }

            #[cfg(not(windows))]
            {
                // Show password dialog
                self.password_dialog.open_with_message(format!(
                    "Command '{}' requires administrator privileges.\n\nEnter your password:",
                    command
                ));
                self.pending_sudo_command = Some(command);
                return;
            }
        }

        let (tx, rx) = channel::<CommandExecResult>();
        self.command_result_rx = Some(rx);
        // Set thinking for current mode
        self.thinking_mode = Some(self.current_mode);
        self.is_thinking.insert(self.current_mode, true);
        self.thinking_status.insert(self.current_mode, format!("Running {}", command));

        std::thread::spawn(move || {
            let output = run_user_command(&command);
            let _ = tx.send(CommandExecResult { command, output });
        });
    }

    /// Execute a sudo command with the provided password
    pub fn execute_sudo_command(&mut self, command: String, password: String) {
        #[cfg(windows)]
        {
            let _ = password;
            self.push_chat(ChatMessage {
                role: "assistant".to_string(),
                content: format!(
                    "I can’t run `{}` with admin privileges on Windows yet.",
                    command
                ),
                details: None,
                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
            });
            self.pending_sudo_command = None;
            self.is_thinking.insert(self.current_mode, false);
            self.thinking_status.insert(self.current_mode, String::new());
            return;
        }

        #[cfg(not(windows))]
        {
        let (tx, rx) = channel::<CommandExecResult>();
        self.command_result_rx = Some(rx);
        // Set thinking for current mode
        self.thinking_mode = Some(self.current_mode);
        self.is_thinking.insert(self.current_mode, true);
        self.thinking_status.insert(self.current_mode, format!("Running {} (with privileges)", command));
        self.pending_sudo_command = None;

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new();
            let output = match runtime {
                Ok(rt) => rt.block_on(async {
                    execute_with_sudo(&command, &password, 60).await
                        .map_err(|e| e.to_string())
                }),
                Err(e) => Err(format!("Failed to create runtime: {}", e)),
            };
            let _ = tx.send(CommandExecResult { command, output });
        });
        }
    }

    /// Load the mascot image as a texture (custom or default)
    pub fn load_mascot_texture(&mut self, ctx: &egui::Context) {
        if self.mascot_loaded {
            return;
        }
        self.mascot_loaded = true;

        // Try custom image first, fall back to default
        let image_result = if let Some(path_str) = &self.settings.user_profile.mascot_image_path {
            let path = Path::new(path_str);
            if path.exists() {
                image::open(path).ok()
            } else {
                None
            }
        } else {
            None
        };

        // Use custom image or fall back to embedded default
        let image_data =
            image_result.or_else(|| image::load_from_memory(crate::DEFAULT_MASCOT).ok());

        if let Some(img) = image_data {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.into_raw();

            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
            let texture = ctx.load_texture("mascot", color_image, egui::TextureOptions::LINEAR);
            self.mascot_texture = Some(texture);
        }
    }

    /// Reload mascot texture when path changes
    #[allow(dead_code)] // Available for settings UI
    pub fn reload_mascot_texture(&mut self, ctx: &egui::Context) {
        self.mascot_loaded = false;
        self.mascot_texture = None;
        self.load_mascot_texture(ctx);
    }

    pub fn send_message(&mut self) {
        if self.input_text.trim().is_empty() {
            return;
        }

        // Only allow one in-flight request at a time.
        if let Some(active_mode) = self.thinking_mode {
            if active_mode != self.current_mode
                && self.is_thinking.get(&active_mode).copied().unwrap_or(false)
            {
                self.push_chat(ChatMessage {
                    role: "assistant".to_string(),
                    content: format!(
                        "{} is still working. If you want to switch tasks, click 'Stop it' in the banner at the top.",
                        match active_mode {
                            ChatMode::Find => "Find Helper",
                            ChatMode::Fix => "Fix Helper",
                            ChatMode::Research => "Research Helper",
                            ChatMode::Data => "Data Helper",
                            ChatMode::Content => "Content Helper",
                            ChatMode::Build => "Spec",
                        }
                    ),
                    details: None,
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                });
                return;
            }
        }

        // (No hidden triggers in public builds)

        // Add user message to chat
        let user_msg = ChatMessage {
            role: "user".to_string(),
            content: self.input_text.clone(),
            details: None,
            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
        };
        self.push_chat(user_msg);

        // Track this conversation in thread history
        let mode = self.current_mode;
        self.sync_thread_history(mode);

        // Model/provider safety: avoid picking a cloud provider with no key.
        let primary_provider = self
            .settings
            .model
            .provider_preference
            .first()
            .map(|s| s.as_str())
            .unwrap_or("local");

        if primary_provider != "local" && !self.provider_has_api_key(primary_provider) {
            if Self::ollama_reachable() {
                self.settings.model.provider_preference = vec![
                    "local".to_string(),
                    "anthropic".to_string(),
                    "openai".to_string(),
                    "gemini".to_string(),
                ];
                crate::utils::save_settings(&self.settings);
                self.push_chat(ChatMessage {
                    role: "assistant".to_string(),
                    content: "No cloud API key is set yet, so I’m using the local model (Ollama). You can add keys in Settings if you want faster cloud replies.".to_string(),
                    details: None,
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                });
            } else {
                self.push_chat(ChatMessage {
                    role: "assistant".to_string(),
                    content: "No cloud API key is set, and Ollama doesn’t look reachable on this machine. Start Ollama (or install it), or add a cloud API key in Settings → AI Model.".to_string(),
                    details: None,
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                });
                // Nothing to run.
                self.thinking_mode = None;
                self.is_thinking.insert(self.current_mode, false);
                return;
            }
        }

        // Clear input and show thinking state for current mode
        let _query = self.input_text.clone();
        self.input_text.clear();
        self.thinking_mode = Some(self.current_mode);
        self.is_thinking.insert(self.current_mode, true);

        // Don't show Matrix immediately — it's distracting for quick responses.
        // The main loop will switch to Matrix after a few seconds of thinking.
        self.show_preview = true;

        // Prepare context based on current mode
        let user_name = if self.settings.user_profile.name.is_empty() {
            "friend".to_string()
        } else {
            self.settings.user_profile.name.clone()
        };

        // Detect OS for platform-specific commands
        #[cfg(target_os = "windows")]
        let is_windows = true;
        #[cfg(not(target_os = "windows"))]
        let is_windows = false;

        // Core capabilities the agent should know about (terminal access controlled by permission)
        let terminal_enabled = self.settings.user_profile.terminal_permission_granted;

        let capabilities = if terminal_enabled {
            format!("
YOU HAVE TERMINAL ACCESS. Propose concrete commands with <command> tags so I can review and approve them.
Do not merely describe steps—share the exact commands you want to run.

CAPABILITIES:
- You can REQUEST TERMINAL COMMANDS using <command>your command</command> tags. I will queue them for approval.
- You can SEARCH THE WEB using <search>your query</search> tags when you need current info.
- You can AUTO-OPEN FILES in the preview panel using <preview>/path/to/file</preview> tags.
- Supported preview types: text files, images (png/jpg/gif), CSV/data files, JSON, HTML, Markdown

When the user asks for an action, provide the exact commands you'd run. They will only execute after approval.

{}
", get_campaign_summary(&self.settings))
        } else {
            format!(
                "
CAPABILITIES (Limited Mode - Terminal Disabled):
- You can SEARCH THE WEB using <search>your query</search> tags when you need current info.
- You can reference files in the preview panel using <preview>/path/to/file</preview> tags.
- Supported preview types: text files, images (png/jpg/gif), CSV/data files, JSON, HTML, Markdown

NOTE: Terminal command execution is disabled. You cannot run <command> tags.
Instead, provide instructions the user can run manually.

{}
",
                get_campaign_summary(&self.settings)
            )
        };

        // Platform-specific Find mode hints
        let find_hint = if is_windows {
            r#"Platform: Windows. Common dirs: Documents, Desktop, Downloads.
SEARCH TECHNIQUES — always use wildcards for partial/fuzzy matching:
- By name: <command>dir /s /b "C:\Users\%USERNAME%\*keyword*"</command>
- By extension: <command>dir /s /b "C:\Users\%USERNAME%\Documents\*.pdf"</command>
- By content: <command>findstr /s /i "keyword" "C:\Users\%USERNAME%\Documents\*.*"</command>
IMPORTANT: Always use *wildcards* around search terms (e.g. *tax* not tax). Search multiple common folders."#
        } else {
            r#"Platform: Unix/Mac. Common dirs: ~/Documents, ~/Desktop, ~/Downloads.
SEARCH TECHNIQUES — always use wildcards and -iname for case-insensitive partial matching:
- By name: <command>find ~ -iname "*keyword*" -type f 2>/dev/null | head -30</command>
- By extension: <command>find ~/Documents -iname "*.pdf" 2>/dev/null | head -30</command>
- By content: <command>grep -ril "keyword" ~/Documents 2>/dev/null | head -20</command>
- Recent files: <command>find ~ -iname "*keyword*" -mtime -30 -type f 2>/dev/null | head -20</command>
IMPORTANT: Always use -iname (case-insensitive) with *wildcards* (e.g. *tax* not tax). Search broadly first, then narrow down."#
        };

        // Platform-specific Fix mode hints (kept minimal to reduce prompt size)
        let fix_hint = if is_windows {
            "Diagnostics: systeminfo, wmic, ipconfig, ping, tasklist, netstat. Use PowerShell when needed."
        } else {
            "Diagnostics: uname -a, df -h, free -h, ip addr, ping, ps aux, systemctl, journalctl, lsof."
        };

        let system_prompt = match self.current_mode {
            ChatMode::Find => format!(
                r#"You are Little Helper in FIND mode, helping {user_name}.
YOUR JOB: Locate files and content. Use <command>cmd</command> to search. Use <preview>path</preview> to show files.
Keep commands read-only and single-step. {find_hint}
RESPONSE STYLE: After commands run, always reply with a friendly plain-language summary of what was found (e.g. "I found 3 files matching 'mandate':" followed by a clean list). Never show raw terminal commands to the user. The user is non-technical.
{capabilities}"#
            ),
            ChatMode::Fix => format!(
                r#"You are Little Helper in FIX mode, helping {user_name}.
YOUR JOB: Tech support — diagnose, find files, fix issues. Run commands, don't just explain.
{find_hint}
{fix_hint}
Workflow: run diagnostics → <search>search solutions</search> if needed → explain → fix. Use <preview>path</preview> to show files.
RESPONSE STYLE: After commands run, always reply with a friendly plain-language summary of what you found and what you recommend. Never show raw terminal commands to the user. The user is non-technical.
{capabilities}"#
            ),
            ChatMode::Research => {
                format!(
                    r#"You are Little Helper in DEEP RESEARCH mode, helping {user_name}.
YOUR ROLE: Thorough researcher. Search multiple angles, cross-reference sources, cite everything.

TOOLS:
- <search>query</search> — ALWAYS use this to search the web. Do NOT suggest websites — search for the user.
- <command>cmd</command> — Run scripts (python3, curl, jq available). Use to save reports to files.
- <preview>path</preview> — Show documents in the preview panel.

IMPORTANT: When the user asks you to research something, you MUST use <search> tags to actually search. Never just list websites.
When asked to write a report or create a document, save it using <command>cat > ~/Documents/report-title.md << 'ENDREPORT' ... ENDREPORT</command> then <preview> the file.

Distinguish facts from speculation. Cite your sources with URLs.
RESPONSE STYLE: Explain findings in plain language. The user is non-technical.
{capabilities}"#
                )
            },
            ChatMode::Data => format!(
                "You are Little Helper, a data assistant helping {}. Help work with CSV files, JSON data, and databases. Use <command></command> to examine files. ALWAYS open data files in the preview panel so the user can see what you're working with. Walk them through the data visually.\n{}",
                user_name, capabilities
            ),
            ChatMode::Content => {
                // Load full campaign context + personas + DDD workflow for Content mode
                let campaign_docs = if self.settings.enable_campaign_context {
                    load_campaign_context()
                } else {
                    "Project context is disabled. Enable it in Settings to connect your files."
                        .to_string()
                };
                let personas = if self.settings.enable_persona_context {
                    load_personas()
                } else {
                    "PERSONA CONTEXT DISABLED. Enable it in Settings to include persona guidance."
                        .to_string()
                };
                let ddd_workflow = load_ddd_workflow();

                format!(
                    r#"You are Little Helper in CONTENT CREATION mode, helping {}.

YOUR ROLE: Content strategist using Data Driven Designs methodology.

{}

{}

{}

FOLDERS:
- Drafts folder: Choose a folder in Settings and save drafts there

WORKFLOW (Data Driven Designs):
1. Identify the target PERSONA for this content
2. Review campaign materials for relevant facts/data
3. Draft content matching persona's language and concerns
4. Save drafts with format: YYYY-MM-DD_platform_topic.md

CONTENT TYPES:
- Twitter/X: Short, punchy, hashtags (280 chars) - match persona voice
- LinkedIn: Professional, detailed, stats-focused - use persona's trusted language
- Facebook: Community-focused, engaging questions - address persona's concerns
- Instagram: Visual-first, storytelling - emotional connection

PERSONA-DRIVEN CONTENT:
- ALWAYS identify which persona you're targeting
- Use the persona's preferred language and phrases
- Address their specific concerns and motivations
- Avoid words/phrases the persona dislikes
- Include the "Sample Voice" tone from the persona

ALWAYS:
- Name the target persona at the start of each draft
- Match language to persona (use their words, avoid their turn-offs)
- Include relevant stats from campaign materials
- Reference specific facts from loaded documents
- Save drafts to the drafts folder you chose

{}
"#,
                    user_name, ddd_workflow, personas, campaign_docs, capabilities
                )
            },
            ChatMode::Build => {
                let spec_kit_path = self.spec_kit_path();
                let spec_kit_available = spec_kit_path.exists();
                let folder = self.build_folder_input.trim().to_string();
                let project_name = self.build_project_name_input.trim().to_string();

                let spec_kit_section = if spec_kit_available {
                    let sk = spec_kit_path.to_string_lossy();
                    let mut section = format!(
                        r#"SPEC KIT ASSISTANT:
Spec Kit is available at: {sk}
Run it with: <command>cd "{folder}" && node "{sk}" SUBCOMMAND</command>

Available subcommands (run them in order):
  init PROJECTNAME  — Create a new project with constitution
  specify           — Create the feature spec
  clarify           — Ask clarifying questions about the spec
  plan              — Generate an implementation plan
  analyze           — Cross-check spec, plan for consistency
  tasks             — Break the plan into tasks
  implement         — Execute tasks one by one

"#);
                    if !folder.is_empty() {
                        section.push_str(&format!("Current project folder: {}\n", folder));
                    }
                    if !project_name.is_empty() {
                        section.push_str(&format!("Project name: {}\n", project_name));
                    }
                    section
                } else {
                    "SPEC KIT: Not found. Help the user set it up in Settings, or scaffold the project manually with <command> tags.\n".to_string()
                };

                format!(
                    r#"You are Little Helper in BUILD mode, helping {user_name}.

YOUR ROLE: Practical builder who creates projects and runs spec-driven workflows without asking the user to use a terminal.

RULES:
- Always say "folder" (never "directory")
- Offer simple steps and buttons; avoid terminal jargon
- When you run spec-kit commands, use <command> tags — they will execute automatically if safe
- If the user hasn't set a project folder or name yet, ask for them before running spec-kit

{spec_kit_section}

WORKFLOW:
1. Ask what they want to build (if not already clear)
2. Confirm the folder and project name shown above (or ask if missing)
3. Run spec-kit commands via <command> tags to scaffold and build
4. Summarize progress clearly after each step

{capabilities}
"#)
            },
        };

        let (api_messages, prompt_tokens_est, dropped) =
            self.build_api_messages_with_budget(system_prompt);

        self.last_prompt_tokens_est = prompt_tokens_est;
        self.session_input_tokens_est = self
            .session_input_tokens_est
            .saturating_add(prompt_tokens_est as u64);

        if dropped > 0 {
            if let Some(mode) = self.thinking_mode {
                self.thinking_status.insert(
                    mode,
                    format!("Making room (trimmed {} older messages)...", dropped),
                );
            }
        }

        // Start async AI generation with capability flags
        self.start_ai_generation(
            api_messages,
            terminal_enabled,
            self.settings.enable_internet_research,
        );
    }

    pub fn start_ai_generation(
        &mut self,
        messages: Vec<ApiChatMessage>,
        allow_terminal: bool,
        allow_web: bool,
    ) {
        let (tx, rx) = channel::<AiResult>();
        self.ai_result_rx = Some(rx);

        let (status_tx, status_rx) = channel::<String>();
        self.ai_status_rx = Some(status_rx);

        // Streaming channel: UI polls this per frame for incremental text
        let (stream_tx, stream_rx) = channel::<StreamChunk>();
        self.ai_stream_rx = Some(stream_rx);

        let mode = self.thinking_mode.unwrap_or(self.current_mode);
        // Clear any leftover streaming partial for this mode
        self.streaming_partial.remove(&mode);

        let (abort_handle, abort_reg) = futures::future::AbortHandle::new_pair();
        self.ai_abort_handles.insert(mode, abort_handle);
        // Set thinking status for the mode that initiated the request (unless already set)
        if let Some(mode) = self.thinking_mode {
            let current = self.thinking_status.get(&mode).cloned().unwrap_or_default();
            if current.trim().is_empty() {
                self.thinking_status.insert(mode, "Thinking...".to_string());
            }
        }
        self.thinking_started_at
            .insert(mode, std::time::Instant::now());
        self.slow_response_hint_shown.insert(mode, false);

        // Set Brave Search API key as env var if configured
        if let Some(ref key) = self.settings.brave_search_api_key {
            if !key.is_empty() {
                std::env::set_var("BRAVE_SEARCH_API_KEY", key);
            }
        }

        let settings = self.settings.model.clone();
        let allowed_dirs = self.settings.allowed_dirs.clone();

        // Spawn background thread for AI work
        std::thread::spawn(move || {
            let tx_panic = tx.clone();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_ai_generation(
                    messages,
                    settings,
                    allow_terminal,
                    allow_web,
                    allowed_dirs,
                    tx,
                    status_tx,
                    stream_tx,
                    abort_reg,
                );
            }));
            if res.is_err() {
                let _ = tx_panic.send(AiResult {
                    response: String::new(),
                    preview_file: None,
                    error: Some(
                        "Something went wrong while processing that request. Please try again; if it keeps happening, open Settings and re-check your model + keys.".to_string(),
                    ),
                    executed_commands: Vec::new(),
                    pending_commands: Vec::new(),
                });
            }
        });
    }

    pub fn cancel_ai(&mut self, mode: ChatMode) {
        if let Some(handle) = self.ai_abort_handles.remove(&mode) {
            handle.abort();
        }
        self.thinking_status
            .insert(mode, "Stopping...".to_string());
    }

    /// Open a file in the preview panel
    pub fn open_file(&mut self, path: &Path, ctx: &egui::Context) {
        if !self.is_path_permitted(path) {
            return;
        }
        self.preview_panel.open_file(path, ctx);
        self.active_viewer = ActiveViewer::Panel;
        self.show_preview = true;
    }

    pub fn close_preview(&mut self) {
        self.show_preview = false;
        self.preview_panel.hide();
        self.active_viewer = ActiveViewer::Panel;
    }
}
