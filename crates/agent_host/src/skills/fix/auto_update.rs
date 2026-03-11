//! Self-update skill for Little Helper.
//!
//! Checks GitHub Releases for newer versions, downloads the appropriate
//! platform binary, and replaces the running application. On macOS this
//! means downloading the DMG, mounting it, copying the .app bundle to
//! /Applications, and relaunching. On Windows, running the installer
//! silently. On Linux, replacing the AppImage.
//!
//! # Signing & Notarization
//!
//! For a smooth UX the release artifacts should be codesigned:
//! - **macOS**: Developer ID Application cert + `xcrun notarytool` stapling.
//!   Without this, Gatekeeper will warn or block the replacement app.
//! - **Windows**: Authenticode signing (optional but eliminates SmartScreen).
//! - **Linux**: No signing required for AppImage.
//!
//! # Safety
//!
//! - The old binary is backed up before replacement (archive, not delete).
//! - If the download or replacement fails, the backup is restored.
//! - The skill requires Sensitive permission (user must confirm).
//!
//! # TODO (v0.4.0)
//!
//! - [ ] Implement `check_for_update` (GitHub Releases API)
//! - [ ] Implement `download_update` (platform-specific artifact fetch)
//! - [ ] Implement `install_update` (replace binary, relaunch)
//! - [ ] Add compiled-in version constant for comparison
//! - [ ] Add update check on app startup (non-blocking, once per day)
//! - [ ] Add UI notification when update is available
//! - [ ] Codesign + notarize macOS builds (requires Apple Developer cert)
//! - [ ] Optional: delta updates (binary diff) to reduce download size

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, SkillContext, SkillInput, SkillOutput};

/// The GitHub repo to check for releases.
const GITHUB_REPO: &str = "M0nkeyFl0wer/your-little-helper-public";

/// Compiled-in version — compared against the latest GitHub Release tag.
/// Set via `LITTLE_HELPER_VERSION` env var at build time, or falls back to
/// the Cargo package version.
const CURRENT_VERSION: &str = match option_env!("LITTLE_HELPER_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Semantic version tag (e.g. "v0.4.0")
    pub tag: String,
    /// Release title / name
    pub name: String,
    /// Release notes (markdown)
    pub body: String,
    /// Direct download URL for the current platform's artifact
    pub download_url: String,
    /// File size in bytes (for progress display)
    pub size_bytes: Option<u64>,
}

#[derive(Default)]
pub struct AutoUpdateSkill;

impl AutoUpdateSkill {
    pub fn new() -> Self {
        Self
    }

    /// Check the GitHub Releases API for a version newer than `CURRENT_VERSION`.
    ///
    /// Returns `Some(UpdateInfo)` if an update is available, `None` if current.
    async fn check_for_update(&self) -> Result<Option<UpdateInfo>> {
        // TODO: GET https://api.github.com/repos/{GITHUB_REPO}/releases/latest
        // Compare response.tag_name against CURRENT_VERSION (semver)
        // Find the asset matching the current platform:
        //   - macOS:   *macOS* or *.dmg
        //   - Windows: *Windows* or *Setup.exe
        //   - Linux:   *Linux* or *.AppImage
        // Return UpdateInfo with download_url from the matching asset

        let _ = GITHUB_REPO;
        let _ = CURRENT_VERSION;

        Ok(None) // Stub: no update available
    }

    /// Download the update artifact to a temporary file.
    ///
    /// Returns the path to the downloaded file. Reports progress via the
    /// status channel if one is available in the context.
    async fn download_update(&self, _info: &UpdateInfo) -> Result<std::path::PathBuf> {
        // TODO:
        // 1. Create temp file in std::env::temp_dir()
        // 2. Stream download with progress reporting
        // 3. Verify file size matches info.size_bytes
        // 4. (Future) Verify signature / checksum
        anyhow::bail!("Auto-update download not yet implemented")
    }

    /// Replace the running application with the downloaded update.
    ///
    /// Platform-specific:
    /// - **macOS**: Mount DMG, copy .app to /Applications, unmount, relaunch
    /// - **Windows**: Run installer silently, or replace exe + relaunch
    /// - **Linux**: Replace AppImage, chmod +x, relaunch
    ///
    /// The old binary is archived (not deleted) before replacement.
    async fn install_update(&self, _download_path: &std::path::Path) -> Result<()> {
        // TODO:
        // 1. Determine current exe path: std::env::current_exe()
        // 2. Archive old binary: rename to .backup or move to versioned dir
        // 3. Platform-specific install:
        //    macOS:
        //      hdiutil attach <dmg> -mountpoint /tmp/lh-update
        //      cp -R "/tmp/lh-update/Little Helper.app" "/Applications/"
        //      hdiutil detach /tmp/lh-update
        //    Windows:
        //      Start-Process <installer> -ArgumentList "/S" -Wait
        //    Linux:
        //      cp <appimage> <current_exe_path>
        //      chmod +x <current_exe_path>
        // 4. Relaunch: std::process::Command::new(new_exe).spawn()
        // 5. Exit current process: std::process::exit(0)
        anyhow::bail!("Auto-update install not yet implemented")
    }
}

#[async_trait]
impl shared::skill::Skill for AutoUpdateSkill {
    fn id(&self) -> &'static str {
        "auto_update"
    }

    fn name(&self) -> &'static str {
        "Auto Update"
    }

    fn description(&self) -> &'static str {
        "Check for and install Little Helper updates from GitHub Releases"
    }

    fn permission_level(&self) -> PermissionLevel {
        // Replacing the binary is a sensitive operation
        PermissionLevel::Sensitive
    }

    fn modes(&self) -> &'static [Mode] {
        // Available in Fix mode (system maintenance)
        &[Mode::Fix]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let action = input
            .params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("check");

        match action {
            "check" => match self.check_for_update().await? {
                Some(info) => Ok(SkillOutput::text(format!(
                    "Update available: {} ({})\n\n{}\n\nRun with action=install to update.",
                    info.name, info.tag, info.body
                ))),
                None => Ok(SkillOutput::text(format!(
                    "You're on the latest version ({}). No update available.",
                    CURRENT_VERSION
                ))),
            },
            "install" => {
                let info = self
                    .check_for_update()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("No update available"))?;

                let download_path = self.download_update(&info).await?;
                self.install_update(&download_path).await?;

                // If we get here, something went wrong (install should relaunch)
                Ok(SkillOutput::text(
                    "Update installed. Please restart Little Helper.",
                ))
            }
            _ => Ok(SkillOutput::error(format!(
                "Unknown action '{}'. Use 'check' or 'install'.",
                action
            ))),
        }
    }
}
