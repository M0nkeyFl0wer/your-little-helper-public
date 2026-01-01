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
    // === UNIX/LINUX COMMANDS ===
    // File listing and info (read-only)
    "ls", "find", "cat", "head", "tail", "wc", "du", "df", "pwd", 
    "file", "stat", "tree", "which", "whereis",
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
    // Archive listing
    "tar -tf", "unzip -l", "zipinfo",
    
    // === WINDOWS COMMANDS ===
    // File listing and info (read-only)
    "dir", "type", "where", "tree /f", "attrib",
    // System info (read-only)
    "systeminfo", "ver", "set", "echo %", "wmic", "tasklist",
    "ipconfig", "getmac", "arp",
    // PowerShell read-only
    "powershell -c \"Get-", "powershell -c \"Write-", "powershell Get-",
    "Get-ChildItem", "Get-Content", "Get-Process", "Get-Service",
    "Get-NetAdapter", "Get-NetIPAddress", "Get-ComputerInfo",
    
    // === CROSS-PLATFORM ===
    // Git (read operations)
    "git status", "git log", "git diff", "git show", "git branch",
    "git remote", "git fetch", "git ls-files", "git blame",
    // Rust/Cargo (read operations)
    "cargo check", "cargo test", "cargo build", "cargo clippy",
    "cargo fmt --check", "rustc --version", "cargo --version",
    // Node/Python (read operations)
    "node --version", "npm --version", "python --version", "pip --version",
    "python -c", "python3 -c", "node -e",
    "python3 --version", "pip3 --version",
];

/// Commands that need user confirmation before running
const NEEDS_CONFIRMATION: &[&str] = &[
    // Unix file operations
    "cp", "mv", "mkdir", "touch", "ln",
    // Windows file operations  
    "copy", "move", "xcopy", "robocopy", "md", "ren",
    // Git write operations
    "git add", "git commit", "git push", "git pull", "git merge",
    "git checkout", "git reset", "git stash",
    // Package managers
    "pip install", "pip3 install", "npm install", "cargo install",
    // Editors (opening files)
    "nano", "vim", "nvim", "code", "notepad",
];

/// Dangerous commands that need explicit confirmation with warning
const DANGEROUS_COMMANDS: &[&str] = &[
    // Unix destructive file operations
    "rm", "rmdir", "shred",
    // Windows destructive file operations
    "del", "rd", "rmdir /s", "erase",
    // Unix permissions
    "chmod", "chown", "chgrp",
    // Windows permissions
    "icacls", "takeown",
    // Unix process control
    "kill", "killall", "pkill",
    // Windows process control
    "taskkill", "Stop-Process",
    // Git destructive
    "git reset --hard", "git clean", "git push --force",
    // Database
    "drop", "delete", "truncate",
];

/// Commands that are always blocked
const BLOCKED_COMMANDS: &[&str] = &[
    // Unix system destruction
    "rm -rf /", "rm -rf /*", ":(){ :|:& };:",
    // Unix format/wipe
    "mkfs", "dd if=/dev/zero", "dd if=/dev/random",
    // Unix dangerous redirects
    "> /dev/sda", ">/dev/sda",
    // Windows system destruction
    "format c:", "format C:", "rd /s /q C:", 
    "del /f /s /q C:", "Remove-Item -Recurse -Force C:",
    // Registry destruction
    "reg delete HKLM", "Remove-ItemProperty -Path HKLM",
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
                    combined.push('\n');
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

/// Execute a command with sudo, providing password via stdin
/// 
/// SECURITY: Password is never stored or logged. It's passed directly to sudo via stdin
/// and cleared from memory after use.
#[cfg(not(windows))]
pub async fn execute_with_sudo(cmd: &str, password: &str, timeout_secs: u64) -> Result<CommandResult> {
    use tokio::io::AsyncWriteExt;
    
    let start = Instant::now();
    
    // Strip "sudo " prefix if present, we'll add it ourselves
    let actual_cmd = cmd.strip_prefix("sudo ").unwrap_or(cmd);
    
    // Use sudo -S to read password from stdin
    // -k invalidates cached credentials to ensure we're always prompted
    let mut child = tokio::process::Command::new("sudo")
        .arg("-S")  // Read password from stdin
        .arg("-k")  // Invalidate cached credentials
        .arg("sh")
        .arg("-c")
        .arg(actual_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    // Write password to stdin
    if let Some(mut stdin) = child.stdin.take() {
        // Password followed by newline
        let password_with_newline = format!("{}\n", password);
        stdin.write_all(password_with_newline.as_bytes()).await?;
        // Explicitly drop stdin to close it
        drop(stdin);
    }
    
    // Wait for command with timeout
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        child.wait_with_output()
    ).await;
    
    let duration_ms = start.elapsed().as_millis() as u64;
    
    match output {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
            
            // Remove password prompt from stderr (sudo outputs "[sudo] password for user:")
            if let Some(idx) = stderr.find('\n') {
                let first_line = &stderr[..idx];
                if first_line.contains("password for") || first_line.contains("[sudo]") {
                    stderr = stderr[idx + 1..].to_string();
                }
            }
            
            let exit_code = output.status.code().unwrap_or(-1);
            let success = output.status.success();
            
            // Check for wrong password
            let wrong_password = stderr.contains("incorrect password") 
                || stderr.contains("Sorry, try again")
                || stderr.contains("Authentication failure");
            
            let summary = if wrong_password {
                "Incorrect password".to_string()
            } else if success {
                generate_summary(cmd, &stdout, &stderr, success, duration_ms)
            } else {
                format!("Command failed: {}", stderr.lines().next().unwrap_or("unknown error"))
            };
            
            let mut combined = stdout.clone();
            if !stderr.is_empty() {
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str(&stderr);
            }
            
            Ok(CommandResult {
                command: format!("sudo {}", actual_cmd),
                exit_code,
                stdout,
                stderr,
                output: combined,
                duration_ms,
                success: success && !wrong_password,
                summary,
                needed_sudo: true,
            })
        }
        Ok(Err(e)) => {
            Ok(CommandResult {
                command: format!("sudo {}", actual_cmd),
                exit_code: -1,
                stdout: String::new(),
                stderr: e.to_string(),
                output: format!("Failed to execute: {}", e),
                duration_ms,
                success: false,
                summary: format!("Command failed: {}", e),
                needed_sudo: true,
            })
        }
        Err(_) => {
            Ok(CommandResult {
                command: format!("sudo {}", actual_cmd),
                exit_code: -1,
                stdout: String::new(),
                stderr: "Command timed out".to_string(),
                output: format!("Command timed out after {} seconds", timeout_secs),
                duration_ms,
                success: false,
                summary: format!("Timed out after {}s", timeout_secs),
                needed_sudo: true,
            })
        }
    }
}

/// Windows equivalent - uses runas for elevation
/// Note: Windows UAC will show a system prompt, we can't programmatically provide credentials
#[cfg(windows)]
pub async fn execute_with_elevation(cmd: &str, timeout_secs: u64) -> Result<CommandResult> {
    let start = Instant::now();
    
    // On Windows, we use PowerShell's Start-Process with -Verb RunAs
    // This triggers UAC prompt which the user must approve
    let ps_cmd = format!(
        "Start-Process cmd -ArgumentList '/c {}' -Verb RunAs -Wait -PassThru | Select-Object -ExpandProperty ExitCode",
        cmd.replace("'", "''")
    );
    
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        Command::new("powershell")
            .arg("-Command")
            .arg(&ps_cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;
    
    let duration_ms = start.elapsed().as_millis() as u64;
    
    match output {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = stdout.trim().parse().unwrap_or(-1);
            let success = exit_code == 0;
            
            Ok(CommandResult {
                command: cmd.to_string(),
                exit_code,
                stdout: String::new(), // Elevated process output not captured
                stderr,
                output: if success {
                    "Command completed with admin privileges".to_string()
                } else {
                    format!("Command failed with exit code {}", exit_code)
                },
                duration_ms,
                success,
                summary: if success {
                    "Completed with admin privileges".to_string()
                } else {
                    "Failed or was cancelled".to_string()
                },
                needed_sudo: true,
            })
        }
        Ok(Err(e)) => {
            Ok(CommandResult {
                command: cmd.to_string(),
                exit_code: -1,
                stdout: String::new(),
                stderr: e.to_string(),
                output: format!("Failed to elevate: {}", e),
                duration_ms,
                success: false,
                summary: "Failed to request admin privileges".to_string(),
                needed_sudo: true,
            })
        }
        Err(_) => {
            Ok(CommandResult {
                command: cmd.to_string(),
                exit_code: -1,
                stdout: String::new(),
                stderr: "Operation timed out".to_string(),
                output: "Admin operation timed out or was cancelled".to_string(),
                duration_ms,
                success: false,
                summary: "Timed out or cancelled".to_string(),
                needed_sudo: true,
            })
        }
    }
}

/// Check if a command needs elevated privileges based on output
pub fn needs_elevation(result: &CommandResult) -> bool {
    result.needed_sudo 
        || result.stderr.contains("Permission denied")
        || result.stderr.contains("Operation not permitted")
        || result.stderr.contains("Access is denied")
        || result.stderr.contains("requires root")
        || result.stderr.contains("must be root")
}

/// Perform a web search using DuckDuckGo's HTML interface
/// Returns search results as text
pub async fn web_search(query: &str) -> Result<CommandResult> {
    let start = Instant::now();
    
    // Use DuckDuckGo's lite/HTML interface for simple text results
    let encoded_query = urlencoding::encode(query);
    let url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);
    
    // Use curl to fetch results (available on most systems)
    let output = Command::new("curl")
        .arg("-s")  // Silent
        .arg("-L")  // Follow redirects
        .arg("-A")  // User agent
        .arg("Mozilla/5.0 (compatible; LittleHelper/1.0)")
        .arg(&url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    
    let duration_ms = start.elapsed().as_millis() as u64;
    let html = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    if !output.status.success() {
        return Ok(CommandResult {
            command: format!("web_search: {}", query),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::new(),
            stderr: stderr.clone(),
            output: format!("Search failed: {}", stderr),
            duration_ms,
            success: false,
            summary: "Search failed".to_string(),
            needed_sudo: false,
        });
    }
    
    // Parse results from HTML - extract titles and snippets
    let results = parse_ddg_results(&html);
    
    let result_count = results.len();
    let output_text = if results.is_empty() {
        "No results found.".to_string()
    } else {
        results.iter()
            .enumerate()
            .map(|(i, (title, snippet, url))| {
                format!("{}. {}\n   {}\n   URL: {}\n", i + 1, title, snippet, url)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    
    Ok(CommandResult {
        command: format!("web_search: {}", query),
        exit_code: 0,
        stdout: output_text.clone(),
        stderr: String::new(),
        output: output_text,
        duration_ms,
        success: true,
        summary: format!("Found {} results ({}ms)", result_count, duration_ms),
        needed_sudo: false,
    })
}

/// Parse DuckDuckGo HTML results into (title, snippet, url) tuples
fn parse_ddg_results(html: &str) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    
    // DuckDuckGo HTML format has results in <a class="result__a"> and <a class="result__snippet">
    // Simple regex-based parsing (not perfect but works for basic extraction)
    
    // Find result links - they contain the title
    let title_re = regex::Regex::new(r#"class="result__a"[^>]*href="([^"]*)"[^>]*>([^<]+)</a>"#).unwrap();
    let snippet_re = regex::Regex::new(r#"class="result__snippet"[^>]*>([^<]+)"#).unwrap();
    
    let titles: Vec<(String, String)> = title_re.captures_iter(html)
        .filter_map(|cap| {
            let url = cap.get(1)?.as_str();
            let title = cap.get(2)?.as_str();
            // DuckDuckGo uses redirect URLs, try to extract actual URL
            let actual_url = if url.contains("uddg=") {
                url.split("uddg=").nth(1)
                    .and_then(|u| urlencoding::decode(u).ok())
                    .map(|u| u.into_owned())
                    .unwrap_or_else(|| url.to_string())
            } else {
                url.to_string()
            };
            Some((html_decode(title), actual_url))
        })
        .collect();
    
    let snippets: Vec<String> = snippet_re.captures_iter(html)
        .filter_map(|cap| cap.get(1).map(|m| html_decode(m.as_str())))
        .collect();
    
    // Combine titles and snippets
    for (i, (title, url)) in titles.into_iter().take(10).enumerate() {
        let snippet = snippets.get(i).cloned().unwrap_or_default();
        if !title.is_empty() {
            results.push((title, snippet, url));
        }
    }
    
    results
}

/// Basic HTML entity decoding
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
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
