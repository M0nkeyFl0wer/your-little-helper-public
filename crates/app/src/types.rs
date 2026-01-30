//! Core types for the Little Helper app
//!
//! This module contains all the main type definitions used throughout the app,
//! including result types, screen states, chat types, and the main AppState.

use agent_host::CommandResult;
use eframe::egui;
use services::web_preview::WebPreviewService;
use shared::settings::AppSettings;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

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

/// Current app screen
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppScreen {
    Onboarding,
    Chat,
}

/// Chat mode - determines agent behavior and available skills
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChatMode {
    /// Tech support - diagnose and fix problems
    Fix,
    /// Deep research with citations
    Research,
    /// Work with data and files
    Data,
    /// Content creation with personas
    Content,
}

impl ChatMode {
    /// Get the mode name as a string for the agent system
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMode::Fix => "fix",
            ChatMode::Research => "research",
            ChatMode::Data => "data",
            ChatMode::Content => "content",
        }
    }
}

/// A chat message
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
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
    /// Current input text
    pub input_text: String,
    /// Preserve input per mode
    pub mode_input_drafts: std::collections::HashMap<ChatMode, String>,
    /// Per-mode chat threads
    pub mode_chat_histories: std::collections::HashMap<ChatMode, Vec<ChatMessage>>,
    /// Whether the AI is currently thinking/processing
    pub is_thinking: bool,
    /// What the agent is currently doing
    pub thinking_status: String,
    /// Available for future agentic features
    #[allow(dead_code)]
    pub agent_host: agent_host::AgentHost,

    // Preview panel (new interactive preview companion)
    pub preview_panel: crate::preview_panel::PreviewPanel,

    // Legacy preview panel (for file viewers)
    pub show_preview: bool,
    pub active_viewer: ActiveViewer,
    /// File to auto-open after response
    pub pending_preview: Option<PathBuf>,

    // Onboarding
    pub onboarding_name: String,

    // Pending command approvals
    pub pending_commands: Vec<String>,

    // Background command execution channel
    pub command_result_rx: Option<Receiver<CommandExecResult>>,

    // Background mascot texture
    pub mascot_texture: Option<egui::TextureHandle>,
    pub mascot_loaded: bool,

    // Async AI response channel
    pub ai_result_rx: Option<Receiver<AiResult>>,

    // Web preview service and async fetch channel
    pub web_preview_service: Arc<WebPreviewService>,
    pub web_preview_rx: Option<Receiver<WebPreviewResult>>,

    // Slack integration
    pub show_slack_dialog: bool,
    pub slack_message_to_send: Option<String>,
    pub slack_selected_channel: String,
    pub slack_status: Option<String>,
    pub show_settings_dialog: bool,
    pub new_allowed_dir: String,
    pub settings_status: Option<String>,
    pub settings_status_is_error: bool,
}
