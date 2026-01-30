//! Utility functions for the Little Helper app
//!
//! This module contains helper functions for path handling, settings management,
//! and other utility operations.

use agent_host::CommandResult;
use shared::settings::AppSettings;
use std::path::{Path, PathBuf};

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
    // Extract any paths from the command
    for word in command.split_whitespace() {
        let path = expand_user_path(word);
        if path.exists() && !is_path_in_allowed_dirs(&path, allowed_dirs) {
            return Err(format!("Path '{}' is outside allowed directories", word));
        }
    }
    Ok(())
}
