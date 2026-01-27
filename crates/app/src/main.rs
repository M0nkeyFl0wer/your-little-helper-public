use agent_host::{AgentHost, CommandResult};
use eframe::egui;
use parking_lot::Mutex;
use shared::agent_api::ChatMessage as ApiChatMessage;
use shared::preview_types::{parse_preview_tags, strip_preview_tags, PreviewContent};
use shared::settings::AppSettings;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, OnceLock};
/// Result from background AI generation
struct AiResult {
    response: String,
    preview_file: Option<PathBuf>,
    error: Option<String>,
    /// Commands that were executed (for transparency)
    executed_commands: Vec<(String, String, bool)>, // (command, output, success)
    pending_commands: Vec<String>,
}

struct CommandExecResult {
    command: String,
    output: Result<CommandResult, String>,
}

// Default mascot image (boss's dog!)
const DEFAULT_MASCOT: &[u8] = include_bytes!("../assets/default_mascot.png");

// Pre-loaded secrets (gitignored secrets.rs, or empty for CI builds)
mod secrets;
use secrets::{OPENAI_API_KEY, PRELOAD_SKIP_ONBOARDING, PRELOAD_USER_NAME};

// Support contact info (gitignored - your personal contact stays private)
mod support_info;
use support_info::{SUPPORT_BUTTON_TEXT, SUPPORT_LINK};

// Interactive Preview Companion modules
mod ascii_art;
mod onboarding;
mod preview_panel;

// Campaign context loader
mod context;
use context::{get_campaign_summary, load_campaign_context, load_ddd_workflow, load_personas};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppScreen {
    Onboarding,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ChatMode {
    Fix,      // Tech support - diagnose and fix problems
    Research, // Deep research with citations
    Data,     // Work with data and files
    Content,  // Content creation with personas
}

impl ChatMode {
    /// Get the mode name as a string for the agent system
    fn as_str(&self) -> &'static str {
        match self {
            ChatMode::Fix => "fix",
            ChatMode::Research => "research",
            ChatMode::Data => "data",
            ChatMode::Content => "content",
        }
    }
}

#[derive(Clone)]
struct ChatMessage {
    role: String, // "user" or "assistant"
    content: String,
    #[allow(dead_code)] // Will be used for chat history display
    timestamp: String,
}

/// Active viewer in the preview panel
enum ActiveViewer {
    Panel,                         // Default preview panel content (mode intro, files, etc)
    Matrix,                        // Matrix rain animation while processing
    RickRoll,                      // Easter egg!
    CommandOutput(String, String), // (command, output) for showing command results
}

struct AppState {
    settings: AppSettings,
    current_screen: AppScreen,
    current_mode: ChatMode,
    previous_mode: Option<ChatMode>, // For detecting mode changes
    input_text: String,
    mode_input_drafts: std::collections::HashMap<ChatMode, String>, // Preserve input per mode
    chat_history: Vec<ChatMessage>,
    is_thinking: bool,
    thinking_status: String, // What the agent is currently doing
    #[allow(dead_code)] // Available for future agentic features
    agent_host: AgentHost,

    // Preview panel (new interactive preview companion)
    preview_panel: preview_panel::PreviewPanel,

    // Legacy preview panel (for file viewers)
    show_preview: bool,
    active_viewer: ActiveViewer,
    pending_preview: Option<PathBuf>, // File to auto-open after response

    // Onboarding
    onboarding_name: String,

    // Pending command approvals
    pending_commands: Vec<String>,

    // Background command execution channel
    command_result_rx: Option<Receiver<CommandExecResult>>,

    // Background mascot texture
    mascot_texture: Option<egui::TextureHandle>,
    mascot_loaded: bool,

    // Async AI response channel
    ai_result_rx: Option<Receiver<AiResult>>,

    // Slack integration
    show_slack_dialog: bool,
    slack_message_to_send: Option<String>,
    slack_selected_channel: String,
    slack_status: Option<String>, // Status message after send attempt
    show_settings_dialog: bool,
    new_allowed_dir: String,
    settings_status: Option<String>,
    settings_status_is_error: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let (mut settings, _) = load_settings_or_default();

        // Apply preloaded user info if available (bespoke builds)
        if PRELOAD_SKIP_ONBOARDING {
            settings.user_profile.onboarding_complete = true;
            settings.user_profile.terminal_permission_granted = true;
            if !PRELOAD_USER_NAME.is_empty() {
                settings.user_profile.name = PRELOAD_USER_NAME.to_string();
            }
        }

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

        // Initialize preview panel with mode intro
        let mut preview_panel = preview_panel::PreviewPanel::new();
        preview_panel.show_mode_intro("fix");

        Self {
            settings: settings.clone(),
            current_screen: if needs_onboarding {
                AppScreen::Onboarding
            } else {
                AppScreen::Chat
            },
            current_mode: ChatMode::Fix,
            previous_mode: None,
            input_text: String::new(),
            mode_input_drafts: std::collections::HashMap::new(),
            chat_history: vec![welcome_msg],
            is_thinking: false,
            thinking_status: String::new(),
            agent_host: AgentHost::new(settings),
            preview_panel,
            show_preview: true,                 // Preview visible by default
            active_viewer: ActiveViewer::Panel, // Start with panel intro
            pending_preview: None,
            onboarding_name: String::new(),
            pending_commands: Vec::new(),
            command_result_rx: None,
            mascot_texture: None,
            mascot_loaded: false,
            ai_result_rx: None,
            show_slack_dialog: false,
            slack_message_to_send: None,
            slack_selected_channel: "#general".to_string(),
            slack_status: None,
            show_settings_dialog: false,
            new_allowed_dir: String::new(),
            settings_status: None,
            settings_status_is_error: false,
        }
    }
}

impl AppState {
    fn is_path_permitted(&self, path: &Path) -> bool {
        is_path_in_allowed_dirs(path, &self.settings.allowed_dirs)
    }

    /// Check for completed AI responses (called each frame)
    fn poll_ai_response(&mut self) {
        if let Some(rx) = &self.ai_result_rx {
            // Non-blocking check for result
            if let Ok(result) = rx.try_recv() {
                self.is_thinking = false;
                self.thinking_status.clear();
                self.ai_result_rx = None;

                // Return to welcome view (unless Rick Roll is showing)
                if matches!(self.active_viewer, ActiveViewer::Matrix) {
                    self.active_viewer = ActiveViewer::Panel;
                }

                if let Some(error) = result.error {
                    self.pending_commands.clear();
                    // Format error message with helpful info
                    let error_content = format_error_message(&error);
                    let error_msg = ChatMessage {
                        role: "assistant".to_string(),
                        content: error_content,
                        timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                    };
                    self.chat_history.push(error_msg);
                } else {
                    // Store file to preview
                    self.pending_preview = result.preview_file;
                    self.pending_commands = result.pending_commands.clone();

                    // Show executed commands in preview panel if any ran
                    if !result.executed_commands.is_empty() {
                        // Show last command output in preview
                        if let Some((cmd, output, _)) = result.executed_commands.last() {
                            self.active_viewer =
                                ActiveViewer::CommandOutput(cmd.clone(), output.clone());
                        }

                        // Also add summary to chat
                        let mut cmd_summary = String::from("**Commands executed:**\n");
                        for (cmd, output, success) in &result.executed_commands {
                            let status = if *success { "[OK]" } else { "[FAILED]" };
                            cmd_summary.push_str(&format!("\n`{}` {}\n", cmd, status));
                            // Show truncated output
                            let output_preview = if output.len() > 300 {
                                format!("{}...", &output[..300])
                            } else {
                                output.clone()
                            };
                            if !output_preview.trim().is_empty() {
                                cmd_summary
                                    .push_str(&format!("```\n{}\n```\n", output_preview.trim()));
                            }
                        }
                        self.chat_history.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: cmd_summary,
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                    }

                    // Parse for preview tags (<preview type="..." ...>)
                    for tag in parse_preview_tags(&result.response) {
                        if let Some(content) = tag.to_content() {
                            match &content {
                                PreviewContent::Web { url, .. } => {
                                    self.preview_panel.show_content(PreviewContent::Web {
                                        url: url.clone(),
                                        title: None,
                                        screenshot: None,
                                        og_image: None,
                                        snippet: Some(tag.caption.clone()),
                                    });
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

                    // Clean up response - remove action tags (both old and new style)
                    let clean_response = clean_ai_response(&result.response);
                    // Also strip new-style preview tags
                    let clean_response = strip_preview_tags(&clean_response);

                    let assistant_msg = ChatMessage {
                        role: "assistant".to_string(),
                        content: if clean_response.is_empty() {
                            result.response.clone()
                        } else {
                            clean_response
                        },
                        timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                    };
                    self.chat_history.push(assistant_msg);

                    if !self.pending_commands.is_empty() {
                        let mut summary =
                            String::from("I need your approval before running these commands:\n");
                        for cmd in &self.pending_commands {
                            summary.push_str(&format!("\n`{}`", cmd));
                        }
                        self.chat_history.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: summary,
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                    }
                }
            }
        }
    }

    fn poll_command_result(&mut self) {
        if let Some(rx) = &self.command_result_rx {
            if let Ok(result) = rx.try_recv() {
                self.command_result_rx = None;
                self.is_thinking = false;
                self.thinking_status.clear();

                match result.output {
                    Ok(cmd_result) => {
                        self.active_viewer = ActiveViewer::CommandOutput(
                            result.command.clone(),
                            cmd_result.output.clone(),
                        );
                        self.chat_history.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: format!(
                                "Command `{}` completed.\n\n```\n{}\n```",
                                result.command, cmd_result.output
                            ),
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                    }
                    Err(err) => {
                        self.chat_history.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: format!("Command `{}` failed to run: {}", result.command, err),
                            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
                        });
                    }
                }
            }
        }
    }

    fn approve_command(&mut self, command: String) {
        self.pending_commands.retain(|c| c != &command);
        if let Err(reason) = validate_command_against_allowed(&command, &self.settings.allowed_dirs)
        {
            self.chat_history.push(ChatMessage {
                role: "assistant".to_string(),
                content: format!("Command `{}` blocked: {}", command, reason),
                timestamp: chrono::Utc::now().format("%H:%M").to_string(),
            });
            self.command_result_rx = None;
            self.is_thinking = false;
            self.thinking_status.clear();
            return;
        }

        let (tx, rx) = channel::<CommandExecResult>();
        self.command_result_rx = Some(rx);
        self.is_thinking = true;
        self.thinking_status = format!("Running {}", command);

        std::thread::spawn(move || {
            let output = run_user_command(&command);
            let _ = tx.send(CommandExecResult { command, output });
        });
    }

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
    #[allow(dead_code)] // Available for settings UI
    fn reload_mascot_texture(&mut self, ctx: &egui::Context) {
        self.mascot_loaded = false;
        self.mascot_texture = None;
        self.load_mascot_texture(ctx);
    }

    fn send_message(&mut self) {
        if self.input_text.trim().is_empty() {
            return;
        }

        // Easter egg: Rick Roll when asking about Ben West
        let input_lower = self.input_text.to_lowercase();
        if input_lower.contains("ben west") || input_lower.contains("benwest") {
            self.active_viewer = ActiveViewer::RickRoll;
            self.show_preview = true;
            // Still process the message normally, but show the meme
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

        // Show Matrix animation while processing (unless Rick Roll is showing)
        if !matches!(self.active_viewer, ActiveViewer::RickRoll) {
            self.active_viewer = ActiveViewer::Matrix;
        }
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

        // Platform-specific Find mode commands
        let find_commands = if is_windows {
            r#"
WINDOWS COMMANDS TO USE:
- List files: <command>dir /s /b "C:\Users\%USERNAME%\Documents\*.pdf"</command>
- Find by name: <command>dir /s /b "C:\Users\%USERNAME%\*report*"</command>
- Search content: <command>findstr /s /i "keyword" "C:\Users\%USERNAME%\Documents\*.txt"</command>
- List recent: <command>dir /od "C:\Users\%USERNAME%\Documents"</command>
- Show file info: <command>dir "filepath"</command>

COMMON PATHS:
- Documents: C:\Users\%USERNAME%\Documents
- Desktop: C:\Users\%USERNAME%\Desktop
- Downloads: C:\Users\%USERNAME%\Downloads

EXAMPLE - User asks "find my tax documents":
<command>dir /s /b "C:\Users\%USERNAME%\Documents\*tax*"</command>
<command>dir /s /b "C:\Users\%USERNAME%\Downloads\*tax*"</command>
"#
        } else {
            r#"
UNIX/MAC COMMANDS TO USE:
- List files: <command>find ~/Documents -name "*.pdf" 2>/dev/null</command>
- Find by name: <command>find ~ -name "*report*" 2>/dev/null | head -20</command>
- Search content: <command>grep -r "keyword" ~/Documents --include="*.txt" 2>/dev/null</command>
- List recent: <command>ls -lt ~/Documents | head -20</command>
- Show file info: <command>ls -la "filepath"</command>

COMMON PATHS:
- Documents: ~/Documents
- Desktop: ~/Desktop
- Downloads: ~/Downloads

EXAMPLE - User asks "find my tax documents":
<command>find ~/Documents -iname "*tax*" 2>/dev/null</command>
<command>find ~/Downloads -iname "*tax*" 2>/dev/null</command>
"#
        };

        // Platform-specific Fix mode commands
        let fix_commands = if is_windows {
            r#"
WINDOWS DIAGNOSTIC COMMANDS:
- System info: <command>systeminfo</command>
- Disk space: <command>wmic logicaldisk get size,freespace,caption</command>
- Memory: <command>wmic OS get FreePhysicalMemory,TotalVisibleMemorySize /Value</command>
- Network: <command>ipconfig /all</command>
- Ping test: <command>ping -n 3 google.com</command>
- DNS test: <command>nslookup google.com</command>
- Running processes: <command>tasklist</command>
- Services: <command>sc query</command>
- Ports in use: <command>netstat -an | findstr LISTENING</command>
- Environment: <command>set</command>

EXAMPLE - User says "my internet is slow":
<command>ping -n 5 google.com</command>
<command>ipconfig /all</command>
<command>netstat -an | findstr ESTABLISHED</command>
"#
        } else {
            r#"
UNIX/MAC DIAGNOSTIC COMMANDS:
- System info: <command>uname -a</command>
- Disk space: <command>df -h</command>
- Memory: <command>free -h</command> or <command>vm_stat</command> (Mac)
- Network: <command>ip addr</command> or <command>ifconfig</command>
- Ping test: <command>ping -c 3 google.com</command>
- DNS test: <command>nslookup google.com</command>
- Running processes: <command>ps aux | head -20</command>
- Services: <command>systemctl list-units --type=service --state=running</command>
- Ports in use: <command>netstat -tulpn 2>/dev/null || lsof -i -P</command>
- Logs: <command>tail -50 /var/log/syslog 2>/dev/null || tail -50 /var/log/system.log</command>

EXAMPLE - User says "my computer is slow":
<command>top -bn1 | head -15</command>
<command>df -h</command>
<command>free -h</command>
"#
        };

        let system_prompt = match self.current_mode {
            ChatMode::Fix => format!(
                r#"You are Little Helper in FIX mode, a terminal agent helping {}.

YOUR JOB: Tech support! Diagnose problems, find files, fix issues. EXECUTE COMMANDS - don't just explain!

FILE FINDING:
{}

DIAGNOSTICS:
{}

WORKFLOW:
1. When user describes a problem, IMMEDIATELY run diagnostic commands
2. If they need to find files, run search commands
3. <search>search for solutions</search> if needed
4. Analyze output, explain findings
5. Run fix commands (with explanation)
6. Use <preview>path</preview> to show files in preview panel

{}
"#,
                user_name, find_commands, fix_commands, capabilities
            ),
            ChatMode::Research => {
                // Cross-platform research prompt
                #[cfg(target_os = "windows")]
                let script_example = r#"PYTHON SCRIPTING (Windows):
You can create and run Python scripts for research:
<command>echo import requests > research_script.py && echo import json >> research_script.py && python research_script.py</command>

Or for longer scripts, save to a file first:
<command>python -c "import requests; print(requests.get('https://api.example.com').text)"</command>

API RESEARCH (when needed):
- Use curl for quick API tests: <command>curl -s "https://api.example.com/data"</command>
- Use PowerShell: <command>powershell -c "Invoke-WebRequest -Uri 'https://api.example.com/data'"</command>
- Write Python for complex API interactions

AVAILABLE TOOLS:
- python, pip (can install packages)
- curl (HTTP requests)
- PowerShell for advanced scripting"#;

                #[cfg(not(target_os = "windows"))]
                let script_example = r#"PYTHON SCRIPTING:
You can create and run Python scripts for research:
<command>cat << 'EOF' > /tmp/research_script.py
import requests
import json
# Your research code here
print(json.dumps(results, indent=2))
EOF
python3 /tmp/research_script.py</command>

API RESEARCH (when needed):
- Use curl for quick API tests: <command>curl -s "https://api.example.com/data" | jq</command>
- Write Python for complex API interactions
- Save results to files for analysis

AVAILABLE TOOLS:
- python3, pip (can install packages)
- curl, wget (HTTP requests)
- jq (JSON processing)
- Standard Unix tools"#;

                format!(
                    r#"You are Little Helper in DEEP RESEARCH mode, helping {}.

YOUR ROLE: Thorough researcher with ability to search, analyze, and create tools.

RESEARCH WORKFLOW:
1. Understand the research question - ask clarifying questions
2. <search>initial broad search</search> to understand the landscape
3. <search>more specific searches</search> based on initial findings
4. Cross-reference multiple sources
5. If needed, write Python scripts to analyze data or call APIs

{}

ALWAYS:
- Search multiple times from different angles
- Cite your sources
- Show relevant documents in preview: <preview>path/to/doc</preview>
- Summarize findings clearly
- Distinguish facts from speculation
- Note when information might be outdated

{}
"#,
                    user_name, script_example, capabilities
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
                    "CAMPAIGN CONTEXT DISABLED. Enable it in Settings to preload MCP materials."
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

CONTENT CALENDAR LOCATION: ~/Projects/MCP-research-content-automation-engine/FINAL_MCP_Content_Calendar.json
DRAFTS FOLDER: ~/Process/drafts/

WORKFLOW (Data Driven Designs):
1. Identify the target PERSONA for this content
2. Review campaign materials for relevant facts/data
3. Draft content matching persona's language and concerns
4. Save drafts to ~/Process/drafts/ with format: YYYY-MM-DD_platform_topic.md
5. Content will sync to Google Drive for team review

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
- Save drafts to ~/Process/drafts/

{}
"#,
                    user_name, ddd_workflow, personas, campaign_docs, capabilities
                )
            },
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

        // Start async AI generation with capability flags
        self.start_ai_generation(
            api_messages,
            terminal_enabled,
            self.settings.enable_internet_research,
        );
    }

    fn start_ai_generation(
        &mut self,
        messages: Vec<ApiChatMessage>,
        allow_terminal: bool,
        allow_web: bool,
    ) {
        let (tx, rx) = channel::<AiResult>();
        self.ai_result_rx = Some(rx);
        self.thinking_status = "Thinking...".to_string();

        let settings = self.settings.model.clone();
        let allowed_dirs = self.settings.allowed_dirs.clone();

        // Spawn background thread for AI work
        std::thread::spawn(move || {
            run_ai_generation(
                messages,
                settings,
                allow_terminal,
                allow_web,
                allowed_dirs,
                tx,
            );
        });
    }

    /// Open a file in the preview panel
    fn open_file(&mut self, path: &Path, ctx: &egui::Context) {
        if !self.is_path_permitted(path) {
            return;
        }
        self.preview_panel.open_file(path, ctx);
        self.active_viewer = ActiveViewer::Panel;
        self.show_preview = true;
    }

    fn close_preview(&mut self) {
        self.show_preview = false;
        self.preview_panel.hide();
        self.active_viewer = ActiveViewer::Panel;
    }
}

/// Run AI generation in background thread (non-blocking)
fn run_ai_generation(
    messages: Vec<ApiChatMessage>,
    settings: shared::settings::ModelProvider,
    allow_terminal: bool,
    allow_web: bool,
    allowed_dirs: Vec<String>,
    tx: Sender<AiResult>,
) {
    use agent_host::{classify_command, web_search, DangerLevel};
    use providers::router::ProviderRouter;

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(AiResult {
                response: String::new(),
                preview_file: None,
                error: Some(format!("Failed to start async runtime: {}", e)),
                executed_commands: Vec::new(),
                pending_commands: Vec::new(),
            });
            return;
        }
    };

    let router = ProviderRouter::new(settings);

    // Pre-compile regexes
    let search_re = regex::Regex::new(r"(?s)<search>(.*?)</search>").unwrap();
    let cmd_re = regex::Regex::new(r"(?s)<command>(.*?)</command>").unwrap();

    let result = rt.block_on(async {
        let mut msgs = messages;
        let mut file_to_preview: Option<PathBuf> = None;
        let mut all_executed_commands: Vec<(String, String, bool)> = Vec::new();
        let mut pending_commands: Vec<String> = Vec::new();

        // Loop for multi-turn interactions (max 5 iterations)
        for _iteration in 0..5 {
            // Get AI response
            let response = router.generate(msgs.clone()).await?;

            // Check for preview tags
            for tag in shared::preview_types::parse_preview_tags(&response) {
                if tag.content_type == "file" {
                    if let Some(path_str) = tag.path {
                        let expanded = expand_user_path(&path_str);
                        if expanded.exists() && is_path_in_allowed_dirs(&expanded, &allowed_dirs) {
                            file_to_preview = Some(expanded);
                        }
                    }
                }
            }

            // Check for search and command tags
            let searches: Vec<String> = search_re
                .captures_iter(&response)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();

            let commands: Vec<String> = cmd_re
                .captures_iter(&response)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();

            // If no actions needed, return the response
            if searches.is_empty() && commands.is_empty() {
                return Ok::<
                    (
                        String,
                        Option<PathBuf>,
                        Vec<(String, String, bool)>,
                        Vec<String>,
                    ),
                    anyhow::Error,
                >((
                    response,
                    file_to_preview,
                    all_executed_commands,
                    pending_commands,
                ));
            }

            // Add assistant response to conversation
            msgs.push(ApiChatMessage {
                role: "assistant".to_string(),
                content: response.clone(),
            });

            let mut results = Vec::new();

            // Execute searches
            for query in &searches {
                if !allow_web {
                    results.push(format!(
                        "[Search blocked: Internet access disabled]\nQuery: {}",
                        query
                    ));
                    continue;
                }
                match web_search(query).await {
                    Ok(result) => {
                        results.push(format!(
                            "[Search Results for '{}']\n{}",
                            query, result.output
                        ));
                    }
                    Err(e) => {
                        results.push(format!("[Search failed for '{}']: {}", query, e));
                    }
                }
            }

            // Queue commands for approval (no longer auto-execute)
            for cmd in &commands {
                if !allow_terminal {
                    all_executed_commands.push((
                        cmd.clone(),
                        "Terminal access disabled in settings".to_string(),
                        false,
                    ));
                    results.push(format!(
                        "[Command blocked: terminal access disabled]\n$ {}",
                        cmd
                    ));
                    continue;
                }
                let danger = classify_command(cmd);
                if danger == DangerLevel::Blocked {
                    all_executed_commands.push((
                        cmd.clone(),
                        "Blocked for safety".to_string(),
                        false,
                    ));
                    results.push(format!("[Command blocked for safety: {}]", cmd));
                    continue;
                }

                results.push(format!("[Command '{}' queued for user approval]", cmd));
                if !pending_commands.iter().any(|c| c == cmd) {
                    pending_commands.push(cmd.clone());
                }
            }

            // Add results back to conversation
            if !results.is_empty() {
                msgs.push(ApiChatMessage {
                    role: "user".to_string(),
                    content: results.join("\n\n"),
                });
            }
        }

        Ok((
            "I've done several steps of research. Let me know if you need more details!"
                .to_string(),
            file_to_preview,
            all_executed_commands,
            pending_commands,
        ))
    });

    // Send result back to UI
    let ai_result = match result {
        Ok((response, preview_file, executed_commands, pending_commands)) => AiResult {
            response,
            preview_file,
            error: None,
            executed_commands,
            pending_commands,
        },
        Err(e) => AiResult {
            response: String::new(),
            preview_file: None,
            error: Some(e.to_string()),
            executed_commands: Vec::new(),
            pending_commands: Vec::new(),
        },
    };

    let _ = tx.send(ai_result);
}

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

        // Request repaint if we're waiting for AI (to keep polling)
        if s.is_thinking {
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
            s.input_text = s
                .mode_input_drafts
                .get(&new_mode)
                .cloned()
                .unwrap_or_default();

            let mode_str = s.current_mode.as_str();
            s.preview_panel.show_mode_intro(mode_str);
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

                    // Mode buttons (4 tabs: Fix, Research, Data, Content)
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
                            .on_hover_text("Configure privacy and allowed directories")
                            .clicked()
                        {
                            s.show_settings_dialog = true;
                        }

                        ui.add_space(12.0);

                        // Model indicator
                        let provider = s
                            .settings
                            .model
                            .provider_preference
                            .first()
                            .map(|s| s.as_str())
                            .unwrap_or("none");
                        let model_name = match provider {
                            "openai" => &s.settings.model.openai_model,
                            "anthropic" => &s.settings.model.anthropic_model,
                            "gemini" => &s.settings.model.gemini_model,
                            "local" => &s.settings.model.local_model,
                            _ => "unknown",
                        };
                        ui.label(
                            egui::RichText::new(format!("⚡ {}", model_name))
                                .size(11.0)
                                .color(if dark {
                                    egui::Color32::from_rgb(140, 180, 140)
                                } else {
                                    egui::Color32::from_rgb(80, 130, 80)
                                }),
                        )
                        .on_hover_text(format!("Provider: {}", provider));

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
                            ActiveViewer::Matrix => "Processing...".to_string(),
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
                // Paint mascot as watermark FIRST (background layer)
                let panel_rect = ui.max_rect();
                if let Some(texture) = &s.mascot_texture {
                    let tex_size = texture.size_vec2();

                    // Scale larger - about 50% of panel width for more presence
                    let max_width = panel_rect.width() * 0.50;
                    let max_height = panel_rect.height() * 0.60;
                    let scale = (max_width / tex_size.x).min(max_height / tex_size.y);
                    let img_size = tex_size * scale;

                    // Center in the panel (behind chat bubbles)
                    let pos = egui::pos2(
                        panel_rect.center().x - img_size.x / 2.0,
                        panel_rect.center().y - img_size.y / 2.0 + 20.0,
                    );
                    let rect = egui::Rect::from_min_size(pos, img_size);

                    // Subtle watermark - visible but won't obstruct text
                    ui.painter().image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_white_alpha(25), // Slightly more visible
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
                        s.chat_history = vec![welcome];
                        // Show mode intro in preview
                        s.preview_panel.show_mode_intro(mode.as_str());
                    }

                    ui.separator();

                    // Thread count indicator
                    let thread_count = s.chat_history.len();
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
                            s.chat_history = vec![welcome];
                            s.preview_panel.show_mode_intro(mode.as_str());
                        }
                    });
                });

                ui.add_space(4.0);

                // Chat messages scroll area
                let chat_height = ui.available_height() - 70.0;

                let mut clicked_path: Option<PathBuf> = None;
                let mut slack_msg: Option<String> = None;

                egui::ScrollArea::vertical()
                    .max_height(chat_height)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &s.chat_history {
                            ui.add_space(6.0);
                            let action = render_message(ui, msg, dark, &s.settings.allowed_dirs);
                            if action.clicked_path.is_some() {
                                clicked_path = action.clicked_path;
                            }
                            if action.send_to_slack.is_some() {
                                slack_msg = action.send_to_slack;
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
                                    ui.horizontal(|ui| {
                                        // Animated spinner dots
                                        let time = ui.input(|i| i.time);
                                        let dots = match ((time * 2.0) as i32) % 4 {
                                            0 => "   ",
                                            1 => ".  ",
                                            2 => ".. ",
                                            _ => "...",
                                        };

                                        let status = if s.thinking_status.is_empty() {
                                            "Thinking".to_string()
                                        } else {
                                            s.thinking_status.clone()
                                        };

                                        ui.label(
                                            egui::RichText::new(format!("{}{}", status, dots))
                                                .color(if dark {
                                                    egui::Color32::from_rgb(160, 160, 180)
                                                } else {
                                                    egui::Color32::from_rgb(100, 100, 120)
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
                        ChatMode::Fix => "What's broken? Need to find a file?",
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

        // Settings dialog
        if s.show_settings_dialog {
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(420.0);
                    ui.heading("Privacy & Context");
                    ui.add_space(8.0);

                    let mut needs_save = false;

                    if ui
                        .checkbox(
                            &mut s.settings.enable_campaign_context,
                            "Load MCP campaign materials automatically",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }
                    if ui
                        .checkbox(
                            &mut s.settings.enable_persona_context,
                            "Load persona files from ~/Process/personas",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }
                    if ui
                        .checkbox(
                            &mut s.settings.share_system_summary,
                            "Share system summary (hostname, tools) with the AI",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }
                    if ui
                        .checkbox(
                            &mut s.settings.enable_internet_research,
                            "Allow internet research (web searches & articles)",
                        )
                        .changed()
                    {
                        needs_save = true;
                    }

                    if needs_save {
                        save_settings(&s.settings);
                        s.settings_status = Some("Saved privacy preferences".to_string());
                        s.settings_status_is_error = false;
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.heading("Allowed Directories");
                    ui.label(
                        "Little Helper only previews files and proposes commands inside these folders.",
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
                            "No directories allowed. Add at least one path.",
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
                        s.settings_status =
                            Some(format!("Removed {}", target));
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
                                    Some("Enter a directory path before adding.".to_string());
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
                                        Some("Directory already in allowlist.".to_string());
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
                                    Some("Directory must exist on disk.".to_string());
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
                                    "#mcp-campaign",
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
            ChatMode::Fix => (
                "Fix Mode",
                vec![
                    "Tell me what's broken - I'll diagnose it",
                    "Need a file? I can find that too",
                    "Diagnostics and logs show up here",
                    "Try: \"my wifi keeps disconnecting\"",
                    "Try: \"find my tax documents\"",
                ],
            ),
            ChatMode::Research => (
                "Research Mode",
                vec![
                    "Ask me to research any topic",
                    "I'll search multiple sources with citations",
                    "Results and sources shown here",
                    "Try: \"research the latest on Alberta politics\"",
                ],
            ),
            ChatMode::Data => (
                "Data Mode",
                vec![
                    "Work with CSV, JSON, Excel files",
                    "Data tables render right here",
                    "I can analyze and transform data",
                    "Try: \"analyze this spreadsheet\" + drop a file",
                ],
            ),
            ChatMode::Content => (
                "Content Mode",
                vec![
                    "Create content for any platform",
                    "I know your campaign personas",
                    "Drafts preview here before saving",
                    "Try: \"write a tweet about healthcare\"",
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

            ui.label(
                egui::RichText::new("(Nice try searching for Ben West though)")
                    .size(14.0)
                    .weak(),
            );

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
    let warm_coral = egui::Color32::from_rgb(230, 115, 100);
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

                // Welcome header
                ui.label(
                    egui::RichText::new("I'm your Little Helper")
                        .size(24.0)
                        .color(if dark {
                            egui::Color32::from_rgb(240, 235, 230)
                        } else {
                            warm_brown
                        }),
                );

                ui.add_space(20.0);

                ui.label(
                    egui::RichText::new("I'm here to make your day a little easier. Here's what I can do:")
                        .size(15.0)
                        .color(if dark {
                            egui::Color32::from_rgb(200, 190, 180)
                        } else {
                            egui::Color32::from_rgb(120, 100, 85)
                        }),
                );

                ui.add_space(20.0);

                // Feature bullets with warm styling
                let features = [
                    ("Run terminal commands", "so you never have to"),
                    ("Tech support", "patient help when things go wrong"),
                    ("Deep research", "thorough answers with real sources"),
                    ("Content creation", "drafting, scheduling, and managing"),
                ];

                for (title, desc) in features {
                    ui.horizontal(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new("~")
                                .size(16.0)
                                .color(warm_coral),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(title)
                                .size(14.0)
                                .strong()
                                .color(if dark {
                                    egui::Color32::from_rgb(230, 220, 210)
                                } else {
                                    warm_brown
                                }),
                        );
                        ui.label(
                            egui::RichText::new(format!(" - {}", desc))
                                .size(14.0)
                                .color(if dark {
                                    warm_tan
                                } else {
                                    egui::Color32::from_rgb(140, 120, 100)
                                }),
                        );
                    });
                    ui.add_space(4.0);
                }

                ui.add_space(30.0);

                // Form container with warm styling
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

                        // Name input - friendlier
                        ui.label(
                            egui::RichText::new("First, what's your name?")
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
                                egui::RichText::new("Can I run terminal commands for you?")
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
                                    "This lets me list files, gather diagnostics, and fix issues automatically."
                                )
                                .size(12.0)
                                .color(if dark { warm_tan } else { egui::Color32::from_rgb(140, 120, 100) }),
                            );
                            ui.add_space(6.0);
                            let mut permission = s.settings.user_profile.terminal_permission_granted;
                            if ui
                                .checkbox(&mut permission, "Yes, allow terminal access (recommended)")
                                .changed()
                            {
                                s.settings.user_profile.terminal_permission_granted = permission;
                            }
                            ui.label(
                                egui::RichText::new("You can change this later from settings.")
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
                                if let Some(first_msg) = s.chat_history.first_mut() {
                                    first_msg.content = format!(
                                        "Hey {}! Great to meet you.\n\n\
                                        I'm here whenever you need a hand. Just tell me what you're working on \
                                        and I'll do my best to help out.",
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

static COMMAND_PATH_REGEX: OnceLock<regex::Regex> = OnceLock::new();

fn validate_command_against_allowed(command: &str, allowed_dirs: &[String]) -> Result<(), String> {
    if allowed_dirs.is_empty() {
        return Err("No directories are allowed. Add one in Settings first.".to_string());
    }

    let regex = COMMAND_PATH_REGEX
        .get_or_init(|| regex::Regex::new(r#"(?P<path>(?:~|/|[A-Za-z]:\\)[^\s"'`]+)"#).unwrap());

    for capture in regex.captures_iter(command) {
        if let Some(path_match) = capture.name("path") {
            let raw = path_match.as_str();
            let candidate = expand_user_path(raw);
            if !is_path_in_allowed_dirs(&candidate, allowed_dirs) {
                return Err(format!(
                    "Path `{}` is outside the allowed directories.",
                    raw
                ));
            }
        }
    }

    Ok(())
}
