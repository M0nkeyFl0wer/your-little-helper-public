//! Compile-time secrets for bespoke builds.
//!
//! **This file is gitignored** -- it must never contain real credentials in version
//! control. Public/CI builds compile with all constants empty; users configure
//! keys at runtime through the Settings dialog.
//!
//! For bespoke builds (e.g., distributing to a specific client), set environment
//! variables before `cargo build` to bake credentials into the binary.
//! Google OAuth credentials can also be loaded at runtime from a JSON file
//! at `~/.config/little-helper/google_oauth.json`.

/// OpenAI API key (baked in at compile time, or empty for public builds).
pub const OPENAI_API_KEY: &str = match option_env!("LITTLE_HELPER_OPENAI_KEY") {
    Some(v) => v,
    None => "",
};

/// Google OAuth 2.0 Client ID for "Sign in with Google" (Gemini).
/// Create at: https://console.cloud.google.com/apis/credentials (Desktop app).
const GOOGLE_OAUTH_CLIENT_ID: &str = match option_env!("LITTLE_HELPER_GOOGLE_CLIENT_ID") {
    Some(v) => v,
    None => "",
};

/// Google OAuth 2.0 Client Secret (paired with the Client ID above).
const GOOGLE_OAUTH_CLIENT_SECRET: &str = match option_env!("LITTLE_HELPER_GOOGLE_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};

/// Load Google OAuth credentials — checks (in order):
/// 1. Compile-time env vars (bespoke builds)
/// 2. Runtime JSON at ~/.config/little-helper/google_oauth.json
#[allow(clippy::const_is_empty)]
pub fn google_oauth_credentials() -> Option<(String, Option<String>)> {
    // 1. Compile-time constants
    if !GOOGLE_OAUTH_CLIENT_ID.is_empty() {
        let secret = if GOOGLE_OAUTH_CLIENT_SECRET.is_empty() {
            None
        } else {
            Some(GOOGLE_OAUTH_CLIENT_SECRET.to_string())
        };
        return Some((GOOGLE_OAUTH_CLIENT_ID.to_string(), secret));
    }

    // 2. Runtime JSON file (accepts Google's native client_secret download format)
    let config_dir = directories::ProjectDirs::from("", "", "little-helper")?;
    let json_path = config_dir.config_dir().join("google_oauth.json");
    let data = std::fs::read_to_string(&json_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;

    let installed = parsed.get("installed").or(Some(&parsed));
    let client_id = installed?.get("client_id")?.as_str()?.to_string();
    let client_secret = installed
        .and_then(|v| v.get("client_secret"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if client_id.is_empty() {
        return None;
    }

    Some((client_id, client_secret))
}

/// Custom base URL for OpenAI-compatible APIs (OpenRouter, Kimi, Together, etc.).
/// When set, the "openai" provider routes to this URL instead of api.openai.com.
pub const OPENAI_BASE_URL: &str = match option_env!("LITTLE_HELPER_OPENAI_BASE_URL") {
    Some(v) => v,
    None => "",
};

/// Default model to use with the OpenAI-compatible provider.
pub const OPENAI_MODEL: &str = match option_env!("LITTLE_HELPER_OPENAI_MODEL") {
    Some(v) => v,
    None => "",
};

// ── Preloaded user info (for bespoke builds that skip onboarding) ──

pub const PRELOAD_USER_NAME: &str = match option_env!("LITTLE_HELPER_USER_NAME") {
    Some(v) => v,
    None => "",
};

/// Whether to skip the onboarding screen (bespoke builds).
pub fn should_skip_onboarding() -> bool {
    matches!(
        option_env!("LITTLE_HELPER_SKIP_ONBOARDING"),
        Some("1") | Some("true") | Some("yes")
    )
}
