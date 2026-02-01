use agent_host::CommandResult;
use eframe::egui;
use parking_lot::Mutex;
use shared::settings::AppSettings;
use shared::preview_types::PreviewContent;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;


// Default mascot image (boss's dog!)
pub(crate) const DEFAULT_MASCOT: &[u8] = include_bytes!("../assets/default_mascot.png");

// Pre-loaded secrets (gitignored secrets.rs, or empty for CI builds)
mod secrets;
use secrets::OPENAI_API_KEY;

// Support contact info (gitignored - your personal contact stays private)
mod support_info;
use support_info::{SUPPORT_BUTTON_TEXT, SUPPORT_LINK};

// Interactive Preview Companion modules
mod ascii_art;
mod modals;
mod onboarding;
mod preview_panel;
mod thread_history;

// Campaign context loader
mod context;

// Types module - core type definitions
mod types;
pub use types::*;

// Utils module - helper functions
mod utils;

mod state;
pub use state::*;

/// Extract file paths from text
fn extract_paths(text: &str, allowed_dirs: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Match absolute paths like /home/user/file.txt or ~/file.txt
    // Match paths like /home/user/file.txt or ~/file.txt
    let path_regex = regex::Regex::new(r#"(?:^|[\s"'(])([~/][^\s"'()]+\.[a-zA-Z0-9]+)"#).unwrap();

    for cap in path_regex.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let path_str = m.as_str();
            // Expand ~ to home directory
            let expanded = expand_user_path(path_str);

            if expanded.exists() && is_path_in_allowed_dirs(&expanded, allowed_dirs) {
                paths.push(expanded);
            }
        }
    }

    paths
}

fn expand_user_path(path_str: &str) -> PathBuf {
    if let Some(stripped) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path_str)
}

fn is_path_in_allowed_dirs(path: &Path, allowed_dirs: &[String]) -> bool {
    if allowed_dirs.is_empty() {
        return false;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    allowed_dirs.iter().any(|allowed| {
        let expanded = expand_user_path(allowed);
        let allow_canon = expanded.canonicalize().unwrap_or(expanded);
        canonical.starts_with(&allow_canon)
    })
}

fn run_user_command(command: &str) -> Result<CommandResult, String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime
        .block_on(agent_host::execute_command(command, 60))
        .map_err(|e| e.to_string())
}

fn preload_openai_enabled() -> bool {
    match std::env::var("LH_DISABLE_PRELOAD_OPENAI") {
        Ok(val) => {
            let v = val.trim().to_ascii_lowercase();
            !(v == "1" || v == "true" || v == "yes")
        }
        Err(_) => true,
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    if let Some(proj) = directories::ProjectDirs::from("com.local", "Little Helper", "LittleHelper")
    {
        let p = proj.config_dir().join("settings.json");
        let _ = fs::create_dir_all(proj.config_dir());
        Some(p)
    } else {
        None
    }
}

fn load_settings_or_default() -> (AppSettings, bool) {
    if let Some(path) = config_path() {
        if path.exists() {
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(s) = serde_json::from_slice::<AppSettings>(&bytes) {
                    let mut settings = s;
                    ensure_allowed_dirs(&mut settings);
                    return (settings, false);
                }
            }
        }
    }
    // Fresh install - honor app defaults, optionally seed OpenAI key for bespoke builds
    let mut default_settings = AppSettings::default();
    ensure_allowed_dirs(&mut default_settings);
    if preload_openai_enabled() && !OPENAI_API_KEY.is_empty() {
        default_settings.model.openai_auth.api_key = Some(OPENAI_API_KEY.to_string());
    }
    (default_settings, true)
}

/// Clean up AI response by removing action tags
fn clean_ai_response(response: &str) -> String {
    // Remove <preview>, <search>, <command> tags and their content
    let re_preview = regex::Regex::new(r"(?s)<preview[^>]*>.*?</preview>").unwrap();
    let re_search = regex::Regex::new(r"(?s)<search>.*?</search>").unwrap();
    let re_command = regex::Regex::new(r"(?s)<command>.*?</command>").unwrap();

    let cleaned = re_preview.replace_all(response, "");
    let cleaned = re_search.replace_all(&cleaned, "");
    let cleaned = re_command.replace_all(&cleaned, "");

    // Clean up extra whitespace
    cleaned.trim().to_string()
}

/// Format error message with helpful troubleshooting info
fn format_error_message(error: &str) -> String {
    let error_lower = error.to_lowercase();

    // API key issues
    if error_lower.contains("unauthorized")
        || error_lower.contains("401")
        || error_lower.contains("invalid api key")
    {
        return format!(
            "I couldn't connect to the AI service - there may be an issue with the API key.\n\n\
            Error: {}\n\n\
            If this keeps happening, please let the team know!",
            error
        );
    }

    // Rate limiting
    if error_lower.contains("rate limit")
        || error_lower.contains("429")
        || error_lower.contains("too many requests")
    {
        return format!(
            "The AI service is temporarily busy. Please wait a moment and try again.\n\n\
            Error: {}",
            error
        );
    }

    // Network issues
    if error_lower.contains("connection")
        || error_lower.contains("network")
        || error_lower.contains("timeout")
        || error_lower.contains("dns")
        || error_lower.contains("could not resolve")
    {
        return format!(
            "I'm having trouble connecting to the internet. Please check your network connection.\n\n\
            Error: {}",
            error
        );
    }

    // Quota/billing issues
    if error_lower.contains("quota")
        || error_lower.contains("billing")
        || error_lower.contains("insufficient")
    {
        return format!(
            "The AI service quota may have been exceeded. Please let the team know!\n\n\
            Error: {}",
            error
        );
    }

    // Generic error
    format!(
        "Sorry, I ran into an issue. Here's what happened:\n\n{}\n\n\
        If this keeps happening, try restarting the app or checking your internet connection.",
        error
    )
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        vsync: true, // Limit to monitor refresh rate
        ..Default::default()
    };
    eframe::run_native(
        "Little Helper",
        options,
        Box::new(|_cc| {
            Box::new(LittleHelperApp {
                state: Arc::new(Mutex::new(AppState::default())),
            })
        }),
    )
}

struct LittleHelperApp {
    state: Arc<Mutex<AppState>>,
}

impl eframe::App for LittleHelperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut s = self.state.lock();

        // Poll for AI response (non-blocking)
        s.poll_ai_response();
        s.poll_command_result();
        s.poll_web_preview();

        // Request repaint if we're waiting for AI or web preview
        if s.web_preview_rx.is_some() {
            ctx.request_repaint();
        }

        // Request repaint if any mode is waiting for AI (to keep polling)
        let any_thinking = s.is_thinking.values().any(|&v| v);
        if any_thinking {
            ctx.request_repaint();
        }

        // Detect mode change and show mode introduction
        let mode_changed = s.previous_mode.map_or(false, |prev| prev != s.current_mode);
        if mode_changed {
            // Save current input text for the old mode
            if let Some(prev_mode) = s.previous_mode {
                if !s.input_text.is_empty() {
                    let draft = s.input_text.clone();
                    s.mode_input_drafts.insert(prev_mode, draft);
                }
            }

            // Restore input text for the new mode (or clear it)
            let new_mode = s.current_mode;
            // Mark this mode as "read"
            s.unread_modes.remove(&new_mode);
            s.input_text = s
                .mode_input_drafts
                .get(&new_mode)
                .cloned()
                .unwrap_or_default();

            let mode_str = s.current_mode.as_str();
            s.preview_panel.show_mode_intro(mode_str);
            
            // Load context documents and skills for the new mode
            let shared_mode = match s.current_mode {
                ChatMode::Find => shared::skill::Mode::Find,
                ChatMode::Fix => shared::skill::Mode::Fix,
                ChatMode::Research => shared::skill::Mode::Research,
                ChatMode::Data => shared::skill::Mode::Data,
                ChatMode::Content => shared::skill::Mode::Content,
                ChatMode::Build => shared::skill::Mode::Build,
            };
            
            // Get available skills for this mode and show them in preview
            let skills_info = s.skill_registry.skills_info_for_mode(shared_mode);
            let skill_previews: Vec<shared::preview_types::SkillPreviewInfo> = skills_info
                .into_iter()
                .map(|info| shared::preview_types::SkillPreviewInfo {
                    id: info.id.to_string(),
                    name: info.name.to_string(),
                    description: info.description.to_string(),
                    permission_level: format!("{:?}", info.permission_level),
                    requires_approval: info.user_permission == shared::skill::Permission::Ask,
                })
                .collect();
            
            if !skill_previews.is_empty() {
                s.preview_panel.show_skills(mode_str, skill_previews);
            }
        }
        s.previous_mode = Some(s.current_mode);

        // Set up theme (dark or light mode) with accessibility enhancements
        let mut style = (*ctx.style()).clone();
        style.visuals.window_rounding = egui::Rounding::same(12.0);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);

        if s.settings.user_profile.dark_mode {
            style.visuals = egui::Visuals::dark();
            style.visuals.panel_fill = egui::Color32::from_rgb(30, 30, 35);
            // Enhanced focus states for accessibility (T502)
            style.visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 255));
            style.visuals.selection.stroke =
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 255));
        } else {
            style.visuals.panel_fill = egui::Color32::from_rgb(250, 250, 252);
            // Enhanced focus states for accessibility (T502)
            style.visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(2.0, egui::Color32::from_rgb(50, 100, 200));
            style.visuals.selection.stroke =
                egui::Stroke::new(2.0, egui::Color32::from_rgb(50, 100, 200));
        }
        ctx.set_style(style);

        // Route to appropriate screen
        match s.current_screen {
            AppScreen::Onboarding => {
                render_onboarding_screen(&mut s, ctx);
                return;
            }
            AppScreen::Chat => {
                // Load mascot texture if not already loaded
                s.load_mascot_texture(ctx);
            }
        }

        let dark = s.settings.user_profile.dark_mode;

        if let Some(mode) = s.thinking_mode {
            if let Some(started_at) = s.thinking_started_at.get(&mode) {
                let shown = s.slow_response_hint_shown.get(&mode).copied().unwrap_or(false);
                if !shown && started_at.elapsed() >= Duration::from_secs(20) {
                    s.slow_response_hint_shown.insert(mode, true);
                    s.show_model_hint = true;
                    s.model_hint_started_at = Some(std::time::Instant::now());

                    let tip_message =
                        "This is taking longer than usual. Cloud models often respond faster.";
                    s.preview_panel
                        .show_tip_if_idle("Want faster replies?", tip_message);
                }
            }
        }

        if s.show_model_hint {
            if let Some(started) = s.model_hint_started_at {
                if started.elapsed() >= Duration::from_secs(10) {
                    s.show_model_hint = false;
                }
            }
        }

        // Top header with mode buttons
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::none().fill(if dark {
                egui::Color32::from_rgb(35, 35, 42)
            } else {
                egui::Color32::from_rgb(245, 247, 250)
            }))
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.heading(
                        egui::RichText::new("Little Helper")
                            .size(24.0)
                            .color(if dark {
                                egui::Color32::from_rgb(220, 220, 230)
                            } else {
                                egui::Color32::from_rgb(60, 60, 80)
                            }),
                    );

                    ui.add_space(32.0);

                    // Mode buttons - check processing states first to avoid borrow issues
                    let find_processing = s.is_thinking.get(&ChatMode::Find).copied().unwrap_or(false);
                    let fix_processing = s.is_thinking.get(&ChatMode::Fix).copied().unwrap_or(false);
                    let research_processing = s.is_thinking.get(&ChatMode::Research).copied().unwrap_or(false);
                    let _data_processing = s.is_thinking.get(&ChatMode::Data).copied().unwrap_or(false);
                    let _content_processing = s.is_thinking.get(&ChatMode::Content).copied().unwrap_or(false);
                    let build_processing = s.is_thinking.get(&ChatMode::Build).copied().unwrap_or(false);
                    
                    let find_unread = s.unread_modes.contains(&ChatMode::Find);
                    let fix_unread = s.unread_modes.contains(&ChatMode::Fix);
                    let research_unread = s.unread_modes.contains(&ChatMode::Research);
                    let build_unread = s.unread_modes.contains(&ChatMode::Build);

                    mode_button(
                        ui,
                        "Find",
                        ChatMode::Find,
                        &mut s.current_mode,
                        find_processing,
                        find_unread,
                    );
                    mode_button(
                        ui,
                        "Fix",
                        ChatMode::Fix,
                        &mut s.current_mode,
                        fix_processing,
                        fix_unread,
                    );
                    mode_button(
                        ui,
                        "Research",
                        ChatMode::Research,
                        &mut s.current_mode,
                        research_processing,
                        research_unread,
                    );
                    mode_button(
                        ui,
                        "Build",
                        ChatMode::Build,
                        &mut s.current_mode,
                        build_processing,
                        build_unread,
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);

                        // Dark mode toggle
                        let dark_icon = if s.settings.user_profile.dark_mode {
                            "☀" // Sun icon - click to switch to light
                        } else {
                            "🌙" // Moon icon - click to switch to dark
                        };
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(dark_icon).size(18.0))
                                    .frame(false),
                            )
                            .on_hover_text(if s.settings.user_profile.dark_mode {
                                "Switch to light mode"
                            } else {
                                "Switch to dark mode"
                            })
                            .clicked()
                        {
                            s.settings.user_profile.dark_mode = !s.settings.user_profile.dark_mode;
                            save_settings(&s.settings);
                        }

                        ui.add_space(12.0);

                        // Support button - links to Signal
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("💬 {}", SUPPORT_BUTTON_TEXT))
                                        .size(12.0),
                                )
                                .fill(egui::Color32::from_rgb(60, 130, 180))
                                .rounding(egui::Rounding::same(4.0)),
                            )
                            .on_hover_text("Get help or send feedback")
                            .clicked()
                        {
                            // Open Signal link in browser
                            let _ = open::that(SUPPORT_LINK);
                        }

                        ui.add_space(12.0);

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Settings")
                                        .size(12.0)
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(90, 90, 140))
                                .rounding(egui::Rounding::same(4.0)),
                            )
                            .on_hover_text("Configure privacy and allowed folders")
                            .clicked()
                        {
                            s.show_settings_dialog = true;
                        }

                        ui.add_space(12.0);

                        // Model indicator - clone provider string to avoid borrow issues
                        let provider_str: String = s
                            .settings
                            .model
                            .provider_preference
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "none".to_string());
                        let provider = provider_str.as_str();
                        let model_name: String = match provider {
                            "openai" => s.settings.model.openai_model.clone(),
                            "anthropic" => s.settings.model.anthropic_model.clone(),
                            "gemini" => s.settings.model.gemini_model.clone(),
                            "local" => s.settings.model.local_model.clone(),
                            _ => "unknown".to_string(),
                        };
                        let show_hint = s.show_model_hint
                            && s
                                .model_hint_started_at
                                .map(|t| t.elapsed() < Duration::from_secs(10))
                                .unwrap_or(false);
                        let blink = ((ui.input(|i| i.time) * 2.0) as i32) % 2 == 0;

                        // Clickable model indicator
                        ui.vertical(|ui| {
                            let model_btn = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(format!("⚡ {}", model_name))
                                        .size(11.0)
                                        .color(if dark {
                                            egui::Color32::from_rgb(140, 180, 140)
                                        } else {
                                            egui::Color32::from_rgb(80, 130, 80)
                                        }),
                                )
                                .frame(false),
                            );
                            if model_btn.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if model_btn.clicked() {
                                s.show_settings_dialog = true;
                            }
                            model_btn
                                .on_hover_text(format!("Provider: {} (click to change)", provider));

                            if show_hint && blink {
                                ui.label(
                                    egui::RichText::new("v")
                                        .size(12.0)
                                        .color(if dark {
                                            egui::Color32::from_rgb(120, 180, 255)
                                        } else {
                                            egui::Color32::from_rgb(50, 100, 200)
                                        }),
                                );
                            }
                        });

                        ui.add_space(8.0);

                        // Preview panel toggle
                        if s.show_preview {
                            if ui.button("Hide Preview").clicked() {
                                s.close_preview();
                            }
                        } else {
                            if ui
                                .button("Show Preview")
                                .on_hover_text("Show the preview panel")
                                .clicked()
                            {
                                s.show_preview = true;
                                // Show mode intro if no other content
                                if matches!(s.active_viewer, ActiveViewer::Panel) {
                                    let mode_str = s.current_mode.as_str();
                                    s.preview_panel.show_mode_intro(mode_str);
                                }
                            }
                        }
                    });
                });
                ui.add_space(12.0);
            });

        // Preview panel (right side)
        if s.show_preview {
            egui::SidePanel::right("preview")
                .default_width(500.0)
                .min_width(300.0)
                .frame(
                    egui::Frame::none()
                        .fill(if dark {
                            egui::Color32::from_rgb(35, 35, 42)
                        } else {
                            egui::Color32::from_rgb(255, 255, 255)
                        })
                        .inner_margin(egui::Margin::same(12.0)),
                )
                .show(ctx, |ui| {
                    // Header - context-aware
                    ui.horizontal(|ui| {
                        let title = match &s.active_viewer {
                            ActiveViewer::Panel => "Preview Panel".to_string(),
                            ActiveViewer::CommandOutput(cmd, _) => {
                                format!("Output: {}", cmd.chars().take(30).collect::<String>())
                            }
                            ActiveViewer::Matrix => {
                                // Only show "Processing..." if current mode is the one processing
                                if s.thinking_mode == Some(s.current_mode.clone()) {
                                    "Processing...".to_string()
                                } else {
                                    "Preview Panel".to_string()
                                }
                            }
                            ActiveViewer::RickRoll => "Never Gonna Give You Up".to_string(),
                        };

                        ui.label(egui::RichText::new(title).size(16.0).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Only show close button if not the welcome view
                            if !matches!(s.active_viewer, ActiveViewer::Panel) {
                                if ui.small_button("X").clicked() {
                                    let mode_name = s.current_mode.as_str().to_string();
                                    s.active_viewer = ActiveViewer::Panel;
                                    s.preview_panel.show_mode_intro(&mode_name);
                                }
                            }
                        });
                    });

                    // File/web action buttons
                    if let Some(path) = s.preview_panel.current_file_path() {
                        ui.horizontal(|ui| {
                            if ui
                                .small_button("Open in App")
                                .on_hover_text("Open with default application")
                                .clicked()
                            {
                                let _ = open::that(&path);
                            }
                            if ui
                                .small_button("Show in Folder")
                                .on_hover_text("Open containing folder")
                                .clicked()
                            {
                                if let Some(parent) = path.parent() {
                                    let _ = open::that(parent);
                                }
                            }
                            if ui
                                .small_button("Copy Path")
                                .on_hover_text("Copy full path to clipboard")
                                .clicked()
                            {
                                ui.output_mut(|o| o.copied_text = path.display().to_string());
                            }
                            ui.label(
                                egui::RichText::new(path.to_string_lossy().to_string())
                                    .size(10.0)
                                    .weak(),
                            )
                            .on_hover_text("Full path");
                        });
                        ui.separator();
                    } else if let Some(url) = s.preview_panel.current_web_url() {
                        ui.horizontal(|ui| {
                            if ui.small_button("Open in Browser").clicked() {
                                let _ = open::that(&url);
                            }
                            if ui.small_button("Copy URL").clicked() {
                                ui.output_mut(|o| o.copied_text = url.clone());
                            }
                            ui.label(egui::RichText::new(url).size(10.0).weak());
                        });
                        ui.separator();
                    } else {
                        ui.separator();
                    }

                    // Render active viewer
                    match &mut s.active_viewer {
                        ActiveViewer::Panel => {
                            // Idle dashboard: make the preview panel useful even when nothing is open.
                            let is_idle = matches!(
                                s.preview_panel.content(),
                                None
                                    | Some(PreviewContent::ModeIntro { .. })
                                    | Some(PreviewContent::Ascii { .. })
                                    | Some(PreviewContent::SkillsList { .. })
                                    | Some(PreviewContent::Tip { .. })
                            );

                            if is_idle {
                                let accent = if dark {
                                    egui::Color32::from_rgb(140, 180, 140)
                                } else {
                                    egui::Color32::from_rgb(60, 120, 80)
                                };
                                let subtle = if dark {
                                    egui::Color32::from_rgb(170, 170, 190)
                                } else {
                                    egui::Color32::from_rgb(90, 90, 110)
                                };

                                // Mini context/capacity meter
                                let comfort_total: f32 = 8000.0;
                                let used = s.last_prompt_tokens_est as f32;
                                let ratio = (used / comfort_total).clamp(0.0, 1.0);

                                egui::Frame::none()
                                    .fill(if dark {
                                        egui::Color32::from_rgb(32, 32, 40)
                                    } else {
                                        egui::Color32::from_rgb(248, 248, 252)
                                    })
                                    .rounding(egui::Rounding::same(10.0))
                                    .inner_margin(egui::Margin::same(10.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new("Context")
                                                    .strong()
                                                    .color(accent)
                                                    .size(12.0),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "~{} / 8000",
                                                    s.last_prompt_tokens_est
                                                ))
                                                .size(11.0)
                                                .color(subtle),
                                            );
                                        });

                                        ui.add(
                                            egui::ProgressBar::new(ratio)
                                                .desired_width(ui.available_width()),
                                        );

                                        if ratio > 0.85 {
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(
                                                    "If things feel slow, I may trim older messages to make room.",
                                                )
                                                .size(11.0)
                                                .color(subtle),
                                            );
                                        }

                                        ui.add_space(8.0);

                                        // Quick actions
                                        ui.label(
                                            egui::RichText::new("Quick actions")
                                                .strong()
                                                .color(accent)
                                                .size(12.0),
                                        );

                                        let prompts: &[&str] = match s.current_mode {
                                            ChatMode::Find => &[
                                                "Find my latest downloads",
                                                "Find my resume",
                                                "Find photos of my dog",
                                            ],
                                            ChatMode::Fix => &[
                                                "Run a quick health check",
                                                "Why is my computer slow?",
                                                "Help free up disk space",
                                            ],
                                            ChatMode::Research => &[
                                                "Research a topic for me",
                                                "Summarize this webpage",
                                                "Compare two options",
                                            ],
                                            ChatMode::Build => &[
                                                "Start a new project",
                                                "Check my project",
                                                "Run a spec",
                                            ],
                                            _ => &["What can you do?"],
                                        };

                                        for p in prompts {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new(*p)
                                                            .size(12.0)
                                                            .color(if dark {
                                                                egui::Color32::from_rgb(220, 220, 230)
                                                            } else {
                                                                egui::Color32::from_rgb(40, 40, 50)
                                                            }),
                                                    )
                                                    .frame(false),
                                                )
                                                .clicked()
                                            {
                                                s.input_text = (*p).to_string();
                                            }
                                        }
                                    });

                                ui.add_space(8.0);
                            }

                            s.preview_panel.ui(ui);

                            if let Some(prompt) = s.preview_panel.take_clicked_prompt() {
                                s.input_text = prompt;
                            }
                        }
                        ActiveViewer::Matrix => {
                            render_matrix_rain(ui, ctx);
                        }
                        ActiveViewer::RickRoll => {
                            render_rick_roll(ui, dark);
                        }
                        ActiveViewer::CommandOutput(cmd, output) => {
                            render_command_output(ui, dark, cmd, output);
                        }
                    }
                });
        }

        // Chat area (center)
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(if dark {
                        egui::Color32::from_rgb(25, 25, 30)
                    } else {
                        egui::Color32::from_rgb(250, 250, 252)
                    })
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                // Paint mascot as wallpaper FIRST (background layer)
                let panel_rect = ui.max_rect();
                if let Some(texture) = &s.mascot_texture {
                    let tex_size = texture.size_vec2();

                    // Scale up like a chat wallpaper (large, soft)
                    let max_width = panel_rect.width() * 0.68;
                    let max_height = panel_rect.height() * 0.78;
                    let scale = (max_width / tex_size.x).min(max_height / tex_size.y);
                    let img_size = tex_size * scale;

                    // Center in the panel (behind chat bubbles)
                    let pos = egui::pos2(
                        panel_rect.center().x - img_size.x / 2.0,
                        panel_rect.center().y - img_size.y / 2.0 + 20.0,
                    );
                    let rect = egui::Rect::from_min_size(pos, img_size);

                    // Soft rounded frame to make the wallpaper feel intentional
                    let rounding = egui::Rounding::same(28.0);
                    let frame_rect = rect.expand2(egui::vec2(14.0, 14.0));
                    let frame_fill = if dark {
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 26)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)
                    };
                    ui.painter().rect_filled(frame_rect, rounding, frame_fill);

                    // Wallpaper image (low alpha so chat stays readable)
                    ui.painter().image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_white_alpha(30),
                    );

                    // Subtle border
                    ui.painter().rect_stroke(
                        frame_rect,
                        rounding,
                        egui::Stroke::new(
                            1.0,
                            if dark {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 20)
                            },
                        ),
                    );
                }

                // Thread controls bar (T116-T118)
                ui.horizontal(|ui| {
                    // New Thread button (T116)
                    if ui
                        .small_button("+ New Thread")
                        .on_hover_text("Start a fresh conversation")
                        .clicked()
                    {
                        // Clear current chat and start fresh
                        let user_name = if s.settings.user_profile.name.is_empty() {
                            "friend"
                        } else {
                            &s.settings.user_profile.name
                        };
                        let mode = s.current_mode;
                        let welcome = ChatMessage {
                            role: "assistant".to_string(),
                            content: format!(
                                "Starting a fresh conversation! How can I help you today, {}?",
                                user_name
                            ),
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        };
                        s.mode_chat_histories.insert(mode, vec![welcome]);
                        // Show mode intro in preview
                        s.preview_panel.show_mode_intro(mode.as_str());
                    }

                    ui.separator();

                    // Thread count indicator
                    let thread_count = s.mode_chat_histories.get(&s.current_mode).map_or(0, |h| h.len());
                    ui.label(
                        egui::RichText::new(format!("{} messages", thread_count))
                            .small()
                            .weak(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Clear History button (T118)
                        if ui
                            .small_button("🗑")
                            .on_hover_text("Clear this conversation")
                            .clicked()
                        {
                            let user_name = if s.settings.user_profile.name.is_empty() {
                                "friend"
                            } else {
                                &s.settings.user_profile.name
                            };
                            let mode = s.current_mode;
                            let welcome = ChatMessage {
                                role: "assistant".to_string(),
                                content: format!(
                                    "Conversation cleared. What would you like to work on, {}?",
                                    user_name
                                ),
                                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                            };
                            s.mode_chat_histories.insert(mode, vec![welcome]);
                            s.preview_panel.show_mode_intro(mode.as_str());
                        }
                    });
                });

                ui.add_space(4.0);

                // Chat messages scroll area
                let chat_height = ui.available_height() - 70.0;

                let mut clicked_path: Option<PathBuf> = None;
                let mut slack_msg: Option<String> = None;

                if s.current_mode == ChatMode::Build {
                    render_build_panel(&mut *s, ui, dark);
                    ui.add_space(12.0);
                }

                // Handover notification: show if another mode is processing
                if let Some(thinking_mode) = s.thinking_mode {
                    if thinking_mode != s.current_mode && s.is_thinking.get(&thinking_mode).copied().unwrap_or(false) {
                        let mode_name = match thinking_mode {
                            ChatMode::Find => "Find Helper",
                            ChatMode::Fix => "Fix Helper",
                            ChatMode::Research => "Research Helper",
                            ChatMode::Data => "Data Helper",
                            ChatMode::Content => "Content Helper",
                            ChatMode::Build => "Build Helper",
                        };
                        let elapsed = s
                            .thinking_started_at
                            .get(&thinking_mode)
                            .map(|t| t.elapsed().as_secs())
                            .unwrap_or(0);
                        let time_str = if elapsed < 60 {
                            format!("{}s", elapsed)
                        } else {
                            format!("{}m {}s", elapsed / 60, elapsed % 60)
                        };
                        
                        egui::Frame::none()
                            .fill(if dark {
                                egui::Color32::from_rgb(45, 45, 55)
                            } else {
                                egui::Color32::from_rgb(240, 240, 245)
                            })
                            .rounding(egui::Rounding::same(8.0))
                            .inner_margin(egui::Margin::same(10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("⏳ {} is still working... ({})", mode_name, time_str))
                                            .size(13.0)
                                            .color(if dark {
                                                egui::Color32::from_rgb(180, 180, 200)
                                            } else {
                                                egui::Color32::from_rgb(80, 80, 100)
                                            }),
                                    );
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Stop it").clicked() {
                                            // Cancel the operation
                                            s.cancel_ai(thinking_mode);
                                        }
                                    });
                                });
                            });
                        ui.add_space(8.0);
                    }
                }

                // Get current mode's chat history (clone to avoid borrow issues)
                let current_mode = s.current_mode;
                let chat_history: Vec<ChatMessage> = s.mode_chat_histories
                    .get(&current_mode)
                    .cloned()
                    .unwrap_or_default();
                let allowed_dirs = s.settings.allowed_dirs.clone();
                // Only show thinking if current mode matches the thinking mode
                let is_thinking = s.thinking_mode == Some(current_mode) && 
                    s.is_thinking.get(&current_mode).copied().unwrap_or(false);
                let thinking_status = s.thinking_status.get(&current_mode).cloned().unwrap_or_default();

                egui::ScrollArea::vertical()
                    .max_height(chat_height)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in chat_history.iter() {
                            ui.add_space(6.0);
                            let action = render_message(ui, msg, dark, &allowed_dirs);
                            if action.clicked_path.is_some() {
                                clicked_path = action.clicked_path;
                            }
                            if action.send_to_slack.is_some() {
                                slack_msg = action.send_to_slack;
                            }
                            ui.add_space(6.0);
                        }

                        if is_thinking {
                            ui.add_space(6.0);
                            egui::Frame::none()
                                .fill(if dark {
                                    egui::Color32::from_rgb(50, 50, 58)
                                } else {
                                    egui::Color32::from_rgb(230, 230, 235)
                                })
                                .rounding(egui::Rounding::same(12.0))
                                .inner_margin(egui::Margin::same(12.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Animated spinner dots
                                        let time = ui.input(|i| i.time);
                                        let dots = match ((time * 2.0) as i32) % 4 {
                                            0 => "   ",
                                            1 => ".  ",
                                            2 => ".. ",
                                            _ => "...",
                                        };

                                        let status = if thinking_status.is_empty() {
                                            "Thinking".to_string()
                                        } else {
                                            thinking_status.clone()
                                        };

                                        ui.label(
                                            egui::RichText::new(format!("{}{}", status, dots))
                                                .color(if dark {
                                                    egui::Color32::from_rgb(160, 160, 180)
                                                } else {
                                                    egui::Color32::from_rgb(60, 60, 70)
                                                })
                                                .italics(),
                                        );
                                    });
                                });
                            // Request repaint to animate
                            ctx.request_repaint();
                        }
                    });

                // Handle clicked path after iteration
                if let Some(path) = clicked_path {
                    s.open_file(&path, ctx);
                }

                // Handle pending preview from agent (auto-open)
                if let Some(path) = s.pending_preview.take() {
                    s.open_file(&path, ctx);
                }

                // Handle Slack send request
                if let Some(msg) = slack_msg {
                    s.slack_message_to_send = Some(msg);
                    s.show_slack_dialog = true;
                    s.slack_status = None;
                }

                ui.add_space(8.0);

                if !s.pending_commands.is_empty() {
                    ui.group(|ui| {
                        ui.label(
                            egui::RichText::new("Commands awaiting approval")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 150, 80)),
                        );
                        ui.add_space(6.0);
                        let pending = s.pending_commands.clone();
                        for cmd in pending {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("$ {}", cmd)).monospace());
                                if ui.button("Run").clicked() {
                                    s.approve_command(cmd.clone());
                                }
                                if ui.button("Dismiss").clicked() {
                                    s.pending_commands.retain(|c| c != &cmd);
                                }
                            });
                        }
                    });
                    ui.add_space(8.0);
                }

                // Input area
                ui.horizontal(|ui| {
                    let hint = match s.current_mode {
                        ChatMode::Find => "What are you trying to find?",
                        ChatMode::Fix => "What's broken? Need to find a file?",
                        ChatMode::Research => "What should I research?",
                        ChatMode::Data => "What data would you like to work with?",
                        ChatMode::Content => "What content would you like to create?",
                        ChatMode::Build => "What would you like to build?",
                    };

                    // Subtle attention nudge when approvals are waiting
                    if !s.pending_commands.is_empty() {
                        let blink = ((ui.input(|i| i.time) * 2.0) as i32) % 2 == 0;
                        if blink {
                            ui.label(
                                egui::RichText::new("↑")
                                    .size(18.0)
                                    .color(if dark {
                                        egui::Color32::from_rgb(220, 180, 100)
                                    } else {
                                        egui::Color32::from_rgb(160, 120, 60)
                                    }),
                            );
                        } else {
                            ui.add_space(18.0);
                        }
                    }

                    let response = ui.add_sized(
                        [ui.available_width() - 80.0, 40.0],
                        egui::TextEdit::singleline(&mut s.input_text)
                            .hint_text(hint)
                            .font(egui::FontId::new(15.0, egui::FontFamily::Proportional)),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        s.send_message();
                    }

                    if ui
                        .add_sized(
                            [70.0, 40.0],
                            egui::Button::new("Send").fill(egui::Color32::from_rgb(70, 130, 180)),
                        )
                        .clicked()
                    {
                        s.send_message();
                    }
                });
            });

        // Settings dialog
        if s.show_settings_dialog {
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(420.0);
                    ui.heading(
                        egui::RichText::new("Privacy")
                            .color(if dark {
                                egui::Color32::from_rgb(220, 220, 230)
                            } else {
                                egui::Color32::from_rgb(40, 40, 50)
                            }),
                    );
                    ui.add_space(8.0);

                    let mut needs_save = false;

                    if ui
                        .checkbox(
                            &mut s.settings.enable_campaign_context,
                            "Load campaign materials automatically",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }
                    if ui
                        .checkbox(
                            &mut s.settings.enable_persona_context,
                            "Load persona files from your profiles folder",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }
                    if ui
                        .checkbox(
                            &mut s.settings.share_system_summary,
                            "Share basic system info with the AI",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }
                    if ui
                        .checkbox(
                            &mut s.settings.enable_internet_research,
                            "Allow web research (searches and articles)",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }

                    if needs_save {
                        save_settings(&s.settings);
                        s.settings_status = Some("Saved privacy settings".to_string());
                        s.settings_status_is_error = false;
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.heading(
                        egui::RichText::new("AI Model")
                            .color(if dark {
                                egui::Color32::from_rgb(220, 220, 230)
                            } else {
                                egui::Color32::from_rgb(40, 40, 50)
                            }),
                    );
                    ui.label(
                        egui::RichText::new("Pick where replies come from. Cloud options need a key.")
                            .color(if dark {
                                egui::Color32::from_rgb(180, 180, 190)
                            } else {
                                egui::Color32::from_rgb(80, 80, 90)
                            }),
                    );
                    ui.add_space(6.0);

                    // Provider selection combo box
                    let providers = [("local", "🖥️ Local (Ollama - free, runs on your computer)"),
                                    ("openai", "🤖 OpenAI (GPT-4o-mini, requires API key)"),
                                    ("anthropic", "🧠 Anthropic (Claude, requires API key)"),
                                    ("gemini", "✨ Google (Gemini, requires API key)")];

                    // Get current primary provider as owned string
                    let current_provider: String = s.settings.model.provider_preference.first()
                        .cloned()
                        .unwrap_or_else(|| "local".to_string());

                    let mut selected_provider = current_provider.clone();
                    egui::ComboBox::from_id_source("provider_select")
                        .selected_text(providers.iter().find(|(k, _)| *k == current_provider.as_str())
                            .map(|(_, v)| *v)
                            .unwrap_or("Select provider..."))
                        .show_ui(ui, |ui| {
                            for (key, label) in &providers {
                                if ui.selectable_value(&mut selected_provider, key.to_string(), *label).clicked() {
                                    // Update provider preference
                                    s.settings.model.provider_preference = vec![key.to_string()];
                                    save_settings(&s.settings);
                                    s.settings_status = Some(format!("Switched to {}", label.split(' ').next().unwrap_or(key)));
                                    s.settings_status_is_error = false;
                                }
                            }
                        });

                    ui.add_space(4.0);

                    // Show API key status
                    let has_api_key = match current_provider.as_str() {
                        "openai" => s.settings.model.openai_auth.api_key.is_some(),
                        "anthropic" => s.settings.model.anthropic_auth.api_key.is_some(),
                        "gemini" => s.settings.model.gemini_auth.api_key.is_some(),
                        _ => true, // Local doesn't need API key
                    };

                    if current_provider != "local" {
                        if has_api_key {
                            ui.label(egui::RichText::new("✓ API key configured").color(egui::Color32::from_rgb(100, 180, 100)).size(11.0));
                        } else {
                            ui.label(egui::RichText::new("⚠ No API key configured").color(egui::Color32::from_rgb(220, 140, 60)).size(11.0));
                        }
                    }

                    // API Key input section
                    ui.add_space(8.0);
                    ui.collapsing("Add API keys", |ui| {
                        ui.label(egui::RichText::new("Add a key if you want to use cloud models.").size(11.0).weak());
                        ui.add_space(4.0);

                        // OpenAI API Key
                        ui.horizontal(|ui| {
                            ui.label("OpenAI:");
                            let mut openai_key = s.openai_api_key_input.clone();
                            if s.settings.model.openai_auth.api_key.is_some() {
                                ui.label(egui::RichText::new("✓ Set").color(egui::Color32::from_rgb(100, 180, 100)).size(11.0));
                            }
                            let edit = egui::TextEdit::singleline(&mut openai_key).password(true);
                            if ui.add(edit).changed() {
                                s.openai_api_key_input = openai_key;
                            }
                            if !s.openai_api_key_input.is_empty() && ui.button("Save").clicked() {
                                s.settings.model.openai_auth.api_key = Some(s.openai_api_key_input.clone());
                                save_settings(&s.settings);
                                s.openai_api_key_input.clear();
                                s.settings_status = Some("OpenAI API key saved".to_string());
                                s.settings_status_is_error = false;
                            }
                        });

                        // Anthropic API Key
                        ui.horizontal(|ui| {
                            ui.label("Anthropic:");
                            if s.settings.model.anthropic_auth.api_key.is_some() {
                                ui.label(egui::RichText::new("✓ Set").color(egui::Color32::from_rgb(100, 180, 100)).size(11.0));
                            }
                            let mut anthropic_key = s.anthropic_api_key_input.clone();
                            let edit = egui::TextEdit::singleline(&mut anthropic_key).password(true);
                            if ui.add(edit).changed() {
                                s.anthropic_api_key_input = anthropic_key;
                            }
                            if !s.anthropic_api_key_input.is_empty() && ui.button("Save").clicked() {
                                s.settings.model.anthropic_auth.api_key = Some(s.anthropic_api_key_input.clone());
                                save_settings(&s.settings);
                                s.anthropic_api_key_input.clear();
                                s.settings_status = Some("Anthropic API key saved".to_string());
                                s.settings_status_is_error = false;
                            }
                        });

                        // Gemini API Key
                        ui.horizontal(|ui| {
                            ui.label("Gemini:");
                            if s.settings.model.gemini_auth.api_key.is_some() {
                                ui.label(egui::RichText::new("✓ Set").color(egui::Color32::from_rgb(100, 180, 100)).size(11.0));
                            }
                            let mut gemini_key = s.gemini_api_key_input.clone();
                            let edit = egui::TextEdit::singleline(&mut gemini_key).password(true);
                            if ui.add(edit).changed() {
                                s.gemini_api_key_input = gemini_key;
                            }
                            if !s.gemini_api_key_input.is_empty() && ui.button("Save").clicked() {
                                s.settings.model.gemini_auth.api_key = Some(s.gemini_api_key_input.clone());
                                save_settings(&s.settings);
                                s.gemini_api_key_input.clear();
                                s.settings_status = Some("Gemini API key saved".to_string());
                                s.settings_status_is_error = false;
                            }
                        });

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Keys stay on this device.").size(10.0).weak());
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.collapsing("Status (advanced)", |ui| {
                        s.update_settings_perf();
                        let ctx_hint = s.model_context_hint_tokens();
                        let comfort_total: f32 = 8000.0;
                        let used = s.last_prompt_tokens_est as f32;
                        let ratio = (used / comfort_total).clamp(0.0, 1.0);

                        ui.label(
                            egui::RichText::new("A quick snapshot of performance and conversation size.")
                                .size(11.0)
                                .weak(),
                        );
                        ui.add_space(6.0);

                        egui::Grid::new("settings_status_grid")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("CPU (app)");
                                ui.label(format!("{:.0}%", s.settings_cpu_percent));
                                ui.end_row();

                                ui.label("Memory (app)");
                                ui.label(format!("{} MB", s.settings_mem_mb));
                                ui.end_row();

                                ui.label("Last prompt");
                                ui.label(format!("~{} tokens", s.last_prompt_tokens_est));
                                ui.end_row();

                                ui.label("Last reply");
                                ui.label(format!("~{} tokens", s.last_response_tokens_est));
                                ui.end_row();

                                ui.label("Session total");
                                ui.label(format!(
                                    "~{} in / ~{} out",
                                    s.session_input_tokens_est, s.session_output_tokens_est
                                ));
                                ui.end_row();

                                ui.label("Model context (approx)");
                                ui.label(format!("~{} tokens", ctx_hint));
                                ui.end_row();
                            });

                        ui.add_space(8.0);
                        ui.label("Conversation capacity (comfort window)");
                        ui.add(
                            egui::ProgressBar::new(ratio)
                                .show_percentage()
                                .text(format!(
                                    "~{} / 8000 tokens",
                                    s.last_prompt_tokens_est
                                )),
                        );

                        if ratio > 0.85 {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "If things feel slow, I may trim older messages to make room."
                                )
                                .size(11.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 180, 100)
                                } else {
                                    egui::Color32::from_rgb(160, 120, 60)
                                }),
                            );
                        }
                    });

                    ui.heading(
                        egui::RichText::new("Build tools")
                            .color(if dark {
                                egui::Color32::from_rgb(220, 220, 230)
                            } else {
                                egui::Color32::from_rgb(40, 40, 50)
                            }),
                    );
                    ui.label(
                        egui::RichText::new("Set up Spec Kit so Build Helper can work without the terminal.")
                            .color(if dark {
                                egui::Color32::from_rgb(180, 180, 190)
                            } else {
                                egui::Color32::from_rgb(80, 80, 90)
                            }),
                    );
                    ui.add_space(6.0);

                    ui.label(egui::RichText::new("Spec Kit location").size(11.0));
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut s.spec_kit_path_input);
                        if ui.button("Use default").clicked() {
                            if let Some(home) = dirs::home_dir() {
                                s.spec_kit_path_input = home
                                    .join("Projects/spec-kit-assistant/spec-assistant.js")
                                    .to_string_lossy()
                                    .to_string();
                            }
                        }
                        if ui.button("Save").clicked() {
                            let trimmed = s.spec_kit_path_input.trim();
                            if trimmed.is_empty() {
                                s.settings.build.spec_kit_path = None;
                            } else {
                                s.settings.build.spec_kit_path = Some(trimmed.to_string());
                            }
                            save_settings(&s.settings);
                            s.settings_status = Some("Saved build tools settings".to_string());
                            s.settings_status_is_error = false;
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.heading(
                        egui::RichText::new("Allowed folders")
                            .color(if dark {
                                egui::Color32::from_rgb(220, 220, 230)
                            } else {
                                egui::Color32::from_rgb(40, 40, 50)
                            }),
                    );
                    ui.label(
                        egui::RichText::new("Little Helper only works inside these folders.")
                            .color(if dark {
                                egui::Color32::from_rgb(180, 180, 190)
                            } else {
                                egui::Color32::from_rgb(80, 80, 90)
                            }),
                    );
                    ui.add_space(6.0);

                    if let Some(msg) = &s.settings_status {
                        let color = if s.settings_status_is_error {
                            egui::Color32::from_rgb(200, 120, 120)
                        } else {
                            egui::Color32::from_rgb(120, 200, 150)
                        };
                        ui.colored_label(color, msg);
                        ui.add_space(6.0);
                    }

                    if s.settings.allowed_dirs.is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 120, 120),
                            "No folders allowed. Add at least one folder.",
                        );
                    }

                    let current_dirs = s.settings.allowed_dirs.clone();
                    let mut dir_to_remove: Option<String> = None;
                    for dir in &current_dirs {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(dir)
                                    .family(egui::FontFamily::Monospace)
                                    .size(12.0),
                            );
                            if s.settings.allowed_dirs.len() > 1 {
                                if ui.small_button("Remove").clicked() {
                                    dir_to_remove = Some(dir.clone());
                                }
                            }
                        });
                    }

                    if let Some(target) = dir_to_remove {
                        s.settings
                            .allowed_dirs
                            .retain(|existing| existing != &target);
                        ensure_allowed_dirs(&mut s.settings);
                        save_settings(&s.settings);
                        s.settings_status = Some(format!("Removed {}", target));
                        s.settings_status_is_error = false;
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let text_edit = egui::TextEdit::singleline(&mut s.new_allowed_dir)
                            .hint_text("~/Documents or /data/projects");
                        ui.add(text_edit);
                        if ui.button("Add").clicked() {
                            let input = s.new_allowed_dir.trim();
                            if input.is_empty() {
                                s.settings_status =
                                    Some("Enter a folder path before adding.".to_string());
                                s.settings_status_is_error = true;
                            } else if let Some(normalized) =
                                normalize_allowed_dir_input(input)
                            {
                                let path_str = normalized.to_string_lossy().to_string();
                                if s.settings
                                    .allowed_dirs
                                    .iter()
                                    .any(|dir| dir == &path_str)
                                {
                                    s.settings_status =
                                        Some("Folder already in allowlist.".to_string());
                                    s.settings_status_is_error = true;
                                } else {
                                    s.settings.allowed_dirs.push(path_str.clone());
                                    save_settings(&s.settings);
                                    s.settings_status =
                                        Some(format!("Added {}", path_str));
                                    s.settings_status_is_error = false;
                                }
                                s.new_allowed_dir.clear();
                            } else {
                                s.settings_status =
                                    Some("Folder must exist on disk.".to_string());
                                s.settings_status_is_error = true;
                            }
                        }
                    });

                    ui.add_space(12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            s.show_settings_dialog = false;
                        }
                    });
                });
        }

        // Slack dialog window (modal-ish)
        if s.show_slack_dialog {
            egui::Window::new("Send to Slack")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(400.0);

                    ui.add_space(8.0);

                    // Channel selector
                    ui.horizontal(|ui| {
                        ui.label("Channel:");
                        egui::ComboBox::from_id_source("slack_channel")
                            .selected_text(&s.slack_selected_channel)
                            .show_ui(ui, |ui| {
                                // Common channel options
                                let channels = [
                                    "#general",
                                    "#content",
                                    "#drafts",
                                    "#general",
                                    "#team",
                                    "#review",
                                ];
                                for channel in channels {
                                    ui.selectable_value(&mut s.slack_selected_channel, channel.to_string(), channel);
                                }
                            });
                    });

                    ui.add_space(8.0);

                    // Preview of message
                    ui.label("Message preview:");
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            if let Some(msg) = &s.slack_message_to_send {
                                let preview = if msg.len() > 500 {
                                    format!("{}...", &msg[..500])
                                } else {
                                    msg.clone()
                                };
                                ui.label(&preview);
                            }
                        });

                    ui.add_space(8.0);

                    // Status message
                    if let Some(status) = &s.slack_status {
                        if status.starts_with("Error") {
                            ui.colored_label(egui::Color32::RED, status);
                        } else {
                            ui.colored_label(egui::Color32::GREEN, status);
                        }
                        ui.add_space(8.0);
                    }

                    // Webhook URL check
                    if s.settings.slack.webhook_url.is_none() {
                        ui.colored_label(
                            egui::Color32::from_rgb(200, 150, 50),
                            "Slack webhook not configured. Set SLACK_WEBHOOK_URL environment variable."
                        );
                        ui.add_space(8.0);
                    }

                    // Buttons
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            s.show_slack_dialog = false;
                            s.slack_message_to_send = None;
                            s.slack_status = None;
                        }

                        let can_send = s.settings.slack.webhook_url.is_some() || std::env::var("SLACK_WEBHOOK_URL").is_ok();

                        if ui.add_enabled(can_send, egui::Button::new("Send")).clicked() {
                            // Send to Slack
                            if let Some(msg) = &s.slack_message_to_send {
                                let webhook_url = s.settings.slack.webhook_url.clone()
                                    .or_else(|| std::env::var("SLACK_WEBHOOK_URL").ok());

                                if let Some(url) = webhook_url {
                                    let channel = s.slack_selected_channel.clone();
                                    let message = msg.clone();

                                    // Spawn async send
                                    let result = send_slack_message_sync(&url, &channel, &message);
                                    match result {
                                        Ok(_) => {
                                            s.slack_status = Some(format!("Sent to {}", channel));
                                            // Close after short delay would be nice, but for now just show success
                                        }
                                        Err(e) => {
                                            s.slack_status = Some(format!("Error: {}", e));
                                        }
                                    }
                                }
                            }
                        }
                    });
                });
        }
    }
}

/// Send a Slack message synchronously (for UI thread)
fn send_slack_message_sync(webhook_url: &str, channel: &str, message: &str) -> Result<(), String> {
    // Build JSON payload
    let payload = serde_json::json!({
        "channel": channel,
        "username": "Little Helper",
        "icon_emoji": ":robot_face:",
        "text": message
    });

    // Use ureq for simple sync HTTP (or we could spawn a thread)
    // For now, use std::process to call curl as a simple solution
    let payload_str = payload.to_string();

    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-S",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload_str,
            webhook_url,
        ])
        .output()
        .map_err(|e| format!("Failed to send: {}", e))?;

    if output.status.success() {
        let response = String::from_utf8_lossy(&output.stdout);
        if response.contains("ok") || response.is_empty() {
            Ok(())
        } else {
            Err(format!("Slack error: {}", response))
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Request failed: {}", stderr))
    }
}

fn render_build_panel(s: &mut AppState, ui: &mut egui::Ui, dark: bool) {
    let heading_color = if dark {
        egui::Color32::from_rgb(220, 220, 230)
    } else {
        egui::Color32::from_rgb(40, 40, 50)
    };

    let subtle_color = if dark {
        egui::Color32::from_rgb(160, 160, 180)
    } else {
        egui::Color32::from_rgb(80, 80, 90)
    };

    let spec_kit_path = s.spec_kit_path();
    let spec_kit_ready = spec_kit_path.exists();

    egui::Frame::none()
        .fill(if dark {
            egui::Color32::from_rgb(35, 35, 42)
        } else {
            egui::Color32::from_rgb(245, 247, 250)
        })
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.heading(
                egui::RichText::new("Build Helper")
                    .color(heading_color)
                    .size(16.0),
            );
            ui.label(
                egui::RichText::new("Spec Kit keeps projects organized with specs, plans, and tasks.")
                    .color(subtle_color)
                    .size(11.0),
            );

            ui.add_space(6.0);

            let status_text = if spec_kit_ready {
                "Spec Kit: Ready"
            } else {
                "Spec Kit: Not found"
            };
            let status_color = if spec_kit_ready {
                egui::Color32::from_rgb(100, 180, 100)
            } else {
                egui::Color32::from_rgb(220, 140, 60)
            };
            ui.label(egui::RichText::new(status_text).color(status_color).size(11.0));
            ui.label(
                egui::RichText::new(format!("Path: {}", spec_kit_path.display()))
                    .color(subtle_color)
                    .size(10.0),
            );

            if let Some(status) = &s.build_status {
                let color = if s.build_status_is_error {
                    egui::Color32::from_rgb(220, 120, 120)
                } else {
                    egui::Color32::from_rgb(100, 180, 100)
                };
                ui.add_space(6.0);
                ui.label(egui::RichText::new(status).color(color).size(11.0));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            ui.label(egui::RichText::new("Project folder").color(subtle_color));
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut s.build_folder_input);
                if ui.button("Use Home").clicked() {
                    if let Some(home) = dirs::home_dir() {
                        s.build_folder_input = home.to_string_lossy().to_string();
                    }
                }
            });

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Project name").color(subtle_color));
            ui.text_edit_singleline(&mut s.build_project_name_input);

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Spec name (optional)").color(subtle_color));
            ui.text_edit_singleline(&mut s.build_spec_name_input);

            ui.add_space(6.0);
            ui.label(egui::RichText::new("What should we build?").color(subtle_color));
            ui.text_edit_singleline(&mut s.build_description_input);

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("Start Project").clicked() {
                    let name = s.build_project_name_input.trim();
                    if name.is_empty() {
                        s.build_status = Some("Please enter a project name.".to_string());
                        s.build_status_is_error = true;
                    } else {
                        s.settings.build.default_project_folder = Some(s.build_folder_input.trim().to_string());
                        save_settings(&s.settings);
                        s.run_spec_kit_command(vec!["init".to_string(), name.to_string()]);
                    }
                }

                if ui.button("Check Project").clicked() {
                    s.settings.build.default_project_folder = Some(s.build_folder_input.trim().to_string());
                    save_settings(&s.settings);
                    s.run_spec_kit_command(vec!["check".to_string()]);
                }

                if ui.button("Run Spec").clicked() {
                    let description = s.build_description_input.trim();
                    if description.is_empty() {
                        s.build_status = Some("Please describe what to build.".to_string());
                        s.build_status_is_error = true;
                    } else {
                        let mut args = vec!["run".to_string(), description.to_string()];
                        let spec_name = s.build_spec_name_input.trim();
                        if !spec_name.is_empty() {
                            args.push("--spec".to_string());
                            args.push(spec_name.to_string());
                        }
                        s.settings.build.default_project_folder = Some(s.build_folder_input.trim().to_string());
                        save_settings(&s.settings);
                        s.run_spec_kit_command(args);
                    }
                }
            });
        });
}

fn mode_button(
    ui: &mut egui::Ui,
    label: &str,
    mode: ChatMode,
    current: &mut ChatMode,
    is_processing: bool,
    has_unread: bool,
) {
    let is_selected = *current == mode;
    
    // Build label with processing indicator
    let label_text = if is_processing && !is_selected {
        // Pulsing dot effect based on time
        let time = ui.ctx().input(|i| i.time);
        let pulse = ((time * 4.0).sin() + 1.0) / 2.0; // 0.0 to 1.0
        let _alpha = (128.0 + pulse * 127.0) as u8; // 128-255 range
        format!("{} ●", label)
    } else if has_unread && !is_selected {
        format!("{} •", label)
    } else {
        label.to_string()
    };
    
    let text_color = if is_selected {
        egui::Color32::WHITE
    } else if is_processing {
        // Pulsing color for processing indicator
        let time = ui.ctx().input(|i| i.time);
        let pulse = ((time * 4.0).sin() + 1.0) / 2.0;
        let alpha = (128.0 + pulse * 127.0) as u8;
        egui::Color32::from_rgba_unmultiplied(100, 200, 255, alpha)
    } else if has_unread {
        egui::Color32::from_rgb(120, 140, 230)
    } else {
        egui::Color32::from_rgb(70, 70, 90)
    };
    
    let btn = egui::Button::new(egui::RichText::new(label_text).size(14.0).color(text_color))
    .fill(if is_selected {
        egui::Color32::from_rgb(70, 130, 180)
    } else {
        egui::Color32::TRANSPARENT
    })
    .rounding(egui::Rounding::same(8.0));

    if ui.add_sized([80.0, 32.0], btn).clicked() {
        *current = mode;
    }
}

/// Render the welcome panel shown by default
fn render_welcome_panel(ui: &mut egui::Ui, dark: bool, current_mode: &ChatMode) {
    let text_color = if dark {
        egui::Color32::from_rgb(200, 200, 210)
    } else {
        egui::Color32::from_rgb(60, 60, 70)
    };

    let accent_color = if dark {
        egui::Color32::from_rgb(100, 160, 220)
    } else {
        egui::Color32::from_rgb(70, 130, 180)
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        // Mode-specific tips
        let (mode_name, tips) = match current_mode {
            ChatMode::Find => (
                "Find Helper",
                vec![
                    "Tell me what you're looking for",
                    "I can search folders and open results",
                    "Try: \"find my resume\"",
                ],
            ),
            ChatMode::Fix => (
                "Fix Helper",
                vec![
                    "Tell me what's broken - I'll diagnose it",
                    "Need a file? I can find that too",
                    "Diagnostics and logs show up here",
                    "Try: \"my wifi keeps disconnecting\"",
                    "Try: \"find my tax documents\"",
                ],
            ),
            ChatMode::Research => (
                "Research Helper",
                vec![
                    "Ask me to research any topic",
                    "I'll search multiple sources with citations",
                    "Results and sources shown here",
                    "Try: \"research the latest on Alberta politics\"",
                ],
            ),
            ChatMode::Data => (
                "Data Helper",
                vec![
                    "Work with CSV, JSON, Excel files",
                    "Data tables render right here",
                    "I can analyze and transform data",
                    "Try: \"analyze this spreadsheet\" + drop a file",
                ],
            ),
            ChatMode::Content => (
                "Content Helper",
                vec![
                    "Create content for any platform",
                    "I know your campaign personas",
                    "Drafts preview here before saving",
                    "Try: \"write a tweet about healthcare\"",
                ],
            ),
            ChatMode::Build => (
                "Build Helper",
                vec![
                    "Start a new project with Spec Kit",
                    "Plan and run specs without using a terminal",
                    "We'll use folders and buttons only",
                    "Try: \"start a new project\"",
                ],
            ),
        };

        ui.label(
            egui::RichText::new(format!("📋 {}", mode_name))
                .size(18.0)
                .color(accent_color)
                .strong(),
        );
        ui.add_space(12.0);

        ui.label(
            egui::RichText::new("This panel shows live previews:")
                .size(14.0)
                .color(text_color),
        );
        ui.add_space(8.0);

        for tip in tips {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("•").color(accent_color));
                ui.label(egui::RichText::new(tip).size(13.0).color(text_color));
            });
            ui.add_space(4.0);
        }

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(12.0);

        // Capabilities reminder
        ui.label(
            egui::RichText::new("🛠 I can:")
                .size(14.0)
                .color(accent_color),
        );
        ui.add_space(8.0);

        let capabilities = [
            (
                "⌨️",
                "Run terminal commands",
                "Safe commands execute automatically",
            ),
            ("🔍", "Search the web", "With sources and citations"),
            ("📄", "Preview files", "Text, images, CSV, JSON, HTML, PDF"),
            ("💬", "Send to Slack", "Share responses to your channels"),
        ];

        for (icon, name, desc) in capabilities {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).size(14.0));
                ui.label(
                    egui::RichText::new(name)
                        .size(13.0)
                        .strong()
                        .color(text_color),
                );
                ui.label(egui::RichText::new(format!("- {}", desc)).size(12.0).weak());
            });
            ui.add_space(2.0);
        }
    });
}

/// Render command output in the preview panel
fn render_command_output(ui: &mut egui::Ui, dark: bool, cmd: &str, output: &str) {
    let bg_color = if dark {
        egui::Color32::from_rgb(20, 20, 25)
    } else {
        egui::Color32::from_rgb(245, 245, 250)
    };

    let text_color = if dark {
        egui::Color32::from_rgb(200, 220, 200)
    } else {
        egui::Color32::from_rgb(40, 60, 40)
    };

    ui.add_space(8.0);

    // Command that was run
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("$")
                .size(14.0)
                .color(egui::Color32::from_rgb(100, 200, 100))
                .strong(),
        );
        ui.label(
            egui::RichText::new(cmd)
                .size(13.0)
                .color(text_color)
                .monospace(),
        );
    });

    ui.add_space(8.0);

    // Output in a scrollable code block
    egui::Frame::none()
        .fill(bg_color)
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 20.0)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(output)
                            .size(12.0)
                            .color(text_color)
                            .monospace(),
                    );
                });
        });
}

/// Render Matrix-style rain animation while processing
fn render_matrix_rain(ui: &mut egui::Ui, ctx: &egui::Context) {
    let rect = ui.available_rect_before_wrap();
    let time = ui.input(|i| i.time);

    // Matrix green
    let matrix_green = egui::Color32::from_rgb(0, 255, 65);

    // Fill background black
    ui.painter()
        .rect_filled(rect, 0.0, egui::Color32::from_rgb(0, 0, 0));

    // Matrix characters
    let chars: Vec<char> = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン0123456789".chars().collect();

    let col_width = 14.0;
    let row_height = 16.0;
    let cols = (rect.width() / col_width) as i32;
    let rows = (rect.height() / row_height) as i32;

    for col in 0..cols {
        // Each column has its own speed and offset
        let col_seed = (col as f64 * 7.3).sin() * 1000.0;
        let speed = 2.0 + (col_seed.cos() * 1.5);
        let offset = (col_seed * 3.7) % (rows as f64 * 2.0);

        for row in 0..rows {
            let y_pos =
                ((time * speed + offset + row as f64) % (rows as f64 * 1.5)) - rows as f64 * 0.25;

            if y_pos >= 0.0 && y_pos < rows as f64 {
                let char_idx =
                    ((time * 10.0 + col as f64 * 3.0 + row as f64) as usize) % chars.len();
                let ch = chars[char_idx];

                // Fade based on position in trail
                let fade = (1.0 - (y_pos / rows as f64)).max(0.0).min(1.0);
                let alpha = (fade * 255.0) as u8;

                let color = if row as f64 == y_pos.floor() {
                    egui::Color32::from_rgba_unmultiplied(200, 255, 200, alpha) // Bright head
                } else {
                    egui::Color32::from_rgba_unmultiplied(0, 255, 65, alpha / 2)
                };

                let pos = egui::pos2(
                    rect.left() + col as f32 * col_width,
                    rect.top() + y_pos as f32 * row_height,
                );

                ui.painter().text(
                    pos,
                    egui::Align2::LEFT_TOP,
                    ch.to_string(),
                    egui::FontId::monospace(14.0),
                    color,
                );
            }
        }
    }

    // "PROCESSING..." text in center
    let center = rect.center();
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        "PROCESSING...",
        egui::FontId::monospace(24.0),
        matrix_green,
    );

    // Request repaint for animation
    ctx.request_repaint();
}

/// Render Rick Roll easter egg
fn render_rick_roll(ui: &mut egui::Ui, _dark: bool) {
    let rect = ui.available_rect_before_wrap();

    // Fun gradient background
    ui.painter()
        .rect_filled(rect, 12.0, egui::Color32::from_rgb(30, 30, 50));

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(40.0);

        ui.vertical_centered(|ui| {
            // Big emoji
            ui.label(egui::RichText::new("🕺💃🎵").size(60.0));

            ui.add_space(20.0);

            // The reveal
            ui.label(
                egui::RichText::new("Never Gonna Give You Up!")
                    .size(28.0)
                    .strong()
                    .color(egui::Color32::from_rgb(255, 100, 100)),
            );

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Never Gonna Let You Down!")
                    .size(22.0)
                    .color(egui::Color32::from_rgb(255, 150, 100)),
            );

            ui.add_space(30.0);

            // The message
            ui.label(
                egui::RichText::new("You just got Rick Rolled! 🎤")
                    .size(18.0)
                    .italics()
                    .color(egui::Color32::from_rgb(200, 200, 255)),
            );

            ui.add_space(20.0);

            // (No personal callouts in public builds)

            ui.add_space(40.0);

            // Link to the real thing
            if ui.link("🔗 Watch the classic").clicked() {
                let _ = open::that("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            }
        });
    });
}

/// Result from rendering a message
struct MessageAction {
    clicked_path: Option<PathBuf>,
    send_to_slack: Option<String>,
}

/// Render a chat message, returning any actions taken
fn render_message(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    dark: bool,
    allowed_dirs: &[String],
) -> MessageAction {
    let is_user = msg.role == "user";
    let mut action = MessageAction {
        clicked_path: None,
        send_to_slack: None,
    };

    if is_user {
        // User message - right aligned, blue
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(8.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(70, 130, 180))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_max_width(500.0);
                    ui.label(
                        egui::RichText::new(&msg.content)
                            .color(egui::Color32::WHITE)
                            .size(15.0),
                    );
                });
        });
    } else {
        // Assistant message - left aligned, with clickable paths
        egui::Frame::none()
            .fill(if dark {
                egui::Color32::from_rgb(50, 50, 58)
            } else {
                egui::Color32::from_rgb(245, 245, 248)
            })
            .rounding(egui::Rounding::same(12.0))
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_max_width(600.0);

                // Check for file paths in the message
                let paths = extract_paths(&msg.content, allowed_dirs);

                let text_color = if dark {
                    egui::Color32::from_rgb(220, 220, 230)
                } else {
                    egui::Color32::from_rgb(40, 40, 50)
                };

                if paths.is_empty() {
                    ui.label(
                        egui::RichText::new(&msg.content)
                            .color(text_color)
                            .size(15.0),
                    );
                } else {
                    // Render text with clickable paths
                    ui.label(
                        egui::RichText::new(&msg.content)
                            .color(text_color)
                            .size(15.0),
                    );

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Files found:").size(12.0).weak());

                    for path in paths {
                        let file_name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();

                        if ui.link(&file_name).clicked() {
                            action.clicked_path = Some(path);
                        }
                    }
                }

                // Action buttons for assistant responses
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .small_button("Copy")
                        .on_hover_text("Copy to clipboard")
                        .clicked()
                    {
                        ui.output_mut(|o| o.copied_text = msg.content.clone());
                    }
                    ui.add_space(8.0);
                    if ui
                        .small_button("Send to Slack")
                        .on_hover_text("Share this response to a Slack channel")
                        .clicked()
                    {
                        action.send_to_slack = Some(msg.content.clone());
                    }
                });
            });
    }

    action
}

/// Render the onboarding screen for first-time users
fn render_onboarding_screen(s: &mut AppState, ctx: &egui::Context) {
    let dark = s.settings.user_profile.dark_mode;

    // Warm color palette
    let warm_orange = egui::Color32::from_rgb(235, 140, 75);
    let _warm_coral = egui::Color32::from_rgb(230, 115, 100);
    let soft_cream = egui::Color32::from_rgb(255, 250, 245);
    let warm_brown = egui::Color32::from_rgb(90, 70, 60);
    let warm_tan = egui::Color32::from_rgb(180, 140, 110);

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(if dark {
                    egui::Color32::from_rgb(35, 30, 28)  // Warm dark brown
                } else {
                    soft_cream
                })
                .inner_margin(egui::Margin::same(40.0)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);

                // Friendly wave emoji as visual warmth
                ui.label(
                    egui::RichText::new("Hey there!")
                        .size(42.0)
                        .strong()
                        .color(warm_orange),
                );

                ui.add_space(8.0);

                // Welcome header with dog icon
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("🐕")
                            .size(32.0)
                    );
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("I'm your Little Helper")
                            .size(24.0)
                            .color(if dark {
                                egui::Color32::from_rgb(240, 235, 230)
                            } else {
                                warm_brown
                            }),
                    );
                });

                ui.add_space(30.0);

                // Form container with warm styling - ask for name FIRST
                egui::Frame::none()
                    .fill(if dark {
                        egui::Color32::from_rgb(50, 45, 42)
                    } else {
                        egui::Color32::WHITE
                    })
                    .rounding(egui::Rounding::same(20.0))
                    .inner_margin(egui::Margin::same(32.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 6.0),
                        blur: 25.0,
                        spread: 0.0,
                        color: egui::Color32::from_rgba_unmultiplied(90, 70, 50, 25),
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);

                        // Name input - ask right away
                        ui.label(
                            egui::RichText::new("What's your name?")
                                .size(15.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 210, 200)
                                } else {
                                    warm_brown
                                }),
                        );
                        ui.add_space(8.0);

                        ui.add_sized(
                            [360.0, 40.0],
                            egui::TextEdit::singleline(&mut s.onboarding_name)
                                .hint_text("Type your name here...")
                                .font(egui::FontId::new(16.0, egui::FontFamily::Proportional)),
                        );

                        ui.add_space(24.0);

                        // Mascot image upload - friendlier
                        ui.label(
                            egui::RichText::new("Want to add a friendly face?")
                                .size(15.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 210, 200)
                                } else {
                                    warm_brown
                                }),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Pick a pet photo or image you love - it'll hang out in the background")
                                .size(13.0)
                                .color(if dark {
                                    warm_tan
                                } else {
                                    egui::Color32::from_rgb(150, 130, 110)
                                }),
                        );
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if let Some(path) = &s.settings.user_profile.mascot_image_path {
                                let file_name = Path::new(path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();
                                ui.label(
                                    egui::RichText::new(format!("Got it: {}", file_name))
                                        .size(13.0)
                                        .color(warm_orange),
                                );
                                if ui.small_button("change").clicked() {
                                    s.settings.user_profile.mascot_image_path = None;
                                }
                            } else {
                                let btn = egui::Button::new(
                                    egui::RichText::new("Browse pictures...")
                                        .size(14.0)
                                        .color(warm_brown),
                                )
                                .fill(egui::Color32::from_rgb(255, 240, 220))
                                .rounding(egui::Rounding::same(8.0));

                                if ui.add(btn).clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                                        .pick_file()
                                    {
                                        s.settings.user_profile.mascot_image_path =
                                            Some(path.to_string_lossy().to_string());
                                    }
                                }

                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("(or skip - there's a cute default!)")
                                        .size(12.0)
                                        .italics()
                                        .color(warm_tan),
                                );
                            }
                        });

                        ui.add_space(30.0);
                        
                        // Show what I can do
                        ui.label(
                            egui::RichText::new("Here's what I can help you with:")
                                .size(14.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 210, 200)
                                } else {
                                    warm_brown
                                }),
                        );
                        ui.add_space(12.0);

                        let features = [
                            ("🔧", "Fix problems", "without touching the terminal"),
                            ("🔍", "Find anything", "files, answers, solutions"),
                            ("📊", "Work with data", "CSV, JSON, spreadsheets"),
                            ("✍️", "Create content", "drafts, ideas, schedules"),
                        ];

                        for (icon, title, desc) in features {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(icon)
                                        .size(16.0),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(title)
                                        .size(13.0)
                                        .strong()
                                        .color(if dark {
                                            egui::Color32::from_rgb(230, 220, 210)
                                        } else {
                                            warm_brown
                                        }),
                                );
                                ui.label(
                                    egui::RichText::new(format!(" - {}", desc))
                                        .size(13.0)
                                        .color(if dark {
                                            warm_tan
                                        } else {
                                            egui::Color32::from_rgb(140, 120, 100)
                                        }),
                                );
                            });
                            ui.add_space(6.0);
                        }

                        ui.add_space(24.0);

                        // Dark mode toggle - friendlier
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Prefer darker colors?")
                                    .size(14.0)
                                    .color(if dark {
                                        egui::Color32::from_rgb(220, 210, 200)
                                    } else {
                                        warm_brown
                                    }),
                            );
                            ui.add_space(8.0);
                            ui.add(egui::widgets::Checkbox::new(
                                &mut s.settings.user_profile.dark_mode,
                                "",
                            ));
                        });

                        ui.add_space(24.0);

                        ui.group(|ui| {
                            ui.label(
                                egui::RichText::new("Privacy preferences")
                                    .size(14.0)
                                    .color(if dark {
                                        egui::Color32::from_rgb(220, 210, 200)
                                    } else {
                                        warm_brown
                                    }),
                            );
                            ui.add_space(6.0);

                            ui.checkbox(
                                &mut s.settings.enable_campaign_context,
                                "Load project materials automatically",
                            );
                            ui.checkbox(
                                &mut s.settings.enable_persona_context,
                                "Load persona files from ~/Process/personas",
                            );
                            ui.checkbox(
                                &mut s.settings.share_system_summary,
                                "Share basic system info with the AI",
                            );
                            ui.checkbox(
                                &mut s.settings.enable_internet_research,
                                "Allow web research (searches and articles)",
                            );
                        });

                        ui.add_space(24.0);

                        ui.group(|ui| {
                            ui.label(
                                egui::RichText::new("Allowed folders")
                                    .size(14.0)
                                    .color(if dark {
                                        egui::Color32::from_rgb(220, 210, 200)
                                    } else {
                                        warm_brown
                                    }),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "Little Helper only works inside these folders.",
                                )
                                .size(12.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(210, 200, 190)
                                } else {
                                    egui::Color32::from_rgb(100, 80, 70)
                                }),
                            );
                            ui.add_space(8.0);
                            
                            // Privacy explanation for local models
                            ui.label(
                                egui::RichText::new(
                                    "💡 If you're using a private local model, it's safe to give full access. \
                                    Your AI never connects to the outside world (except for web searches when you ask). \
                                    The more access you give, the more I can help you find things and get things done!"
                                )
                                .size(11.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(160, 180, 160)
                                } else {
                                    egui::Color32::from_rgb(60, 100, 60)
                                }),
                            );
                            ui.add_space(12.0);

                            if s.settings.allowed_dirs.is_empty() {
                                ui.colored_label(
                                    egui::Color32::from_rgb(230, 120, 120),
                                    "No folders selected yet.",
                                );
                                ui.add_space(4.0);
                            }

                            let mut to_remove: Option<String> = None;
                            for dir in &s.settings.allowed_dirs {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(dir)
                                            .family(egui::FontFamily::Monospace)
                                            .size(12.0),
                                    );
                                    if s.settings.allowed_dirs.len() > 1 {
                                        if ui.small_button("Remove").clicked() {
                                            to_remove = Some(dir.clone());
                                        }
                                    }
                                });
                            }

                            if let Some(target) = to_remove {
                                s.settings.allowed_dirs.retain(|d| d != &target);
                                ensure_allowed_dirs(&mut s.settings);
                            }

                            ui.add_space(4.0);
                            if ui
                                .button("Add folder…")
                                .on_hover_text("Choose a folder Little Helper can access")
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                    let path_str =
                                        path.canonicalize().unwrap_or(path).to_string_lossy().to_string();
                                    if !s.settings.allowed_dirs.contains(&path_str) {
                                        s.settings.allowed_dirs.push(path_str);
                                    }
                                }
                            }
                        });

                        ui.add_space(24.0);

                        ui.group(|ui| {
                            ui.label(
                                egui::RichText::new("Can I help fix and protect your computer?")
                                    .size(14.0)
                                    .color(if dark {
                                        egui::Color32::from_rgb(220, 210, 200)
                                    } else {
                                        warm_brown
                                    }),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "With your permission, I can check for problems, clean up junk, and keep things secure."
                                )
                                .size(12.0)
                                .color(if dark { warm_tan } else { egui::Color32::from_rgb(140, 120, 100) }),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(
                                    "You'll always see what I want to do and approve it first."
                                )
                                .size(12.0)
                                .strong()
                                .color(if dark { warm_tan } else { egui::Color32::from_rgb(140, 120, 100) }),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("• Safe actions like reading files and checking info happen automatically")
                                    .size(11.0)
                                    .color(if dark { warm_tan } else { egui::Color32::from_rgb(140, 120, 100) }),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new("• Changes like moving files show you exactly what I'll do and wait for your OK")
                                    .size(11.0)
                                    .color(if dark { warm_tan } else { egui::Color32::from_rgb(140, 120, 100) }),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new("• Destructive actions like deleting or formatting are blocked unless you specifically allow them")
                                    .size(11.0)
                                    .color(if dark { warm_tan } else { egui::Color32::from_rgb(140, 120, 100) }),
                            );
                            ui.add_space(6.0);
                            let mut permission = s.settings.user_profile.terminal_permission_granted;
                            if ui
                                .checkbox(&mut permission, "Yes, I'd like help keeping my computer safe")
                                .changed()
                            {
                                s.settings.user_profile.terminal_permission_granted = permission;
                            }
                            ui.label(
                                egui::RichText::new("You can change this anytime in settings.")
                                    .size(11.0)
                                    .color(egui::Color32::from_gray(120)),
                            );
                        });

                        ui.add_space(24.0);

                        // Get Started button - warm orange
                        ui.vertical_centered(|ui| {
                            let btn = egui::Button::new(
                                egui::RichText::new("Let's go!")
                                    .size(17.0)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(warm_orange)
                            .rounding(egui::Rounding::same(12.0))
                            .min_size(egui::vec2(220.0, 48.0));

                            if ui.add(btn).clicked() {
                                // Save name to profile
                                if !s.onboarding_name.trim().is_empty() {
                                    s.settings.user_profile.name = s.onboarding_name.trim().to_string();
                                }
                                s.settings.user_profile.onboarding_complete = true;

                                // Update welcome message with user's name - warm and friendly
                                let user_name = if s.settings.user_profile.name.is_empty() {
                                    "friend".to_string()
                                } else {
                                    s.settings.user_profile.name.clone()
                                };
                                let mode = s.current_mode;
                                if let Some(history) = s.mode_chat_histories.get_mut(&mode) {
                                    if let Some(first_msg) = history.first_mut() {
                                        first_msg.content = format!(
                                            "Hey {}! Great to meet you.\n\n\
                                            I'm here whenever you need a hand. Just tell me what you're working on \
                                            and I'll do my best to help out.\n\n\
                                            💡 Tip: Click 'Fix' at the top if you'd like me to run a security check \
                                            and find things to improve on your computer!",
                                            user_name
                                        );
                                    }
                                }

                                // Save settings
                                save_settings(&s.settings);

                                // Switch to chat
                                s.current_screen = AppScreen::Chat;
                            }
                        });
                    });

                ui.add_space(24.0);

                // Skip option - subtle but warm
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("I'll set this up later")
                                .size(13.0)
                                .color(warm_tan),
                        )
                        .frame(false),
                    )
                    .on_hover_text("No worries, you can always change settings later")
                    .clicked()
                {
                    s.settings.user_profile.onboarding_complete = true;
                    save_settings(&s.settings);
                    s.current_screen = AppScreen::Chat;
                }
            });
        });
}

/// Save settings to disk
fn save_settings(settings: &AppSettings) {
    if let Some(path) = config_path() {
        if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
            let _ = fs::write(path, bytes);
        }
    }
}

fn ensure_allowed_dirs(settings: &mut AppSettings) {
    if settings.allowed_dirs.is_empty() {
        if let Some(home) = dirs::home_dir() {
            settings.allowed_dirs = vec![home.to_string_lossy().to_string()];
        }
    }
}

fn normalize_allowed_dir_input(input: &str) -> Option<PathBuf> {
    let expanded = expand_user_path(input);
    let absolute = if expanded.is_absolute() {
        expanded
    } else if let Some(home) = dirs::home_dir() {
        home.join(expanded)
    } else {
        expanded
    };

    if !absolute.exists() {
        return None;
    }

    absolute.canonicalize().ok().or(Some(absolute))
}

// Command validation lives in crates/app/src/utils.rs
