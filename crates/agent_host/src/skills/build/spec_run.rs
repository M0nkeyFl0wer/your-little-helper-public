//! Spec Run Skill - Deploy AI swarms to implement specs

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};
use std::process::Command;

use super::spec_utils::{resolve_spec_kit_path, resolve_target_folder};

/// Run AI swarms to implement project specs
pub struct SpecRunSkill;

#[async_trait]
impl Skill for SpecRunSkill {
    fn id(&self) -> &'static str {
        "spec_run"
    }

    fn name(&self) -> &'static str {
        "Spec Run"
    }

    fn description(&self) -> &'static str {
        "Deploy AI swarms to implement features based on project specs"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive // Modifies code
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Build]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let description = if input.query.is_empty() {
            input
                .params
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("implement the spec")
                .to_string()
        } else {
            input.query.clone()
        };

        let folder = resolve_target_folder(&input);

        let spec = input.params.get("spec").and_then(|v| v.as_str());

        // Check if spec-kit-assistant is available
        let spec_kit_path = resolve_spec_kit_path(&input);

        if !spec_kit_path.exists() {
            return Ok(SkillOutput::text(format!(
                "Spec Kit Assistant not found.\n\n\
                To use AI swarms for implementation:\n\
                1. Put spec-kit-assistant in your Projects folder\n\
                2. Or set the Spec Kit path in Settings → Build\n\n\
                In the meantime, I can help you implement '{}' manually.\n\
                Would you like me to break this down into steps?",
                description
            )));
        }

        // Build command
        let mut cmd = Command::new("node");
        cmd.arg(&spec_kit_path)
            .arg("run")
            .arg(&description)
            .current_dir(&folder);

        if let Some(spec_name) = spec {
            cmd.arg("--spec").arg(spec_name);
        }

        // Run spec-kit
        let output = cmd.output();

        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);

                if result.status.success() {
                    Ok(SkillOutput::text(format!(
                        "Swarm deployed to implement: {}\n\n\
                        {}\n\n\
                        The AI agents are working on your request. \
                        Check the project folder for changes.",
                        description, stdout
                    )))
                } else {
                    Ok(SkillOutput::text(format!(
                        "Swarm encountered an issue:\n{}\n{}",
                        stdout, stderr
                    )))
                }
            }
            Err(e) => Ok(SkillOutput::text(format!(
                "Failed to run spec swarm: {}",
                e
            ))),
        }
    }
}
