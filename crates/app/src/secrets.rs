// THIS FILE IS GITIGNORED - Never commit API keys or user info!
//
// Public builds: all constants are empty — users configure keys at runtime.
//
// Bespoke builds: set environment variables before `cargo build` to bake
// credentials into the binary. Nothing in git changes.
//
//   LITTLE_HELPER_OPENAI_KEY="sk-..."  \
//   LITTLE_HELPER_GOOGLE_CLIENT_ID="390..." \
//   LITTLE_HELPER_GOOGLE_CLIENT_SECRET="GOCSPX-..." \
//   LITTLE_HELPER_USER_NAME="Flower" \
//   LITTLE_HELPER_SKIP_ONBOARDING=1 \
//   cargo build --release

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

// ── Preloaded user info (for bespoke builds that skip onboarding) ──

pub const PRELOAD_USER_NAME: &str = match option_env!("LITTLE_HELPER_USER_NAME") {
    Some(v) => v,
    None => "",
};

/// Whether to skip the onboarding screen (bespoke builds).
pub fn should_skip_onboarding() -> bool {
    match option_env!("LITTLE_HELPER_SKIP_ONBOARDING") {
        Some("1") | Some("true") | Some("yes") => true,
        _ => false,
    }
}
