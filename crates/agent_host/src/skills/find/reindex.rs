//! Skill to manually trigger a re-index of the file system.

use anyhow::Result;
use async_trait::async_trait;
use services::file_index::FileIndexService;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};

use std::sync::Arc;

/// Manually trigger a file index scan
pub struct ForceReindexSkill {
    file_index: Arc<FileIndexService>,
}

impl ForceReindexSkill {
    pub fn new(file_index: Arc<FileIndexService>) -> Self {
        Self { file_index }
    }
}

#[async_trait]
impl Skill for ForceReindexSkill {
    fn id(&self) -> &'static str {
        "reindex_files"
    }

    fn name(&self) -> &'static str {
        "Re-index Files"
    }

    fn description(&self) -> &'static str {
        "Force a re-scan of the file index to find new files"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Find]
    }

    async fn execute(&self, _input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput> {
        // Use shared service
        let service = &self.file_index;

        // Scan user's home/project directory (or just current working dir?)
        let scan_path = &ctx.working_dir;

        // Start scan
        let stats = service.scan_drive(scan_path, "local")?;

        Ok(SkillOutput::text(format!(
            "Index updated for `{}`.\n\nStats:\n- Scanned: {}\n- Indexed: {}\n- Errors: {}",
            scan_path.display(),
            stats.total_files,
            stats.indexed,
            stats.errors
        )))
    }
}
