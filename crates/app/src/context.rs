//! Context loader for Little Helper
//!
//! Preloads knowledge for the agent:
//! - System information for tech support
//! - Campaign documents for content creation
//! - Project knowledge for research

use std::fs;
use std::process::Command;

/// Load campaign context documents for the agent
/// Returns full content of key campaign files for deep context
pub fn load_campaign_context() -> String {
    let mut context = String::new();

    // MCP project paths
    let mcp_base = dirs::home_dir()
        .map(|h| h.join("Projects/MCP-research-content-automation-engine"))
        .unwrap_or_default();

    // Check if the project exists
    if !mcp_base.exists() {
        return "CAMPAIGN CONTEXT: MCP project not found on this system. Campaign-specific features will be limited.\n".to_string();
    }

    // Priority documents to load (in order of importance)
    let docs = [
        // Core campaign strategy
        (
            "Campaign Spec",
            "specs/002-mcp-marine-conservation/spec.md",
            15000,
        ),
        (
            "Campaign Plan",
            "specs/002-mcp-marine-conservation/plan.md",
            15000,
        ),
        // Content
        ("Content Summary", "MCP_Content_Summary_FINAL.md", 20000),
        ("Content Calendar", "FINAL_MCP_Content_Calendar.json", 20000),
        // Video content
        ("Video Specs", "MCP_Video_Content_Specifications.md", 10000),
        (
            "Video Scripts (Northern BC Voice)",
            "MCP_Video_Scripts_NorthernBC_Voice.md",
            12000,
        ),
        // Visual assets
        (
            "Visual Requirements",
            "MCP_Visual_Asset_Requirements.md",
            8000,
        ),
    ];

    context.push_str("=== MCP MARINE CONSERVATION CAMPAIGN - FULL CONTEXT ===\n\n");
    context.push_str(
        "You have FULL ACCESS to the Marine Conservation Plan (MCP) campaign materials.\n",
    );
    context
        .push_str("Use this detailed knowledge for content creation, research, and strategy.\n\n");

    let mut loaded_count = 0;
    for (name, path, max_chars) in docs {
        let full_path = mcp_base.join(path);
        if let Ok(content) = fs::read_to_string(&full_path) {
            loaded_count += 1;
            // Truncate if over limit but keep more content
            let truncated = if content.len() > max_chars {
                format!(
                    "{}...\n[Truncated at {} chars - full file has {} chars]",
                    &content[..max_chars],
                    max_chars,
                    content.len()
                )
            } else {
                content
            };

            context.push_str(&format!("=== {} ===\n", name));
            context.push_str(&format!("Source: {}\n\n", path));
            context.push_str(&truncated);
            context.push_str("\n\n");
        }
    }

    // Load research reports if they exist
    let reports_dir = mcp_base.join("data/reports");
    if reports_dir.exists() {
        if let Ok(entries) = fs::read_dir(&reports_dir) {
            let reports: Vec<_> = entries.flatten().collect();
            if !reports.is_empty() {
                context.push_str("=== Available Research Reports ===\n");
                for entry in reports {
                    let name = entry.file_name().to_string_lossy().to_string();
                    context.push_str(&format!("- {}\n", name));
                }
                context.push('\n');
            }
        }
    }

    context.push_str(&format!(
        "=== END CAMPAIGN CONTEXT ({} documents loaded) ===\n\n",
        loaded_count
    ));

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
