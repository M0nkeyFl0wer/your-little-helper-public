//! Write File Skill - Create or update files with version control and preview.
//!
//! This skill replaces raw shell commands (echo, cat) for file manipulation,
//! ensuring that:
//! 1. All changes are versioned via SafeFileOps
//! 2. A <preview> tag is returned to the user automatically

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput};
use std::path::PathBuf;

use super::CommonInfrastructure;

pub struct WriteFileSkill {
    infra: std::sync::Arc<CommonInfrastructure>,
}

impl WriteFileSkill {
    pub fn new(infra: std::sync::Arc<CommonInfrastructure>) -> Self {
        Self { infra }
    }
}

#[async_trait]
impl Skill for WriteFileSkill {
    fn id(&self) -> &'static str {
        "write_file"
    }

    fn name(&self) -> &'static str {
        "Write File"
    }

    fn description(&self) -> &'static str {
        "Create or update a file. Automatically versions changes and shows a preview."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Build, Mode::Fix, Mode::Content, Mode::Data, Mode::Research]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let path_str = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;

        let content = input
            .params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;

        let path = PathBuf::from(path_str);

        // Use SafeFileOps to write the file (handles versioning)
        self.infra.safe_file_ops.write_file(&path, content.as_bytes())?;

        // Generate the preview tag
        let preview_tag = format!(
            r#"<preview type="file" path="{}" />"#,
            path.display()
        );

        Ok(SkillOutput::text(format!(
            "File written successfully to {}.\n{}",
            path.display(),
            preview_tag
        )))
    }
}
