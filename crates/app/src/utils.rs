//! Utility functions for the Little Helper app
//!
//! This module contains helper functions for path handling, settings management,
//! and other utility operations.

use agent_host::CommandResult;
use shared::settings::AppSettings;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static COMMAND_PATH_REGEX: OnceLock<regex::Regex> = OnceLock::new();

fn contains_forbidden_shell_ops(command: &str) -> Option<&'static str> {
    // Allow pipes and simple redirects, but block multi-command chaining and substitution.
    // This prevents common shell-injection vectors when commands are executed via a shell.
    //
    // Forbidden (outside quotes): ; && || ` $( ) <<
    let mut in_single = false;
    let mut in_double = false;
    let mut prev = '\0';
    let mut chars = command.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            // Skip escaped char
            let _ = chars.next();
            prev = c;
            continue;
        }
        if !in_double && c == '\'' {
            in_single = !in_single;
            prev = c;
            continue;
        }
        if !in_single && c == '"' {
            in_double = !in_double;
            prev = c;
            continue;
        }

        if in_single || in_double {
            prev = c;
            continue;
        }

        // Multi-command chaining
        if c == ';' {
            return Some(";");
        }
        if c == '&' {
            if chars.peek().copied() == Some('&') {
                return Some("&&");
            }
            // Allow 2>&1 only
            if prev.is_ascii_digit() && chars.peek().copied() == Some('>') {
                // ok
            } else {
                return Some("&");
            }
        }
        if c == '|' {
            if chars.peek().copied() == Some('|') {
                return Some("||");
            }
        }
        // Substitution / heredocs
        if c == '`' {
            return Some("`");
        }
        if c == '$' {
            if chars.peek().copied() == Some('(') {
                return Some("$()");
            }
        }
        if c == '<' {
            if chars.peek().copied() == Some('<') {
                return Some("<<");
            }
        }

        prev = c;
    }

    None
}

fn strip_glob_prefix(path: &str) -> &str {
    let wildcard_pos = path
        .find(|c| matches!(c, '*' | '?' | '[' | ']'))
        .unwrap_or(path.len());
    if wildcard_pos == path.len() {
        return path;
    }

    // Trim to the last separator before the wildcard
    let prefix = &path[..wildcard_pos];
    let sep_pos = prefix.rfind(|c| c == '/' || c == '\\').unwrap_or(0);
    if sep_pos == 0 {
        prefix
    } else {
        &prefix[..sep_pos]
    }
}

fn normalize_windows_env_vars(s: &str) -> String {
    // Best-effort expansion for common Windows env vars used in example commands.
    let mut out = s.to_string();
    if out.contains("%USERNAME%") {
        if let Ok(user) = std::env::var("USERNAME") {
            out = out.replace("%USERNAME%", &user);
        }
    }
    out
}

fn is_sensitive_path(path: &Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    // Credentials and secrets commonly stored here.
    s.contains("/.ssh/")
        || s.contains("\\\\.ssh\\\\")
        || s.contains("/.aws/")
        || s.contains("\\\\.aws\\\\")
        || s.contains("/.gnupg/")
        || s.contains("\\\\.gnupg\\\\")
        || s.contains("/library/keychains")
        || s.contains("\\\\library\\\\keychains")
        || s.ends_with("/.npmrc")
        || s.ends_with("\\\\.npmrc")
        || s.ends_with("/.env")
        || s.ends_with("\\\\.env")
}

/// Expand a path string that may start with ~ to the full home directory path
pub fn expand_user_path(path_str: &str) -> PathBuf {
    if let Some(stripped) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path_str)
}

/// Check if a path is within the allowed directories
pub fn is_path_in_allowed_dirs(path: &Path, allowed_dirs: &[String]) -> bool {
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

/// Run a user command using the agent_host
pub fn run_user_command(command: &str) -> Result<CommandResult, String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime
        .block_on(agent_host::execute_command(command, 60))
        .map_err(|e| e.to_string())
}

/// Check if preloaded OpenAI is enabled
pub fn preload_openai_enabled() -> bool {
    match std::env::var("LH_DISABLE_PRELOAD_OPENAI") {
        Ok(v) if v == "1" || v.to_lowercase() == "true" => false,
        _ => true,
    }
}

/// Get the config file path
pub fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|mut p| {
        p.push("little_helper");
        p.push("settings.json");
        p
    })
}

/// Load settings from disk or return defaults
pub fn load_settings_or_default() -> (AppSettings, bool) {
    if let Some(path) = config_path() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&contents) {
                return (settings, true);
            }
        }
    }

    // First-run helper: import a seed settings file if bundled with the app.
    // This can power a "just works" tester build without requiring setup.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let mut seed_dirs = vec![exe_dir.to_path_buf()];

            // On macOS app bundles, also check Contents/Resources
            if let Some(contents_dir) = exe_dir.parent().and_then(|p| p.parent()) {
                seed_dirs.push(contents_dir.join("Resources"));
            }

            for dir in seed_dirs {
                let seed_path = dir.join("seed-settings.json");
                if let Ok(contents) = std::fs::read_to_string(&seed_path) {
                    if let Ok(mut settings) = serde_json::from_str::<AppSettings>(&contents) {
                        // Optional: import a bundled mascot image too.
                        let mascot_seed = [
                            dir.join("seed-mascot.png"),
                            dir.join("seed-mascot.jpg"),
                            dir.join("seed-mascot.jpeg"),
                        ]
                        .into_iter()
                        .find(|p| p.exists());

                        if let Some(mascot_seed) = mascot_seed {
                            if let Some(cfg) =
                                config_path().and_then(|p| p.parent().map(|p| p.to_path_buf()))
                            {
                                let _ = std::fs::create_dir_all(&cfg);
                                let dest = cfg.join("mascot.png");
                                if std::fs::copy(&mascot_seed, &dest).is_ok() {
                                    settings.user_profile.mascot_image_path =
                                        Some(dest.to_string_lossy().to_string());
                                }
                            }
                        }

                        save_settings(&settings);

                        // Best-effort cleanup (may fail if running from a read-only volume)
                        let _ = std::fs::remove_file(&seed_path);
                        let _ = std::fs::remove_file(&dir.join("seed-mascot.png"));
                        let _ = std::fs::remove_file(&dir.join("seed-mascot.jpg"));
                        let _ = std::fs::remove_file(&dir.join("seed-mascot.jpeg"));

                        return (settings, true);
                    }
                }
            }
        }
    }

    (AppSettings::default(), false)
}

/// Clean up AI response by removing thinking tags and normalizing whitespace
pub fn clean_ai_response(response: &str) -> String {
    // Remove <thinking> tags and their content
    let thinking_regex = regex::Regex::new(r"<thinking>.*?</thinking>").unwrap();
    let cleaned = thinking_regex.replace_all(response, "");

    // Remove any leading/trailing whitespace and normalize newlines
    cleaned
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Format an error message with helpful context
pub fn format_error_message(error: &str) -> String {
    format!(
        "I encountered an error while processing your request:\n\n```\n{}\n```\n\n\
        Please try again or rephrase your request.",
        error
    )
}

/// Extract file paths from text that should be clickable
pub fn extract_paths(text: &str, allowed_dirs: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Simple path extraction - look for common path patterns
    for word in text.split_whitespace() {
        let cleaned =
            word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '~');
        if cleaned.starts_with('/') || cleaned.starts_with("~/") {
            let path = expand_user_path(cleaned);
            if path.exists() && is_path_in_allowed_dirs(&path, allowed_dirs) {
                paths.push(path);
            }
        }
    }

    paths
}

pub fn extract_previewable_file(text: &str, allowed_dirs: &[String]) -> Option<PathBuf> {
    // Prefer a single, useful file preview (images/pdfs) if the model references one.
    // This is a best-effort helper; strict permission checks still apply.
    let re = regex::Regex::new(r#"(?P<p>(?:~\/|\/|\./|\.\./)[^\s"']+\.(?:png|jpg|jpeg|gif|pdf))"#)
        .ok()?;
    for cap in re.captures_iter(text) {
        if let Some(m) = cap.name("p") {
            let p = expand_user_path(m.as_str());
            if p.exists() && is_path_in_allowed_dirs(&p, allowed_dirs) {
                return Some(p);
            }
        }
    }
    None
}

/// Save settings to disk
pub fn save_settings(settings: &AppSettings) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = std::fs::write(&path, json);
        }
    }
}

/// Ensure allowed directories are set up correctly
pub fn ensure_allowed_dirs(settings: &mut AppSettings) {
    if settings.allowed_dirs.is_empty() {
        if let Some(home) = dirs::home_dir() {
            settings
                .allowed_dirs
                .push(home.to_string_lossy().to_string());
        }
    }
}

/// Normalize allowed directory input
pub fn normalize_allowed_dir_input(input: &str) -> Option<PathBuf> {
    let expanded = expand_user_path(input.trim());
    let canonical = expanded.canonicalize().unwrap_or(expanded);

    if canonical.exists() && canonical.is_dir() {
        Some(canonical)
    } else {
        None
    }
}

/// Validate a command against allowed directories
pub fn validate_command_against_allowed(
    command: &str,
    allowed_dirs: &[String],
) -> Result<(), String> {
    if allowed_dirs.is_empty() {
        return Err("No folders are allowed. Add one in Settings first.".to_string());
    }

    if let Some(op) = contains_forbidden_shell_ops(command) {
        return Err(format!(
            "This command includes a blocked shell feature ({}). Please run one step at a time.",
            op
        ));
    }

    // Block environment dumps (high risk for accidental secret exposure)
    let cmd_trim = command.trim().to_lowercase();
    if cmd_trim == "env" || cmd_trim.starts_with("env ") || cmd_trim == "printenv" {
        return Err("For privacy, printing all environment variables is blocked.".to_string());
    }

    let regex = COMMAND_PATH_REGEX.get_or_init(|| {
        regex::Regex::new(r#"(?P<path>(?:~|/|\./|\.\./|[A-Za-z]:\\)[^\s"'`]+)"#).unwrap()
    });

    for capture in regex.captures_iter(command) {
        if let Some(path_match) = capture.name("path") {
            let raw = path_match.as_str();
            let raw = normalize_windows_env_vars(raw);
            let raw = strip_glob_prefix(raw.as_str());
            let candidate = expand_user_path(raw);

            // Sensitive locations are blocked by default.
            if is_sensitive_path(&candidate) {
                return Err(format!(
                    "This command touches a sensitive path (`{}`). For safety, Little Helper blocks this by default.",
                    raw
                ));
            }

            // If the path doesn't exist (e.g. redirect target), validate its parent.
            let to_check = if candidate.exists() {
                candidate
            } else {
                candidate
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or(candidate)
            };

            if !is_path_in_allowed_dirs(&to_check, allowed_dirs) {
                return Err(format!("Path `{}` is outside the allowed folders.", raw));
            }
        }
    }

    Ok(())
}
