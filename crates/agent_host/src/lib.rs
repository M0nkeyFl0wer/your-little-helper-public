//! Agent Host - AI agent with command execution capabilities
//!
//! This crate provides the AI agent that can:
//! - Chat with users via multiple AI providers
//! - Execute shell commands safely on behalf of users
//! - Parse and extract commands from AI responses
//! - Provide user-friendly summaries of command output

pub mod executor;

use anyhow::Result;
use regex::Regex;
use shared::agent_api::ChatMessage;
use shared::settings::AppSettings;

pub use executor::{CommandResult, DangerLevel, classify_command, execute_command, parse_progress, needs_elevation, web_search};

#[cfg(not(windows))]
pub use executor::execute_with_sudo;

#[cfg(windows)]
pub use executor::execute_with_elevation;

/// Tool result from command execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub command: String,
    pub result: CommandResult,
}

/// Agent host manages AI chat and command execution
pub struct AgentHost {
    pub settings: AppSettings,
}

impl AgentHost {
    pub fn new(settings: AppSettings) -> Self {
        Self { settings }
    }

    /// Simple chat - just AI response, no command execution
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        use providers::router::ProviderRouter;
        let router = ProviderRouter::new(self.settings.model.clone());
        router.generate(messages).await
    }

    /// Agent chat - AI can request command execution
    /// Returns the final response and any tool results
    pub async fn agent_chat(
        &self,
        messages: Vec<ChatMessage>,
        auto_execute_safe: bool,
    ) -> Result<(String, Vec<ToolResult>)> {
        use providers::router::ProviderRouter;
        
        let router = ProviderRouter::new(self.settings.model.clone());
        let mut all_messages = messages.clone();
        let mut tool_results = Vec::new();
        
        // Add agent system prompt
        let system_prompt = self.get_agent_system_prompt();
        all_messages.insert(0, ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        });
        
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
                    let result = execute_command(&cmd, 30).await?;
                    
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
            "I've reached the maximum number of command iterations. Please continue manually.".to_string(),
            tool_results,
        ))
    }

    /// Extract commands from AI response
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

        format!(r#"You are Little Helper, a friendly AI assistant with the ability to run commands and search the web.

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
"#, os_context)
    }

    /// Execute a specific command (for UI-triggered execution)
    pub async fn execute(&self, cmd: &str) -> Result<CommandResult> {
        execute_command(cmd, 60).await
    }

    /// Check if a command needs confirmation
    pub fn needs_confirmation(&self, cmd: &str) -> bool {
        let danger = classify_command(cmd);
        matches!(danger, DangerLevel::NeedsConfirmation | DangerLevel::Dangerous | DangerLevel::NeedsSudo)
    }

    /// Get danger level for a command
    pub fn get_danger_level(&self, cmd: &str) -> DangerLevel {
        classify_command(cmd)
    }
}
