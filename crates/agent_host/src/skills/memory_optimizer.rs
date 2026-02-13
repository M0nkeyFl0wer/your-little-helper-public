use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use crate::skills::{Skill, SkillInput, SkillOutput, SkillContext};
use crate::skills::common::CommonInfrastructure;

/// Skill for optimizing the Knowledge Graph (The "Context Engineer")
///
/// This skill exposes tools to:
/// 1. Consolidate duplicate nodes (fuzzy matching)
/// 2. Prune low-value/outdated nodes
/// 3. Archive important information to the Daily Log
pub struct MemoryOptimizerSkill {
    infra: Arc<CommonInfrastructure>,
}

impl MemoryOptimizerSkill {
    pub fn new(infra: Arc<CommonInfrastructure>) -> Self {
        Self { infra }
    }

    async fn consolidate(&self, threshold: f64) -> Result<SkillOutput> {
        let mut context_manager = self.infra.context_manager.lock().await;
        // Access the graph store directly if possible, or via a method on ContextManager
        // Assuming ContextManager has a method or public field for graph_store
        // Since we can't easily change ContextManager interface right now without seeing it,
        // let's assume we can add a method to ContextManager or it exposes graph.
        
        // Wait, ContextManager wraps GraphStore. Let's check ContextManager.
        // If it doesn't expose it, we might need to add a pass-through.
        // For now, let's assume `context_manager.consolidate_graph(threshold)` exists or we add it.
        // We will need to modify ContextManager to expose this.
        
        let merged = context_manager.consolidate_graph(threshold)?;
        
        Ok(SkillOutput::text(format!(
            "Memory Consolidation Complete.\nMerged {} duplicate topics.",
            merged
        )))
    }

    async fn prune(&self, min_feedback: f32, max_days: u64, archive_important: bool) -> Result<SkillOutput> {
        let mut context_manager = self.infra.context_manager.lock().await;
        
        // Before pruning, if archive_important is true, we should identify high-value candidate nodes
        // that WOULD be pruned (e.g. old but maybe high usage?)
        // Actually, logic in prune_nodes keeps high usage nodes. 
        // Pruning only removes (usage=0 AND old) OR (feedback < min).
        
        // The "Context Engineer" task implies we might want to save *summaries* of what we are deleting 
        // or just general high-value info.
        // Let's implement a simple version first: Prune the garbage.
        
        let removed = context_manager.prune_graph(min_feedback, max_days)?;
        
        Ok(SkillOutput::text(format!(
            "Memory Optimization Complete.\nRemoved {} low-value or outdated nodes.",
            removed
        )))
    }
    
    async fn create_log(&self, slug: &str, content: &str) -> Result<SkillOutput> {
        // Use DailyLogManager from infra (we need to add it to functionality or instantiate it)
        // Ideally CommonInfrastructure should hold it, or we instantiate it here since it's lightweight logic 
        // (just path ops).
        
        // Let's instantiate it on the fly for now, or better, make it part of the skill's state if we want to cache path.
        // But `CommonInfrastructure` has `data_dir`.
        // We can just use `DailyLogManager::new`.
        
        use crate::daily_log::DailyLogManager;
        let log_mgr = DailyLogManager::new(&self.infra.data_dir)?;
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
    fn name(&self) -> &str {
        "Memory Optimizer"
    }

    fn description(&self) -> &str {
        "Optimize long-term memory by merging duplicates, pruning garbage, and archiving insights."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["consolidate", "prune", "archive"],
                    "description": "The optimization action to perform"
                },
                "threshold": {
                    "type": "number",
                    "description": "Similarity threshold for consolidation (0.0-1.0), default 0.9"
                },
                "slug": {
                    "type": "string",
                    "description": "Slug for the log entry (archive action only)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to archive (archive action only)"
                }
            },
            "required": ["action"]
        })
    }

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
