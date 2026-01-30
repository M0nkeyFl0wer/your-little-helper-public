//! Preview Panel component for the Interactive Preview Companion feature.
//!
//! This module provides the PreviewPanel struct which manages the display of
//! contextual preview content including files, web pages, images, and ASCII art.

use agent_host::get_mode_introduction;
use anyhow::Result;
use shared::preview_types::{AsciiState, FileType, ImageSource, PreviewContent, SearchResultItem};
use std::path::{Path, PathBuf};
use viewers::{
    csv_viewer::CsvViewer, html_viewer::HtmlViewer, image_viewer::ImageViewer,
    json_viewer::JsonViewer, pdf_viewer::PdfViewer, text_viewer::TextViewer,
};

use crate::ascii_art::{get_ascii_art, get_mode_art};

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
    /// Example prompt that was clicked (to populate chat input)
    pub clicked_prompt: Option<String>,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            visible: true,
            content: None,
            zoom: 1.0,
            scroll_offset: egui::Vec2::ZERO,
            fullscreen: false,
            clicked_prompt: None,
        }
    }
}

/// The preview panel component
pub struct PreviewPanel {
    state: PreviewState,
    file_viewer: Option<FileViewer>,
    file_error: Option<String>,
    current_file_path: Option<PathBuf>,
}

impl PreviewPanel {
    /// Create a new preview panel
    pub fn new() -> Self {
        Self {
            state: PreviewState::default(),
            file_viewer: None,
            file_error: None,
            current_file_path: None,
        }
    }

    /// Show content in the preview panel
    /// Automatically makes panel visible if hidden
    pub fn show_content(&mut self, content: PreviewContent) {
        if !matches!(content, PreviewContent::File { .. }) {
            self.file_viewer = None;
            self.file_error = None;
            self.current_file_path = None;
        }
        self.state.content = Some(content);
        self.state.visible = true;
        // Reset zoom/scroll when showing new content
        self.state.zoom = 1.0;
        self.state.scroll_offset = egui::Vec2::ZERO;

        if let Some(PreviewContent::File { path, .. }) = &self.state.content {
            self.current_file_path = Some(path.clone());
        }
    }

    /// Show mode introduction
    pub fn show_mode_intro(&mut self, mode: &str) {
        self.show_content(PreviewContent::ModeIntro {
            mode: mode.to_string(),
        });
    }

    /// Show skills list for a mode
    pub fn show_skills(
        &mut self,
        mode: &str,
        skills: Vec<shared::preview_types::SkillPreviewInfo>,
    ) {
        self.show_content(PreviewContent::SkillsList {
            mode: mode.to_string(),
            skills,
        });
    }

    /// Show ASCII art state
    pub fn show_ascii(&mut self, state: AsciiState) {
        self.show_content(PreviewContent::Ascii { state });
    }

    pub fn show_tip_if_idle(&mut self, title: &str, message: &str) {
        let can_replace = matches!(
            self.state.content,
            None | Some(PreviewContent::ModeIntro { .. })
                | Some(PreviewContent::Ascii { .. })
                | Some(PreviewContent::SkillsList { .. })
        );

        if can_replace {
            self.show_content(PreviewContent::Tip {
                title: title.to_string(),
                message: message.to_string(),
            });
        }
    }

    /// Show a web preview (with optional metadata)
    pub fn show_web_preview(&mut self, url: &str, title: Option<String>, snippet: Option<String>) {
        self.show_content(PreviewContent::Web {
            url: url.to_string(),
            title,
            screenshot: None,
            og_image: None,
            snippet,
        });
    }

    /// Show an error state
    pub fn show_error(&mut self, message: &str, source: &str) {
        self.show_content(PreviewContent::Error {
            message: message.to_string(),
            source: source.to_string(),
        });
    }

    /// Show search results from fuzzy file finder
    pub fn show_search_results(
        &mut self,
        query: &str,
        results: Vec<SearchResultItem>,
        total_count: usize,
        search_time_ms: u64,
    ) {
        self.show_content(PreviewContent::SearchResults {
            query: query.to_string(),
            results,
            total_count,
            search_time_ms,
        });
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

    /// Take the clicked prompt (if any) and clear it
    /// Call this after ui() to check if user clicked an example prompt
    pub fn take_clicked_prompt(&mut self) -> Option<String> {
        self.state.clicked_prompt.take()
    }

    /// Get current content
    pub fn content(&self) -> Option<&PreviewContent> {
        self.state.content.as_ref()
    }

    pub fn current_file_path(&self) -> Option<PathBuf> {
        self.current_file_path.clone()
    }

    pub fn current_web_url(&self) -> Option<String> {
        if let Some(PreviewContent::Web { url, .. }) = &self.state.content {
            Some(url.clone())
        } else {
            None
        }
    }

    pub fn open_file(&mut self, path: &Path, ctx: &egui::Context) {
        let file_type = FileType::from_path(path);
        let path_buf = path.to_path_buf();
        self.state.content = Some(PreviewContent::File {
            path: path_buf.clone(),
            file_type: file_type.clone(),
        });
        self.state.visible = true;
        self.state.zoom = 1.0;
        self.state.scroll_offset = egui::Vec2::ZERO;
        self.current_file_path = Some(path_buf);
        self.file_error = None;
        if let Err(err) = self.prepare_file_viewer(path, &file_type, ctx) {
            self.file_error = Some(err.to_string());
        }
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
                PreviewContent::SearchResults { .. } => {
                    // Search results have their own inline actions per item
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
                    PreviewContent::File { path, .. } => path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("File")
                        .to_string(),
                    PreviewContent::Web { url, title, .. } => {
                        title.clone().unwrap_or_else(|| url.clone())
                    }
                    PreviewContent::ModeIntro { mode } => match mode.to_lowercase().as_str() {
                        "fix" => "Fix Helper".to_string(),
                        "research" => "Research Helper".to_string(),
                        "data" => "Data Helper".to_string(),
                        "content" => "Content Helper".to_string(),
                        "build" => "Build Helper".to_string(),
                        _ => format!("{} Helper", mode),
                    },
                    PreviewContent::Ascii { state } => format!("{}", state),
                    PreviewContent::Image { .. } => "Image".to_string(),
                    PreviewContent::SearchResults {
                        query, total_count, ..
                    } => {
                        format!("Search: \"{}\" ({} results)", query, total_count)
                    }
                    PreviewContent::VersionHistory {
                        file_name,
                        versions,
                        ..
                    } => {
                        format!("Versions: {} ({} versions)", file_name, versions.len())
                    }
                    PreviewContent::Error { message, .. } => format!("Error: {}", message),
                    PreviewContent::Security(_) => "Security Dashboard".to_string(),
                    PreviewContent::SkillsList { mode, .. } => format!("{} Skills", mode),
                    PreviewContent::Tip { title, .. } => title.clone(),
                };
                ui.label(label);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Minimize/Collapse button
                if ui.button("−").clicked() {
                    self.hide();
                }

                // Zoom out button
                if ui.button("⊖").clicked() {
                    self.zoom_out();
                }

                // Zoom level display
                ui.label(format!("{}%", (self.state.zoom * 100.0) as i32));

                // Zoom in button
                if ui.button("⊕").clicked() {
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
        let is_dark_mode = ui.visuals().dark_mode;
        let text_color = if is_dark_mode {
            egui::Color32::from_rgb(200, 200, 200)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        };
        let accent_color = if is_dark_mode {
            egui::Color32::from_rgb(100, 180, 255)
        } else {
            egui::Color32::from_rgb(50, 100, 200)
        };

        match self.state.content.clone() {
            Some(PreviewContent::ModeIntro { mode }) => {
                let intro = get_mode_introduction(&mode);
                let ascii_art = get_mode_art(&mode);

                ui.vertical_centered(|ui| {
                    // ASCII art mascot
                    ui.add(egui::Label::new(
                        egui::RichText::new(ascii_art).monospace().color(text_color),
                    ));

                    ui.add_space(10.0);

                    // Agent greeting
                    ui.heading(
                        egui::RichText::new(format!("Hi, I'm {}!", intro.agent_name))
                            .color(accent_color),
                    );

                    ui.add_space(5.0);
                    ui.label(egui::RichText::new(intro.greeting).italics().size(16.0));

                    ui.add_space(15.0);

                    // Description
                    ui.label(intro.description);

                    ui.add_space(20.0);

                    // Capabilities section
                    ui.heading(egui::RichText::new("What I can help with:").size(14.0));
                    ui.add_space(5.0);

                    for capability in intro.capabilities.iter().take(4) {
                        ui.horizontal(|ui| {
                            ui.colored_label(accent_color, "•");
                            ui.label(*capability);
                        });
                    }

                    ui.add_space(20.0);

                    // Example prompts section - clickable to populate chat input
                    ui.heading(egui::RichText::new("Try asking me:").size(14.0));
                    ui.add_space(5.0);

                    for example in intro.example_prompts.iter().take(3) {
                        let example_text = example.to_string();
                        let response = ui
                            .horizontal(|ui| {
                                ui.colored_label(accent_color, "→");
                                let btn = ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(format!("\"{}\"", example)).italics(),
                                    )
                                    .frame(false),
                                );
                                btn
                            })
                            .inner;

                        if response.clicked() {
                            self.state.clicked_prompt = Some(example_text);
                        }
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                });
            }
            Some(PreviewContent::Ascii { state }) => {
                let ascii_art = get_ascii_art(state);
                ui.vertical_centered(|ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new(ascii_art).monospace().color(text_color),
                    ));
                });
            }
            Some(PreviewContent::Tip { title, message }) => {
                let card_fill = if is_dark_mode {
                    egui::Color32::from_rgb(45, 45, 55)
                } else {
                    egui::Color32::from_rgb(245, 245, 250)
                };

                ui.vertical_centered(|ui| {
                    egui::Frame::none()
                        .fill(card_fill)
                        .rounding(egui::Rounding::same(10.0))
                        .inner_margin(egui::Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.heading(egui::RichText::new(title).color(accent_color));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(message).color(text_color));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "Tip: click the ⚡ model name above to switch models.",
                                )
                                .size(11.0)
                                .color(text_color),
                            );
                        });
                });
            }
            Some(PreviewContent::File { path, file_type }) => {
                self.render_file(&path, &file_type, ui);
            }
            Some(PreviewContent::Web {
                ref url,
                ref title,
                ref snippet,
                ref og_image,
                ref screenshot,
            }) => {
                let has_screenshot = screenshot.as_ref().map_or(false, |p| p.exists());

                ui.vertical(|ui| {
                    // Web preview header
                    ui.horizontal(|ui| {
                        ui.colored_label(accent_color, "🌐");
                        ui.label(egui::RichText::new("Web Preview").strong().size(14.0));
                    });

                    ui.add_space(8.0);

                    // Title (if available)
                    if let Some(title) = title {
                        if title == "Loading..." {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(egui::RichText::new("Fetching preview...").weak());
                            });
                        } else {
                            ui.heading(egui::RichText::new(title).color(text_color));
                        }
                        ui.add_space(4.0);
                    }

                    // URL as clickable link
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Source:").small().weak());
                        ui.hyperlink(url);
                    });

                    ui.add_space(12.0);

                    // Screenshot (if available)
                    if let Some(screenshot_path) = screenshot {
                        if screenshot_path.exists() {
                            // Render screenshot as image preview
                            self.render_file(screenshot_path, &FileType::Image, ui);
                            ui.add_space(12.0);
                        }
                    }

                    // Description/snippet
                    if let Some(snippet) = snippet {
                        egui::Frame::none()
                            .fill(if is_dark_mode {
                                egui::Color32::from_rgb(40, 40, 48)
                            } else {
                                egui::Color32::from_rgb(248, 248, 250)
                            })
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(12.0))
                            .show(ui, |ui| {
                                ui.label(snippet);
                            });
                    }

                    // OG image URL hint (if available but not loaded as screenshot)
                    if !has_screenshot {
                        if let Some(og_url) = og_image {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Preview image:").small().weak());
                                ui.hyperlink(og_url);
                            });
                        }
                    }

                    ui.add_space(16.0);

                    // Action button to open in browser
                    if ui.button("🔗 Open in Browser").clicked() {
                        let _ = open::that(url);
                    }
                });
            }
            Some(PreviewContent::Image { source }) => match source {
                ImageSource::File(path) => {
                    self.render_file(&path, &FileType::Image, ui);
                }
                ImageSource::Url(url) => {
                    ui.vertical_centered(|ui| {
                        ui.label("Image from URL:");
                        ui.hyperlink(url);
                    });
                }
                ImageSource::Bytes(_) => {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "Image bytes provided but inline rendering is not supported yet.",
                    );
                }
            },
            Some(PreviewContent::VersionHistory {
                file_path,
                file_name,
                versions,
            }) => {
                ui.vertical(|ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.colored_label(accent_color, "");
                        ui.label(
                            egui::RichText::new(format!("Version History: {}", file_name))
                                .strong()
                                .size(14.0)
                        );
                    });

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} saved version{}",
                            versions.len(),
                            if versions.len() == 1 { "" } else { "s" }
                        ))
                        .small()
                        .weak()
                    );

                    ui.add_space(12.0);

                    if versions.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new("No saved versions")
                                    .color(text_color)
                                    .size(16.0)
                            );
                            ui.add_space(8.0);
                            ui.label("Versions are automatically saved when files are modified through Little Helper.");
                        });
                    } else {
                        // Display versions in reverse order (newest first)
                        for version in versions.iter().rev() {
                            let is_current = version.is_current;

                            let bg_color = if is_dark_mode {
                                egui::Color32::from_rgb(35, 35, 42)
                            } else {
                                egui::Color32::from_rgb(250, 250, 252)
                            };

                            egui::Frame::none()
                                .fill(bg_color)
                                .rounding(egui::Rounding::same(4.0))
                                .inner_margin(egui::Margin::same(10.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Version badge
                                        let badge_color = if is_current {
                                            egui::Color32::from_rgb(100, 180, 100)
                                        } else {
                                            egui::Color32::from_rgb(120, 120, 140)
                                        };

                                        egui::Frame::none()
                                            .fill(badge_color)
                                            .rounding(egui::Rounding::same(3.0))
                                            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    egui::RichText::new(format!("v{}", version.version_number))
                                                        .small()
                                                        .color(egui::Color32::WHITE)
                                                );
                                            });

                                        if is_current {
                                            ui.label(
                                                egui::RichText::new("current")
                                                    .small()
                                                    .color(egui::Color32::from_rgb(100, 180, 100))
                                            );
                                        }

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(
                                                egui::RichText::new(version.formatted_size())
                                                    .small()
                                                    .weak()
                                            );
                                        });
                                    });

                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(&version.description).color(text_color));
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(version.relative_time()).small().weak());
                                    });
                                });

                            ui.add_space(6.0);
                        }
                    }

                    ui.add_space(16.0);

                    // Action buttons
                    ui.horizontal(|ui| {
                    if ui.button("Open File").clicked() {
                        let _ = open::that(&file_path);
                    }
                    if ui.button("Show in Folder").clicked() {
                        if let Some(parent) = file_path.parent() {
                                let _ = open::that(parent);
                            }
                        }
                    });
                });
            }
            Some(PreviewContent::SearchResults {
                query,
                results,
                total_count,
                search_time_ms,
            }) => {
                ui.vertical(|ui| {
                    // Search header
                    ui.horizontal(|ui| {
                        ui.colored_label(accent_color, "🔍");
                        ui.label(
                            egui::RichText::new(format!("Search Results for \"{}\"", query))
                                .strong()
                                .size(14.0),
                        );
                    });

                    ui.add_space(4.0);

                    // Stats line
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} files found in {}ms",
                                total_count, search_time_ms
                            ))
                            .small()
                            .weak(),
                        );
                    });

                    ui.add_space(12.0);

                    if results.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(egui::RichText::new("No files found").weak().size(16.0));
                            ui.add_space(8.0);
                            ui.label("Try a different search term or index more directories.");
                        });
                    } else {
                        // Results list
                        for result in results.iter() {
                            let bg_color = if is_dark_mode {
                                egui::Color32::from_rgb(35, 35, 42)
                            } else {
                                egui::Color32::from_rgb(250, 250, 252)
                            };

                            egui::Frame::none()
                                .fill(bg_color)
                                .rounding(egui::Rounding::same(4.0))
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // File icon based on extension
                                        let icon = get_file_icon(&result.path);
                                        ui.label(egui::RichText::new(icon).size(18.0));

                                        ui.vertical(|ui| {
                                            // File name with score indicator
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&result.name)
                                                        .strong()
                                                        .color(accent_color),
                                                );

                                                // Score bar
                                                let score_color = score_to_color(result.score);
                                                ui.add_space(8.0);
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{:.0}%",
                                                        result.score * 100.0
                                                    ))
                                                    .small()
                                                    .color(score_color),
                                                );
                                            });

                                            // Parent directory
                                            ui.label(
                                                egui::RichText::new(&result.parent).small().weak(),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                // File size
                                                ui.label(
                                                    egui::RichText::new(format_file_size(
                                                        result.size,
                                                    ))
                                                    .small()
                                                    .weak(),
                                                );

                                                // Action buttons
                                                let path_clone = result.path.clone();
                                                if ui
                                                    .small_button("📂")
                                                    .on_hover_text("Reveal in folder")
                                                    .clicked()
                                                {
                                                    if let Some(parent) = path_clone.parent() {
                                                        let _ = open::that(parent);
                                                    }
                                                }

                                                let path_clone = result.path.clone();
                                                if ui
                                                    .small_button("📄")
                                                    .on_hover_text("Open file")
                                                    .clicked()
                                                {
                                                    let _ = open::that(&path_clone);
                                                }

                                                let path_clone = result.path.clone();
                                                if ui
                                                    .small_button("👁")
                                                    .on_hover_text("Preview in side panel")
                                                    .clicked()
                                                {
                                                    self.open_file(path_clone.as_path(), ui.ctx());
                                                    return;
                                                }
                                            },
                                        );
                                    });
                                });

                            ui.add_space(4.0);
                        }

                        // Show count if more results exist
                        if total_count > results.len() {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Showing {} of {} results",
                                    results.len(),
                                    total_count
                                ))
                                .small()
                                .weak(),
                            );
                        }
                    }
                });
            }
            Some(PreviewContent::Error { message, source }) => {
                ui.vertical_centered(|ui| {
                    ui.colored_label(egui::Color32::RED, "Error");
                    ui.label(message);
                    ui.small(format!("Source: {}", source));
                });
            }
            Some(PreviewContent::Security(_)) => {
                // Security dashboard - placeholder for now
                ui.vertical_centered(|ui| {
                    ui.heading("Security Dashboard");
                    ui.label("Security features coming soon!");
                });
            }
            Some(PreviewContent::SkillsList { mode, skills }) => {
                ui.vertical(|ui| {
                    ui.heading(format!("Available Skills - {} Mode", mode));
                    ui.add_space(16.0);

                    if skills.is_empty() {
                        ui.label("No skills available for this mode.");
                    } else {
                        for skill in skills {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(accent_color, "•");
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(&skill.name).strong().size(14.0));
                                    if skill.requires_approval {
                                        ui.label(
                                            egui::RichText::new("(requires approval)")
                                                .small()
                                                .color(egui::Color32::YELLOW),
                                        );
                                    }
                                });
                                ui.add_space(4.0);
                                ui.label(&skill.description);
                                ui.add_space(8.0);
                            });
                        }
                    }
                });
            }
            None => {
                // Show welcome ASCII art when no content
                let welcome_art = get_ascii_art(AsciiState::Welcome);
                ui.vertical_centered(|ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new(welcome_art)
                            .monospace()
                            .color(text_color),
                    ));
                    ui.add_space(10.0);
                    ui.label("Select a mode to get started!");
                });
            }
        }
    }

    fn render_file(&mut self, path: &Path, file_type: &FileType, ui: &mut egui::Ui) {
        let needs_reload = self
            .current_file_path
            .as_ref()
            .map(|existing| existing != path)
            .unwrap_or(true);

        if needs_reload {
            self.current_file_path = Some(path.to_path_buf());
            self.file_error = None;
            if let Err(err) = self.prepare_file_viewer(path, file_type, ui.ctx()) {
                self.file_error = Some(err.to_string());
            }
        }

        if let Some(err) = &self.file_error {
            ui.colored_label(egui::Color32::RED, err);
        } else if let Some(viewer) = &mut self.file_viewer {
            viewer.ui(ui);
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Loading preview…");
            });
        }
    }

    fn prepare_file_viewer(
        &mut self,
        path: &Path,
        file_type: &FileType,
        ctx: &egui::Context,
    ) -> Result<()> {
        let viewer = match file_type {
            FileType::Image => {
                let mut v = ImageViewer::new();
                v.load(path, ctx)?;
                FileViewer::Image(v)
            }
            FileType::Csv => {
                let mut v = CsvViewer::new();
                v.load(path)?;
                FileViewer::Csv(v)
            }
            FileType::Json => {
                let mut v = JsonViewer::new();
                v.load(path)?;
                FileViewer::Json(v)
            }
            FileType::Html => {
                let mut v = HtmlViewer::new();
                v.load(path)?;
                FileViewer::Html(v)
            }
            FileType::Pdf => {
                let mut v = PdfViewer::new();
                v.load(path)?;
                FileViewer::Pdf(v)
            }
            FileType::Text | FileType::Markdown | FileType::Unknown => {
                let mut v = TextViewer::new();
                v.load(path)?;
                FileViewer::Text(v)
            }
        };
        self.file_viewer = Some(viewer);
        Ok(())
    }
}

impl Default for PreviewPanel {
    fn default() -> Self {
        Self::new()
    }
}

enum FileViewer {
    Text(TextViewer),
    Image(ImageViewer),
    Csv(CsvViewer),
    Json(JsonViewer),
    Html(HtmlViewer),
    Pdf(PdfViewer),
}

impl FileViewer {
    fn ui(&mut self, ui: &mut egui::Ui) {
        match self {
            FileViewer::Text(viewer) => viewer.ui(ui),
            FileViewer::Image(viewer) => viewer.ui(ui),
            FileViewer::Csv(viewer) => viewer.ui(ui),
            FileViewer::Json(viewer) => viewer.ui(ui),
            FileViewer::Html(viewer) => viewer.ui(ui),
            FileViewer::Pdf(viewer) => viewer.ui(ui),
        }
    }
}

/// Get a file icon emoji based on extension
fn get_file_icon(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        // Documents
        Some("pdf") => "📕",
        Some("doc" | "docx") => "📘",
        Some("xls" | "xlsx" | "csv") => "📊",
        Some("ppt" | "pptx") => "📙",
        Some("txt" | "md" | "markdown") => "📄",
        // Code
        Some("rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "java") => "💻",
        Some("html" | "css" | "scss" | "sass") => "🌐",
        Some("json" | "yaml" | "yml" | "toml" | "xml") => "⚙️",
        Some("sh" | "bash" | "zsh" | "fish") => "🐚",
        // Images
        Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg") => "🖼️",
        // Audio/Video
        Some("mp3" | "wav" | "ogg" | "flac" | "m4a") => "🎵",
        Some("mp4" | "avi" | "mkv" | "mov" | "webm") => "🎬",
        // Archives
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar") => "📦",
        // Executables
        Some("exe" | "msi" | "dmg" | "app") => "⚡",
        // Directories handled elsewhere
        _ => "📄",
    }
}

/// Convert a score (0.0-1.0) to a color
fn score_to_color(score: f32) -> egui::Color32 {
    if score >= 0.8 {
        egui::Color32::from_rgb(100, 200, 100) // Green - excellent match
    } else if score >= 0.6 {
        egui::Color32::from_rgb(180, 200, 100) // Yellow-green - good match
    } else if score >= 0.4 {
        egui::Color32::from_rgb(200, 180, 100) // Yellow - fair match
    } else {
        egui::Color32::from_rgb(180, 140, 100) // Orange - weak match
    }
}

/// Format file size in human-readable form
fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
