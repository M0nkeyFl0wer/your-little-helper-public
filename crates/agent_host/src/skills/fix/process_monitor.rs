//! Process monitoring skill for Fix mode.
//!
//! Lists running processes and helps identify resource-heavy applications
//! that might be causing performance issues.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use std::process::Command;

/// Process monitoring skill.
pub struct ProcessMonitor;

impl ProcessMonitor {
    pub fn new() -> Self {
        Self
    }

    /// Get list of running processes sorted by resource usage
    fn get_processes() -> Result<Vec<ProcessInfo>> {
        let mut processes = Vec::new();

        #[cfg(target_os = "windows")]
        {
            // Windows: Use tasklist with CSV format
            if let Ok(output) = Command::new("tasklist")
                .args(["/FO", "CSV", "/NH"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    // Parse CSV: "name","pid","session","session#","mem"
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim_matches('"')).collect();
                    if parts.len() >= 5 {
                        let name = parts[0].to_string();
                        let pid: u32 = parts[1].parse().unwrap_or(0);
                        // Memory format: "12,345 K"
                        let mem_str = parts[4].replace(',', "").replace(" K", "");
                        let memory_kb: u64 = mem_str.trim().parse().unwrap_or(0);

                        processes.push(ProcessInfo {
                            name,
                            pid,
                            memory_mb: memory_kb / 1024,
                            cpu_percent: 0.0, // Windows tasklist doesn't show CPU
                        });
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Linux/macOS: Use ps command
            if let Ok(output) = Command::new("ps").args(["aux", "--sort=-rss"]).output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().skip(1).take(50) {
                    // Top 50 processes
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 11 {
                        let pid: u32 = parts[1].parse().unwrap_or(0);
                        let cpu_percent: f32 = parts[2].parse().unwrap_or(0.0);
                        let _mem_percent: f32 = parts[3].parse().unwrap_or(0.0);
                        let rss_kb: u64 = parts[5].parse().unwrap_or(0);
                        let name = parts[10..].join(" ");

                        processes.push(ProcessInfo {
                            name,
                            pid,
                            memory_mb: rss_kb / 1024,
                            cpu_percent,
                        });
                    }
                }
            }
        }

        // Sort by memory usage (descending)
        processes.sort_by(|a, b| b.memory_mb.cmp(&a.memory_mb));

        Ok(processes)
    }

    /// Identify potentially problematic processes
    fn analyze_processes(processes: &[ProcessInfo]) -> Vec<ProcessIssue> {
        let mut issues = Vec::new();

        // Check for high memory usage processes
        for proc in processes.iter().take(10) {
            if proc.memory_mb > 2000 {
                issues.push(ProcessIssue {
                    process: proc.clone(),
                    issue_type: IssueType::HighMemory,
                    message: format!(
                        "'{}' is using {:.1} GB of memory",
                        proc.name,
                        proc.memory_mb as f32 / 1024.0
                    ),
                });
            }

            if proc.cpu_percent > 50.0 {
                issues.push(ProcessIssue {
                    process: proc.clone(),
                    issue_type: IssueType::HighCpu,
                    message: format!("'{}' is using {:.1}% CPU", proc.name, proc.cpu_percent),
                });
            }
        }

        // Check for duplicate process names (potential issue)
        let mut name_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        for proc in processes {
            *name_counts.entry(proc.name.clone()).or_insert(0) += 1;
        }
        for (name, count) in name_counts {
            if count > 10 && !is_expected_multiple(&name) {
                issues.push(ProcessIssue {
                    process: ProcessInfo {
                        name: name.clone(),
                        pid: 0,
                        memory_mb: 0,
                        cpu_percent: 0.0,
                    },
                    issue_type: IssueType::ManyInstances,
                    message: format!("'{}' has {} instances running", name, count),
                });
            }
        }

        issues
    }

    /// Format the process report
    fn format_report(processes: &[ProcessInfo], issues: &[ProcessIssue], query: &str) -> String {
        let mut report = String::new();

        // Check if user asked about specific process
        let filter = if !query.is_empty() && !query.to_lowercase().contains("process") {
            Some(query.to_lowercase())
        } else {
            None
        };

        report.push_str("## Running Processes\n\n");

        // Show issues first if any
        if !issues.is_empty() {
            report.push_str("### Potential Issues\n\n");
            for issue in issues {
                let icon = match issue.issue_type {
                    IssueType::HighMemory => "🔴",
                    IssueType::HighCpu => "🟠",
                    IssueType::ManyInstances => "🟡",
                };
                report.push_str(&format!("{} {}\n", icon, issue.message));
            }
            report.push('\n');
        }

        // Top processes by memory
        report.push_str("### Top Processes by Memory\n\n");
        report.push_str("| Process | PID | Memory | CPU |\n");
        report.push_str("|---------|-----|--------|-----|\n");

        let filtered: Vec<&ProcessInfo> = if let Some(ref f) = filter {
            processes
                .iter()
                .filter(|p| p.name.to_lowercase().contains(f))
                .take(20)
                .collect()
        } else {
            processes.iter().take(15).collect()
        };

        if filtered.is_empty() && filter.is_some() {
            report.push_str(&format!(
                "\nNo processes found matching '{}'\n",
                filter.unwrap()
            ));
        } else {
            for proc in filtered {
                let mem_display = if proc.memory_mb > 1024 {
                    format!("{:.1} GB", proc.memory_mb as f32 / 1024.0)
                } else {
                    format!("{} MB", proc.memory_mb)
                };

                let cpu_display = if proc.cpu_percent > 0.0 {
                    format!("{:.1}%", proc.cpu_percent)
                } else {
                    "-".to_string()
                };

                // Truncate long process names
                let name_display = if proc.name.len() > 30 {
                    format!("{}...", &proc.name[..27])
                } else {
                    proc.name.clone()
                };

                report.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    name_display, proc.pid, mem_display, cpu_display
                ));
            }
        }

        report.push_str("\n*Sorted by memory usage (highest first)*\n");

        report
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a process is expected to have many instances
fn is_expected_multiple(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    // Browser processes, system processes, etc.
    name_lower.contains("chrome")
        || name_lower.contains("firefox")
        || name_lower.contains("edge")
        || name_lower.contains("helper")
        || name_lower.contains("worker")
        || name_lower.contains("svchost")
        || name_lower.contains("kworker")
        || name_lower.contains("thread")
}

#[derive(Clone)]
struct ProcessInfo {
    name: String,
    pid: u32,
    memory_mb: u64,
    cpu_percent: f32,
}

#[derive(Clone)]
enum IssueType {
    HighMemory,
    HighCpu,
    ManyInstances,
}

struct ProcessIssue {
    process: ProcessInfo,
    issue_type: IssueType,
    message: String,
}

#[async_trait]
impl Skill for ProcessMonitor {
    fn id(&self) -> &'static str {
        "process_monitor"
    }

    fn name(&self) -> &'static str {
        "Process Monitor"
    }

    fn description(&self) -> &'static str {
        "List running processes and identify resource-heavy applications"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let processes = Self::get_processes()?;
        let issues = Self::analyze_processes(&processes);
        let report = Self::format_report(&processes, &issues, &input.query);

        // Calculate total memory usage
        let total_memory_mb: u64 = processes.iter().map(|p| p.memory_mb).sum();

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(report),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "process_count": processes.len(),
                "total_memory_mb": total_memory_mb,
                "issues_count": issues.len(),
                "top_process": processes.first().map(|p| &p.name),
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
    fn test_expected_multiples() {
        assert!(is_expected_multiple("Google Chrome Helper"));
        assert!(is_expected_multiple("svchost.exe"));
        assert!(!is_expected_multiple("notepad.exe"));
    }
}
