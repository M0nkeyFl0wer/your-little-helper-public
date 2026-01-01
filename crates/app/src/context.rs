//! Context loader for Little Helper
//!
//! Preloads knowledge for the agent:
//! - System information for tech support
//! - Campaign documents for content creation
//! - Project knowledge for research

use std::fs;
use std::process::Command;

/// Load campaign context documents for the agent
#[allow(dead_code)] // Available for Content mode deep context loading
pub fn load_campaign_context() -> String {
    let mut context = String::new();

    // MCP project paths
    let mcp_base = dirs::home_dir()
        .map(|h| h.join("Projects/MCP-research-content-automation-engine"))
        .unwrap_or_default();

    // Priority documents to load
    let docs = [
        ("Campaign Spec", "specs/002-mcp-marine-conservation/spec.md"),
        ("Campaign Plan", "specs/002-mcp-marine-conservation/plan.md"),
        ("Content Calendar", "FINAL_MCP_Content_Calendar.json"),
        ("Video Specs", "MCP_Video_Content_Specifications.md"),
        ("Integration Guide", "MCP_INTEGRATIONS_README.md"),
    ];

    context.push_str("=== CAMPAIGN CONTEXT ===\n\n");
    context.push_str("You have access to the Marine Conservation Plan (MCP) campaign materials.\n");
    context.push_str(
        "Use this knowledge to provide informed support, research, and content creation.\n\n",
    );

    for (name, path) in docs {
        let full_path = mcp_base.join(path);
        if let Ok(content) = fs::read_to_string(&full_path) {
            // Truncate very long files
            let truncated = if content.len() > 8000 {
                format!(
                    "{}...\n[Truncated - {} total chars]",
                    &content[..8000],
                    content.len()
                )
            } else {
                content
            };

            context.push_str(&format!("--- {} ---\n", name));
            context.push_str(&truncated);
            context.push_str("\n\n");
        }
    }

    // Load research reports if they exist
    let reports_dir = mcp_base.join("data/reports");
    if reports_dir.exists() {
        if let Ok(entries) = fs::read_dir(&reports_dir) {
            context.push_str("--- Available Research Reports ---\n");
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                context.push_str(&format!("- {}\n", name));
            }
            context.push('\n');
        }
    }

    context.push_str("=== END CAMPAIGN CONTEXT ===\n\n");

    context
}

/// Get system information for tech support context (cross-platform)
pub fn get_system_info() -> String {
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

/// Get a brief campaign summary for system prompts (includes system info)
pub fn get_campaign_summary() -> String {
    let system_info = get_system_info();

    format!(
        r#"
SYSTEM CONTEXT:
{}

CAMPAIGN KNOWLEDGE:
You have deep knowledge of the Marine Conservation Plan (MCP) campaign:
- BC Marine Protected Areas policy and implementation
- Fishing industry impact data (150+ businesses, $50-100M revenue at risk)
- Aquaculture conflicts (Mowi Canada West facilities)
- Content calendar with 7+ days of social media content
- Stakeholder analysis (lodges, charter operations, indigenous communities)
- Key zones: Central Coast 100-213, Caamano Sound 310-316, Kitkatla Inlet 330-333

PROJECT LOCATIONS:
- MCP Content Engine: ~/Projects/MCP-research-content-automation-engine/
- Content Calendar: ~/Projects/MCP-research-content-automation-engine/FINAL_MCP_Content_Calendar.json
- Little Helper App: ~/Projects/little-helper/

When discussing marine conservation, fishing policy, or BC coastal issues, draw on this knowledge.
For content creation, reference the established content calendar and messaging strategies.
"#,
        system_info
    )
}
