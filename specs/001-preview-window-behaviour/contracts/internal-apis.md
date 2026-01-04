# Internal APIs Contract

**Feature**: 001-preview-window-behaviour
**Date**: 2026-01-04

## Overview

This document defines the internal Rust APIs between crates for the Interactive Preview Companion feature.

---

## Preview Panel API

**Crate**: `app`
**Module**: `preview_panel`

### PreviewPanel

```rust
pub struct PreviewPanel {
    state: PreviewState,
}

impl PreviewPanel {
    /// Create new preview panel
    pub fn new() -> Self;

    /// Show content in the preview panel
    /// Automatically makes panel visible if hidden
    pub fn show_content(&mut self, content: PreviewContent);

    /// Show mode introduction
    pub fn show_mode_intro(&mut self, mode: ChatMode);

    /// Show ASCII art state
    pub fn show_ascii(&mut self, state: AsciiState);

    /// Hide the preview panel
    pub fn hide(&mut self);

    /// Toggle visibility
    pub fn toggle(&mut self);

    /// Set zoom level (clamped to 0.25-4.0)
    pub fn set_zoom(&mut self, zoom: f32);

    /// Get current zoom level
    pub fn zoom(&self) -> f32;

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&mut self);

    /// Check if panel is visible
    pub fn is_visible(&self) -> bool;

    /// Check if in fullscreen mode
    pub fn is_fullscreen(&self) -> bool;

    /// Render the preview panel UI
    pub fn ui(&mut self, ui: &mut egui::Ui);

    /// Render fullscreen overlay (call from main app if fullscreen)
    pub fn fullscreen_ui(&mut self, ctx: &egui::Context);

    /// Get actions available for current content
    pub fn available_actions(&self) -> Vec<PreviewAction>;
}
```

### PreviewAction

```rust
pub enum PreviewAction {
    OpenInApp,
    RevealInFolder,
    OpenInBrowser,
    CopyPath,
    CopyUrl,
    Close,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Fullscreen,
}

impl PreviewPanel {
    /// Execute an action on current content
    pub fn execute_action(&mut self, action: PreviewAction) -> Result<()>;
}
```

---

## Session Manager API

**Crate**: `app`
**Module**: `sessions`

### Existing API (unchanged)

```rust
impl SessionManager {
    pub fn new() -> Self;
    pub fn list(&self, mode: ChatMode) -> &[ChatSession];
    pub fn get(&self, mode: ChatMode, id: &str) -> Option<&ChatSession>;
    pub fn get_mut(&mut self, mode: ChatMode, id: &str) -> Option<&mut ChatSession>;
    pub fn current(&self, mode: ChatMode) -> Option<&ChatSession>;
    pub fn create(&mut self, mode: ChatMode, user_name: &str) -> String;
    pub fn add_message(&mut self, mode: ChatMode, id: &str, msg: ChatMessage);
    pub fn delete(&mut self, mode: ChatMode, id: &str);
    pub fn search(&self, mode: ChatMode, query: &str) -> Vec<SearchResult>;
    pub fn get_memory_summary(&self, mode: ChatMode) -> String;
}
```

### New Methods

```rust
impl SessionManager {
    /// Get recent messages for AI context window
    /// Returns last N messages across recent sessions
    pub fn get_context_messages(&self, mode: ChatMode, max_messages: usize) -> Vec<&ChatMessage>;

    /// Get session by ID, loading from disk if needed (lazy loading)
    pub fn get_or_load(&mut self, mode: ChatMode, id: &str) -> Option<&ChatSession>;

    /// Force reload sessions from disk
    pub fn reload(&mut self, mode: ChatMode);
}
```

---

## Agent Host API

**Crate**: `agent_host`
**Module**: `prompts`

### System Prompt Generation

```rust
/// Get the complete system prompt for a mode
pub fn get_system_prompt(
    mode: ChatMode,
    user_name: &str,
    memory_summary: &str,
    permissions: &Permissions,
) -> String;

/// Get just the mode-specific portion
pub fn get_mode_prompt(mode: ChatMode) -> &'static ModePrompt;

/// Permissions that affect prompt content
pub struct Permissions {
    pub terminal_enabled: bool,
    pub web_search_enabled: bool,
    pub file_access_dirs: Vec<PathBuf>,
}
```

### Mode Prompt Data

```rust
pub struct ModePrompt {
    pub mode: ChatMode,
    pub name: &'static str,
    pub personality: &'static str,
    pub expertise: &'static [&'static str],
    pub example_questions: &'static [&'static str],
    pub tools_description: &'static str,
    pub tone: &'static str,
}
```

---

## Preview Content API

**Crate**: `shared`
**Module**: `preview_types`

### Content Types

```rust
#[derive(Clone, Debug)]
pub enum PreviewContent {
    File {
        path: PathBuf,
        file_type: FileType,
    },
    Web {
        url: String,
        title: Option<String>,
        screenshot: Option<PathBuf>,
        og_image: Option<String>,
        snippet: Option<String>,
    },
    Image {
        source: ImageSource,
    },
    Ascii {
        state: AsciiState,
    },
    ModeIntro {
        mode: ChatMode,
    },
    Error {
        message: String,
        source: String,
    },
}

#[derive(Clone, Debug)]
pub enum ImageSource {
    File(PathBuf),
    Url(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsciiState {
    Welcome,
    Thinking,
    Success,
    Error,
}
```

### Preview Tag Parsing

```rust
/// Parse preview tags from agent response
pub fn parse_preview_tag(response: &str) -> Option<ParsedPreviewTag>;

/// Strip preview tags from response (for display)
pub fn strip_preview_tags(response: &str) -> String;

#[derive(Clone, Debug)]
pub struct ParsedPreviewTag {
    pub content_type: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub state: Option<String>,
    pub caption: String,
}

impl ParsedPreviewTag {
    /// Convert to PreviewContent (may involve loading/fetching)
    pub async fn to_content(&self) -> Result<PreviewContent>;
}
```

---

## Onboarding API

**Crate**: `app`
**Module**: `onboarding`

### Onboarding Flow

```rust
pub struct OnboardingFlow {
    state: OnboardingState,
}

impl OnboardingFlow {
    /// Create new onboarding flow
    pub fn new() -> Self;

    /// Check if onboarding is needed
    pub fn is_needed(settings: &AppSettings) -> bool;

    /// Get current step
    pub fn current_step(&self) -> &OnboardingStep;

    /// Move to next step
    pub fn next(&mut self);

    /// Go back to previous step
    pub fn back(&mut self);

    /// Skip current step (if allowed)
    pub fn skip(&mut self);

    /// Complete onboarding
    pub fn complete(&mut self) -> OnboardingResult;

    /// Render onboarding UI
    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppContext);
}

pub struct OnboardingResult {
    pub terminal_enabled: bool,
    pub dependencies_installed: Vec<String>,
    pub verification_passed: bool,
}
```

### Dependency Checking

```rust
/// Check for a specific dependency
pub async fn check_dependency(name: &str) -> DependencyStatus;

/// Check all required dependencies
pub async fn check_all_dependencies() -> Vec<DependencyStatus>;

/// Attempt to install a dependency
pub async fn install_dependency(name: &str) -> Result<(), String>;

pub struct DependencyStatus {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
    pub install_command: Option<String>,
}
```

---

## Viewer Enhancements

**Crate**: `viewers`

### Zoomable Trait

```rust
/// Trait for viewers that support zoom/scroll
pub trait Zoomable {
    /// Set zoom level (0.25 to 4.0)
    fn set_zoom(&mut self, zoom: f32);

    /// Get current zoom level
    fn zoom(&self) -> f32;

    /// Reset to default zoom
    fn reset_zoom(&mut self) {
        self.set_zoom(1.0);
    }

    /// Zoom in by step
    fn zoom_in(&mut self) {
        self.set_zoom((self.zoom() * 1.2).min(4.0));
    }

    /// Zoom out by step
    fn zoom_out(&mut self) {
        self.set_zoom((self.zoom() / 1.2).max(0.25));
    }
}
```

### Enhanced Viewer Trait

```rust
pub trait Viewer: Zoomable {
    fn load(&mut self, path: &Path) -> Result<()>;
    fn ui(&mut self, ui: &mut egui::Ui);
    fn path(&self) -> Option<&Path>;
    fn is_loaded(&self) -> bool;

    /// Handle keyboard shortcuts
    fn handle_input(&mut self, ui: &egui::Ui) {
        if ui.input(|i| i.modifiers.ctrl) {
            let scroll = ui.input(|i| i.scroll_delta.y);
            if scroll > 0.0 {
                self.zoom_in();
            } else if scroll < 0.0 {
                self.zoom_out();
            }
        }
    }
}
```

---

## Error Handling

All APIs use `anyhow::Result` for error handling. Key error types:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Failed to load web preview: {0}")]
    WebPreviewFailed(String),

    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("Screenshot capture failed: {0}")]
    ScreenshotFailed(String),
}
```
