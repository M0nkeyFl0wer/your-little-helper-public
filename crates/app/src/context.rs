//! Context loader for Little Helper
//!
//! Preloads knowledge for the agent:
//! - System information for tech support
//! - Campaign documents for content creation
//! - Persona files for audience targeting
//! - Project knowledge for research

use shared::settings::AppSettings;
use std::fs;
use std::process::Command;
use std::sync::OnceLock;

static CAMPAIGN_CONTEXT: OnceLock<String> = OnceLock::new();
static PERSONA_CONTEXT: OnceLock<String> = OnceLock::new();
static SYSTEM_INFO: OnceLock<String> = OnceLock::new();

/// Load campaign context documents for the agent
/// Returns full content of key campaign files for deep context
pub fn load_campaign_context() -> String {
    CAMPAIGN_CONTEXT
        .get_or_init(|| build_campaign_context())
        .clone()
}

fn build_campaign_context() -> String {
    let _context = String::new();

    // Public build: no campaign context shipped.
    // Users can paste relevant text into chat or connect their own files.
    "CAMPAIGN CONTEXT: Not configured in this build.\n".to_string()
}

/// Load persona files from ~/Process/personas/
/// Returns all personas as context for content generation
pub fn load_personas() -> String {
    PERSONA_CONTEXT
        .get_or_init(|| build_persona_context())
        .clone()
}

fn build_persona_context() -> String {
    let mut context = String::new();

    // Check multiple possible persona locations
    let persona_dirs: Vec<std::path::PathBuf> = vec![
        dirs::home_dir()
            .map(|h| h.join("Process/personas"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join("Projects/personas"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join("Documents/personas"))
            .unwrap_or_default(),
    ];

    let mut loaded_count = 0;
    let mut all_personas = Vec::new();

    for persona_dir in &persona_dirs {
        if !persona_dir.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(persona_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                        all_personas.push((name, content, path.display().to_string()));
                        loaded_count += 1;
                    }
                }
            }
        }
    }

    if all_personas.is_empty() {
        return "PERSONAS: No persona files found. Add persona .md files in your personas folder (Settings).\n\n".to_string();
    }

    context.push_str("=== TARGET AUDIENCE PERSONAS ===\n\n");
    context.push_str("Use these personas to tailor content to specific audiences.\n");
    context.push_str("Match language, concerns, and messaging to the target persona.\n\n");

    for (name, content, path) in all_personas {
        context.push_str(&format!(
            "=== PERSONA: {} ===\n",
            name.to_uppercase().replace("-", " ")
        ));
        context.push_str(&format!("Source: {}\n\n", path));
        context.push_str(&content);
        context.push_str("\n\n");
    }

    context.push_str(&format!(
        "=== END PERSONAS ({} loaded) ===\n\n",
        loaded_count
    ));

    context
}

/// Load DDD workflow context
pub fn load_ddd_workflow() -> String {
    r#"=== CONTENT WORKFLOW ===

FOLDERS:
- Choose a drafts folder in Settings
- (Optional) add persona .md files in a personas folder

WORKFLOW:
1. Identify the audience
2. Draft content
3. Save drafts to your drafts folder

OUTPUT FORMAT:
YYYY-MM-DD_platform_topic.md

=== END WORKFLOW ===
"#
    .to_string()
}

/// Get system information for tech support context (cross-platform)
pub fn get_system_info() -> String {
    SYSTEM_INFO.get_or_init(|| build_system_info()).clone()
}

fn build_system_info() -> String {
    let mut info = String::new();

    // OS info - cross-platform
    #[cfg(target_os = "windows")]
    {
        info.push_str("OS: Windows\n");
        if let Ok(output) = Command::new("cmd").args(["/C", "ver"]).output() {
            let version = String::from_utf8_lossy(&output.stdout);
            if !version.trim().is_empty() {
                info.push_str(&format!("Version: {}", version));
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = Command::new("uname").arg("-a").output() {
            info.push_str("OS: ");
            info.push_str(&String::from_utf8_lossy(&output.stdout));
        }
    }

    // Hostname - works on both platforms
    if let Ok(output) = Command::new("hostname").output() {
        info.push_str("Hostname: ");
        info.push_str(&String::from_utf8_lossy(&output.stdout));
    }

    // User - works on both platforms
    if let Ok(output) = Command::new("whoami").output() {
        info.push_str("User: ");
        info.push_str(&String::from_utf8_lossy(&output.stdout));
    }

    // Available tools - cross-platform
    #[cfg(target_os = "windows")]
    let tools = [
        "python",
        "pip",
        "curl",
        "git",
        "node",
        "npm",
        "cargo",
        "rustc",
        "powershell",
    ];
    #[cfg(not(target_os = "windows"))]
    let tools = [
        "python3", "pip3", "curl", "wget", "jq", "git", "docker", "node", "npm", "cargo", "rustc",
    ];

    let mut available_tools = Vec::new();
    for tool in tools {
        // Use 'where' on Windows, 'which' on Unix
        #[cfg(target_os = "windows")]
        let check = Command::new("where").arg(tool).output();
        #[cfg(not(target_os = "windows"))]
        let check = Command::new("which").arg(tool).output();

        if check.map(|o| o.status.success()).unwrap_or(false) {
            available_tools.push(tool);
        }
    }
    info.push_str(&format!(
        "Available tools: {}\n",
        available_tools.join(", ")
    ));

    // Home directory
    if let Some(home) = dirs::home_dir() {
        info.push_str(&format!("Home: {}\n", home.display()));
    }

    // Projects/Documents directory listing
    if let Some(home) = dirs::home_dir() {
        // Check common project locations
        let project_dirs = [
            home.join("Projects"),
            home.join("Documents"),
            home.join("repos"),
        ];

        for projects in project_dirs {
            if projects.exists() {
                if let Ok(entries) = fs::read_dir(&projects) {
                    let dirs: Vec<_> = entries
                        .flatten()
                        .filter(|e| e.path().is_dir())
                        .take(10) // Limit to first 10
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    if !dirs.is_empty() {
                        info.push_str(&format!(
                            "{}: {}\n",
                            projects.file_name().unwrap_or_default().to_string_lossy(),
                            dirs.join(", ")
                        ));
                    }
                }
            }
        }
    }

    info
}

/// Get a brief campaign summary for system prompts (includes system info when allowed)
pub fn get_campaign_summary(settings: &AppSettings) -> String {
    let mut summary = String::new();

    summary.push_str("SYSTEM CONTEXT:\n");
    if settings.share_system_summary {
        summary.push_str(&get_system_info());
    } else {
        summary.push_str("System summary sharing is disabled.\n");
    }
    summary.push('\n');

    if settings.enable_campaign_context {
        summary.push_str(
            "PROJECT KNOWLEDGE:\n\
Project context is enabled. If you connect a project folder, I can use those files to help you.\n",
        );
    } else {
        summary.push_str(
            "PROJECT KNOWLEDGE:\nProject context is disabled. You can enable it in Settings.\n",
        );
    }

    summary
}
