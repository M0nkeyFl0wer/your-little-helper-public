//! Command executor with safety classification and structured output
//!
//! This module handles running shell commands on behalf of the AI agent,
//! with safety checks, confirmation requirements, and user-friendly output.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;

/// Danger level for commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DangerLevel {
    /// Safe read-only commands (ls, cat, grep, etc.)
    Safe,
    /// Commands that modify files but are reversible (cp, mv, mkdir)
    NeedsConfirmation,
    /// Potentially destructive commands (rm, chmod, chown)
    Dangerous,
    /// Commands that require elevated privileges
    NeedsSudo,
    /// Blocked commands that should never run
    Blocked,
}

/// Result of command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// The command that was run
    pub command: String,
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Combined output for display
    pub output: String,
    /// Execution duration
    pub duration_ms: u64,
    /// Whether the command succeeded
    pub success: bool,
    /// User-friendly summary of what happened
    pub summary: String,
    /// Whether sudo/password was required
    pub needed_sudo: bool,
}

/// Safe commands that can run without confirmation
const SAFE_COMMANDS: &[&str] = &[
    // File listing and info (read-only)
    "ls", "find", "cat", "head", "tail", "wc", "du", "df", "pwd", 
    "file", "stat", "tree", "which", "whereis", "type",
    // Text processing (read-only)
    "grep", "rg", "ag", "awk", "sed", "sort", "uniq", "cut", "tr",
    "diff", "comm", "join", "paste", "column",
    // System info (read-only)  
    "uname", "hostname", "uptime", "free", "ps", "top", "htop",
    "lscpu", "lsblk", "lsusb", "lspci", "lsof", "id", "whoami",
    "date", "cal", "env", "printenv",
    // Network info (read-only)
    "ip", "ifconfig", "netstat", "ss", "ping", "nslookup", "dig",
    "host", "traceroute", "curl", "wget",
    // Git (read operations)
    "git status", "git log", "git diff", "git show", "git branch",
    "git remote", "git fetch", "git ls-files", "git blame",
    // Rust/Cargo (read operations)
    "cargo check", "cargo test", "cargo build", "cargo clippy",
    "cargo fmt --check", "rustc --version", "cargo --version",
    // Node/Python (read operations)
    "node --version", "npm --version", "python --version", "pip --version",
    "python -c", "node -e",
    // Archive listing
    "tar -tf", "unzip -l", "zipinfo",
];

/// Commands that need user confirmation before running
const NEEDS_CONFIRMATION: &[&str] = &[
    // File operations
    "cp", "mv", "mkdir", "touch", "ln",
    // Git write operations
    "git add", "git commit", "git push", "git pull", "git merge",
    "git checkout", "git reset", "git stash",
    // Package managers
    "pip install", "npm install", "cargo install",
    // Editors (opening files)
    "nano", "vim", "nvim", "code",
];

/// Dangerous commands that need explicit confirmation with warning
const DANGEROUS_COMMANDS: &[&str] = &[
    // Destructive file operations
    "rm", "rmdir", "shred",
    // Permissions
    "chmod", "chown", "chgrp",
    // System modifications
    "kill", "killall", "pkill",
    // Git destructive
    "git reset --hard", "git clean", "git push --force",
    // Database
    "drop", "delete", "truncate",
];

/// Commands that are always blocked
const BLOCKED_COMMANDS: &[&str] = &[
    // System destruction
    "rm -rf /", "rm -rf /*", ":(){ :|:& };:",
    // Format/wipe
    "mkfs", "dd if=/dev/zero", "dd if=/dev/random",
    // Dangerous redirects
    "> /dev/sda", ">/dev/sda",
    // Network attacks
    "nc -l", "nmap",
];

/// Classify a command by danger level
pub fn classify_command(cmd: &str) -> DangerLevel {
    let cmd_lower = cmd.to_lowercase();
    let cmd_trimmed = cmd_lower.trim();
    
    // Check blocked first
    for blocked in BLOCKED_COMMANDS {
        if cmd_trimmed.contains(blocked) {
            return DangerLevel::Blocked;
        }
    }
    
    // Check if sudo is needed
    if cmd_trimmed.starts_with("sudo ") {
        return DangerLevel::NeedsSudo;
    }
    
    // Check dangerous
    for dangerous in DANGEROUS_COMMANDS {
        if cmd_trimmed.starts_with(dangerous) || cmd_trimmed.contains(&format!(" {}", dangerous)) {
            return DangerLevel::Dangerous;
        }
    }
    
    // Check needs confirmation
    for confirm in NEEDS_CONFIRMATION {
        if cmd_trimmed.starts_with(confirm) {
            return DangerLevel::NeedsConfirmation;
        }
    }
    
    // Check safe
    for safe in SAFE_COMMANDS {
        if cmd_trimmed.starts_with(safe) {
            return DangerLevel::Safe;
        }
    }
    
    // Default to needs confirmation for unknown commands
    DangerLevel::NeedsConfirmation
}

/// Execute a command and return structured result
pub async fn execute_command(cmd: &str, timeout_secs: u64) -> Result<CommandResult> {
    let danger = classify_command(cmd);
    
    if danger == DangerLevel::Blocked {
        return Ok(CommandResult {
            command: cmd.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: "This command is blocked for safety reasons.".to_string(),
            output: "This command is blocked for safety reasons.".to_string(),
            duration_ms: 0,
            success: false,
            summary: "Command blocked for safety".to_string(),
            needed_sudo: false,
        });
    }
    
    let start = Instant::now();
    
    // Determine shell based on OS
    let (shell, shell_arg) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        Command::new(shell)
            .arg(shell_arg)
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;
    
    let duration_ms = start.elapsed().as_millis() as u64;
    
    match output {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            let success = output.status.success();
            
            // Combine output, truncate if too long
            let mut combined = stdout.clone();
            if !stderr.is_empty() {
                if !combined.is_empty() {
                    combined.push_str("\n");
                }
                combined.push_str(&stderr);
            }
            
            // Truncate to reasonable size
            if combined.len() > 10000 {
                combined = format!("{}...\n[Output truncated, {} bytes total]", 
                    &combined[..10000], combined.len());
            }
            
            // Generate user-friendly summary
            let summary = generate_summary(cmd, &stdout, &stderr, success, duration_ms);
            
            // Check if command failed due to permission denied
            let needed_sudo = stderr.contains("Permission denied") 
                || stderr.contains("Operation not permitted")
                || stderr.contains("password");
            
            Ok(CommandResult {
                command: cmd.to_string(),
                exit_code,
                stdout,
                stderr,
                output: combined,
                duration_ms,
                success,
                summary,
                needed_sudo,
            })
        }
        Ok(Err(e)) => {
            Ok(CommandResult {
                command: cmd.to_string(),
                exit_code: -1,
                stdout: String::new(),
                stderr: e.to_string(),
                output: format!("Failed to execute: {}", e),
                duration_ms,
                success: false,
                summary: format!("Command failed: {}", e),
                needed_sudo: false,
            })
        }
        Err(_) => {
            Ok(CommandResult {
                command: cmd.to_string(),
                exit_code: -1,
                stdout: String::new(),
                stderr: "Command timed out".to_string(),
                output: format!("Command timed out after {} seconds", timeout_secs),
                duration_ms,
                success: false,
                summary: format!("Timed out after {}s", timeout_secs),
                needed_sudo: false,
            })
        }
    }
}

/// Generate a user-friendly summary of command execution
fn generate_summary(cmd: &str, stdout: &str, stderr: &str, success: bool, duration_ms: u64) -> String {
    let cmd_base = cmd.split_whitespace().next().unwrap_or(cmd);
    
    if !success {
        if stderr.contains("command not found") {
            return format!("'{}' is not installed", cmd_base);
        }
        if stderr.contains("No such file") {
            return "File or directory not found".to_string();
        }
        if stderr.contains("Permission denied") {
            return "Permission denied - may need admin access".to_string();
        }
        return format!("Command failed ({}ms)", duration_ms);
    }
    
    // Success summaries based on command type
    match cmd_base {
        "ls" | "find" | "tree" => {
            let lines = stdout.lines().count();
            format!("Found {} items ({}ms)", lines, duration_ms)
        }
        "grep" | "rg" | "ag" => {
            let matches = stdout.lines().count();
            if matches == 0 {
                "No matches found".to_string()
            } else {
                format!("Found {} matches ({}ms)", matches, duration_ms)
            }
        }
        "cat" | "head" | "tail" => {
            let lines = stdout.lines().count();
            format!("Displayed {} lines ({}ms)", lines, duration_ms)
        }
        "cp" | "mv" => "File operation complete".to_string(),
        "mkdir" => "Directory created".to_string(),
        "rm" | "rmdir" => "Deleted successfully".to_string(),
        "git" => {
            if cmd.contains("status") {
                if stdout.contains("nothing to commit") {
                    "Working tree clean".to_string()
                } else {
                    "Changes detected".to_string()
                }
            } else if cmd.contains("commit") {
                "Committed successfully".to_string()
            } else if cmd.contains("push") {
                "Pushed to remote".to_string()
            } else {
                format!("Git operation complete ({}ms)", duration_ms)
            }
        }
        "cargo" => {
            if cmd.contains("build") {
                if stdout.contains("Finished") || stderr.contains("Finished") {
                    "Build complete".to_string()
                } else {
                    "Build in progress...".to_string()
                }
            } else if cmd.contains("test") {
                if stdout.contains("passed") {
                    "Tests passed".to_string()
                } else {
                    "Tests complete".to_string()
                }
            } else {
                format!("Cargo complete ({}ms)", duration_ms)
            }
        }
        _ => format!("Complete ({}ms)", duration_ms),
    }
}

/// Parse progress from command output (for long-running commands)
pub fn parse_progress(output: &str) -> Option<u8> {
    // Look for percentage patterns
    let re = regex::Regex::new(r"(\d{1,3})%").ok()?;
    
    // Find the last percentage in the output
    let mut last_percent = None;
    for cap in re.captures_iter(output) {
        if let Some(m) = cap.get(1) {
            if let Ok(p) = m.as_str().parse::<u8>() {
                if p <= 100 {
                    last_percent = Some(p);
                }
            }
        }
    }
    
    last_percent
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_classify_safe() {
        assert_eq!(classify_command("ls -la"), DangerLevel::Safe);
        assert_eq!(classify_command("cat file.txt"), DangerLevel::Safe);
        assert_eq!(classify_command("git status"), DangerLevel::Safe);
    }
    
    #[test]
    fn test_classify_dangerous() {
        assert_eq!(classify_command("rm file.txt"), DangerLevel::Dangerous);
        assert_eq!(classify_command("chmod 777 file"), DangerLevel::Dangerous);
    }
    
    #[test]
    fn test_classify_blocked() {
        assert_eq!(classify_command("rm -rf /"), DangerLevel::Blocked);
    }
    
    #[test]
    fn test_classify_sudo() {
        assert_eq!(classify_command("sudo apt update"), DangerLevel::NeedsSudo);
    }
    
    #[test]
    fn test_parse_progress() {
        assert_eq!(parse_progress("Downloading... 50%"), Some(50));
        assert_eq!(parse_progress("Progress: 100%"), Some(100));
        assert_eq!(parse_progress("No progress here"), None);
    }
}
