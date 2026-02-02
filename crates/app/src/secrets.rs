// THIS FILE IS GITIGNORED - Never commit API keys or user info!

// Public build defaults (no preloaded keys)
//
// For internal builds, inject keys at build time.
pub const OPENAI_API_KEY: &str = "";

// Google OAuth 2.0 credentials for "Sign in with Google" (Gemini).
// Create at: https://console.cloud.google.com/apis/credentials
// Type: Desktop app — download client_secret.json for these values.
//
// For bespoke builds, hardcode here. Otherwise, the app loads from
// ~/.config/little-helper/google_oauth.json at runtime.
pub const GOOGLE_OAUTH_CLIENT_ID: &str = "";
pub const GOOGLE_OAUTH_CLIENT_SECRET: &str = "";

/// Load Google OAuth credentials from the config directory JSON file,
/// falling back to the compile-time constants above.
pub fn google_oauth_credentials() -> Option<(String, Option<String>)> {
    // Try compile-time constants first
    if !GOOGLE_OAUTH_CLIENT_ID.is_empty() {
        let secret = if GOOGLE_OAUTH_CLIENT_SECRET.is_empty() {
            None
        } else {
            Some(GOOGLE_OAUTH_CLIENT_SECRET.to_string())
        };
        return Some((GOOGLE_OAUTH_CLIENT_ID.to_string(), secret));
    }

    // Try runtime JSON file: ~/.config/little-helper/google_oauth.json
    // Accepts Google's client_secret download format:
    //   {"installed":{"client_id":"...","client_secret":"..."}}
    let config_dir = directories::ProjectDirs::from("", "", "little-helper")?;
    let json_path = config_dir.config_dir().join("google_oauth.json");
    let data = std::fs::read_to_string(&json_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;

    // Handle Google's nested format
    let installed = parsed.get("installed").or(Some(&parsed));
    let client_id = installed?
        .get("client_id")?
        .as_str()?
        .to_string();
    let client_secret = installed
        .and_then(|v| v.get("client_secret"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if client_id.is_empty() {
        return None;
    }

    Some((client_id, client_secret))
}

// Preloaded user info - customize per build to skip onboarding
// Set to empty string "" to show onboarding screen
pub const PRELOAD_USER_NAME: &str = "";
pub const PRELOAD_SKIP_ONBOARDING: bool = false; // Set to true to skip onboarding for bespoke builds
