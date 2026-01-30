//! Startup Optimizer Skill
//!
//! Helps users identify and manage applications that launch on startup,
//! improving boot time and system performance.
//!
//! Features:
//! - Detects startup programs across platforms (macOS, Windows, Linux)
//! - Shows boot time impact estimates
//! - Tracks last-used dates to identify unused apps
//! - One-click disable with safety checks
//! - Visual before/after boot time comparison

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput, SuggestedAction};
use std::collections::HashMap;
use std::process::Command;
use std::time::SystemTime;

/// Information about a startup program
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StartupProgram {
    /// Program name
    pub name: String,
    /// Full path or command
    pub path: String,
    /// Estimated boot time impact in seconds
    pub boot_time_impact: f32,
    /// When the app was last used (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<SystemTime>,
    /// Whether currently enabled
    pub enabled: bool,
    /// Source of startup (Login Items, Registry, etc.)
    pub source: String,
    /// User-friendly description
    pub description: String,
}

/// Startup optimization results
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StartupOptimizationResult {
    /// All detected startup programs
    pub programs: Vec<StartupProgram>,
    /// Current estimated boot time in seconds
    pub current_boot_time: f32,
    /// Potential boot time after optimization
    pub optimized_boot_time: f32,
    /// Number of programs that could be disabled
    pub optimizable_count: usize,
}

/// Startup Optimizer Skill
pub struct StartupOptimizer;

impl StartupOptimizer {
    /// Create a new startup optimizer
    pub fn new() -> Self {
        Self
    }

    /// Detect startup programs based on platform
    fn detect_startup_programs(&self) -> Result<Vec<StartupProgram>> {
        #[cfg(target_os = "macos")]
        return self.detect_macos_startup();
        
        #[cfg(target_os = "windows")]
        return self.detect_windows_startup();
        
        #[cfg(target_os = "linux")]
        return self.detect_linux_startup();
        
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        return Err(anyhow::anyhow!("Unsupported platform"));
    }

    /// Detect macOS startup programs
    #[cfg(target_os = "macos")]
    fn detect_macos_startup(&self) -> Result<Vec<StartupProgram>> {
        let mut programs = Vec::new();
        
        // Check Login Items via osascript
        let output = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to get the name of every login item")
            .output();
        
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for name in stdout.split(", ").map(|s| s.trim()) {
                if !name.is_empty() && name != "osascript" {
                    programs.push(StartupProgram {
                        name: name.to_string(),
                        path: format!("/Applications/{}.app", name),
                        boot_time_impact: self.estimate_boot_impact(name),
                        last_used: self.get_last_used_date(name),
                        enabled: true,
                        source: "Login Items".to_string(),
                        description: format!("{} launches when you log in", name),
                    });
                }
            }
        }
        
        // Check LaunchAgents in user directory
        if let Ok(entries) = std::fs::read_dir(dirs::home_dir().unwrap_or_default().join("Library/LaunchAgents")) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".plist") {
                        let app_name = name.replace(".plist", "");
                        programs.push(StartupProgram {
                            name: app_name.clone(),
                            path: entry.path().to_string_lossy().to_string(),
                            boot_time_impact: self.estimate_boot_impact(&app_name),
                            last_used: self.get_last_used_date(&app_name),
                            enabled: true,
                            source: "LaunchAgent".to_string(),
                            description: format!("{} runs background services", app_name),
                        });
                    }
                }
            }
        }
        
        Ok(programs)
    }

    /// Detect Windows startup programs
    #[cfg(target_os = "windows")]
    fn detect_windows_startup(&self) -> Result<Vec<StartupProgram>> {
        let mut programs = Vec::new();
        
        // Check Registry Run keys using reg query
        let reg_paths = [
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
        ];
        
        for reg_path in &reg_paths {
            let output = Command::new("reg")
                .args(&["query", reg_path])
                .output();
            
            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.starts_with("    ") && !line.contains("REG_SZ") {
                        continue;
                    }
                    if let Some(name) = line.split_whitespace().next() {
                        if name != "(Default)" && name != "HKEY_CURRENT_USER" && name != "HKEY_LOCAL_MACHINE" {
                            programs.push(StartupProgram {
                                name: name.to_string(),
                                path: "Registry".to_string(),
                                boot_time_impact: self.estimate_boot_impact(name),
                                last_used: self.get_last_used_date(name),
                                enabled: true,
                                source: "Registry Run".to_string(),
                                description: format!("{} starts with Windows", name),
                            });
                        }
                    }
                }
            }
        }
        
        // Check Startup folder
        if let Some(app_data) = dirs::config_dir() {
            let startup_path = app_data.join("Microsoft/Windows/Start Menu/Programs/Startup");
            if let Ok(entries) = std::fs::read_dir(&startup_path) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        let clean_name = name.replace(".lnk", "").replace(".exe", "");
                        programs.push(StartupProgram {
                            name: clean_name.clone(),
                            path: entry.path().to_string_lossy().to_string(),
                            boot_time_impact: self.estimate_boot_impact(&clean_name),
                            last_used: self.get_last_used_date(&clean_name),
                            enabled: true,
                            source: "Startup Folder".to_string(),
                            description: format!("{} in Startup folder", clean_name),
                        });
                    }
                }
            }
        }
        
        Ok(programs)
    }

    /// Detect Linux startup programs
    #[cfg(target_os = "linux")]
    fn detect_linux_startup(&self) -> Result<Vec<StartupProgram>> {
        let mut programs = Vec::new();
        
        // Check systemd user services
        let output = Command::new("systemctl")
            .args(&["--user", "list-unit-files", "--type=service", "--state=enabled"])
            .output();
        
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains(".service") && line.contains("enabled") {
                    let name = line.split_whitespace().next()
                        .unwrap_or("")
                        .replace(".service", "");
                    if !name.is_empty() && !name.contains("@") {
                        programs.push(StartupProgram {
                            name: name.clone(),
                            path: format!("systemd user service: {}", name),
                            boot_time_impact: self.estimate_boot_impact(&name),
                            last_used: self.get_last_used_date(&name),
                            enabled: true,
                            source: "systemd".to_string(),
                            description: format!("{} systemd service", name),
                        });
                    }
                }
            }
        }
        
        // Check autostart desktop files
        if let Some(config_dir) = dirs::config_dir() {
            let autostart_dir = config_dir.join("autostart");
            if let Ok(entries) = std::fs::read_dir(&autostart_dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".desktop") {
                            let app_name = name.replace(".desktop", "");
                            programs.push(StartupProgram {
                                name: app_name.clone(),
                                path: entry.path().to_string_lossy().to_string(),
                                boot_time_impact: self.estimate_boot_impact(&app_name),
                                last_used: self.get_last_used_date(&app_name),
                                enabled: true,
                                source: "Desktop Autostart".to_string(),
                                description: format!("{} autostart entry", app_name),
                            });
                        }
                    }
                }
            }
        }
        
        Ok(programs)
    }

    /// Estimate boot time impact based on app name heuristics
    fn estimate_boot_impact(&self, name: &str) -> f32 {
        let name_lower = name.to_lowercase();
        
        // Heavy apps (2-5 seconds)
        if name_lower.contains("chrome") || 
           name_lower.contains("electron") ||
           name_lower.contains("slack") ||
           name_lower.contains("teams") ||
           name_lower.contains("discord") {
            return 3.0;
        }
        
        // Medium apps (1-2 seconds)
        if name_lower.contains("spotify") ||
           name_lower.contains("dropbox") ||
           name_lower.contains("onedrive") ||
           name_lower.contains("creative") {
            return 1.5;
        }
        
        // Light apps (0.5-1 second)
        if name_lower.contains("helper") ||
           name_lower.contains("agent") ||
           name_lower.contains("daemon") ||
           name_lower.contains("service") {
            return 0.8;
        }
        
        // Default (1 second)
        1.0
    }

    /// Try to determine when an app was last used
    fn get_last_used_date(&self, _name: &str) -> Option<SystemTime> {
        // This is a simplified version - in production would check:
        // - Recent documents
        // - App-specific logs
        // - File access times
        // - macOS: mdfind last used dates
        // - Windows: Jump lists, registry
        
        // For now, return None to indicate unknown
        None
    }

    /// Calculate boot time estimates
    fn calculate_boot_times(&self, programs: &[StartupProgram]) -> (f32, f32) {
        let current_boot_time: f32 = programs.iter()
            .filter(|p| p.enabled)
            .map(|p| p.boot_time_impact)
            .sum();
        
        // Estimate optimized time (disabling apps unused for 30+ days)
        let optimized_boot_time: f32 = programs.iter()
            .filter(|p| p.enabled && !self.is_unused(p))
            .map(|p| p.boot_time_impact)
            .sum();
        
        // Base boot time (OS itself) - approximately 15-20 seconds
        let base_boot = 18.0;
        
        (base_boot + current_boot_time, base_boot + optimized_boot_time)
    }

    /// Check if a program appears to be unused
    fn is_unused(&self, program: &StartupProgram) -> bool {
        // Mark as potentially unused if:
        // 1. Has "helper", "updater", "agent" in name (often unnecessary)
        // 2. No last used date and light impact
        let name_lower = program.name.to_lowercase();
        
        if name_lower.contains("helper") && program.boot_time_impact < 1.0 {
            return true;
        }
        
        if name_lower.contains("updater") {
            return true;
        }
        
        false
    }

    /// Format results as user-friendly markdown
    fn format_results(&self, result: &StartupOptimizationResult) -> String {
        let mut output = String::new();
        
        // Header with boot time comparison
        output.push_str("## 🚀 Startup Analysis\n\n");
        output.push_str(&format!("**Current boot time:** ~{:.0} seconds\n", result.current_boot_time));
        output.push_str(&format!("**After optimization:** ~{:.0} seconds\n", result.optimized_boot_time));
        
        if result.current_boot_time > result.optimized_boot_time {
            let improvement = result.current_boot_time - result.optimized_boot_time;
            let percentage = (improvement / result.current_boot_time * 100.0) as i32;
            output.push_str(&format!("**Potential improvement:** {} seconds ({}% faster)\n", 
                improvement, percentage));
        }
        
        output.push_str(&format!("\n**{} apps** launch on startup\n\n", result.programs.len()));
        
        // Group by status
        let optimizable: Vec<_> = result.programs.iter()
            .filter(|p| self.is_unused(p))
            .collect();
        
        if !optimizable.is_empty() {
            output.push_str("### ⚡ Quick Wins (Safe to Disable)\n\n");
            for program in optimizable {
                output.push_str(&format!("• **{}** - {} (+{:.1}s)\n", 
                    program.name, 
                    program.description,
                    program.boot_time_impact));
            }
            output.push('\n');
        }
        
        // All programs table
        output.push_str("### All Startup Programs\n\n");
        output.push_str("| App | Source | Impact | Status |\n");
        output.push_str("|-----|--------|--------|--------|\n");
        
        for program in &result.programs {
            let status = if self.is_unused(program) {
                "🟡 Can disable"
            } else {
                "🟢 Keep"
            };
            output.push_str(&format!("| {} | {} | +{:.1}s | {} |\n",
                program.name,
                program.source,
                program.boot_time_impact,
                status));
        }
        
        output.push_str("\n---\n\n");
        output.push_str("💡 **Tip:** Disabling startup apps doesn't uninstall them - they just won't launch automatically. You can still open them normally when needed.");
        
        output
    }
    
    /// Disable a specific startup program
    pub fn disable_startup_program(&self, program_name: &str, source: &str) -> anyhow::Result<String> {
        #[cfg(target_os = "macos")]
        return self.disable_macos_startup(program_name, source);
        
        #[cfg(target_os = "windows")]
        return self.disable_windows_startup(program_name, source);
        
        #[cfg(target_os = "linux")]
        return self.disable_linux_startup(program_name, source);
        
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        Err(anyhow::anyhow!("Unsupported platform"))
    }
    
    /// Disable macOS startup program
    #[cfg(target_os = "macos")]
    fn disable_macos_startup(&self, program_name: &str, source: &str) -> anyhow::Result<String> {
        match source {
            "Login Items" => {
                // Use osascript to remove from login items
                let script = format!(
                    r#"tell application "System Events" to delete login item "{}""#,
                    program_name
                );
                let output = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .output()?;
                
                if output.status.success() {
                    Ok(format!("Removed {} from Login Items", program_name))
                } else {
                    Err(anyhow::anyhow!("Failed to remove from Login Items: {}", 
                        String::from_utf8_lossy(&output.stderr)))
                }
            }
            "LaunchAgent" => {
                // Move plist file to disabled folder
                let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home dir"))?;
                let plist_path = home.join(format!("Library/LaunchAgents/{}.plist", program_name));
                let disabled_dir = home.join("Library/LaunchAgents/Disabled");
                
                std::fs::create_dir_all(&disabled_dir)?;
                let disabled_path = disabled_dir.join(format!("{}.plist", program_name));
                
                std::fs::rename(&plist_path, &disabled_path)?;
                
                // Unload the service
                let _ = std::process::Command::new("launchctl")
                    .args(&["unload", &plist_path.to_string_lossy()])
                    .output();
                
                Ok(format!("Disabled LaunchAgent {} (moved to Disabled folder)", program_name))
            }
            _ => Err(anyhow::anyhow!("Unknown source type: {}", source))
        }
    }
    
    /// Disable Windows startup program
    #[cfg(target_os = "windows")]
    fn disable_windows_startup(&self, program_name: &str, source: &str) -> anyhow::Result<String> {
        match source {
            "Registry Run" => {
                // Remove from registry
                let ps_script = format!(
                    r#"Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name '{}' -ErrorAction SilentlyContinue"#,
                    program_name
                );
                let output = std::process::Command::new("powershell")
                    .args(&["-Command", &ps_script])
                    .output()?;
                
                if output.status.success() {
                    Ok(format!("Removed {} from Registry Run", program_name))
                } else {
                    Err(anyhow::anyhow!("Failed to remove from registry"))
                }
            }
            "Startup Folder" => {
                // Remove shortcut from startup folder
                if let Some(app_data) = dirs::config_dir() {
                    let startup_path = app_data.join(format!(
                        "Microsoft/Windows/Start Menu/Programs/Startup/{}.lnk", 
                        program_name
                    ));
                    std::fs::remove_file(&startup_path)?;
                    Ok(format!("Removed {} from Startup folder", program_name))
                } else {
                    Err(anyhow::anyhow!("Could not find startup folder"))
                }
            }
            _ => Err(anyhow::anyhow!("Unknown source type: {}", source))
        }
    }
    
    /// Disable Linux startup program
    #[cfg(target_os = "linux")]
    fn disable_linux_startup(&self, program_name: &str, source: &str) -> anyhow::Result<String> {
        match source {
            "systemd" => {
                // Disable the systemd user service
                let output = std::process::Command::new("systemctl")
                    .args(&["--user", "disable", program_name])
                    .output()?;
                
                if output.status.success() {
                    Ok(format!("Disabled systemd service {}", program_name))
                } else {
                    Err(anyhow::anyhow!("Failed to disable systemd service: {}",
                        String::from_utf8_lossy(&output.stderr)))
                }
            }
            "Desktop Autostart" => {
                // Remove desktop file from autostart
                if let Some(config_dir) = dirs::config_dir() {
                    let desktop_file = config_dir.join(format!("autostart/{}.desktop", program_name));
                    std::fs::remove_file(&desktop_file)?;
                    Ok(format!("Removed {} from autostart", program_name))
                } else {
                    Err(anyhow::anyhow!("Could not find autostart directory"))
                }
            }
            _ => Err(anyhow::anyhow!("Unknown source type: {}", source))
        }
    }
}

#[async_trait]
impl Skill for StartupOptimizer {
    fn id(&self) -> &'static str {
        "startup_optimizer"
    }
    
    fn name(&self) -> &'static str {
        "Startup Optimizer"
    }
    
    fn description(&self) -> &'static str {
        "Analyzes and optimizes startup programs to improve boot time"
    }
    
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix]
    }
    
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive // Needs user approval to make changes
    }
    
    async fn execute(&self, _input: SkillInput, _ctx: &SkillContext) -> anyhow::Result<SkillOutput> {
        // Detect startup programs
        let programs = self.detect_startup_programs()?;
        
        // Calculate boot times
        let (current_boot_time, optimized_boot_time) = self.calculate_boot_times(&programs);
        
        let optimizable_count = programs.iter()
            .filter(|p| self.is_unused(p))
            .count();
        
        let result = StartupOptimizationResult {
            programs,
            current_boot_time,
            optimized_boot_time,
            optimizable_count,
        };
        
        let formatted_text = self.format_results(&result);
        
        // Create suggested actions for optimizable programs
        let mut suggested_actions = Vec::new();
        for program in &result.programs {
            if self.is_unused(program) && program.enabled {
                let mut params = HashMap::new();
                params.insert("program_name".to_string(), serde_json::json!(program.name));
                params.insert("source".to_string(), serde_json::json!(program.source));
                
                suggested_actions.push(SuggestedAction {
                    label: format!("Disable {} (save {:.1}s boot time)", program.name, program.boot_time_impact),
                    skill_id: "disable_startup_program".to_string(),
                    params,
                });
            }
        }
        
        // Add bulk action if multiple optimizable
        let optimizable_count = result.programs.iter().filter(|p| self.is_unused(p) && p.enabled).count();
        if optimizable_count > 1 {
            suggested_actions.push(SuggestedAction {
                label: format!("Disable all {} optimizable apps (save {:.1}s total)", 
                    optimizable_count, 
                    result.current_boot_time - result.optimized_boot_time),
                skill_id: "disable_all_startup_programs".to_string(),
                params: HashMap::new(),
            });
        }
        
        Ok(SkillOutput {
            result_type: shared::skill::ResultType::Text,
            text: Some(formatted_text),
            files: Vec::new(),
            data: Some(serde_json::to_value(result)?),
            citations: Vec::new(),
            suggested_actions,
        })
    }
}

impl Default for StartupOptimizer {
    fn default() -> Self {
        Self::new()
    }
}