#![allow(dead_code)]
//! Agent Host -- the core "brain" of the Little Helper AI assistant.
//!
//! This crate owns every layer between the user's message and the
//! observable side-effects (shell commands, file writes, web searches,
//! skill invocations). It is organised into three major subsystems:
//!
//! 1. **Executor** (`executor.rs`) -- classifies, sandboxes, and runs
//!    shell commands with safety tiers (Safe / NeedsConfirmation /
//!    Dangerous / NeedsAuth / Blocked).
//!
//! 2. **Skill system** (`skills/`, `skill_executor.rs`) -- a registry of
//!    typed, permission-gated skills (Find, Fix, Research, Data, Content,
//!    Build) that the agent can invoke via `<skill>` tags.
//!
//! 3. **Context & memory** (`context_manager.rs`, `graph_store.rs`,
//!    `embedding.rs`, `daily_log.rs`, `context_token_manager.rs`,
//!    `token_tracker.rs`) -- RAG pipeline with a petgraph knowledge
//!    graph, fastembed vector embeddings, token-budget management, and
//!    daily log archival.
//!
//! Security is enforced at two boundaries:
//! - `security.rs` provides path sandboxing and time-boxed 2FA context.
//! - The executor layer scans commands for leaked secrets and validates
//!   every path token against the sandbox before execution.

pub mod context_manager;
pub mod context_token_manager;
pub mod embedding;
pub mod executor;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
pub mod daily_log;
pub mod prompts;
pub mod security;
pub mod skill_executor;
pub mod skills;
pub mod token_tracker;

pub use prompts::{
    get_mode_introduction, get_mode_prompt, get_system_prompt, ModeIntroduction, ModePrompt,
    Permissions,
};

use anyhow::Result;
use regex::Regex;
use shared::agent_api::ChatMessage;
use shared::settings::AppSettings;

pub use executor::{
    classify_command, execute_command, needs_elevation, parse_progress, web_search, CommandResult,
    DangerLevel, SessionState,
};

#[cfg(not(windows))]
pub use executor::execute_with_sudo;

#[cfg(windows)]
pub use executor::execute_with_elevation;

/// Pairs a command string with its execution result, used to accumulate
/// tool invocations across the multi-turn agent loop.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub command: String,
    pub result: CommandResult,
}

/// Top-level orchestrator: routes user messages to the LLM, extracts
/// `<command>` / `<search>` / `[RUN]` tags from the response, executes
/// them subject to safety classification, and feeds results back for
/// further reasoning (up to 10 turns).
pub struct AgentHost {
    pub settings: AppSettings,
    pub session_state: Arc<AsyncMutex<SessionState>>,
}

impl AgentHost {
    /// Initialise the agent with a filesystem sandbox derived from the
    /// user's `allowed_dirs` setting. Commands that reference paths
    /// outside these directories will be rejected before execution.
    pub fn new(settings: AppSettings) -> Self {
        use security::PathSandbox;
        use std::path::PathBuf;

        let allowed = settings.allowed_dirs.iter().map(PathBuf::from).collect();
        let sandbox = PathSandbox::new(allowed);

        Self {
            settings,
            session_state: Arc::new(AsyncMutex::new(SessionState::new().with_sandbox(sandbox))),
        }
    }

    /// Simple chat - just AI response, no command execution
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        use providers::router::ProviderRouter;
        let router = ProviderRouter::new(self.settings.model.clone());
        router.generate(messages).await
    }

    /// Multi-turn agent loop: sends the conversation to the LLM, parses
    /// any `<command>`, `[RUN]`, or `[EXECUTE]` markers from the response,
    /// and auto-executes commands classified as Safe. Blocked commands are
    /// reported back to the model so it can adjust. The loop runs for at
    /// most 10 iterations to prevent runaway tool use.
    pub async fn agent_chat(
        &self,
        messages: Vec<ChatMessage>,
        auto_execute_safe: bool,
        use_fast_model: bool,
    ) -> Result<(String, Vec<ToolResult>)> {
        use providers::router::ProviderRouter;

        let mut model_settings = self.settings.model.clone();
        if use_fast_model {
            model_settings.openai_model = model_settings.openai_fast_model.clone();
            model_settings.anthropic_model = model_settings.anthropic_fast_model.clone();
            model_settings.gemini_model = model_settings.gemini_fast_model.clone();
        }

        let router = ProviderRouter::new(model_settings);
        let mut all_messages = messages.clone();
        let mut tool_results = Vec::new();

        // Add agent system prompt
        let system_prompt = self.get_agent_system_prompt();
        all_messages.insert(
            0,
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
        );

        // Loop for multi-turn command execution (max 10 iterations)
        for _ in 0..10 {
            let response = router.generate(all_messages.clone()).await?;

            // Extract commands from response
            let commands = self.extract_commands(&response);

            if commands.is_empty() {
                // No commands, return final response
                return Ok((response, tool_results));
            }

            // Process each command
            let mut executed_any = false;
            for cmd in commands {
                let danger = classify_command(&cmd);

                // Only auto-execute safe commands if enabled
                let should_execute = match danger {
                    DangerLevel::Safe => auto_execute_safe,
                    DangerLevel::Blocked => false,
                    _ => false, // Needs confirmation from UI
                };

                if should_execute {
                    let mut state = self.session_state.lock().await;
                    let result = execute_command(&cmd, 30, &mut state).await?;

                    // Add result to conversation
                    all_messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: response.clone(),
                    });
                    all_messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: format!(
                            "[Command Output]\n$ {}\n{}\nExit code: {}",
                            cmd, result.output, result.exit_code
                        ),
                    });

                    tool_results.push(ToolResult {
                        command: cmd.clone(),
                        result,
                    });
                    executed_any = true;
                } else if danger == DangerLevel::Blocked {
                    // Inform AI the command is blocked
                    all_messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: response.clone(),
                    });
                    all_messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: format!(
                            "[Command Blocked]\n$ {}\nThis command is blocked for safety reasons.",
                            cmd
                        ),
                    });
                    executed_any = true;
                }
            }

            if !executed_any {
                // Commands need confirmation, return response with pending commands
                return Ok((response, tool_results));
            }
        }

        // Max iterations reached
        Ok((
            "I've reached the maximum number of command iterations. Please continue manually."
                .to_string(),
            tool_results,
        ))
    }

    /// Parse the LLM response for executable commands using three
    /// complementary patterns, in priority order:
    /// 1. `<command>...</command>` XML tags (preferred format)
    /// 2. `[RUN]` marker followed by a fenced code block
    /// 3. `[EXECUTE]` marker with inline backtick code
    fn extract_commands(&self, response: &str) -> Vec<String> {
        let mut commands = Vec::new();

        // Pattern 1: <command>...</command> tags
        let tag_re = Regex::new(r"<command>(.*?)</command>").unwrap();
        for cap in tag_re.captures_iter(response) {
            if let Some(m) = cap.get(1) {
                let cmd = m.as_str().trim();
                if !cmd.is_empty() {
                    commands.push(cmd.to_string());
                }
            }
        }

        // Pattern 2: ```bash or ```sh code blocks with [RUN] marker
        let block_re = Regex::new(r"(?s)\[RUN\].*?```(?:bash|sh|shell)?\n(.*?)```").unwrap();
        for cap in block_re.captures_iter(response) {
            if let Some(m) = cap.get(1) {
                for line in m.as_str().lines() {
                    let cmd = line.trim();
                    if !cmd.is_empty() && !cmd.starts_with('#') {
                        commands.push(cmd.to_string());
                    }
                }
            }
        }

        // Pattern 3: [EXECUTE] marker followed by inline code
        let exec_re = Regex::new(r"\[EXECUTE\]\s*`([^`]+)`").unwrap();
        for cap in exec_re.captures_iter(response) {
            if let Some(m) = cap.get(1) {
                let cmd = m.as_str().trim();
                if !cmd.is_empty() {
                    commands.push(cmd.to_string());
                }
            }
        }

        commands
    }

    /// Get the agent system prompt (cross-platform aware)
    fn get_agent_system_prompt(&self) -> String {
        let os_context = if cfg!(windows) {
            r#"## Your Environment
- You are running on WINDOWS
- Use Windows commands: dir, type, where, systeminfo, ipconfig, etc.
- Use PowerShell for advanced tasks
- Paths use backslashes: C:\Users\name\Documents
- Python is usually just 'python' not 'python3'"#
        } else {
            r#"## Your Environment  
- You are running on Linux/macOS
- Use Unix commands: ls, cat, grep, find, etc.
- Paths use forward slashes: /home/user/documents
- Python is usually 'python3'"#
        };

        format!(
            r#"You are Little Helper, a friendly AI assistant with the ability to run commands and search the web.

## Your Capabilities
- You can execute shell commands to help users find files, check system status, and perform tasks
- You can SEARCH THE WEB to find current information, answer questions, and research topics
- You can read files, search directories, and gather information

{}

## How to Search the Web
When you need to look something up online, use:
   <search>your search query here</search>

Example:
   <search>weather in San Francisco today</search>
   <search>how to reset Windows password</search>
   <search>best practices for Python error handling</search>

ALWAYS search the web when:
- User asks about current events, weather, news
- User needs factual information you're not 100% sure about
- User asks "what is" or "how do I" questions that benefit from current info
- User asks about products, prices, or availability

## How to Run Commands
When you need to run a command, use:
   <command>your command here</command>

Example:
   <command>dir</command>  (Windows)
   <command>ls -la</command>  (Unix)

## Safety Rules
- NEVER run destructive commands without explicit user confirmation
- NEVER access sensitive files without permission
- NEVER run commands you don't understand
- If a command fails due to permissions, explain what happened and suggest alternatives

## File Viewing
When you find or create files that the user should see, use:
   <preview>path/to/file</preview>

The file will automatically open in the preview panel.

## Response Style
- Be conversational and helpful
- Explain what commands do before running them
- Summarize results in plain English
- If something fails, explain why and suggest alternatives
"#,
            os_context
        )
    }

    /// Execute a single command on behalf of the UI (e.g., when the user
    /// clicks "Run" on a pending command). Uses a 60-second timeout.
    pub async fn execute(&self, cmd: &str) -> Result<CommandResult> {
        let mut state = self.session_state.lock().await;
        execute_command(cmd, 60, &mut state).await
    }

    /// Check if a command needs confirmation
    pub fn needs_confirmation(&self, cmd: &str) -> bool {
        let danger = classify_command(cmd);
        matches!(
            danger,
            DangerLevel::NeedsConfirmation | DangerLevel::Dangerous | DangerLevel::NeedsSudo
        )
    }

    /// Get danger level for a command
    pub fn get_danger_level(&self, cmd: &str) -> DangerLevel {
        classify_command(cmd)
    }
}
pub mod graph_store;
