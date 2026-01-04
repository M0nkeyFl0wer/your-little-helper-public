//! Preview Panel component for the Interactive Preview Companion feature.
//!
//! This module provides the PreviewPanel struct which manages the display of
//! contextual preview content including files, web pages, images, and ASCII art.

use shared::preview_types::{AsciiState, PreviewContent};

/// Actions that can be performed on preview content
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Current state of the preview panel (runtime only, not persisted)
#[derive(Clone)]
pub struct PreviewState {
    /// Whether the panel is visible
    pub visible: bool,
    /// Current content being displayed
    pub content: Option<PreviewContent>,
    /// Zoom level (0.25 to 4.0)
    pub zoom: f32,
    /// Scroll offset for panning
    pub scroll_offset: egui::Vec2,
    /// Whether in fullscreen mode
    pub fullscreen: bool,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            visible: true,
            content: None,
            zoom: 1.0,
            scroll_offset: egui::Vec2::ZERO,
            fullscreen: false,
        }
    }
}

/// The preview panel component
pub struct PreviewPanel {
    state: PreviewState,
}

impl PreviewPanel {
    /// Create a new preview panel
    pub fn new() -> Self {
        Self {
            state: PreviewState::default(),
        }
    }

    /// Show content in the preview panel
    /// Automatically makes panel visible if hidden
    pub fn show_content(&mut self, content: PreviewContent) {
        self.state.content = Some(content);
        self.state.visible = true;
        // Reset zoom/scroll when showing new content
        self.state.zoom = 1.0;
        self.state.scroll_offset = egui::Vec2::ZERO;
    }

    /// Show mode introduction
    pub fn show_mode_intro(&mut self, mode: &str) {
        self.show_content(PreviewContent::ModeIntro {
            mode: mode.to_string(),
        });
    }

    /// Show ASCII art state
    pub fn show_ascii(&mut self, state: AsciiState) {
        self.show_content(PreviewContent::Ascii { state });
    }

    /// Hide the preview panel
    pub fn hide(&mut self) {
        self.state.visible = false;
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        self.state.visible = !self.state.visible;
    }

    /// Set zoom level (clamped to 0.25-4.0)
    pub fn set_zoom(&mut self, zoom: f32) {
        self.state.zoom = zoom.clamp(0.25, 4.0);
    }

    /// Get current zoom level
    pub fn zoom(&self) -> f32 {
        self.state.zoom
    }

    /// Zoom in by a step
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.state.zoom * 1.2);
    }

    /// Zoom out by a step
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.state.zoom / 1.2);
    }

    /// Reset zoom to 100%
    pub fn reset_zoom(&mut self) {
        self.set_zoom(1.0);
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&mut self) {
        self.state.fullscreen = !self.state.fullscreen;
    }

    /// Check if panel is visible
    pub fn is_visible(&self) -> bool {
        self.state.visible
    }

    /// Check if in fullscreen mode
    pub fn is_fullscreen(&self) -> bool {
        self.state.fullscreen
    }

    /// Get current content
    pub fn content(&self) -> Option<&PreviewContent> {
        self.state.content.as_ref()
    }

    /// Get actions available for current content
    pub fn available_actions(&self) -> Vec<PreviewAction> {
        let mut actions = vec![
            PreviewAction::ZoomIn,
            PreviewAction::ZoomOut,
            PreviewAction::ZoomReset,
            PreviewAction::Fullscreen,
            PreviewAction::Close,
        ];

        if let Some(content) = &self.state.content {
            match content {
                PreviewContent::File { .. } => {
                    actions.insert(0, PreviewAction::OpenInApp);
                    actions.insert(1, PreviewAction::RevealInFolder);
                    actions.insert(2, PreviewAction::CopyPath);
                }
                PreviewContent::Web { .. } => {
                    actions.insert(0, PreviewAction::OpenInBrowser);
                    actions.insert(1, PreviewAction::CopyUrl);
                }
                PreviewContent::Image { .. } => {
                    actions.insert(0, PreviewAction::OpenInApp);
                }
                _ => {}
            }
        }

        actions
    }

    /// Execute an action on current content
    pub fn execute_action(&mut self, action: PreviewAction) -> anyhow::Result<()> {
        match action {
            PreviewAction::OpenInApp => {
                if let Some(PreviewContent::File { path, .. }) = &self.state.content {
                    open::that(path)?;
                }
            }
            PreviewAction::RevealInFolder => {
                if let Some(PreviewContent::File { path, .. }) = &self.state.content {
                    if let Some(parent) = path.parent() {
                        open::that(parent)?;
                    }
                }
            }
            PreviewAction::OpenInBrowser => {
                if let Some(PreviewContent::Web { url, .. }) = &self.state.content {
                    open::that(url)?;
                }
            }
            PreviewAction::CopyPath => {
                // TODO: Implement clipboard copy
            }
            PreviewAction::CopyUrl => {
                // TODO: Implement clipboard copy
            }
            PreviewAction::Close => {
                self.hide();
            }
            PreviewAction::ZoomIn => {
                self.zoom_in();
            }
            PreviewAction::ZoomOut => {
                self.zoom_out();
            }
            PreviewAction::ZoomReset => {
                self.reset_zoom();
            }
            PreviewAction::Fullscreen => {
                self.toggle_fullscreen();
            }
        }
        Ok(())
    }

    /// Render the preview panel UI
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if !self.state.visible {
            return;
        }

        // Handle Ctrl+scroll for zoom
        if ui.rect_contains_pointer(ui.max_rect()) && ui.input(|i| i.modifiers.ctrl) {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll > 0.0 {
                self.zoom_in();
            } else if scroll < 0.0 {
                self.zoom_out();
            }
        }

        // Panel header with controls
        ui.horizontal(|ui| {
            // Content source label
            if let Some(content) = &self.state.content {
                let label = match content {
                    PreviewContent::File { path, .. } => {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("File")
                            .to_string()
                    }
                    PreviewContent::Web { url, title, .. } => {
                        title.clone().unwrap_or_else(|| url.clone())
                    }
                    PreviewContent::ModeIntro { mode } => format!("{} Mode", mode),
                    PreviewContent::Ascii { state } => format!("{}", state),
                    PreviewContent::Image { .. } => "Image".to_string(),
                    PreviewContent::Error { message, .. } => format!("Error: {}", message),
                };
                ui.label(label);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Close button
                if ui.button("✕").clicked() {
                    self.hide();
                }

                // Fullscreen button
                let fs_label = if self.state.fullscreen { "⊟" } else { "⊞" };
                if ui.button(fs_label).clicked() {
                    self.toggle_fullscreen();
                }

                // Zoom controls
                if ui.button("−").clicked() {
                    self.zoom_out();
                }
                ui.label(format!("{}%", (self.state.zoom * 100.0) as i32));
                if ui.button("+").clicked() {
                    self.zoom_in();
                }
            });
        });

        ui.separator();

        // Content area with scroll
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.render_content(ui);
            });
    }

    /// Render fullscreen overlay
    pub fn fullscreen_ui(&mut self, ctx: &egui::Context) {
        if !self.state.fullscreen {
            return;
        }

        egui::Window::new("Preview")
            .fixed_rect(ctx.screen_rect())
            .title_bar(false)
            .show(ctx, |ui| {
                // Close/exit fullscreen header
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✕ Exit Fullscreen").clicked() {
                            self.state.fullscreen = false;
                        }
                    });
                });

                ui.separator();

                // Render content
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_content(ui);
                    });
            });

        // Handle Escape key to exit fullscreen
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.state.fullscreen = false;
        }
    }

    /// Render the actual content based on type
    fn render_content(&mut self, ui: &mut egui::Ui) {
        match &self.state.content {
            Some(PreviewContent::ModeIntro { mode }) => {
                ui.vertical_centered(|ui| {
                    ui.heading(format!("Welcome to {} Mode", mode));
                    ui.add_space(20.0);
                    // TODO: Add mode-specific intro content
                    ui.label("Mode introduction will be displayed here.");
                });
            }
            Some(PreviewContent::Ascii { state }) => {
                ui.vertical_centered(|ui| {
                    // TODO: Render actual ASCII art
                    ui.monospace(format!("[{} ASCII Art]", state));
                });
            }
            Some(PreviewContent::File { path, file_type }) => {
                ui.label(format!("File: {:?}", path));
                ui.label(format!("Type: {:?}", file_type));
                // TODO: Use viewers crate to render file content
            }
            Some(PreviewContent::Web { url, title, snippet, .. }) => {
                ui.vertical(|ui| {
                    if let Some(title) = title {
                        ui.heading(title);
                    }
                    ui.hyperlink(url);
                    if let Some(snippet) = snippet {
                        ui.add_space(10.0);
                        ui.label(snippet);
                    }
                    // TODO: Show screenshot or OG image
                });
            }
            Some(PreviewContent::Image { source }) => {
                ui.label(format!("Image: {:?}", source));
                // TODO: Load and render image
            }
            Some(PreviewContent::Error { message, source }) => {
                ui.vertical_centered(|ui| {
                    ui.colored_label(egui::Color32::RED, "Error");
                    ui.label(message);
                    ui.small(format!("Source: {}", source));
                });
            }
            None => {
                ui.vertical_centered(|ui| {
                    ui.label("No content to display");
                    // TODO: Show welcome ASCII art
                });
            }
        }
    }
}

impl Default for PreviewPanel {
    fn default() -> Self {
        Self::new()
    }
}
