use agent_host::AgentHost;
use eframe::egui;
use parking_lot::Mutex;
use shared::agent_api::ChatMessage as ApiChatMessage;
use shared::settings::AppSettings;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use viewers::{
    csv_viewer::CsvViewer, image_viewer::ImageViewer, json_viewer::JsonViewer,
    text_viewer::TextViewer, FileType,
};

// Default mascot image (boss's dog!)
const DEFAULT_MASCOT: &[u8] = include_bytes!("../assets/default_mascot.png");

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppScreen {
    Onboarding,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChatMode {
    Find,     // Help me find something
    Fix,      // Help me fix something
    Research, // Deep research session
    Data,     // Work with data and files
    Content,  // Content creation/management
}

#[derive(Clone)]
struct ChatMessage {
    role: String, // "user" or "assistant"
    content: String,
    timestamp: String,
}

/// Active viewer in the preview panel
enum ActiveViewer {
    None,
    Text(TextViewer),
    Image(ImageViewer),
    Csv(CsvViewer),
    Json(JsonViewer),
}

struct AppState {
    settings: AppSettings,
    current_screen: AppScreen,
    current_mode: ChatMode,
    input_text: String,
    chat_history: Vec<ChatMessage>,
    is_thinking: bool,
    agent_host: AgentHost,

    // Preview panel
    show_preview: bool,
    preview_path: Option<PathBuf>,
    active_viewer: ActiveViewer,

    // Onboarding
    onboarding_name: String,

    // Background mascot texture
    mascot_texture: Option<egui::TextureHandle>,
    mascot_loaded: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let (settings, _) = load_settings_or_default();
        let needs_onboarding = !settings.user_profile.onboarding_complete;

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
            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
        };

        Self {
            settings: settings.clone(),
            current_screen: if needs_onboarding {
                AppScreen::Onboarding
            } else {
                AppScreen::Chat
            },
            current_mode: ChatMode::Find,
            input_text: String::new(),
            chat_history: vec![welcome_msg],
            is_thinking: false,
            agent_host: AgentHost::new(settings),
            show_preview: false,
            preview_path: None,
            active_viewer: ActiveViewer::None,
            onboarding_name: String::new(),
            mascot_texture: None,
            mascot_loaded: false,
        }
    }
}

impl AppState {
    /// Load the mascot image as a texture (custom or default)
    fn load_mascot_texture(&mut self, ctx: &egui::Context) {
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
        let image_data = image_result.or_else(|| image::load_from_memory(DEFAULT_MASCOT).ok());

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
    fn reload_mascot_texture(&mut self, ctx: &egui::Context) {
        self.mascot_loaded = false;
        self.mascot_texture = None;
        self.load_mascot_texture(ctx);
    }

    fn send_message(&mut self) {
        if self.input_text.trim().is_empty() {
            return;
        }

        // Add user message to chat
        let user_msg = ChatMessage {
            role: "user".to_string(),
            content: self.input_text.clone(),
            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
        };
        self.chat_history.push(user_msg);

        // Clear input and show thinking state
        let _query = self.input_text.clone();
        self.input_text.clear();
        self.is_thinking = true;

        // Prepare context based on current mode
        let user_name = if self.settings.user_profile.name.is_empty() {
            "friend".to_string()
        } else {
            self.settings.user_profile.name.clone()
        };

        let system_prompt = match self.current_mode {
            ChatMode::Find => format!(
                "You are Little Helper, a friendly assistant helping {}. Help find files on their computer. When you find files, mention their paths so they can click to preview them.",
                user_name
            ),
            ChatMode::Fix => format!(
                "You are Little Helper, a friendly tech support assistant helping {}. Help troubleshoot and fix technical problems. Be patient and provide step-by-step solutions.",
                user_name
            ),
            ChatMode::Research => format!(
                "You are Little Helper in DEEP RESEARCH mode, helping {}. Conduct thorough, comprehensive research on topics. Ask clarifying questions, explore multiple angles, cite sources, and provide detailed analysis. This is for serious research work.",
                user_name
            ),
            ChatMode::Data => format!(
                "You are Little Helper, a data assistant helping {}. Help work with CSV files, JSON data, and databases. When referencing files, mention their paths so they can preview them.",
                user_name
            ),
            ChatMode::Content => format!(
                "You are Little Helper in CONTENT CREATION mode, helping {}. Help create, manage, and schedule content. You can help draft posts, review content calendars, and manage publishing workflows.",
                user_name
            ),
        };

        // Convert chat history to API format
        let mut api_messages = vec![ApiChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        }];

        // Add recent chat history (last 10 messages to keep context manageable)
        let recent_messages = self.chat_history.iter().rev().take(10).rev();
        for msg in recent_messages {
            api_messages.push(ApiChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        // Start async AI generation
        self.start_ai_generation(api_messages);
    }

    fn start_ai_generation(&mut self, messages: Vec<ApiChatMessage>) {
        let rt = tokio::runtime::Runtime::new().unwrap();

        match rt.block_on(self.agent_host.chat(messages)) {
            Ok(response) => {
                let assistant_msg = ChatMessage {
                    role: "assistant".to_string(),
                    content: response,
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                };
                self.chat_history.push(assistant_msg);
            }
            Err(e) => {
                let error_msg = ChatMessage {
                    role: "assistant".to_string(),
                    content: format!("Sorry, I'm having trouble connecting. Error: {}", e),
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                };
                self.chat_history.push(error_msg);
            }
        }

        self.is_thinking = false;
    }

    /// Open a file in the preview panel
    fn open_file(&mut self, path: &Path, ctx: &egui::Context) {
        let file_type = FileType::from_path(path);

        match file_type {
            FileType::Text | FileType::Markdown | FileType::Unknown => {
                let mut viewer = TextViewer::new();
                if viewer.load(path).is_ok() {
                    self.active_viewer = ActiveViewer::Text(viewer);
                    self.preview_path = Some(path.to_path_buf());
                    self.show_preview = true;
                }
            }
            FileType::Image => {
                let mut viewer = ImageViewer::new();
                if viewer.load(path, ctx).is_ok() {
                    self.active_viewer = ActiveViewer::Image(viewer);
                    self.preview_path = Some(path.to_path_buf());
                    self.show_preview = true;
                }
            }
            FileType::Csv => {
                let mut viewer = CsvViewer::new();
                if viewer.load(path).is_ok() {
                    self.active_viewer = ActiveViewer::Csv(viewer);
                    self.preview_path = Some(path.to_path_buf());
                    self.show_preview = true;
                }
            }
            FileType::Json => {
                let mut viewer = JsonViewer::new();
                if viewer.load(path).is_ok() {
                    self.active_viewer = ActiveViewer::Json(viewer);
                    self.preview_path = Some(path.to_path_buf());
                    self.show_preview = true;
                }
            }
            _ => {
                // Unsupported type - try as text
                let mut viewer = TextViewer::new();
                if viewer.load(path).is_ok() {
                    self.active_viewer = ActiveViewer::Text(viewer);
                    self.preview_path = Some(path.to_path_buf());
                    self.show_preview = true;
                }
            }
        }
    }

    fn close_preview(&mut self) {
        self.show_preview = false;
        self.preview_path = None;
        self.active_viewer = ActiveViewer::None;
    }
}

/// Extract file paths from text
fn extract_paths(text: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Match absolute paths like /home/user/file.txt or ~/file.txt
    // Match paths like /home/user/file.txt or ~/file.txt
    let path_regex = regex::Regex::new(r#"(?:^|[\s"'(])([~/][^\s"'()]+\.[a-zA-Z0-9]+)"#).unwrap();

    for cap in path_regex.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let path_str = m.as_str();
            // Expand ~ to home directory
            let expanded = if path_str.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&path_str[2..])
                } else {
                    PathBuf::from(path_str)
                }
            } else {
                PathBuf::from(path_str)
            };

            if expanded.exists() {
                paths.push(expanded);
            }
        }
    }

    paths
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
                    return (s, false);
                }
            }
        }
    }
    let mut default_settings = AppSettings::default();
    default_settings.allowed_dirs = vec![];
    default_settings.enable_internet_research = true;
    (default_settings, true)
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
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

        // Set up theme (dark or light mode)
        let mut style = (*ctx.style()).clone();
        style.visuals.window_rounding = egui::Rounding::same(12.0);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);

        if s.settings.user_profile.dark_mode {
            style.visuals = egui::Visuals::dark();
            style.visuals.panel_fill = egui::Color32::from_rgb(30, 30, 35);
        } else {
            style.visuals.panel_fill = egui::Color32::from_rgb(250, 250, 252);
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

                    // Mode buttons
                    mode_button(ui, "Find", ChatMode::Find, &mut s.current_mode);
                    mode_button(ui, "Fix", ChatMode::Fix, &mut s.current_mode);
                    mode_button(ui, "Research", ChatMode::Research, &mut s.current_mode);
                    mode_button(ui, "Data", ChatMode::Data, &mut s.current_mode);
                    mode_button(ui, "Content", ChatMode::Content, &mut s.current_mode);

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

                        ui.add_space(8.0);

                        if s.show_preview {
                            if ui.button("Close Preview").clicked() {
                                s.close_preview();
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
                    // Header with file name
                    ui.horizontal(|ui| {
                        if let Some(path) = &s.preview_path {
                            ui.label(
                                egui::RichText::new(
                                    path.file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .to_string(),
                                )
                                .size(16.0)
                                .strong(),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").clicked() {
                                s.close_preview();
                            }
                        });
                    });
                    ui.separator();

                    // Render active viewer
                    match &mut s.active_viewer {
                        ActiveViewer::None => {
                            ui.centered_and_justified(|ui| {
                                ui.label("No file open");
                            });
                        }
                        ActiveViewer::Text(viewer) => viewer.ui(ui),
                        ActiveViewer::Image(viewer) => viewer.ui(ui),
                        ActiveViewer::Csv(viewer) => viewer.ui(ui),
                        ActiveViewer::Json(viewer) => viewer.ui(ui),
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
                // Paint mascot as watermark FIRST (background layer)
                let panel_rect = ui.max_rect();
                if let Some(texture) = &s.mascot_texture {
                    let tex_size = texture.size_vec2();

                    // Scale to fit nicely - about 30% of panel width
                    let max_width = panel_rect.width() * 0.30;
                    let max_height = panel_rect.height() * 0.40;
                    let scale = (max_width / tex_size.x).min(max_height / tex_size.y);
                    let img_size = tex_size * scale;

                    // Position in bottom-right, above where input box will be
                    let pos = egui::pos2(
                        panel_rect.right() - img_size.x - 30.0,
                        panel_rect.bottom() - img_size.y - 90.0,
                    );
                    let rect = egui::Rect::from_min_size(pos, img_size);

                    // Very subtle watermark - won't obstruct text
                    ui.painter().image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_white_alpha(20), // Very faint
                    );
                }

                // Chat messages scroll area
                let chat_height = ui.available_height() - 70.0;

                let mut clicked_path: Option<PathBuf> = None;

                egui::ScrollArea::vertical()
                    .max_height(chat_height)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &s.chat_history {
                            ui.add_space(6.0);
                            if let Some(path) = render_message(ui, msg, dark) {
                                clicked_path = Some(path);
                            }
                            ui.add_space(6.0);
                        }

                        if s.is_thinking {
                            ui.add_space(6.0);
                            egui::Frame::none()
                                .fill(if dark {
                                    egui::Color32::from_rgb(50, 50, 58)
                                } else {
                                    egui::Color32::from_rgb(245, 245, 248)
                                })
                                .rounding(egui::Rounding::same(12.0))
                                .inner_margin(egui::Margin::same(12.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("Thinking...")
                                            .color(if dark {
                                                egui::Color32::from_rgb(160, 160, 180)
                                            } else {
                                                egui::Color32::from_rgb(100, 100, 120)
                                            })
                                            .italics(),
                                    );
                                });
                        }
                    });

                // Handle clicked path after iteration
                if let Some(path) = clicked_path {
                    s.open_file(&path, ctx);
                }

                ui.add_space(8.0);

                // Input area
                ui.horizontal(|ui| {
                    let hint = match s.current_mode {
                        ChatMode::Find => "What would you like me to find?",
                        ChatMode::Fix => "What needs fixing?",
                        ChatMode::Research => "What should I research?",
                        ChatMode::Data => "What data would you like to work with?",
                        ChatMode::Content => "What content would you like to create?",
                    };

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
    }
}

fn mode_button(ui: &mut egui::Ui, label: &str, mode: ChatMode, current: &mut ChatMode) {
    let is_selected = *current == mode;
    let btn = egui::Button::new(egui::RichText::new(label).size(14.0).color(if is_selected {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(70, 70, 90)
    }))
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

/// Render a chat message, returning any clicked file path
fn render_message(ui: &mut egui::Ui, msg: &ChatMessage, dark: bool) -> Option<PathBuf> {
    let is_user = msg.role == "user";
    let mut clicked_path: Option<PathBuf> = None;

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
                let paths = extract_paths(&msg.content);

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
                            clicked_path = Some(path);
                        }
                    }
                }
            });
    }

    clicked_path
}

/// Render the onboarding screen for first-time users
fn render_onboarding_screen(s: &mut AppState, ctx: &egui::Context) {
    let dark = s.settings.user_profile.dark_mode;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(if dark {
                    egui::Color32::from_rgb(25, 25, 30)
                } else {
                    egui::Color32::from_rgb(250, 250, 252)
                })
                .inner_margin(egui::Margin::same(40.0)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);

                // Welcome header
                ui.label(
                    egui::RichText::new("Welcome to Little Helper!")
                        .size(32.0)
                        .strong()
                        .color(if dark {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(50, 50, 70)
                        }),
                );

                ui.add_space(12.0);

                ui.label(
                    egui::RichText::new("Your friendly AI assistant for finding files, fixing problems, and getting things done.")
                        .size(16.0)
                        .color(if dark {
                            egui::Color32::from_rgb(180, 180, 190)
                        } else {
                            egui::Color32::from_rgb(100, 100, 120)
                        }),
                );

                ui.add_space(50.0);

                // Form container
                egui::Frame::none()
                    .fill(if dark {
                        egui::Color32::from_rgb(40, 40, 48)
                    } else {
                        egui::Color32::WHITE
                    })
                    .rounding(egui::Rounding::same(16.0))
                    .inner_margin(egui::Margin::same(32.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 4.0),
                        blur: 20.0,
                        spread: 0.0,
                        color: egui::Color32::from_black_alpha(20),
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(400.0);

                        // Name input
                        ui.label(
                            egui::RichText::new("What should I call you?")
                                .size(14.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(200, 200, 210)
                                } else {
                                    egui::Color32::from_rgb(80, 80, 100)
                                }),
                        );
                        ui.add_space(8.0);

                        ui.add_sized(
                            [350.0, 36.0],
                            egui::TextEdit::singleline(&mut s.onboarding_name)
                                .hint_text("Your name")
                                .font(egui::FontId::new(16.0, egui::FontFamily::Proportional)),
                        );

                        ui.add_space(24.0);

                        // Mascot image upload (optional)
                        ui.label(
                            egui::RichText::new("Add a mascot image (optional)")
                                .size(14.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(200, 200, 210)
                                } else {
                                    egui::Color32::from_rgb(80, 80, 100)
                                }),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("A pet photo or favorite image to personalize your assistant")
                                .size(12.0)
                                .weak(),
                        );
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if let Some(path) = &s.settings.user_profile.mascot_image_path {
                                let file_name = Path::new(path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();
                                ui.label(
                                    egui::RichText::new(format!("Selected: {}", file_name))
                                        .size(13.0)
                                        .color(egui::Color32::from_rgb(70, 130, 180)),
                                );
                                if ui.small_button("Clear").clicked() {
                                    s.settings.user_profile.mascot_image_path = None;
                                }
                            } else {
                                if ui
                                    .button(egui::RichText::new("Choose Image...").size(14.0))
                                    .clicked()
                                {
                                    // Open file dialog
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                                        .pick_file()
                                    {
                                        s.settings.user_profile.mascot_image_path =
                                            Some(path.to_string_lossy().to_string());
                                    }
                                }
                            }
                        });

                        ui.add_space(24.0);

                        // Dark mode toggle
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Dark mode")
                                    .size(14.0)
                                    .color(if dark {
                                        egui::Color32::from_rgb(200, 200, 210)
                                    } else {
                                        egui::Color32::from_rgb(80, 80, 100)
                                    }),
                            );
                            ui.add_space(8.0);
                            if ui
                                .add(egui::widgets::Checkbox::new(
                                    &mut s.settings.user_profile.dark_mode,
                                    "",
                                ))
                                .changed()
                            {
                                // Theme will update on next frame
                            }
                        });

                        ui.add_space(32.0);

                        // Get Started button
                        ui.vertical_centered(|ui| {
                            let btn = egui::Button::new(
                                egui::RichText::new("Get Started")
                                    .size(16.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(70, 130, 180))
                            .rounding(egui::Rounding::same(10.0))
                            .min_size(egui::vec2(200.0, 44.0));

                            if ui.add(btn).clicked() {
                                // Save name to profile
                                if !s.onboarding_name.trim().is_empty() {
                                    s.settings.user_profile.name = s.onboarding_name.trim().to_string();
                                }
                                s.settings.user_profile.onboarding_complete = true;

                                // Update welcome message with user's name
                                let user_name = if s.settings.user_profile.name.is_empty() {
                                    "friend".to_string()
                                } else {
                                    s.settings.user_profile.name.clone()
                                };
                                if let Some(first_msg) = s.chat_history.first_mut() {
                                    first_msg.content = format!(
                                        "Hi {}! I'm your Little Helper. What would you like me to help you with today?\n\n\
                                        You can ask me to find files, fix problems, do deep research, work with data, or create content.",
                                        user_name
                                    );
                                }

                                // Save settings
                                save_settings(&s.settings);

                                // Switch to chat
                                s.current_screen = AppScreen::Chat;
                            }
                        });
                    });

                ui.add_space(20.0);

                // Skip option
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Skip for now")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(120, 120, 140)),
                        )
                        .frame(false),
                    )
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
