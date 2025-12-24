use eframe::egui;
use parking_lot::Mutex;
use shared::settings::AppSettings;
use shared::agent_api::ChatMessage as ApiChatMessage;
use agent_host::AgentHost;
use std::fs;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChatMode { 
    Find,     // Help me find something
    Fix,      // Help me fix something  
    Research, // Help me research something
}

#[derive(Clone)]
struct ChatMessage {
    role: String,    // "user" or "assistant"
    content: String,
    timestamp: String,
}

struct AppState {
    settings: AppSettings,
    current_mode: ChatMode,
    input_text: String,
    chat_history: Vec<ChatMessage>,
    is_thinking: bool,
    agent_host: AgentHost,
}

impl Default for AppState {
    fn default() -> Self {
        let (settings, _) = load_settings_or_default();
        let welcome_msg = ChatMessage {
            role: "assistant".to_string(),
            content: "Hi! 🌸 I'm your Little Helper. What would you like me to help you with today?".to_string(),
            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
        };
        
        Self {
            settings: settings.clone(),
            current_mode: ChatMode::Find,
            input_text: String::new(),
            chat_history: vec![welcome_msg],
            is_thinking: false,
            agent_host: AgentHost::new(settings),
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
        let query = self.input_text.clone();
        self.input_text.clear();
        self.is_thinking = true;
        
        // Prepare context based on current mode
        let system_prompt = match self.current_mode {
            ChatMode::Find => "You are Little Helper, a friendly file-finding assistant. Help users find files on their computer, including mounted drives like Google Drive and SSH connections. Be conversational and ask follow-up questions to better understand what they're looking for.",
            ChatMode::Fix => "You are Little Helper, a friendly tech support assistant. Help users troubleshoot and fix technical problems. Be patient, ask clarifying questions, and provide step-by-step solutions.",
            ChatMode::Research => "You are Little Helper, a friendly research assistant. Help users find information and research topics. Be thorough but conversational, and ask what specific aspects they'd like to explore.",
        };
        
        // Convert chat history to API format
        let mut api_messages = vec![
            ApiChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            }
        ];
        
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
        // For now, we'll use a simple blocking approach
        // In a real app, you'd want to do this async properly
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
                    content: format!("Sorry, I'm having trouble connecting to the AI model. Make sure Ollama is running locally. Error: {}", e),
                    timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                };
                self.chat_history.push(error_msg);
            }
        }
        
        self.is_thinking = false;
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    if let Some(proj) = directories::ProjectDirs::from("com.local", "Little Helper", "LittleHelper") {
        let p = proj.config_dir().join("settings.json");
        let _ = fs::create_dir_all(proj.config_dir());
        Some(p)
    } else { None }
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
    // Enable full access by default, but user can customize
    default_settings.allowed_dirs = vec![];
    default_settings.enable_internet_research = true;
    (default_settings, true)
}

fn save_settings(settings: &AppSettings) {
    if let Some(path) = config_path() {
        if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
            let _ = fs::write(path, bytes);
        }
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Little Helper - Your AI Assistant",
        options,
        Box::new(|_cc| Box::new(LittleHelperApp { state: Arc::new(Mutex::new(AppState::default())) })),
    )
}

struct LittleHelperApp {
    state: Arc<Mutex<AppState>>,
}

impl eframe::App for LittleHelperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut s = self.state.lock();
        
        // Set up modern, rounded theme
        let mut style = (*ctx.style()).clone();
        style.visuals.window_rounding = egui::Rounding::same(16.0);
        style.visuals.panel_fill = egui::Color32::from_rgb(248, 250, 255);
        style.visuals.window_fill = egui::Color32::from_rgb(255, 253, 250);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(240, 248, 255);
        
        // Bigger text sizes
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(28.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        );
        
        ctx.set_style(style);

        // Top header with mode buttons  
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            // Custom background
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                egui::Rounding::ZERO,
                egui::Color32::from_rgb(230, 240, 255)
            );
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.heading(egui::RichText::new("🌸 Little Helper")
                        .size(32.0)
                        .color(egui::Color32::from_rgb(102, 51, 153)));
                    
                    ui.add_space(40.0);
                    
                    // Mode selection buttons - much bigger and modern
                    if ui.add_sized([140.0, 50.0], egui::Button::new(
                        egui::RichText::new("🔍 Find")
                            .size(18.0)
                            .color(if matches!(s.current_mode, ChatMode::Find) { 
                                egui::Color32::WHITE 
                            } else { 
                                egui::Color32::from_rgb(51, 102, 153) 
                            })
                    ).fill(if matches!(s.current_mode, ChatMode::Find) {
                        egui::Color32::from_rgb(51, 102, 153)
                    } else {
                        egui::Color32::from_rgb(240, 248, 255)
                    }).rounding(egui::Rounding::same(12.0))).clicked() {
                        s.current_mode = ChatMode::Find;
                    }
                    
                    ui.add_space(12.0);
                    
                    if ui.add_sized([140.0, 50.0], egui::Button::new(
                        egui::RichText::new("🔧 Fix")
                            .size(18.0)
                            .color(if matches!(s.current_mode, ChatMode::Fix) { 
                                egui::Color32::WHITE 
                            } else { 
                                egui::Color32::from_rgb(51, 153, 102) 
                            })
                    ).fill(if matches!(s.current_mode, ChatMode::Fix) {
                        egui::Color32::from_rgb(51, 153, 102)
                    } else {
                        egui::Color32::from_rgb(248, 255, 248)
                    }).rounding(egui::Rounding::same(12.0))).clicked() {
                        s.current_mode = ChatMode::Fix;
                    }
                    
                    ui.add_space(12.0);
                    
                    if ui.add_sized([160.0, 50.0], egui::Button::new(
                        egui::RichText::new("🔌 Research")
                            .size(18.0)
                            .color(if matches!(s.current_mode, ChatMode::Research) { 
                                egui::Color32::WHITE 
                            } else { 
                                egui::Color32::from_rgb(153, 51, 102) 
                            })
                    ).fill(if matches!(s.current_mode, ChatMode::Research) {
                        egui::Color32::from_rgb(153, 51, 102)
                    } else {
                        egui::Color32::from_rgb(255, 248, 250)
                    }).rounding(egui::Rounding::same(12.0))).clicked() {
                        s.current_mode = ChatMode::Research;
                    }
                });
                ui.add_space(20.0);
            });

        // Chat area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);
            
            // Chat messages scroll area
            let chat_height = ui.available_height() - 80.0; // Leave space for input
            
            egui::ScrollArea::vertical()
                .max_height(chat_height)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for msg in &s.chat_history {
                        ui.add_space(8.0);
                        
                        if msg.role == "user" {
                            // User message - right aligned
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                egui::Frame::none()
                                    .fill(egui::Color32::from_rgb(51, 102, 153))
                                    .rounding(egui::Rounding::same(20.0))
                                    .inner_margin(egui::Margin::same(18.0))
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new(&msg.content)
                                            .color(egui::Color32::WHITE)
                                            .size(18.0));
                                    });
                            });
                        } else {
                            // Assistant message - left aligned
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(248, 250, 255))
                                .rounding(egui::Rounding::same(20.0))
                                .inner_margin(egui::Margin::same(18.0))
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(&msg.content)
                                        .color(egui::Color32::from_rgb(51, 51, 51))
                                        .size(18.0));
                                });
                        }
                        
                        ui.add_space(4.0);
                    }
                    
                    if s.is_thinking {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(248, 250, 255))
                            .rounding(egui::Rounding::same(20.0))
                            .inner_margin(egui::Margin::same(18.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("🤔 Thinking...")
                                    .color(egui::Color32::from_rgb(102, 102, 102))
                                    .size(18.0));
                            });
                    }
                });
        });
        
        // Bottom input panel
        egui::TopBottomPanel::bottom("input").show(ctx, |ui| {
            // Custom background
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                egui::Rounding::ZERO,
                egui::Color32::from_rgb(245, 250, 255)
            );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    
                    let hint = match s.current_mode {
                        ChatMode::Find => "What would you like me to find?",
                        ChatMode::Fix => "What needs fixing?", 
                        ChatMode::Research => "What should I research?",
                    };
                    
                    let input_response = ui.add_sized(
                        [ui.available_width() - 160.0, 50.0],
                        egui::TextEdit::singleline(&mut s.input_text)
                            .hint_text(hint)
                            .font(egui::FontId::new(18.0, egui::FontFamily::Proportional))
                    );
                    
                    if input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        s.send_message();
                    }
                    
                    ui.add_space(12.0);
                    
                    if ui.add_sized([140.0, 50.0], egui::Button::new(
                        egui::RichText::new("➤ Send")
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(102, 51, 153))
                     .rounding(egui::Rounding::same(12.0))).clicked() {
                        s.send_message();
                    }
                    
                    ui.add_space(16.0);
                });
                ui.add_space(20.0);
            });
    }
}
