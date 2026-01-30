//! Spec Check Skill - Validate project specs and status

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};
use std::process::Command;

use super::spec_utils::{resolve_spec_kit_path, resolve_target_folder};

/// Check project spec status and validate structure
pub struct SpecCheckSkill;

#[async_trait]
impl Skill for SpecCheckSkill {
    fn id(&self) -> &'static str {
        "spec_check"
    }

    fn name(&self) -> &'static str {
        "Spec Check"
    }

    fn description(&self) -> &'static str {
        "Check project spec status, validate structure, and show progress"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe // Read-only check
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Build]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let folder = resolve_target_folder(&input);

        // Check for spec-kit markers
        let specs_dir = folder.join("specs");
        let constitution = folder.join("CONSTITUTION.md");
        let speckit_dir = folder.join(".speckit");

        if !specs_dir.exists() && !constitution.exists() {
            return Ok(SkillOutput::text(format!(
                "This doesn't look like a spec-driven project.\n\n\
                Folder: {}\n\n\
                Missing:\n\
                - specs/ directory\n\
                - CONSTITUTION.md\n\n\
                Would you like me to initialize it as a spec-driven project?\n\
                Just say 'init project <name>' to get started.",
                folder.display()
            )));
        }

        // Try running spec-kit check
        let spec_kit_path = resolve_spec_kit_path(&input);

        if spec_kit_path.exists() {
            let output = Command::new("node")
                .arg(&spec_kit_path)
                .arg("check")
                .current_dir(&folder)
                .output();

            if let Ok(result) = output {
                let stdout = String::from_utf8_lossy(&result.stdout);
                return Ok(SkillOutput::text(format!(
                    "Spec Check Results:\n\n{}",
                    stdout
                )));
            }
        }

        // Manual check if spec-kit not available
        let mut status_lines = vec![format!("Project folder: {}", folder.display()), String::new()];

        // Check constitution
        if constitution.exists() {
            status_lines.push("CONSTITUTION.md: Found".to_string());
        } else {
            status_lines.push("CONSTITUTION.md: Missing (recommended)".to_string());
        }

        // Count specs
        if specs_dir.exists() {
            let spec_count = std::fs::read_dir(&specs_dir)
                .map(|entries| entries.filter(|e| e.is_ok()).count())
                .unwrap_or(0);

            status_lines.push(format!("Specs: {} found", spec_count));

            // List specs
            if let Ok(entries) = std::fs::read_dir(&specs_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let name = entry.file_name();
                    let spec_path = entry.path().join("spec.md");
                    let tasks_path = entry.path().join("tasks.md");
                    let plan_path = entry.path().join("plan.md");

                    let mut spec_status = vec![];
                    if spec_path.exists() {
                        spec_status.push("spec");
                    }
                    if plan_path.exists() {
                        spec_status.push("plan");
                    }
                    if tasks_path.exists() {
                        spec_status.push("tasks");
                    }

                    status_lines.push(format!(
                        "  - {}: [{}]",
                        name.to_string_lossy(),
                        spec_status.join(", ")
                    ));
                }
            }
        } else {
            status_lines.push("Specs: None (create specs/ directory)".to_string());
        }

        // Check for .speckit config
        if speckit_dir.exists() {
            status_lines.push(String::new());
            status_lines.push("Spec Kit config: Found".to_string());
        }

        Ok(SkillOutput::text(status_lines.join("\n")))
    }
}
