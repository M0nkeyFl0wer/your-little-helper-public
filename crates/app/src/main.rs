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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChatMode {
    Find,     // Help me find something
    Fix,      // Help me fix something
    Research, // Help me research something
    Data,     // Work with data and files
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
    current_mode: ChatMode,
    input_text: String,
    chat_history: Vec<ChatMessage>,
    is_thinking: bool,
    agent_host: AgentHost,

    // Preview panel
    show_preview: bool,
    preview_path: Option<PathBuf>,
    active_viewer: ActiveViewer,
}

impl Default for AppState {
    fn default() -> Self {
        let (settings, _) = load_settings_or_default();
        let welcome_msg = ChatMessage {
            role: "assistant".to_string(),
            content: "Hi! I'm your Little Helper. What would you like me to help you with today?\n\nYou can ask me to find files, fix problems, research topics, or work with data.".to_string(),
            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
        };

        Self {
            settings: settings.clone(),
            current_mode: ChatMode::Find,
            input_text: String::new(),
            chat_history: vec![welcome_msg],
            is_thinking: false,
            agent_host: AgentHost::new(settings),
            show_preview: false,
            preview_path: None,
            active_viewer: ActiveViewer::None,
        }
    }
}

impl AppState {
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
        let system_prompt = match self.current_mode {
            ChatMode::Find => "You are Little Helper, a friendly file-finding assistant. Help users find files on their computer. When you find files, mention their paths so the user can click to preview them.",
            ChatMode::Fix => "You are Little Helper, a friendly tech support assistant. Help users troubleshoot and fix technical problems. Be patient and provide step-by-step solutions.",
            ChatMode::Research => "You are Little Helper, a friendly research assistant. Help users find information and research topics. Be thorough but conversational.",
            ChatMode::Data => "You are Little Helper, a data assistant. Help users work with CSV files, JSON data, and databases. When referencing files, mention their paths so users can preview them.",
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

fn _save_settings(settings: &AppSettings) {
    if let Some(path) = config_path() {
        if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
            let _ = fs::write(path, bytes);
        }
    }
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

        // Set up modern theme
        let mut style = (*ctx.style()).clone();
        style.visuals.window_rounding = egui::Rounding::same(12.0);
        style.visuals.panel_fill = egui::Color32::from_rgb(250, 250, 252);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        ctx.set_style(style);

        // Top header with mode buttons
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(245, 247, 250)))
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.heading(
                        egui::RichText::new("Little Helper")
                            .size(24.0)
                            .color(egui::Color32::from_rgb(60, 60, 80)),
                    );

                    ui.add_space(32.0);

                    // Mode buttons
                    mode_button(ui, "Find", ChatMode::Find, &mut s.current_mode);
                    mode_button(ui, "Fix", ChatMode::Fix, &mut s.current_mode);
                    mode_button(ui, "Research", ChatMode::Research, &mut s.current_mode);
                    mode_button(ui, "Data", ChatMode::Data, &mut s.current_mode);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
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
                        .fill(egui::Color32::from_rgb(255, 255, 255))
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
                    .fill(egui::Color32::from_rgb(250, 250, 252))
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
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
                            if let Some(path) = render_message(ui, msg) {
                                clicked_path = Some(path);
                            }
                            ui.add_space(6.0);
                        }

                        if s.is_thinking {
                            ui.add_space(6.0);
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(245, 245, 248))
                                .rounding(egui::Rounding::same(12.0))
                                .inner_margin(egui::Margin::same(12.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("Thinking...")
                                            .color(egui::Color32::from_rgb(100, 100, 120))
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
fn render_message(ui: &mut egui::Ui, msg: &ChatMessage) -> Option<PathBuf> {
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
            .fill(egui::Color32::from_rgb(245, 245, 248))
            .rounding(egui::Rounding::same(12.0))
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_max_width(600.0);

                // Check for file paths in the message
                let paths = extract_paths(&msg.content);

                if paths.is_empty() {
                    ui.label(
                        egui::RichText::new(&msg.content)
                            .color(egui::Color32::from_rgb(40, 40, 50))
                            .size(15.0),
                    );
                } else {
                    // Render text with clickable paths
                    ui.label(
                        egui::RichText::new(&msg.content)
                            .color(egui::Color32::from_rgb(40, 40, 50))
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
