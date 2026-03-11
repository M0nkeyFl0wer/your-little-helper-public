//! Git Helper skill for user project management.
//!
//! Provides a friendly interface for Real Git operations, separate from the Shadow Git system.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};
use std::process::Command;

/// Helper for standard Git operations
pub struct GitHelper;

#[async_trait]
impl Skill for GitHelper {
    fn id(&self) -> &'static str {
        "git_helper"
    }

    fn name(&self) -> &'static str {
        "Git Helper"
    }

    fn description(&self) -> &'static str {
        "Manage your project's git repository (Status, Add, Commit, Log)"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive // Modifies git repo
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Build, Mode::Fix]
    }

    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput> {
        let action = input
            .params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        match action {
            "status" => {
                let output = Command::new("git")
                    .arg("status")
                    .current_dir(&ctx.working_dir)
                    .output()?;

                let text = String::from_utf8_lossy(&output.stdout);
                Ok(SkillOutput::text(format!(
                    "## Git Status\n```\n{}\n```",
                    text
                )))
            }
            "init" => {
                let output = Command::new("git")
                    .arg("init")
                    .current_dir(&ctx.working_dir)
                    .output()?;
                let text = String::from_utf8_lossy(&output.stdout);
                Ok(SkillOutput::text(format!("## Git Init\n{}", text)))
            }
            "add" => {
                let files = input
                    .params
                    .get("files")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");

                let output = Command::new("git")
                    .arg("add")
                    .arg(files)
                    .current_dir(&ctx.working_dir)
                    .output()?;

                if output.status.success() {
                    Ok(SkillOutput::text(format!("Added files: `{}`", files)))
                } else {
                    let err = String::from_utf8_lossy(&output.stderr);
                    Ok(SkillOutput::text(format!("Failed to add files:\n{}", err)))
                }
            }
            "commit" => {
                let message = input
                    .params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Update project");

                let output = Command::new("git")
                    .arg("commit")
                    .arg("-m")
                    .arg(message)
                    .current_dir(&ctx.working_dir)
                    .output()?;

                let text = String::from_utf8_lossy(&output.stdout);
                if output.status.success() {
                    Ok(SkillOutput::text(format!(
                        "## Committed\n```\n{}\n```",
                        text
                    )))
                } else {
                    let err = String::from_utf8_lossy(&output.stderr);
                    Ok(SkillOutput::text(format!(
                        "Commit failed:\n{}\n{}",
                        text, err
                    )))
                }
            }
            "log" => {
                let output = Command::new("git")
                    .args(["log", "--oneline", "-n", "5"])
                    .current_dir(&ctx.working_dir)
                    .output()?;
                let text = String::from_utf8_lossy(&output.stdout);
                Ok(SkillOutput::text(format!(
                    "## Recent Commits\n```\n{}\n```",
                    text
                )))
            }
            _ => Ok(SkillOutput::text(format!("Unknown git action: {}", action))),
        }
    }
}
