//! System diagnostics skill for Fix mode.
//!
//! Provides system health checks including CPU, memory, disk usage,
//! and basic system information to help diagnose performance issues.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use std::process::Command;

/// System diagnostics skill for troubleshooting.
pub struct SystemDiagnostics;

impl SystemDiagnostics {
    pub fn new() -> Self {
        Self
    }

    /// Get system information based on OS
    fn get_system_info() -> Result<SystemInfo> {
        let mut info = SystemInfo::default();

        #[cfg(target_os = "windows")]
        {
            // Windows: Use systeminfo and wmic
            if let Ok(output) = Command::new("wmic")
                .args([
                    "os",
                    "get",
                    "Caption,Version,TotalVisibleMemorySize,FreePhysicalMemory",
                    "/format:list",
                ])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some((key, value)) = line.split_once('=') {
                        match key.trim() {
                            "Caption" => info.os_name = value.trim().to_string(),
                            "Version" => info.os_version = value.trim().to_string(),
                            "TotalVisibleMemorySize" => {
                                if let Ok(kb) = value.trim().parse::<u64>() {
                                    info.total_memory_mb = kb / 1024;
                                }
                            }
                            "FreePhysicalMemory" => {
                                if let Ok(kb) = value.trim().parse::<u64>() {
                                    info.free_memory_mb = kb / 1024;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Get CPU info
            if let Ok(output) = Command::new("wmic")
                .args(["cpu", "get", "Name,LoadPercentage", "/format:list"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some((key, value)) = line.split_once('=') {
                        match key.trim() {
                            "Name" => info.cpu_model = value.trim().to_string(),
                            "LoadPercentage" => {
                                if let Ok(pct) = value.trim().parse::<f32>() {
                                    info.cpu_usage_percent = pct;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Get disk info
            if let Ok(output) = Command::new("wmic")
                .args([
                    "logicaldisk",
                    "get",
                    "DeviceID,Size,FreeSpace",
                    "/format:list",
                ])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut current_disk = DiskInfo::default();
                for line in stdout.lines() {
                    if let Some((key, value)) = line.split_once('=') {
                        match key.trim() {
                            "DeviceID" => current_disk.mount_point = value.trim().to_string(),
                            "Size" => {
                                if let Ok(bytes) = value.trim().parse::<u64>() {
                                    current_disk.total_gb = bytes / (1024 * 1024 * 1024);
                                }
                            }
                            "FreeSpace" => {
                                if let Ok(bytes) = value.trim().parse::<u64>() {
                                    current_disk.free_gb = bytes / (1024 * 1024 * 1024);
                                    if current_disk.total_gb > 0 {
                                        info.disks.push(current_disk.clone());
                                    }
                                    current_disk = DiskInfo::default();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Linux/macOS: Use standard Unix commands
            // OS info
            if let Ok(output) = Command::new("uname").args(["-sr"]).output() {
                info.os_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            }

            // Memory info (Linux)
            if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
                for line in content.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                info.total_memory_mb = kb / 1024;
                            }
                        }
                    } else if line.starts_with("MemAvailable:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                info.free_memory_mb = kb / 1024;
                            }
                        }
                    }
                }
            }

            // CPU info
            if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
                for line in content.lines() {
                    if line.starts_with("model name") {
                        if let Some(name) = line.split(':').nth(1) {
                            info.cpu_model = name.trim().to_string();
                            break;
                        }
                    }
                }
            }

            // CPU usage from /proc/stat (simplified)
            if let Ok(content) = std::fs::read_to_string("/proc/stat") {
                if let Some(cpu_line) = content.lines().next() {
                    let parts: Vec<&str> = cpu_line.split_whitespace().collect();
                    if parts.len() >= 5 {
                        let user: u64 = parts[1].parse().unwrap_or(0);
                        let nice: u64 = parts[2].parse().unwrap_or(0);
                        let system: u64 = parts[3].parse().unwrap_or(0);
                        let idle: u64 = parts[4].parse().unwrap_or(0);
                        let total = user + nice + system + idle;
                        if total > 0 {
                            let busy = user + nice + system;
                            info.cpu_usage_percent = (busy as f32 / total as f32) * 100.0;
                        }
                    }
                }
            }

            // Disk info
            if let Ok(output) = Command::new("df")
                .args(["-BG", "--output=target,size,avail"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().skip(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let mount = parts[0].to_string();
                        // Skip virtual filesystems
                        if mount.starts_with("/dev") || mount == "/" || mount.starts_with("/home") {
                            let total: u64 = parts[1].trim_end_matches('G').parse().unwrap_or(0);
                            let free: u64 = parts[2].trim_end_matches('G').parse().unwrap_or(0);
                            if total > 0 {
                                info.disks.push(DiskInfo {
                                    mount_point: mount,
                                    total_gb: total,
                                    free_gb: free,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(info)
    }

    /// Analyze system health and generate recommendations
    fn analyze_health(info: &SystemInfo) -> Vec<HealthIssue> {
        let mut issues = Vec::new();

        // Check memory usage
        if info.total_memory_mb > 0 {
            let used_percent = ((info.total_memory_mb - info.free_memory_mb) as f32
                / info.total_memory_mb as f32)
                * 100.0;
            if used_percent > 90.0 {
                issues.push(HealthIssue {
                    severity: Severity::Critical,
                    component: "Memory".to_string(),
                    message: format!("Memory usage is critically high ({:.1}%)", used_percent),
                    recommendation: "Close unused applications or consider adding more RAM"
                        .to_string(),
                });
            } else if used_percent > 75.0 {
                issues.push(HealthIssue {
                    severity: Severity::Warning,
                    component: "Memory".to_string(),
                    message: format!("Memory usage is elevated ({:.1}%)", used_percent),
                    recommendation: "Consider closing some applications if performance is slow"
                        .to_string(),
                });
            }
        }

        // Check CPU usage
        if info.cpu_usage_percent > 90.0 {
            issues.push(HealthIssue {
                severity: Severity::Critical,
                component: "CPU".to_string(),
                message: format!("CPU usage is very high ({:.1}%)", info.cpu_usage_percent),
                recommendation: "Check for runaway processes using Task Manager or Activity Monitor".to_string(),
            });
        } else if info.cpu_usage_percent > 70.0 {
            issues.push(HealthIssue {
                severity: Severity::Warning,
                component: "CPU".to_string(),
                message: format!("CPU usage is elevated ({:.1}%)", info.cpu_usage_percent),
                recommendation: "This may be normal during intensive tasks".to_string(),
            });
        }

        // Check disk space
        for disk in &info.disks {
            if disk.total_gb > 0 {
                let used_percent =
                    ((disk.total_gb - disk.free_gb) as f32 / disk.total_gb as f32) * 100.0;
                if used_percent > 95.0 {
                    issues.push(HealthIssue {
                        severity: Severity::Critical,
                        component: format!("Disk ({})", disk.mount_point),
                        message: format!(
                            "Disk almost full ({:.1}% used, {} GB free)",
                            used_percent, disk.free_gb
                        ),
                        recommendation: "Delete unnecessary files or move data to external storage"
                            .to_string(),
                    });
                } else if used_percent > 85.0 {
                    issues.push(HealthIssue {
                        severity: Severity::Warning,
                        component: format!("Disk ({})", disk.mount_point),
                        message: format!(
                            "Disk space is low ({:.1}% used, {} GB free)",
                            used_percent, disk.free_gb
                        ),
                        recommendation: "Consider cleaning up old files".to_string(),
                    });
                }
            }
        }

        issues
    }

    /// Format the diagnostic report
    fn format_report(info: &SystemInfo, issues: &[HealthIssue]) -> String {
        let mut report = String::new();

        // Overall status
        let status = if issues.iter().any(|i| i.severity == Severity::Critical) {
            "Needs Attention"
        } else if issues.iter().any(|i| i.severity == Severity::Warning) {
            "Minor Issues"
        } else {
            "Healthy"
        };

        report.push_str(&format!("## System Health: {}\n\n", status));

        // System info
        report.push_str("### System Information\n");
        if !info.os_name.is_empty() {
            report.push_str(&format!("- **OS**: {}", info.os_name));
            if !info.os_version.is_empty() {
                report.push_str(&format!(" ({})", info.os_version));
            }
            report.push('\n');
        }
        if !info.cpu_model.is_empty() {
            report.push_str(&format!("- **CPU**: {}\n", info.cpu_model));
        }
        report.push('\n');

        // Resource usage
        report.push_str("### Resource Usage\n");

        // Memory
        if info.total_memory_mb > 0 {
            let used_mb = info.total_memory_mb - info.free_memory_mb;
            let used_percent = (used_mb as f32 / info.total_memory_mb as f32) * 100.0;
            report.push_str(&format!(
                "- **Memory**: {:.1} GB / {:.1} GB ({:.1}% used)\n",
                used_mb as f32 / 1024.0,
                info.total_memory_mb as f32 / 1024.0,
                used_percent
            ));
        }

        // CPU
        report.push_str(&format!(
            "- **CPU Usage**: {:.1}%\n",
            info.cpu_usage_percent
        ));

        // Disks
        if !info.disks.is_empty() {
            report.push_str("\n### Storage\n");
            for disk in &info.disks {
                let used_gb = disk.total_gb - disk.free_gb;
                let used_percent = if disk.total_gb > 0 {
                    (used_gb as f32 / disk.total_gb as f32) * 100.0
                } else {
                    0.0
                };
                report.push_str(&format!(
                    "- **{}**: {} GB / {} GB ({:.1}% used)\n",
                    disk.mount_point, used_gb, disk.total_gb, used_percent
                ));
            }
        }

        // Issues and recommendations
        if !issues.is_empty() {
            report.push_str("\n### Issues Found\n");
            for issue in issues {
                let icon = match issue.severity {
                    Severity::Critical => "🔴",
                    Severity::Warning => "🟡",
                    Severity::Info => "🔵",
                };
                report.push_str(&format!(
                    "\n{} **{}**: {}\n   → {}\n",
                    icon, issue.component, issue.message, issue.recommendation
                ));
            }
        } else {
            report.push_str("\n✅ No issues detected. Your system looks healthy!\n");
        }

        report
    }
}

impl Default for SystemDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone)]
struct SystemInfo {
    os_name: String,
    os_version: String,
    cpu_model: String,
    cpu_usage_percent: f32,
    total_memory_mb: u64,
    free_memory_mb: u64,
    disks: Vec<DiskInfo>,
}

#[derive(Default, Clone)]
struct DiskInfo {
    mount_point: String,
    total_gb: u64,
    free_gb: u64,
}

#[derive(Clone, PartialEq)]
enum Severity {
    Critical,
    Warning,
    Info,
}

struct HealthIssue {
    severity: Severity,
    component: String,
    message: String,
    recommendation: String,
}

#[async_trait]
impl Skill for SystemDiagnostics {
    fn id(&self) -> &'static str {
        "system_diagnostics"
    }

    fn name(&self) -> &'static str {
        "System Diagnostics"
    }

    fn description(&self) -> &'static str {
        "Check system health including CPU, memory, and disk usage"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix]
    }

    async fn execute(&self, _input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let info = Self::get_system_info()?;
        let issues = Self::analyze_health(&info);
        let report = Self::format_report(&info, &issues);

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(report),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "os": info.os_name,
                "cpu_model": info.cpu_model,
                "cpu_usage_percent": info.cpu_usage_percent,
                "memory_total_mb": info.total_memory_mb,
                "memory_free_mb": info.free_memory_mb,
                "disk_count": info.disks.len(),
                "issues_count": issues.len(),
                "has_critical": issues.iter().any(|i| i.severity == Severity::Critical),
            })),
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_analysis_memory_critical() {
        let info = SystemInfo {
            total_memory_mb: 16000,
            free_memory_mb: 1000, // 93.75% used
            ..Default::default()
        };
        let issues = SystemDiagnostics::analyze_health(&info);
        assert!(issues
            .iter()
            .any(|i| i.severity == Severity::Critical && i.component == "Memory"));
    }

    #[test]
    fn test_health_analysis_healthy() {
        let info = SystemInfo {
            total_memory_mb: 16000,
            free_memory_mb: 8000, // 50% used
            cpu_usage_percent: 30.0,
            disks: vec![DiskInfo {
                mount_point: "/".to_string(),
                total_gb: 500,
                free_gb: 300, // 40% used
            }],
            ..Default::default()
        };
        let issues = SystemDiagnostics::analyze_health(&info);
        assert!(issues.is_empty());
    }
}
