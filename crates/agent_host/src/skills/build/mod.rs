//! Build Mode Skills - Project creation and spec-driven development
//!
//! Integrates with spec-kit-assistant for structured project workflows.

mod project_scaffold;
mod spec_check;
mod spec_init;
mod spec_run;
mod spec_utils;

pub use project_scaffold::ProjectScaffoldSkill;
pub use spec_check::SpecCheckSkill;
pub use spec_init::SpecInitSkill;
pub use spec_run::SpecRunSkill;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all build skills
pub fn register_build_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(SpecInitSkill));
    registry.register(Arc::new(SpecCheckSkill));
    registry.register(Arc::new(SpecRunSkill));
    registry.register(Arc::new(ProjectScaffoldSkill));
}
