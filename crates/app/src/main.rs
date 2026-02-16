#![allow(dead_code)]

use agent_host::CommandResult;
use eframe::egui;
use parking_lot::Mutex;
use services::version_control::VersionControlService;
use shared::preview_types::PreviewContent;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::utils::{save_settings, ensure_allowed_dirs};


fn try_read_clipboard_text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

pub(crate) fn set_primary_provider_preference(pref: &mut Vec<String>, primary: &str) {
    let mut out: Vec<String> = Vec::new();

    let add = |out: &mut Vec<String>, p: &str| {
        if !out.iter().any(|x| x == p) {
            out.push(p.to_string());
        }
    };

    add(&mut out, primary);
    add(&mut out, "local"); // Always keep a local fallback
    add(&mut out, "anthropic");
    add(&mut out, "openai");
    add(&mut out, "gemini");

    *pref = out;
}


// Default mascot image (boss's dog!)
pub(crate) const DEFAULT_MASCOT: &[u8] = include_bytes!("../assets/default_mascot.png");

// Pre-loaded secrets (gitignored secrets.rs, or empty for CI builds)
#[allow(dead_code)]
mod secrets;
// Support contact info (gitignored - your personal contact stays private)
#[allow(dead_code)]
mod support_info;
use support_info::{SUPPORT_BUTTON_TEXT, SUPPORT_LINK};

// Bundled Ollama lifecycle management
pub(crate) mod ollama_manager;

// Interactive Preview Companion modules
mod ascii_art;
mod modals;
#[allow(dead_code)]
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
mod simple_md;

mod state;
pub use state::*;

/// Extract file paths from text
fn extract_paths(text: &str, allowed_dirs: &[String]) -> Vec<PathBuf> {
    use std::sync::OnceLock;
    static PATH_RE: OnceLock<regex::Regex> = OnceLock::new();
    let path_regex = PATH_RE.get_or_init(|| {
        regex::Regex::new(r#"(?:^|[\s"'(])([~/][^\s"'()]+\.[a-zA-Z0-9]+)"#).unwrap()
    });

    let mut paths = Vec::new();

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

// config_path, load_settings_or_default, save_settings, ensure_allowed_dirs
// are all defined in crate::utils and imported above.

/// Clean up AI response by removing action tags
fn clean_ai_response(response: &str) -> String {
    use std::sync::OnceLock;
    static RE_PREVIEW: OnceLock<regex::Regex> = OnceLock::new();
    static RE_SEARCH: OnceLock<regex::Regex> = OnceLock::new();
    static RE_COMMAND: OnceLock<regex::Regex> = OnceLock::new();

    let re_preview = RE_PREVIEW.get_or_init(|| {
        regex::Regex::new(r"(?s)<preview[^>]*>.*?</preview>").unwrap()
    });
    let re_search = RE_SEARCH.get_or_init(|| {
        regex::Regex::new(r"(?s)<search>.*?</search>").unwrap()
    });
    let re_command = RE_COMMAND.get_or_init(|| {
        regex::Regex::new(r"(?s)<command>.*?</command>").unwrap()
    });

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

        // Poll for AI response and live status/stream updates (non-blocking)
        s.poll_ai_stream();
        s.poll_ai_status();
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

            // Show Matrix animation only after 3+ seconds of thinking (avoids flicker on fast responses)
            if let Some(mode) = s.thinking_mode {
                let elapsed = s.thinking_started_at
                    .get(&mode)
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);
                if elapsed >= 5
                    && !matches!(s.active_viewer, ActiveViewer::Matrix | ActiveViewer::RickRoll)
                {
                    s.active_viewer = ActiveViewer::Matrix;
                }
            }
        }

        // Detect mode change and show mode introduction
        let mode_changed = s.previous_mode.map_or(false, |prev| prev != s.current_mode);
        if mode_changed {
            // Dismiss the "try one!" hint on first mode switch
            s.show_mode_picker_hint = false;

            // Save current input text and preview state for the old mode
            if let Some(prev_mode) = s.previous_mode {
                if !s.input_text.is_empty() {
                    let draft = s.input_text.clone();
                    s.mode_input_drafts.insert(prev_mode, draft);
                }
                // Save preview state for the old mode
                let saved_viewer = s.active_viewer.clone();
                let saved_content = s.preview_panel.current_content();
                s.mode_preview_state.insert(prev_mode, (saved_viewer, saved_content));
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

            // Restore preview state for the new mode (or show mode intro)
            let mode_str = s.current_mode.as_str();
            if let Some((viewer, content)) = s.mode_preview_state.get(&new_mode).cloned() {
                s.active_viewer = viewer;
                if let Some(c) = content {
                    s.preview_panel.show_content(c);
                } else {
                    s.preview_panel.show_mode_intro(mode_str);
                }
            } else {
                s.active_viewer = ActiveViewer::Panel;
                s.preview_panel.show_mode_intro(mode_str);
            }

            // First-time Fix intro: Doc introduces itself and offers a health scan
            if s.current_mode == ChatMode::Fix && !s.fix_intro_shown {
                s.fix_intro_shown = true;
                let user_name = if s.settings.user_profile.name.is_empty() {
                    "friend".to_string()
                } else {
                    s.settings.user_profile.name.clone()
                };
                s.push_chat(ChatMessage {
                    role: "assistant".to_string(),
                    content: format!(
                        "Hey {}! I'm Doc — your tech support buddy.\n\n\
                        I can check your computer's health, find what's using up resources, \
                        and help fix things — all in plain English, no jargon.\n\n\
                        Want me to run a quick health check? I'll look at:\n\
                        - How your CPU and memory are doing\n\
                        - Disk space\n\
                        - Anything that looks off\n\n\
                        Just say **\"run a health check\"** or ask me about anything that's bugging you!",
                        user_name
                    ),
                    details: None,
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                });
            }

            // First-time Spec intro happens when the user opens the tab.
            if s.current_mode == ChatMode::Build && !s.spec_intro_shown {
                s.spec_intro_shown = true;

                if !s.settings.user_profile.onboarding_complete {
                    s.settings.user_profile.onboarding_complete = true;
                    save_settings(&s.settings);
                }

                s.show_preview = true;
                s.active_viewer = ActiveViewer::Panel;
                s.preview_panel.show_mode_intro("build");

                // Auto-switch to a cloud provider for Build mode
                let primary = s.settings.model.provider_preference
                    .first().map(|p| p.as_str()).unwrap_or("local");

                if primary == "local" {
                    // Try to auto-switch: prefer gemini (free tier) > openai > anthropic
                    let cloud_pick = if s.settings.model.gemini_auth.has_auth() {
                        Some(("gemini", "Gemini"))
                    } else if s.settings.model.openai_auth.has_auth() {
                        Some(("openai", "OpenAI"))
                    } else if s.settings.model.anthropic_auth.has_auth() {
                        Some(("anthropic", "Anthropic"))
                    } else {
                        None
                    };

                    if let Some((id, name)) = cloud_pick {
                        set_primary_provider_preference(
                            &mut s.settings.model.provider_preference, id,
                        );
                        save_settings(&s.settings);
                        s.push_chat(ChatMessage {
                            role: "assistant".to_string(),
                            content: format!(
                                "Hi! I'm Spec — the builder.\n\n\
                                Build mode works best with a cloud model, so I've switched to **{}**. \
                                Tell me what you want to build and I'll run Spec Kit under the hood!",
                                name
                            ),
                            details: None,
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                    } else {
                        // No cloud provider configured — nudge the user
                        s.show_settings_dialog = true;
                        s.push_chat(ChatMessage {
                            role: "assistant".to_string(),
                            content: "Hi! I'm Spec — the builder.\n\n\
                                Build mode needs a cloud model to drive the spec workflow \
                                (local models aren't reliable enough for multi-step planning).\n\n\
                                I've opened **Settings** so you can connect one:\n\n\
                                **Easiest — Google Gemini** (free tier)\n\
                                Sign in with Google, no API key needed.\n\
                                Minimum: `gemini-2.5-flash` | Best: `gemini-2.5-pro`\n\n\
                                **OpenAI**\n\
                                Paste an API key from platform.openai.com.\n\
                                Minimum: `gpt-4o-mini` | Best: `gpt-4o`\n\n\
                                **Anthropic**\n\
                                Paste an API key from console.anthropic.com.\n\
                                Minimum: `claude-sonnet-4-20250514` | Best: `claude-opus-4-5-20251101`\n\n\
                                Once you've added a key, come back here and we'll start building!"
                                .to_string(),
                            details: None,
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                    }
                } else {
                    s.push_chat(ChatMessage {
                        role: "assistant".to_string(),
                        content: "Hi! I'm Spec — the builder. Tell me what you want to build and I'll run Spec Kit Assistant under the hood (terminal superpowers, with guardrails).".to_string(),
                        details: None,
                        timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                    });
                }
            }
            
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

        // Poll for background Ollama setup completion
        s.poll_ollama_setup();

        // Poll for background OAuth flow completion
        s.poll_oauth_result();

        // Show onboarding for first-run users
        if s.current_screen == AppScreen::Onboarding {
            render_onboarding_screen(&mut s, ctx);
            return;
        }

        // Load mascot texture if not already loaded
        s.load_mascot_texture(ctx);

        let dark = s.settings.user_profile.dark_mode;

        if let Some(mode) = s.thinking_mode {
            if let Some(started_at) = s.thinking_started_at.get(&mode) {
                let shown = s.slow_response_hint_shown.get(&mode).copied().unwrap_or(false);
                let is_local = s
                    .settings
                    .model
                    .provider_preference
                    .first()
                    .map(|p| p == "local")
                    .unwrap_or(true);
                // CPU-only local gets nudged fast (8s) — cloud gets more patience (25s)
                let threshold = if is_local { 8 } else { 25 };
                if !shown && started_at.elapsed() >= Duration::from_secs(threshold) {
                    s.slow_response_hint_shown.insert(mode, true);
                    s.show_model_hint = true;
                    s.model_hint_started_at = Some(std::time::Instant::now());

                    let tip_message = if is_local {
                        "Local AI is thinking hard! It runs privately on your \
                        computer, which can be slow without a powerful GPU. \
                        For faster and smarter answers, switch to a cloud \
                        provider in Settings (click the model name above)."
                    } else {
                        "This is taking longer than usual. The server might be \
                        busy — your request is still processing."
                    };
                    s.preview_panel
                        .show_tip_if_idle("Feeling slow?", tip_message);
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
                    let header_name = if s.settings.user_profile.name.trim().is_empty() {
                        "Little Helper".to_string()
                    } else {
                        format!("{}'s Little Helper", s.settings.user_profile.name.trim())
                    };

                    ui.heading(
                        egui::RichText::new(header_name)
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

                    // Pulsing highlight when hint is active
                    let hint_active = s.show_mode_picker_hint;
                    let time = ui.input(|i| i.time);

                    let (picker_stroke, picker_shadow) = if hint_active {
                        let p = ((time * 3.0).sin() + 1.0) as f32 / 2.0; // 0..1 f32
                        let width = 2.0 + p * 1.5;
                        let alpha = (160.0 + p * 95.0) as u8;
                        let glow_color = egui::Color32::from_rgba_unmultiplied(235, 140, 75, alpha);
                        let shadow = egui::epaint::Shadow {
                            offset: egui::vec2(0.0, 0.0),
                            blur: 12.0 + p * 8.0,
                            spread: 2.0,
                            color: egui::Color32::from_rgba_unmultiplied(235, 140, 75, (80.0 + p * 60.0) as u8),
                        };
                        (egui::Stroke::new(width, glow_color), shadow)
                    } else {
                        let stroke_color = if dark {
                            egui::Color32::from_rgb(50, 50, 58)
                        } else {
                            egui::Color32::from_rgb(210, 215, 225)
                        };
                        (egui::Stroke::new(1.0, stroke_color), egui::epaint::Shadow::NONE)
                    };

                    let picker_response = egui::Frame::none()
                        .fill(if dark {
                            egui::Color32::from_rgb(30, 30, 36)
                        } else {
                            egui::Color32::from_rgb(235, 238, 243)
                        })
                        .rounding(egui::Rounding::same(10.0))
                        .stroke(picker_stroke)
                        .shadow(picker_shadow)
                        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;

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
                        });

                    // Paint a floating arrow + label below the mode picker
                    if hint_active {
                        let picker_rect = picker_response.response.rect;
                        let painter = ui.painter();
                        let p = ((time * 3.0).sin() + 1.0) as f32 / 2.0;
                        let bounce = p * 4.0; // 0–4px vertical bounce
                        let alpha = (180.0 + p * 75.0) as u8;
                        let arrow_color = egui::Color32::from_rgba_unmultiplied(235, 140, 75, alpha);

                        // Arrow triangle pointing up at the mode picker
                        let arrow_center_x = picker_rect.center().x;
                        let arrow_top_y = picker_rect.bottom() + 4.0 + bounce;
                        let arrow_size = 10.0;
                        let triangle = vec![
                            egui::pos2(arrow_center_x, arrow_top_y),
                            egui::pos2(arrow_center_x - arrow_size, arrow_top_y + arrow_size),
                            egui::pos2(arrow_center_x + arrow_size, arrow_top_y + arrow_size),
                        ];
                        painter.add(egui::Shape::convex_polygon(
                            triangle,
                            arrow_color,
                            egui::Stroke::NONE,
                        ));

                        // Label below the arrow
                        let label_pos = egui::pos2(
                            arrow_center_x,
                            arrow_top_y + arrow_size + 4.0,
                        );
                        painter.text(
                            label_pos,
                            egui::Align2::CENTER_TOP,
                            "Pick a helper to get started!",
                            egui::FontId::new(14.0, egui::FontFamily::Proportional),
                            arrow_color,
                        );

                        ctx.request_repaint(); // keep animating
                    }

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

                        // Keep the top bar compact; preview controls live on the preview panel.
                        if !s.show_preview {
                            if ui
                                .small_button("◂ Preview")
                                .on_hover_text("Show the preview panel")
                                .clicked()
                            {
                                s.show_preview = true;
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
                .default_width(420.0)
                .min_width(300.0)
                .max_width(520.0)
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
                            if ui.small_button("Hide").clicked() {
                                s.close_preview();
                            }
                            // Only show close button if not the welcome view
                            if !matches!(s.active_viewer, ActiveViewer::Panel) {
                                if ui.small_button("Back").clicked() {
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

                            if ui
                                .small_button("Versions")
                                .on_hover_text("See earlier versions and restore")
                                .clicked()
                            {
                                let root = path.parent().unwrap_or(path.as_path());
                                if let Ok(vc) = VersionControlService::new(root) {
                                    if let Ok(versions) = vc.list_versions(&path) {
                                        let file_name = path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "file".to_string());
                                        s.preview_panel.show_content(PreviewContent::VersionHistory {
                                            file_path: path.clone(),
                                            file_name,
                                            versions,
                                        });
                                    }
                                }
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

        // Thread history panel (left side, toggled)
        // Auto-hide on narrow windows to avoid squishing the chat
        let window_width = ctx.screen_rect().width();
        if s.show_thread_history && window_width < 700.0 {
            s.show_thread_history = false;
        }
        if s.show_thread_history {
            egui::SidePanel::left("thread_history_panel")
                .default_width(260.0)
                .min_width(200.0)
                .max_width(320.0)
                .frame(
                    egui::Frame::none()
                        .fill(if dark {
                            egui::Color32::from_rgb(30, 30, 38)
                        } else {
                            egui::Color32::from_rgb(248, 248, 252)
                        })
                        .inner_margin(egui::Margin::same(10.0)),
                )
                .show(ctx, |ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.heading(
                            egui::RichText::new("History")
                                .size(16.0)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").clicked() {
                                s.show_thread_history = false;
                            }
                        });
                    });
                    ui.add_space(6.0);

                    // Search input
                    ui.add_sized(
                        [ui.available_width(), 28.0],
                        egui::TextEdit::singleline(&mut s.thread_search_query)
                            .hint_text("Search conversations...")
                            .font(egui::FontId::new(13.0, egui::FontFamily::Proportional)),
                    );
                    ui.add_space(6.0);

                    // Mode filter chips
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let filters: &[(Option<ChatMode>, &str)] = &[
                            (None, "All"),
                            (Some(ChatMode::Find), "Find"),
                            (Some(ChatMode::Fix), "Fix"),
                            (Some(ChatMode::Research), "Research"),
                            (Some(ChatMode::Data), "Data"),
                            (Some(ChatMode::Content), "Content"),
                            (Some(ChatMode::Build), "Build"),
                        ];
                        for (filter_mode, label) in filters {
                            let selected = s.thread_history_mode_filter == *filter_mode;
                            let text = egui::RichText::new(*label).size(11.0);
                            let text = if selected { text.strong() } else { text };
                            if ui.selectable_label(selected, text).clicked() {
                                s.thread_history_mode_filter = *filter_mode;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Get filtered threads
                    let threads: Vec<_> = if !s.thread_search_query.trim().is_empty() {
                        s.thread_history
                            .search(&s.thread_search_query)
                            .into_iter()
                            .filter(|t| {
                                s.thread_history_mode_filter
                                    .map_or(true, |m| t.mode == m)
                            })
                            .map(|t| (t.id.clone(), t.title.clone(), t.mode, t.message_count, t.is_pinned, t.last_activity, t.last_message_preview.clone()))
                            .collect()
                    } else {
                        let all = match s.thread_history_mode_filter {
                            Some(mode) => s.thread_history.get_threads_by_mode(mode),
                            None => s.thread_history.get_all_threads(),
                        };
                        all.into_iter()
                            .map(|t| (t.id.clone(), t.title.clone(), t.mode, t.message_count, t.is_pinned, t.last_activity, t.last_message_preview.clone()))
                            .collect()
                    };

                    // Thread list
                    if threads.is_empty() {
                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new("No conversations yet")
                                .size(13.0)
                                .weak(),
                        );
                        ui.label(
                            egui::RichText::new("Start chatting and your threads will appear here.")
                                .size(11.0)
                                .weak(),
                        );
                    } else {
                        let subtle = if dark {
                            egui::Color32::from_rgb(130, 130, 150)
                        } else {
                            egui::Color32::from_rgb(100, 100, 120)
                        };
                        let accent = if dark {
                            egui::Color32::from_rgb(120, 160, 255)
                        } else {
                            egui::Color32::from_rgb(50, 100, 200)
                        };

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut thread_to_load: Option<String> = None;
                            let mut thread_to_pin: Option<String> = None;

                            for (id, title, mode, msg_count, pinned, last_activity, preview) in &threads {
                                let mode_icon = match mode {
                                    ChatMode::Find => "🔎",
                                    ChatMode::Fix => "🔧",
                                    ChatMode::Research => "🔬",
                                    ChatMode::Data => "📊",
                                    ChatMode::Content => "✍️",
                                    ChatMode::Build => "🐶",
                                };

                                let is_current = s.current_thread_id.as_deref() == Some(id.as_str());
                                let bg = if is_current {
                                    if dark {
                                        egui::Color32::from_rgb(45, 45, 60)
                                    } else {
                                        egui::Color32::from_rgb(230, 235, 250)
                                    }
                                } else {
                                    egui::Color32::TRANSPARENT
                                };

                                egui::Frame::none()
                                    .fill(bg)
                                    .rounding(egui::Rounding::same(6.0))
                                    .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                                    .show(ui, |ui| {
                                        let response = ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                if *pinned {
                                                    ui.label(egui::RichText::new("📌").size(10.0));
                                                }
                                                ui.label(egui::RichText::new(mode_icon).size(12.0));
                                                let title_text = egui::RichText::new(title)
                                                    .size(13.0)
                                                    .color(if is_current { accent } else if dark {
                                                        egui::Color32::from_rgb(220, 220, 230)
                                                    } else {
                                                        egui::Color32::from_rgb(30, 30, 40)
                                                    });
                                                ui.label(title_text);
                                            });
                                            ui.horizontal(|ui| {
                                                let time_str = crate::thread_history::format_time_ago_pub(*last_activity);
                                                ui.label(
                                                    egui::RichText::new(format!("{} msgs · {}", msg_count, time_str))
                                                        .size(11.0)
                                                        .color(subtle),
                                                );
                                            });
                                            // Preview snippet
                                            if !preview.is_empty() {
                                                let snip: String = preview.chars().take(60).collect();
                                                ui.label(
                                                    egui::RichText::new(snip)
                                                        .size(10.0)
                                                        .weak()
                                                        .italics(),
                                                );
                                            }
                                        });

                                        // Click to load
                                        if response.response.interact(egui::Sense::click()).clicked() {
                                            thread_to_load = Some(id.clone());
                                        }

                                        // Right-click to pin
                                        if response.response.interact(egui::Sense::click()).secondary_clicked() {
                                            thread_to_pin = Some(id.clone());
                                        }
                                    });
                                ui.add_space(2.0);
                            }

                            if let Some(id) = thread_to_load {
                                s.load_thread(&id);
                            }
                            if let Some(id) = thread_to_pin {
                                s.thread_history.toggle_pin(&id);
                                s.thread_history.save_to_disk();
                            }
                        });
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
                    // In light mode, a white frame + low-alpha image reads as a "white out".
                    // Use a soft shadow frame and a slightly stronger image tint.
                    let frame_fill = if dark {
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 26)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 12)
                    };
                    ui.painter().rect_filled(frame_rect, rounding, frame_fill);

                    // Wallpaper image (low alpha so chat stays readable)
                    let wallpaper_tint = if dark {
                        egui::Color32::from_white_alpha(30)
                    } else {
                        egui::Color32::from_white_alpha(72)
                    };

                    ui.painter().image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        wallpaper_tint,
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
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 28)
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
                        // Save current thread before clearing
                        let mode = s.current_mode;
                        s.sync_thread_history(mode);
                        s.current_thread_id = None;

                        let user_name = if s.settings.user_profile.name.is_empty() {
                            "friend"
                        } else {
                            &s.settings.user_profile.name
                        };
                        let welcome = ChatMessage {
                            role: "assistant".to_string(),
                            content: format!(
                                "Starting a fresh conversation! How can I help you today, {}?",
                                user_name
                            ),
                            details: None,
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        };
                        s.mode_chat_histories.insert(mode, vec![welcome]);
                        s.preview_panel.show_mode_intro(mode.as_str());
                    }

                    // History toggle
                    let history_label = if s.show_thread_history { "Hide History" } else { "History" };
                    if ui
                        .small_button(history_label)
                        .on_hover_text("Browse past conversations")
                        .clicked()
                    {
                        s.show_thread_history = !s.show_thread_history;
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
                                details: None,
                                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                            };
                            s.mode_chat_histories.insert(mode, vec![welcome]);
                            s.preview_panel.show_mode_intro(mode.as_str());
                        }
                    });
                });

                ui.add_space(4.0);

                // Build mode: minimal setup bar (folder + project name)
                if s.current_mode == ChatMode::Build {
                    egui::Frame::none()
                        .fill(if dark {
                            egui::Color32::from_rgb(35, 35, 45)
                        } else {
                            egui::Color32::from_rgb(245, 245, 250)
                        })
                        .rounding(egui::Rounding::same(4.0))
                        .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let label_color = if dark {
                                    egui::Color32::from_rgb(140, 140, 160)
                                } else {
                                    egui::Color32::from_rgb(90, 90, 110)
                                };
                                ui.label(egui::RichText::new("Folder:").size(11.0).color(label_color));
                                ui.add_sized(
                                    [ui.available_width() * 0.55, 18.0],
                                    egui::TextEdit::singleline(&mut s.build_folder_input)
                                        .hint_text("~/Projects/my-app")
                                        .font(egui::FontId::new(11.0, egui::FontFamily::Monospace)),
                                );
                                ui.label(egui::RichText::new("Name:").size(11.0).color(label_color));
                                ui.add_sized(
                                    [ui.available_width(), 18.0],
                                    egui::TextEdit::singleline(&mut s.build_project_name_input)
                                        .hint_text("my-app")
                                        .font(egui::FontId::new(11.0, egui::FontFamily::Monospace)),
                                );
                            });
                        });
                    ui.add_space(2.0);
                }

                // Chat messages scroll area
                let chat_height = ui.available_height() - 100.0;

                let mut clicked_path: Option<PathBuf> = None;
                // Slack is not included in the public edition

                // Handover notification: show if another mode is processing
                if let Some(thinking_mode) = s.thinking_mode {
                    if thinking_mode != s.current_mode && s.is_thinking.get(&thinking_mode).copied().unwrap_or(false) {
                        let mode_name = match thinking_mode {
                            ChatMode::Find => "Find Helper",
                            ChatMode::Fix => "Fix Helper",
                            ChatMode::Research => "Research Helper",
                            ChatMode::Data => "Data Helper",
                            ChatMode::Content => "Content Helper",
                            ChatMode::Build => "Build",
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
                let streaming_text = s.streaming_partial.get(&current_mode).cloned().unwrap_or_default();

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
                            // Slack sharing not supported (public edition)
                            ui.add_space(6.0);
                        }

                        if is_thinking {
                            ui.add_space(6.0);
                            if !streaming_text.is_empty() {
                                // Render streaming partial response as a live assistant message
                                let text_color = if dark {
                                    egui::Color32::from_rgb(220, 220, 230)
                                } else {
                                    egui::Color32::from_rgb(30, 30, 40)
                                };
                                egui::Frame::none()
                                    .fill(if dark {
                                        egui::Color32::from_rgb(50, 50, 58)
                                    } else {
                                        egui::Color32::from_rgb(230, 230, 235)
                                    })
                                    .rounding(egui::Rounding::same(12.0))
                                    .inner_margin(egui::Margin::same(12.0))
                                    .show(ui, |ui| {
                                        // Show the streamed text with a blinking cursor
                                        let time = ui.input(|i| i.time);
                                        let cursor = if ((time * 2.0) as i32) % 2 == 0 {
                                            "\u{258C}" // left half block as blinking cursor
                                        } else {
                                            " "
                                        };
                                        let display_text = format!("{}{}", streaming_text, cursor);
                                        crate::simple_md::render_markdown(ui, &display_text, text_color);
                                    });
                            } else {
                                // Show animated thinking dots
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
                            }
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
                // (no-op)

                ui.add_space(8.0);

                if !s.pending_commands.is_empty() {
                    ui.group(|ui| {
                        ui.label(
                            egui::RichText::new("I'd like to run something on your computer:")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 150, 80)),
                        );
                        ui.add_space(6.0);
                        let pending = s.pending_commands.clone();
                        for cmd in pending {
                            let friendly = friendly_command_description(&cmd);
                            ui.label(&friendly);
                            ui.horizontal(|ui| {
                                if ui.button("Allow").clicked() {
                                    s.approve_command(cmd.clone());
                                }
                                if ui.button("Skip").clicked() {
                                    s.pending_commands.retain(|c| c != &cmd);
                                }
                                // Show technical detail on hover
                                ui.weak("ℹ").on_hover_text(format!("Technical: {}", cmd));
                            });
                        }
                    });
                    ui.add_space(8.0);
                }

                // Input area
                ui.horizontal(|ui| {
                    let is_busy = s.thinking_mode == Some(s.current_mode)
                        && s
                            .is_thinking
                            .get(&s.current_mode)
                            .copied()
                            .unwrap_or(false);

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

                    // Re-focus the input after AI replies so the user can keep typing
                    if s.refocus_input {
                        s.refocus_input = false;
                        response.request_focus();
                    }

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        s.send_message();
                    }

                    let btn = if is_busy {
                        egui::Button::new("Stop").fill(egui::Color32::from_rgb(180, 80, 80))
                    } else {
                        egui::Button::new("Send").fill(egui::Color32::from_rgb(70, 130, 180))
                    };
                    if ui.add_sized([70.0, 40.0], btn).clicked() {
                        if is_busy {
                            let mode = s.current_mode;
                            s.cancel_ai(mode);
                        } else {
                            s.send_message();
                        }
                    }
                });
            });

        // Password dialog for sudo commands
        {
            use crate::modals::{Modal, ModalResult};
            s.password_dialog.update(ctx);
            let result = s.password_dialog.take_result();
            match result {
                ModalResult::Confirmed(password) => {
                    if let Some(cmd) = s.pending_sudo_command.take() {
                        s.execute_sudo_command(cmd, password);
                    }
                }
                ModalResult::Cancelled => {
                    s.pending_sudo_command = None;
                    let mode = s.current_mode;
                    s.is_thinking.insert(mode, false);
                    s.thinking_status.insert(mode, String::new());
                    s.push_chat(ChatMessage {
                        role: "assistant".to_string(),
                        content: "No worries — I skipped that command. Let me know if you'd like to try a different approach.".to_string(),
                        details: None,
                        timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                    });
                }
                ModalResult::Pending => {}
            }
        }

        // Settings dialog
        if s.show_settings_dialog {
            let mut open = true;
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                open = false;
            }

            let mut wants_close = false;
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(true)
                .open(&mut open)
                // Center the window for stable resizing behavior
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(460.0);
                    ui.set_max_height(640.0);

                    ui.horizontal(|ui| {
                        ui.heading(
                            egui::RichText::new("Settings").color(if dark {
                                egui::Color32::from_rgb(220, 220, 230)
                            } else {
                                egui::Color32::from_rgb(40, 40, 50)
                            }),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Done").clicked() {
                                wants_close = true;
                            }
                        });
                    });

                    ui.add_space(8.0);

                    // Superpowers (always visible)
                    ui.heading(
                        egui::RichText::new("Superpowers")
                            .color(if dark {
                                egui::Color32::from_rgb(220, 220, 230)
                            } else {
                                egui::Color32::from_rgb(40, 40, 50)
                            }),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Turn this on to let me run safe terminal commands for you (I’ll still ask before anything risky).",
                        )
                        .color(if dark {
                            egui::Color32::from_rgb(180, 180, 190)
                        } else {
                            egui::Color32::from_rgb(80, 80, 90)
                        }),
                    );
                    if ui
                        .checkbox(
                            &mut s.settings.user_profile.terminal_permission_granted,
                            "Enable terminal superpowers",
                        )
                        .changed()
                    {
                        save_settings(&s.settings);
                        s.settings_status = Some("Saved superpowers setting".to_string());
                        s.settings_status_is_error = false;
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Scrollable content
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // ── Status messages (shared across sections) ──
                        if let Some(msg) = &s.settings_status {
                            let color = if s.settings_status_is_error {
                                egui::Color32::from_rgb(200, 120, 120)
                            } else {
                                egui::Color32::from_rgb(120, 200, 150)
                            };
                            ui.colored_label(color, msg);
                            ui.add_space(6.0);
                        }

                        // ── Privacy & Permissions (always visible, friendly) ──
                        ui.label(
                            egui::RichText::new("What I'm allowed to do")
                                .size(15.0)
                                .strong()
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 220, 230)
                                } else {
                                    egui::Color32::from_rgb(40, 40, 50)
                                }),
                        );
                        ui.add_space(6.0);

                        let mut needs_save = false;

                        if ui
                            .checkbox(
                                &mut s.settings.share_system_summary,
                                "Share basic system info with the AI",
                            )
                            .on_hover_text("Helps me give better answers about your computer")
                            .changed()
                        {
                            needs_save = true;
                        }
                        if ui
                            .checkbox(
                                &mut s.settings.enable_internet_research,
                                "Allow web research",
                            )
                            .on_hover_text("I can search the web and read articles for you")
                            .changed()
                        {
                            needs_save = true;
                        }

                        // Brave Search API key
                        if s.settings.enable_internet_research {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Search API key:");
                                let mut key = s.settings.brave_search_api_key.clone().unwrap_or_default();
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut key)
                                        .password(true)
                                        .hint_text("Brave Search API key (free at brave.com/search/api)")
                                        .desired_width(200.0),
                                );
                                if resp.changed() {
                                    s.settings.brave_search_api_key = if key.is_empty() { None } else { Some(key) };
                                    needs_save = true;
                                }
                            });
                            if s.settings.brave_search_api_key.is_none() {
                                ui.label(
                                    egui::RichText::new("Without an API key, search uses Wikipedia only. Get a free key at brave.com/search/api")
                                        .size(11.0)
                                        .weak()
                                );
                            }
                        }

                        if needs_save {
                            save_settings(&s.settings);
                            s.settings_status = Some("Saved".to_string());
                            s.settings_status_is_error = false;
                        }

                        ui.add_space(4.0);

                        // ── AI Provider (just the dropdown — friendly) ──
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("AI brain")
                                .size(15.0)
                                .strong()
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 220, 230)
                                } else {
                                    egui::Color32::from_rgb(40, 40, 50)
                                }),
                        );
                        ui.add_space(4.0);

                        let providers = [
                            ("local", "Local (Ollama) — free, private"),
                            ("openai", "OpenAI (GPT-4)"),
                            ("anthropic", "Anthropic (Claude)"),
                            ("gemini", "Google (Gemini)"),
                        ];

                        let current_provider = s
                            .settings
                            .model
                            .provider_preference
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "local".to_string());

                        let provider_label = providers
                            .iter()
                            .find(|(id, _)| *id == current_provider.as_str())
                            .map(|(_, name)| *name)
                            .unwrap_or("Local (Ollama) — free, private");

                        egui::ComboBox::from_label("")
                            .selected_text(provider_label)
                            .width(280.0)
                            .show_ui(ui, |ui| {
                                for (id, name) in providers {
                                    if ui.selectable_label(current_provider == id, name).clicked() {
                                        set_primary_provider_preference(
                                            &mut s.settings.model.provider_preference,
                                            id,
                                        );
                                        save_settings(&s.settings);
                                        s.settings_status = Some(format!("Switched to {}", name));
                                        s.settings_status_is_error = false;
                                    }
                                }
                            });

                        // Contextual explanation based on selected provider
                        let is_cloud = current_provider != "local";
                        let subtle = if dark {
                            egui::Color32::from_rgb(160, 160, 170)
                        } else {
                            egui::Color32::from_rgb(100, 100, 110)
                        };

                        ui.add_space(6.0);
                        if !is_cloud {
                            // Local provider — explain what "local" means
                            ui.label(
                                egui::RichText::new(
                                    "Runs entirely on your computer. Nothing you type \
                                    leaves your machine — your conversations stay private \
                                    and it works without internet."
                                )
                                .size(12.0)
                                .color(subtle),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("  ● Running locally — no API key needed")
                                    .color(egui::Color32::from_rgb(0, 180, 0))
                                    .size(12.0),
                            );
                        } else {
                            // Cloud provider — explain what that means
                            ui.label(
                                egui::RichText::new(
                                    "Cloud providers are smarter but your messages are \
                                    sent to their servers. They require an API key \
                                    (a password for their service) and may cost money \
                                    per use."
                                )
                                .size(12.0)
                                .color(subtle),
                            );
                            let has_auth = match current_provider.as_str() {
                                "openai" => s.settings.model.openai_auth.has_auth(),
                                "anthropic" => s.settings.model.anthropic_auth.has_auth(),
                                "gemini" => s.settings.model.gemini_auth.has_auth(),
                                _ => false,
                            };
                            let has_oauth = match current_provider.as_str() {
                                "gemini" => s.settings.model.gemini_auth.oauth.is_some(),
                                _ => false,
                            };
                            ui.add_space(4.0);
                            if has_oauth {
                                ui.label(
                                    egui::RichText::new("  ● Signed in with Google")
                                        .color(egui::Color32::from_rgb(0, 180, 0))
                                        .size(12.0),
                                );
                            } else if has_auth {
                                ui.label(
                                    egui::RichText::new("  ● API key saved")
                                        .color(egui::Color32::from_rgb(0, 180, 0))
                                        .size(12.0),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("  ○ No API key — paste one below or sign in")
                                        .color(egui::Color32::from_rgb(220, 160, 80))
                                        .size(12.0),
                                );
                            }

                            // ── Keys (visible, right after provider picker) ──
                            ui.add_space(8.0);
                            let key_name = match current_provider.as_str() {
                                "openai" => "OpenAI",
                                "anthropic" => "Anthropic",
                                "gemini" => "Gemini",
                                _ => "",
                            };

                            ui.horizontal(|ui| {
                                let input_field = match current_provider.as_str() {
                                    "openai" => &mut s.openai_api_key_input,
                                    "anthropic" => &mut s.anthropic_api_key_input,
                                    "gemini" => &mut s.gemini_api_key_input,
                                    _ => &mut s.openai_api_key_input,
                                };

                                ui.add(
                                    egui::TextEdit::singleline(input_field)
                                        .password(true)
                                        .hint_text(format!("Paste {} key...", key_name))
                                        .desired_width(200.0),
                                );

                                if ui.button("📋").on_hover_text("Paste from clipboard").clicked() {
                                    if let Some(text) = try_read_clipboard_text() {
                                        *input_field = text;
                                    }
                                }

                                let save_clicked = ui.button("Save").clicked();
                                if save_clicked && !input_field.is_empty() {
                                    let key_value = input_field.clone();
                                    input_field.clear();
                                    match current_provider.as_str() {
                                        "openai" => {
                                            s.settings.model.openai_auth.api_key = Some(key_value);
                                        }
                                        "anthropic" => {
                                            s.settings.model.anthropic_auth.api_key = Some(key_value);
                                        }
                                        "gemini" => {
                                            s.settings.model.gemini_auth.api_key = Some(key_value);
                                        }
                                        _ => {}
                                    }
                                    save_settings(&s.settings);
                                    s.settings_status = Some(format!("{} key saved", key_name));
                                    s.settings_status_is_error = false;
                                }
                            });
                            // ── Google Sign-In (Gemini only, requires client ID) ──
                            let google_creds = crate::secrets::google_oauth_credentials();
                            if current_provider == "gemini" && google_creds.is_some() {
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("— or —")
                                            .size(11.0)
                                            .weak(),
                                    );
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    let btn_enabled = !s.oauth_in_progress;
                                    let btn_text = if s.oauth_in_progress {
                                        "Waiting for browser..."
                                    } else if s.settings.model.gemini_auth.oauth.is_some() {
                                        "Re-sign in with Google"
                                    } else {
                                        "Sign in with Google"
                                    };
                                    let btn = egui::Button::new(
                                        egui::RichText::new(btn_text)
                                            .size(13.0),
                                    );
                                    if ui.add_enabled(btn_enabled, btn).clicked() {
                                        // Start OAuth flow in background
                                        let (tx, rx) = std::sync::mpsc::channel();
                                        s.oauth_result_rx = Some(rx);
                                        s.oauth_in_progress = true;
                                        std::thread::spawn(move || {
                                            let (client_id, client_secret) = google_creds.clone().unwrap();
                                            let rt = tokio::runtime::Builder::new_current_thread()
                                                .enable_all()
                                                .build();
                                            let result = match rt {
                                                Ok(rt) => rt.block_on(async {
                                                    let flow = providers::oauth_helper::OAuthFlow::new(
                                                        client_id,
                                                        client_secret,
                                                        "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                                                        "https://oauth2.googleapis.com/token".to_string(),
                                                        vec![
                                                            "https://www.googleapis.com/auth/generative-language.retriever".to_string(),
                                                        ],
                                                    )?;
                                                    flow.authenticate().await
                                                }),
                                                Err(e) => Err(anyhow::anyhow!("Failed to start runtime: {}", e)),
                                            };
                                            match result {
                                                Ok((access_token, refresh_token)) => {
                                                    let _ = tx.send(crate::types::OAuthResult {
                                                        provider: "gemini".to_string(),
                                                        access_token,
                                                        refresh_token,
                                                        error: None,
                                                    });
                                                }
                                                Err(e) => {
                                                    let _ = tx.send(crate::types::OAuthResult {
                                                        provider: "gemini".to_string(),
                                                        access_token: String::new(),
                                                        refresh_token: None,
                                                        error: Some(e.to_string()),
                                                    });
                                                }
                                            }
                                        });
                                    }
                                    if s.oauth_in_progress {
                                        ui.spinner();
                                    }
                                });
                                ui.label(
                                    egui::RichText::new(
                                        "Sign in with your Google account — no API key needed."
                                    )
                                    .size(11.0)
                                    .weak(),
                                );
                            }

                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Stored locally, never shared.")
                                        .size(11.0)
                                        .weak(),
                                );
                                ui.add_space(8.0);
                                ui.add(egui::Hyperlink::from_label_and_url(
                                    egui::RichText::new("Get free API keys")
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(100, 170, 240)),
                                    "https://openrouter.ai/keys",
                                ));
                            });
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // ── Advanced (collapsible — folders, build tools, performance) ──
                        let adv_header = egui::RichText::new("Advanced")
                            .size(14.0)
                            .color(if dark {
                                egui::Color32::from_rgb(160, 160, 170)
                            } else {
                                egui::Color32::from_rgb(100, 100, 110)
                            });
                        egui::CollapsingHeader::new(adv_header)
                            .default_open(false)
                            .show(ui, |ui| {

                        // ── Allowed Folders ──
                        ui.label(
                            egui::RichText::new("Allowed folders")
                                .size(13.0)
                                .strong()
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 220, 230)
                                } else {
                                    egui::Color32::from_rgb(40, 40, 50)
                                }),
                        );
                        ui.label(
                            egui::RichText::new("I can only see files inside these folders.")
                                .size(12.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(160, 160, 170)
                                } else {
                                    egui::Color32::from_rgb(100, 100, 110)
                                }),
                        );
                        ui.add_space(4.0);

                        if s.settings.allowed_dirs.is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(220, 120, 120),
                                "No folders allowed. Add at least one.",
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

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let text_edit = egui::TextEdit::singleline(&mut s.new_allowed_dir)
                                .hint_text("~/Documents or /data/projects");
                            ui.add(text_edit);
                            if ui.button("Add").clicked() {
                                let input = s.new_allowed_dir.trim();
                                if input.is_empty() {
                                    s.settings_status =
                                        Some("Enter a folder path first.".to_string());
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
                                            Some("Already in the list.".to_string());
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

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // ── Build tools ──
                        ui.label(
                            egui::RichText::new("Build tools")
                                .size(13.0)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("Spec Kit is bundled. Override path only if needed.")
                                .size(11.0)
                                .weak(),
                        );
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut s.spec_kit_path_input)
                                    .desired_width(260.0),
                            );
                            if ui.button("Use default").clicked() {
                                if let Some(home) = dirs::home_dir() {
                                    s.spec_kit_path_input = home
                                        .join("Projects/spec-kit-assistant/archive/legacy-node/spec-assistant.js")
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
                                s.settings_status = Some("Saved build path".to_string());
                                s.settings_status_is_error = false;
                            }
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // ── Status / Performance ──
                        ui.label(
                            egui::RichText::new("Performance")
                                .size(13.0)
                                .strong(),
                        );
                        s.update_settings_perf();
                        let ctx_hint = s.model_context_hint_tokens();
                        let comfort_total: f32 = 8000.0;
                        let used = s.last_prompt_tokens_est as f32;
                        let ratio = (used / comfort_total).clamp(0.0, 1.0);

                        egui::Grid::new("settings_status_grid")
                            .num_columns(2)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("CPU");
                                ui.label(format!("{:.0}%", s.settings_cpu_percent));
                                ui.end_row();
                                ui.label("Memory");
                                ui.label(format!("{} MB", s.settings_mem_mb));
                                ui.end_row();
                                ui.label("Context");
                                ui.label(format!("~{} tokens", ctx_hint));
                                ui.end_row();
                            });

                        ui.add_space(4.0);
                        ui.add(
                            egui::ProgressBar::new(ratio)
                                .show_percentage()
                                .text(format!(
                                    "~{} / 8000 tokens",
                                    s.last_prompt_tokens_est
                                )),
                        );

                        if ratio > 0.85 {
                            ui.label(
                                egui::RichText::new(
                                    "Older messages may be trimmed to make room."
                                )
                                .size(11.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(220, 180, 100)
                                } else {
                                    egui::Color32::from_rgb(160, 120, 60)
                                }),
                            );
                        }
                            }); // end Advanced collapsing header

                        ui.add_space(10.0);
                    });
                });

            if wants_close {
                open = false;
            }
            s.show_settings_dialog = open;
        }

        // Slack is not included in the public edition
    }
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
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 0)
        })
        .rounding(egui::Rounding::same(9.0));

    if ui.add_sized([90.0, 32.0], btn).clicked() {
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
                "Build",
                vec![
                    "Start a new project from an idea",
                    "Run Spec Kit Assistant without touching the terminal",
                    "Watch progress in chat; open Details if you need logs",
                    "Try: \"start a new project called photo-sorter\"",
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
            // Slack is not included in the public edition
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

/// Translate a raw shell command into a plain-English description for non-technical users.
fn friendly_command_description(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let base = parts.first().copied().unwrap_or("");
    // Strip leading sudo
    let (is_sudo, effective) = if base == "sudo" {
        (true, parts.get(1).copied().unwrap_or(""))
    } else {
        (false, base)
    };
    let prefix = if is_sudo { "Install/update (needs admin): " } else { "" };

    let desc = match effective {
        "apt" | "apt-get" | "dnf" | "pacman" | "yum" | "zypper" =>
            "Update or install software packages".to_string(),
        "systemctl" => format!("Manage a system service ({})",
            parts.last().unwrap_or(&"service")),
        "find" => "Search for files on your computer".to_string(),
        "grep" | "rg" => "Search inside files for text".to_string(),
        "ls" | "dir" => "List files in a folder".to_string(),
        "cat" | "less" | "head" | "tail" | "bat" => "Read a file".to_string(),
        "cp" => "Copy a file".to_string(),
        "mv" => "Move or rename a file".to_string(),
        "rm" => "Delete a file".to_string(),
        "mkdir" => "Create a new folder".to_string(),
        "chmod" | "chown" => "Change file permissions".to_string(),
        "df" => "Check disk space".to_string(),
        "du" => "Check folder sizes".to_string(),
        "top" | "htop" | "btop" => "Show running processes".to_string(),
        "ps" => "List running processes".to_string(),
        "kill" | "killall" => "Stop a running program".to_string(),
        "ping" => "Test network connection".to_string(),
        "curl" | "wget" => "Download something from the web".to_string(),
        "uname" => "Check system info".to_string(),
        "free" => "Check memory usage".to_string(),
        "lsblk" | "fdisk" | "blkid" => "Check disk/drive info".to_string(),
        "ip" | "ifconfig" => "Check network settings".to_string(),
        "mount" | "umount" => "Mount or unmount a drive".to_string(),
        _ => format!("Run: {}", cmd),
    };
    format!("{}{}", prefix, desc)
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
            egui::ScrollArea::both()
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
                    simple_md::render_markdown(ui, &msg.content, text_color);
                } else {
                    // Render text with clickable paths + markdown
                    simple_md::render_markdown(ui, &msg.content, text_color);

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Files mentioned:").size(12.0).weak());

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
                        let copied = if let Some(details) = &msg.details {
                            format!("{}\n\nDetails:\n{}", msg.content, details)
                        } else {
                            msg.content.clone()
                        };
                        ui.output_mut(|o| o.copied_text = copied);
                    }
                    // Slack sharing is not included in the public edition
                });

                if let Some(details) = &msg.details {
                    ui.add_space(6.0);
                    let details_color = if dark {
                        egui::Color32::from_rgb(150, 150, 165)
                    } else {
                        egui::Color32::from_rgb(110, 110, 125)
                    };
                    egui::CollapsingHeader::new("Details")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(details)
                                    .monospace()
                                    .size(12.0)
                                    .color(details_color),
                            );
                        });
                }
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
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
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

                // Welcome header with dog ASCII (bigger + on-brand)
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    let time = ui.input(|i| i.time);
                    let dog = crate::ascii_art::get_ascii_art_animated(
                        shared::preview_types::AsciiState::Welcome,
                        time,
                    );
                    ui.label(
                        egui::RichText::new(dog)
                            .monospace()
                            .size(16.0)
                            .color(if dark {
                                egui::Color32::from_rgb(210, 205, 200)
                            } else {
                                egui::Color32::from_rgb(90, 70, 60)
                            }),
                    );
                    ui.add_space(16.0);
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
                            ("🔎", "Find things", "files, photos, and docs"),
                            ("🔧", "Fix problems", "with safe, guided steps"),
                            ("🔬", "Research", "with sources and previews"),
                            ("🐶", "Build", "projects with Spec Kit"),
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
                                egui::RichText::new("Privacy & permissions")
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
                                    "Right now I run on your computer using a local AI — \
                                    nothing you type leaves your machine. You can switch \
                                    to a cloud AI later in Settings (smarter, but sends \
                                    data to their servers).",
                                )
                                .size(12.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(180, 170, 160)
                                } else {
                                    egui::Color32::from_rgb(120, 105, 90)
                                }),
                            );
                            ui.add_space(8.0);

                            // Keep onboarding minimal. Folder access and "help fix" permissions
                            // are handled inside Find/Fix when needed.
                            ui.checkbox(
                                &mut s.settings.share_system_summary,
                                "Share basic system info with the AI",
                            );
                            ui.checkbox(
                                &mut s.settings.enable_internet_research,
                                "Allow web research (searches and articles)",
                            );
                            ui.checkbox(
                                &mut s.settings.user_profile.terminal_permission_granted,
                                "Enable terminal superpowers (recommended)",
                            );
                            ui.label(
                                egui::RichText::new(
                                    "Tip: Safe commands run automatically. I'll still ask before anything risky.",
                                )
                                .size(11.0)
                                .weak(),
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
                                // Show "try one!" hint on the mode picker
                                s.show_mode_picker_hint = true;

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
        });
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
