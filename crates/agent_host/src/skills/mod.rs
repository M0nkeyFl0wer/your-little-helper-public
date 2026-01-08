//! Skill registry and management for agent tool execution.
//!
//! This module provides the SkillRegistry that manages all available skills,
//! handles permission checking, and coordinates skill execution.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use shared::skill::{
    Mode, Permission, PermissionLevel, Skill, SkillContext, SkillError, SkillExecution, SkillInput,
};

pub mod build;
pub mod common;
pub mod content;
pub mod data;
pub mod find;
pub mod fix;
pub mod research;

/// Registry managing all available skills
pub struct SkillRegistry {
    /// All registered skills by ID
    skills: HashMap<String, Arc<dyn Skill>>,
    /// User permission settings per skill
    permissions: HashMap<String, Permission>,
}

impl SkillRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            permissions: HashMap::new(),
        }
    }

    /// Register a skill
    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        let id = skill.id().to_string();

        // Set default permission based on skill's permission level
        if !self.permissions.contains_key(&id) {
            let default_perm = match skill.permission_level() {
                PermissionLevel::Safe => Permission::Enabled,
                PermissionLevel::Sensitive => Permission::Ask,
            };
            self.permissions.insert(id.clone(), default_perm);
        }

        self.skills.insert(id, skill);
    }

    /// Get a skill by ID
    pub fn get(&self, skill_id: &str) -> Option<&Arc<dyn Skill>> {
        self.skills.get(skill_id)
    }

    /// Get all skills available for a mode
    pub fn for_mode(&self, mode: Mode) -> Vec<&Arc<dyn Skill>> {
        self.skills
            .values()
            .filter(|skill| skill.modes().contains(&mode))
            .collect()
    }

    /// Get all registered skills
    pub fn all(&self) -> impl Iterator<Item = &Arc<dyn Skill>> {
        self.skills.values()
    }

    /// Get user permission for a skill
    pub fn get_permission(&self, skill_id: &str) -> Permission {
        self.permissions
            .get(skill_id)
            .copied()
            .unwrap_or(Permission::Ask)
    }

    /// Set user permission for a skill
    pub fn set_permission(&mut self, skill_id: &str, permission: Permission) {
        self.permissions.insert(skill_id.to_string(), permission);
    }

    /// Check if skill is enabled (considering permission and session approval)
    pub fn can_execute(&self, skill_id: &str, ctx: &SkillContext) -> Result<(), SkillError> {
        let skill = self
            .skills
            .get(skill_id)
            .ok_or_else(|| SkillError::NotFound {
                skill_id: skill_id.to_string(),
            })?;

        // Check if skill supports current mode
        if !skill.modes().contains(&ctx.mode) {
            return Err(SkillError::ModeNotSupported {
                skill_id: skill_id.to_string(),
                mode: ctx.mode,
            });
        }

        // Check user permission
        match self.get_permission(skill_id) {
            Permission::Disabled => {
                return Err(SkillError::PermissionDenied {
                    skill_id: skill_id.to_string(),
                });
            }
            Permission::Ask => {
                // For Sensitive skills, check session approval
                if skill.permission_level() == PermissionLevel::Sensitive {
                    if !ctx.is_session_approved(skill_id) {
                        return Err(SkillError::PermissionDenied {
                            skill_id: skill_id.to_string(),
                        });
                    }
                }
            }
            Permission::Enabled => {
                // Always allowed
            }
        }

        Ok(())
    }

    /// Invoke a skill with permission check
    pub async fn invoke(
        &self,
        skill_id: &str,
        input: SkillInput,
        ctx: &SkillContext,
    ) -> Result<SkillExecution, SkillError> {
        // Permission check
        self.can_execute(skill_id, ctx)?;

        let skill = self
            .skills
            .get(skill_id)
            .ok_or_else(|| SkillError::NotFound {
                skill_id: skill_id.to_string(),
            })?;

        // Validate input
        skill
            .validate_input(&input)
            .map_err(|e| SkillError::InvalidInput {
                message: e.to_string(),
            })?;

        // Create execution record
        let execution = SkillExecution::new(skill_id, ctx.mode, input.clone());
        let start = Instant::now();

        // Execute skill
        match skill.execute(input, ctx).await {
            Ok(output) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(execution.complete(output, duration_ms))
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(execution.fail(e.to_string(), duration_ms))
            }
        }
    }

    /// Check if skill requires session approval
    pub fn requires_approval(&self, skill_id: &str, ctx: &SkillContext) -> bool {
        if let Some(skill) = self.skills.get(skill_id) {
            if skill.permission_level() == PermissionLevel::Sensitive {
                if self.get_permission(skill_id) == Permission::Ask {
                    return !ctx.is_session_approved(skill_id);
                }
            }
        }
        false
    }

    /// Get skill metadata for display
    pub fn skill_info(&self, skill_id: &str) -> Option<SkillInfo> {
        self.skills.get(skill_id).map(|skill| SkillInfo {
            id: skill.id(),
            name: skill.name(),
            description: skill.description(),
            permission_level: skill.permission_level(),
            modes: skill.modes().to_vec(),
            user_permission: self.get_permission(skill_id),
        })
    }

    /// Get all skills with their info for a mode
    pub fn skills_info_for_mode(&self, mode: Mode) -> Vec<SkillInfo> {
        self.for_mode(mode)
            .into_iter()
            .map(|skill| SkillInfo {
                id: skill.id(),
                name: skill.name(),
                description: skill.description(),
                permission_level: skill.permission_level(),
                modes: skill.modes().to_vec(),
                user_permission: self.get_permission(skill.id()),
            })
            .collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Skill information for display
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub permission_level: PermissionLevel,
    pub modes: Vec<Mode>,
    pub user_permission: Permission,
}

use services::file_index::FileIndexService;

/// Initialize the skill registry with all available skills
pub fn init_registry(file_index: Arc<FileIndexService>) -> SkillRegistry {
    let mut registry = SkillRegistry::new();

    // Register common skills (available in all modes)
    common::register_common_skills(&mut registry);

    // Register Find mode skills
    find::register_skills(&mut registry, file_index);

    // Register Fix mode skills
    fix::register_skills(&mut registry);

    // Register Research mode skills
    research::register_skills(&mut registry);

    // Register Data mode skills
    data::register_skills(&mut registry);

    // Register Content mode skills
    content::register_skills(&mut registry);

    // Register Build mode skills (spec-kit integration)
    build::register_build_skills(&mut registry);

    registry
}

/// Initialize an empty skill registry (for testing or minimal setup)
pub fn init_empty_registry() -> SkillRegistry {
    SkillRegistry::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use shared::skill::SkillOutput;
    use std::path::PathBuf;

    struct TestSkill;

    #[async_trait]
    impl Skill for TestSkill {
        fn id(&self) -> &'static str {
            "test_skill"
        }
        fn name(&self) -> &'static str {
            "Test Skill"
        }
        fn description(&self) -> &'static str {
            "A test skill"
        }
        fn permission_level(&self) -> PermissionLevel {
            PermissionLevel::Safe
        }
        fn modes(&self) -> &'static [Mode] {
            &[Mode::Find]
        }

        async fn execute(&self, _input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
            Ok(SkillOutput::text("Test result"))
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(TestSkill));

        assert!(registry.get("test_skill").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_registry_for_mode() {
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(TestSkill));

        let find_skills = registry.for_mode(Mode::Find);
        assert_eq!(find_skills.len(), 1);

        let fix_skills = registry.for_mode(Mode::Fix);
        assert!(fix_skills.is_empty());
    }

    #[test]
    fn test_permission_defaults() {
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(TestSkill));

        // Safe skills default to Enabled
        assert_eq!(registry.get_permission("test_skill"), Permission::Enabled);
    }

    #[tokio::test]
    async fn test_invoke_skill() {
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(TestSkill));

        let ctx = SkillContext::new(Mode::Find, PathBuf::from("/tmp"));
        let input = SkillInput::from_query("test");

        let result = registry.invoke("test_skill", input, &ctx).await;
        assert!(result.is_ok());

        let execution = result.unwrap();
        assert_eq!(execution.skill_id, "test_skill");
        assert!(execution.output.is_some());
    }
}
