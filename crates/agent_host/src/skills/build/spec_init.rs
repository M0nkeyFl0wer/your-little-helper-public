//! Spec Init Skill - Initialize spec-driven projects using spec-kit

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};
use std::path::PathBuf;
use std::process::Command;

/// Initialize a new spec-driven project with spec-kit
pub struct SpecInitSkill;

#[async_trait]
impl Skill for SpecInitSkill {
    fn id(&self) -> &'static str {
        "spec_init"
    }

    fn name(&self) -> &'static str {
        "Spec Init"
    }

    fn description(&self) -> &'static str {
        "Initialize a new spec-driven project with structured specs, tasks, and plans"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive // Creates directories and files
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Build]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        // Extract project name from query or params
        let project_name = input
            .params
            .get("project_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Try to extract from query like "init project myapp"
                input
                    .query
                    .split_whitespace()
                    .skip_while(|w| *w != "project" && *w != "init")
                    .nth(1)
                    .unwrap_or("my-project")
                    .to_string()
            });

        let directory = input
            .params
            .get("directory")
            .and_then(|v| v.as_str())
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // Check if spec-kit-assistant is available
        let spec_kit_path = dirs::home_dir()
            .map(|h| h.join("Projects/spec-kit-assistant/spec-assistant.js"))
            .unwrap_or_default();

        if !spec_kit_path.exists() {
            return Ok(SkillOutput::text(format!(
                "Spec Kit Assistant not found at expected location.\n\n\
                To use spec-driven development:\n\
                1. Clone spec-kit-assistant to ~/Projects/\n\
                2. Run: cd ~/Projects/spec-kit-assistant && npm install\n\n\
                Alternatively, I can help you create a basic project structure manually.\n\
                Would you like me to create a simple {} project instead?",
                project_name
            )));
        }

        // Run spec-kit init
        let output = Command::new("node")
            .arg(&spec_kit_path)
            .arg("init")
            .arg(&project_name)
            .current_dir(&directory)
            .output();

        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);

                if result.status.success() {
                    let project_dir = directory.join(&project_name);

                    Ok(SkillOutput::text(format!(
                        "Project '{}' initialized with spec-driven structure!\n\n\
                        Created at: {}\n\n\
                        Next steps:\n\
                        1. cd {}\n\
                        2. Create your spec: specs/001-your-feature/spec.md\n\
                        3. Run 'spec check' to validate\n\
                        4. Use 'spec run' to implement with AI swarms\n\n\
                        {}",
                        project_name,
                        project_dir.display(),
                        project_name,
                        stdout
                    )))
                } else {
                    Ok(SkillOutput::text(format!(
                        "Spec init encountered an issue:\n{}\n{}",
                        stdout, stderr
                    )))
                }
            }
            Err(e) => Ok(SkillOutput::text(format!("Failed to run spec-kit: {}", e))),
        }
    }
}
