// THIS FILE IS GITIGNORED - Never commit API keys or user info!

// Public build defaults (no preloaded keys)
//
// For internal builds, inject keys at build time.
pub const OPENAI_API_KEY: &str = "";

// Preloaded user info - customize per build to skip onboarding
// Set to empty string "" to show onboarding screen
pub const PRELOAD_USER_NAME: &str = "";
pub const PRELOAD_SKIP_ONBOARDING: bool = false; // Set to true to skip onboarding for bespoke builds
