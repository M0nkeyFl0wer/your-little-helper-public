use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use crate::skills::{Skill, SkillInput, SkillContext};
use crate::skills::common::CommonInfrastructure;
use crate::context_manager::ContextManager;
use parking_lot::Mutex;
use shared::skill::{Mode, PermissionLevel, SkillOutput};

/// Skill for optimizing the Knowledge Graph (The "Context Engineer")
///
/// This skill exposes tools to:
/// 1. Consolidate duplicate nodes (fuzzy matching)
/// 2. Prune low-value/outdated nodes
/// 3. Archive important information to the Daily Log
pub struct MemoryOptimizerSkill {
    infra: Arc<CommonInfrastructure>,
    context_manager: Arc<Mutex<ContextManager>>,
}

impl MemoryOptimizerSkill {
    pub fn new(infra: Arc<CommonInfrastructure>, context_manager: Arc<Mutex<ContextManager>>) -> Self {
        Self { infra, context_manager }
    }

    async fn consolidate(&self, threshold: f64) -> Result<SkillOutput> {
        // Lock access to the context manager (synchronous lock)
        let mut mgr = self.context_manager.lock();
        
        let merged = mgr.graph.consolidate_nodes(threshold);
        
        Ok(SkillOutput::text(format!(
            "Memory Consolidation Complete.\nMerged {} duplicate topics.",
            merged
        )))
    }

    async fn prune(&self, min_feedback: f32, max_days: u64, _archive_important: bool) -> Result<SkillOutput> {
         let mut mgr = self.context_manager.lock();
        
        let removed = mgr.graph.prune_nodes(min_feedback, max_days);
        
        Ok(SkillOutput::text(format!(
            "Memory Optimization Complete.\nRemoved {} low-value or outdated nodes.",
            removed
        )))
    }
    
    async fn create_log(&self, slug: &str, content: &str) -> Result<SkillOutput> {
        use crate::daily_log::DailyLogManager;
        
        // Use public accessor for archive_dir
        let data_dir = self.infra.safe_file_ops.archive_dir().parent()
            .unwrap_or(&self.infra.safe_file_ops.archive_dir());
            
        let log_mgr = DailyLogManager::new(data_dir)?;
        let path = log_mgr.create_entry(slug, content)?;
        
        let preview = format!(r#"<preview type="file" path="{}">Daily Log Entry</preview>"#, path.display());
        
        Ok(SkillOutput::text(format!(
            "Archived to Daily Log: {}\n{}", 
            path.display(),
            preview
        )))
    }
}

#[async_trait]
impl Skill for MemoryOptimizerSkill {
    fn id(&self) -> &'static str {
        "memory_optimizer"
    }

    fn name(&self) -> &'static str {
        "Memory Optimizer"
    }

    fn description(&self) -> &'static str {
        "Optimize long-term memory by merging duplicates, pruning garbage, and archiving insights."
    }
    
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive
    }
    
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix, Mode::Data, Mode::Build, Mode::Research]
    }

    // Note: 'parameters' is not part of the Skill trait in this codebase.
    // The schema is inferred from description or handled dynamically.

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        let action = input.params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        
        match action {
            "consolidate" => {
                let threshold = input.params.get("threshold").and_then(|v| v.as_f64()).unwrap_or(0.9);
                self.consolidate(threshold).await
            }
            "prune" => {
                // Default settings: Remove hated nodes (-0.5) and very old unused ones (30 days)
                self.prune(-0.5, 30, true).await
            }
            "archive" => {
                let slug = input.params.get("slug").and_then(|v| v.as_str()).context("Missing slug")?;
                let content = input.params.get("content").and_then(|v| v.as_str()).context("Missing content")?;
                self.create_log(slug, content).await
            }
            _ => {
                Ok(SkillOutput::text("Unknown action. Use consolidate, prune, or archive."))
            }
        }
    }
}
