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
    "ls",
    "find",
    "cat",
    "head",
    "tail",
    "wc",
    "du",
    "df",
    "pwd",
    "file",
    "stat",
    "tree",
    "which",
    "whereis",
    // Text processing (read-only — sed/awk excluded, they can modify files)
    "grep",
    "rg",
    "ag",
    "sort",
    "uniq",
    "cut",
    "tr",
    "diff",
    "comm",
    "join",
    "paste",
    "column",
    // System info (read-only)
    "uname",
    "hostname",
    "uptime",
    "free",
    "ps",
    "top",
    "htop",
    "lscpu",
    "lsblk",
    "lsusb",
    "lspci",
    "lsof",
    "id",
    "whoami",
    "date",
    "cal",
    "env",
    "printenv",
    // Network info (read-only)
    "ip",
    "ifconfig",
    "netstat",
    "ss",
    "ping",
    "nslookup",
    "dig",
    "host",
    "traceroute",
    "curl",
    "wget",
    // Archive listing
    "tar -tf",
    "unzip -l",
    "zipinfo",
    // === WINDOWS COMMANDS ===
    // File listing and info (read-only)
    "dir",
    "type",
    "where",
    "tree /f",
    "attrib",
    // Text search (read-only)
    "findstr",
    // System info (read-only)
    "systeminfo",
    "ver",
    "set",
    "echo %",
    "wmic",
    "tasklist",
    "ipconfig",
    "getmac",
    "arp",
    "netstat",
    // PowerShell read-only
    "powershell -c \"Get-",
    "powershell -c \"Write-",
    "powershell Get-",
    "Get-ChildItem",
    "Get-Content",
    "Get-Process",
    "Get-Service",
    "Get-NetAdapter",
    "Get-NetIPAddress",
    "Get-ComputerInfo",
    // === CROSS-PLATFORM ===
    // Git (read operations)
    "git status",
    "git log",
    "git diff",
    "git show",
    "git branch",
    "git remote",
    "git fetch",
    "git ls-files",
    "git blame",
    // Rust/Cargo (read-only operations — build/test excluded, they run arbitrary code)
    "cargo check",
    "cargo clippy",
    "cargo fmt --check",
    "rustc --version",
    "cargo --version",
    // Node/Python (read operations)
    "node --version",
    "npm --version",
    "python --version",
    "pip --version",
    "python -c",
    "python3 -c",
    "node -e",
    "python3 --version",
    "pip3 --version",
];

/// Commands that need user confirmation before running
const NEEDS_CONFIRMATION: &[&str] = &[
    // Unix file operations
    "cp",
    "mv",
    "mkdir",
    "touch",
    "ln",
    // Windows file operations
    "copy",
    "move",
    "xcopy",
    "robocopy",
    "md",
    "ren",
    // Git write operations
    "git add",
    "git commit",
    "git push",
    "git pull",
    "git merge",
    "git checkout",
    "git reset",
    "git stash",
    // Package managers
    "pip install",
    "pip3 install",
    "npm install",
    "cargo install",
    // Editors (opening files)
    "nano",
    "vim",
    "nvim",
    "code",
    "notepad",
];

/// Dangerous commands that need explicit confirmation with warning
const DANGEROUS_COMMANDS: &[&str] = &[
    // Unix destructive file operations
    "rm",
    "rmdir",
    "shred",
    // Windows destructive file operations
    "del",
    "rd",
    "rmdir /s",
    "erase",
    // Unix permissions
    "chmod",
    "chown",
    "chgrp",
    // Windows permissions
    "icacls",
    "takeown",
    // Unix process control
    "kill",
    "killall",
    "pkill",
    // Windows process control
    "taskkill",
    "Stop-Process",
    // Git destructive
    "git reset --hard",
    "git clean",
    "git push --force",
    // Database
    "drop",
    "delete",
    "truncate",
];

/// Commands that are always blocked
const BLOCKED_COMMANDS: &[&str] = &[
    // Unix system destruction
    "rm -rf /",
    "rm -rf /*",
    ":(){ :|:& };:",
    // Unix format/wipe
    "mkfs",
    "dd if=/dev/zero",
    "dd if=/dev/random",
    // Unix dangerous redirects
    "> /dev/sda",
    ">/dev/sda",
    // Windows system destruction
    "format c:",
    "format C:",
    "rd /s /q C:",
    "del /f /s /q C:",
    "Remove-Item -Recurse -Force C:",
    // Registry destruction
    "reg delete HKLM",
    "Remove-ItemProperty -Path HKLM",
    // Network attacks
    "nc -l",
    "nmap",
];

/// Translate common Unix commands to Windows equivalents.
///
/// LLMs frequently suggest Unix commands even when prompted for Windows.
/// Rather than failing silently, we translate the most common ones so the
/// experience works out of the box. This only runs on Windows builds.
#[cfg(windows)]
fn translate_unix_to_windows(cmd: &str) -> String {
    let trimmed = cmd.trim();
    let lower = trimmed.to_lowercase();

    // Split into first token and the rest
    let (first, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((f, r)) => (f, r.trim()),
        None => (trimmed, ""),
    };
    let first_lower = first.to_lowercase();

    match first_lower.as_str() {
        // ls → dir
        "ls" => {
            if rest.is_empty() {
                "dir".to_string()
            } else {
                // Strip common ls flags the user won't notice
                let cleaned: Vec<&str> = rest
                    .split_whitespace()
                    .filter(|a| !a.starts_with('-'))
                    .collect();
                if cleaned.is_empty() {
                    "dir".to_string()
                } else {
                    format!("dir {}", cleaned.join(" "))
                }
            }
        }
        // cat → type
        "cat" => {
            if rest.is_empty() {
                trimmed.to_string()
            } else {
                format!("type {}", rest)
            }
        }
        // grep → findstr
        "grep" => {
            // Very rough: grep -r "pattern" path  →  findstr /s /i "pattern" path\*
            let args: Vec<&str> = rest.split_whitespace().collect();
            let mut pattern = None;
            let mut path = None;
            let mut recursive = false;
            let mut case_insensitive = false;
            let mut skip_next = false;
            for (i, arg) in args.iter().enumerate() {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if *arg == "-r" || *arg == "-R" || *arg == "--recursive" {
                    recursive = true;
                } else if *arg == "-i" || *arg == "--ignore-case" {
                    case_insensitive = true;
                } else if arg.starts_with("--include=") {
                    // Skip file type filters for simplicity
                } else if arg.starts_with('-') {
                    // Skip other flags
                } else if pattern.is_none() {
                    pattern = Some(*arg);
                } else {
                    path = Some(*arg);
                }
            }
            let pat = pattern.unwrap_or("\"\"");
            let mut findstr = String::from("findstr");
            if recursive {
                findstr.push_str(" /s");
            }
            if case_insensitive {
                findstr.push_str(" /i");
            }
            findstr.push_str(&format!(" {}", pat));
            if let Some(p) = path {
                findstr.push_str(&format!(" {}\\*", p));
            }
            findstr
        }
        // pwd → cd (with no args, prints current dir on Windows)
        "pwd" => "cd".to_string(),
        // which → where
        "which" => format!("where {}", rest),
        // uname → systeminfo (rough equivalent)
        "uname" => "systeminfo".to_string(),
        // df → wmic logicaldisk
        "df" => "wmic logicaldisk get caption,freespace,size".to_string(),
        // free → systeminfo (contains memory info)
        "free" => {
            "wmic OS get FreePhysicalMemory,TotalVisibleMemorySize /Value".to_string()
        }
        // ps → tasklist
        "ps" => "tasklist".to_string(),
        // kill → taskkill
        "kill" => format!("taskkill /PID {}", rest),
        // head → powershell Select-Object
        "head" => {
            // head -n 20 file → powershell -c "Get-Content file | Select-Object -First 20"
            let args: Vec<&str> = rest.split_whitespace().collect();
            let mut n = 10;
            let mut file = "";
            let mut skip_next = false;
            for (i, arg) in args.iter().enumerate() {
                if skip_next { skip_next = false; continue; }
                if *arg == "-n" || *arg == "-" {
                    if let Some(next) = args.get(i + 1) {
                        n = next.parse().unwrap_or(10);
                        skip_next = true;
                    }
                } else if arg.starts_with('-') && arg.len() > 1 {
                    // -20 style
                    n = arg[1..].parse().unwrap_or(10);
                } else {
                    file = arg;
                }
            }
            if file.is_empty() {
                trimmed.to_string()
            } else {
                format!(
                    "powershell -c \"Get-Content '{}' | Select-Object -First {}\"",
                    file, n
                )
            }
        }
        // tail → powershell Select-Object -Last
        "tail" => {
            let args: Vec<&str> = rest.split_whitespace().collect();
            let mut n = 10;
            let mut file = "";
            let mut skip_next = false;
            for (i, arg) in args.iter().enumerate() {
                if skip_next { skip_next = false; continue; }
                if *arg == "-n" {
                    if let Some(next) = args.get(i + 1) {
                        n = next.parse().unwrap_or(10);
                        skip_next = true;
                    }
                } else if arg.starts_with('-') && arg.len() > 1 {
                    n = arg[1..].parse().unwrap_or(10);
                } else {
                    file = arg;
                }
            }
            if file.is_empty() {
                trimmed.to_string()
            } else {
                format!(
                    "powershell -c \"Get-Content '{}' | Select-Object -Last {}\"",
                    file, n
                )
            }
        }
        // find (the Unix one, not Windows find.exe) → dir /s /b
        "find" if rest.contains("-name") || rest.contains("-iname") => {
            // Rough: find /path -name "*.txt"  →  dir /s /b "path\*.txt"
            let args: Vec<&str> = rest.split_whitespace().collect();
            let mut search_path = ".";
            let mut pattern = "*";
            let mut skip_next = false;
            for (i, arg) in args.iter().enumerate() {
                if skip_next { skip_next = false; continue; }
                if *arg == "-name" || *arg == "-iname" {
                    if let Some(next) = args.get(i + 1) {
                        pattern = next.trim_matches('"').trim_matches('\'');
                        skip_next = true;
                    }
                } else if !arg.starts_with('-') && i == 0 {
                    search_path = arg;
                }
            }
            let win_path = search_path.replace('/', "\\").replace("~", "%USERPROFILE%");
            format!("dir /s /b \"{}\\{}\"", win_path, pattern)
        }
        // chmod, chown → no-op on Windows, just explain
        "chmod" | "chown" => {
            format!("echo Permission commands are not needed on Windows (was: {} {})", first, rest)
        }
        // Everything else passes through unchanged
        _ => trimmed.to_string(),
    }
}

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

    // On Windows, translate common Unix commands so the AI doesn't have to
    // get it right every time. This is a safety net, not a replacement for
    // platform-aware prompting.
    #[cfg(windows)]
    let cmd = &translate_unix_to_windows(cmd);

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
            .output(),
    )
    .await;

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

            // Truncate to reasonable size (char-safe to avoid UTF-8 panics)
            if combined.len() > 10000 {
                let total = combined.len();
                let truncated: String = combined.chars().take(10000).collect();
                combined = format!(
                    "{}...\n[Output truncated, {} bytes total]",
                    truncated, total
                );
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
        Ok(Err(e)) => Ok(CommandResult {
            command: cmd.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: e.to_string(),
            output: format!("Failed to execute: {}", e),
            duration_ms,
            success: false,
            summary: format!("Command failed: {}", e),
            needed_sudo: false,
        }),
        Err(_) => Ok(CommandResult {
            command: cmd.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: "Command timed out".to_string(),
            output: format!("Command timed out after {} seconds", timeout_secs),
            duration_ms,
            success: false,
            summary: format!("Timed out after {}s", timeout_secs),
            needed_sudo: false,
        }),
    }
}

/// Generate a user-friendly summary of command execution
fn generate_summary(
    cmd: &str,
    stdout: &str,
    stderr: &str,
    success: bool,
    duration_ms: u64,
) -> String {
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
pub async fn execute_with_sudo(
    cmd: &str,
    password: &str,
    timeout_secs: u64,
) -> Result<CommandResult> {
    use tokio::io::AsyncWriteExt;

    let start = Instant::now();

    // Strip "sudo " prefix if present, we'll add it ourselves
    let actual_cmd = cmd.strip_prefix("sudo ").unwrap_or(cmd);

    // Use sudo -S to read password from stdin
    // -k invalidates cached credentials to ensure we're always prompted
    let mut child = tokio::process::Command::new("sudo")
        .arg("-S") // Read password from stdin
        .arg("-k") // Invalidate cached credentials
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
    let output =
        tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;

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
                format!(
                    "Command failed: {}",
                    stderr.lines().next().unwrap_or("unknown error")
                )
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
        Ok(Err(e)) => Ok(CommandResult {
            command: format!("sudo {}", actual_cmd),
            exit_code: -1,
            stdout: String::new(),
            stderr: e.to_string(),
            output: format!("Failed to execute: {}", e),
            duration_ms,
            success: false,
            summary: format!("Command failed: {}", e),
            needed_sudo: true,
        }),
        Err(_) => Ok(CommandResult {
            command: format!("sudo {}", actual_cmd),
            exit_code: -1,
            stdout: String::new(),
            stderr: "Command timed out".to_string(),
            output: format!("Command timed out after {} seconds", timeout_secs),
            duration_ms,
            success: false,
            summary: format!("Timed out after {}s", timeout_secs),
            needed_sudo: true,
        }),
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
            .output(),
    )
    .await;

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
        Ok(Err(e)) => Ok(CommandResult {
            command: cmd.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: e.to_string(),
            output: format!("Failed to elevate: {}", e),
            duration_ms,
            success: false,
            summary: "Failed to request admin privileges".to_string(),
            needed_sudo: true,
        }),
        Err(_) => Ok(CommandResult {
            command: cmd.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: "Operation timed out".to_string(),
            output: "Admin operation timed out or was cancelled".to_string(),
            duration_ms,
            success: false,
            summary: "Timed out or cancelled".to_string(),
            needed_sudo: true,
        }),
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

/// Web search using Brave Search API (free tier: 2000 queries/month).
/// Falls back to Wikipedia API if no Brave key is set.
pub async fn web_search(query: &str) -> Result<CommandResult> {
    // Try Brave Search API first (if key is available)
    if let Ok(api_key) = std::env::var("BRAVE_SEARCH_API_KEY") {
        return brave_search(query, &api_key).await;
    }
    // Fallback: Wikipedia API (always works, no CAPTCHA)
    fallback_search(query).await
}

async fn brave_search(query: &str, api_key: &str) -> Result<CommandResult> {
    let start = Instant::now();
    let encoded = urlencoding::encode(query);
    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}&count=8",
        encoded
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| anyhow::anyhow!("HTTP client error: {}", e))?;

    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .header("X-Subscription-Token", api_key)
        .send()
        .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    let body = match resp {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        Ok(r) => {
            let status = r.status();
            return Ok(CommandResult {
                command: format!("web_search: {}", query),
                exit_code: status.as_u16() as i32,
                stdout: String::new(),
                stderr: format!("Brave API HTTP {}", status),
                output: format!("Search failed: HTTP {} — check your BRAVE_SEARCH_API_KEY", status),
                duration_ms,
                success: false,
                summary: "Search failed".to_string(),
                needed_sudo: false,
            });
        }
        Err(e) => {
            return Ok(CommandResult {
                command: format!("web_search: {}", query),
                exit_code: -1,
                stdout: String::new(),
                stderr: e.to_string(),
                output: format!("Search failed: {}", e),
                duration_ms,
                success: false,
                summary: "Search failed".to_string(),
                needed_sudo: false,
            });
        }
    };

    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let web_results = parsed["web"]["results"].as_array();

    let output_text = match web_results {
        Some(results) if !results.is_empty() => {
            results
                .iter()
                .take(8)
                .enumerate()
                .map(|(i, r)| {
                    let title = r["title"].as_str().unwrap_or("");
                    let desc = r["description"].as_str().unwrap_or("");
                    let url = r["url"].as_str().unwrap_or("");
                    format!("{}. {}\n   {}\n   URL: {}\n", i + 1, title, desc, url)
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => "No results found.".to_string(),
    };

    let count = web_results.map(|r| r.len().min(8)).unwrap_or(0);

    Ok(CommandResult {
        command: format!("web_search: {}", query),
        exit_code: 0,
        stdout: output_text.clone(),
        stderr: String::new(),
        output: output_text,
        duration_ms,
        success: true,
        summary: format!("Found {} results ({}ms)", count, duration_ms),
        needed_sudo: false,
    })
}

async fn fallback_search(query: &str) -> Result<CommandResult> {
    let start = Instant::now();

    // Use Wikipedia API as minimal fallback — always works, no CAPTCHA
    let encoded = urlencoding::encode(query);
    let url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit=5",
        encoded
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("LittleHelper/1.0 (Desktop App)")
        .build()
        .map_err(|e| anyhow::anyhow!("HTTP client error: {}", e))?;

    let resp = client.get(&url).send().await;
    let duration_ms = start.elapsed().as_millis() as u64;

    let body = match resp {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => {
            return Ok(CommandResult {
                command: format!("web_search: {}", query),
                exit_code: 1,
                stdout: String::new(),
                stderr: "All search backends failed".to_string(),
                output: "Search is currently unavailable. To enable full web search, add a free Brave Search API key in Settings.\nGet one at: https://api-dashboard.search.brave.com".to_string(),
                duration_ms,
                success: false,
                summary: "Search unavailable".to_string(),
                needed_sudo: false,
            });
        }
    };

    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let search_results = parsed["query"]["search"].as_array();
    let html_tag_re = regex::Regex::new(r"<[^>]+>").unwrap();

    let output_text = match search_results {
        Some(results) if !results.is_empty() => {
            let mut out = String::from("Results from Wikipedia (for full web search, add a Brave Search API key in Settings):\n\n");
            for (i, r) in results.iter().take(5).enumerate() {
                let title = r["title"].as_str().unwrap_or("");
                let snippet = r["snippet"].as_str().unwrap_or("");
                let clean_snippet = html_tag_re.replace_all(snippet, "").to_string();
                out.push_str(&format!(
                    "{}. {}\n   {}\n   URL: https://en.wikipedia.org/wiki/{}\n\n",
                    i + 1, title, clean_snippet,
                    urlencoding::encode(title)
                ));
            }
            out
        }
        _ => "No results found. For full web search, add a Brave Search API key in Settings.\nGet one free at: https://api-dashboard.search.brave.com".to_string(),
    };

    let count = search_results.map(|r| r.len().min(5)).unwrap_or(0);

    Ok(CommandResult {
        command: format!("web_search: {}", query),
        exit_code: 0,
        stdout: output_text.clone(),
        stderr: String::new(),
        output: output_text,
        duration_ms,
        success: count > 0,
        summary: format!("Found {} results ({}ms)", count, duration_ms),
        needed_sudo: false,
    })
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
