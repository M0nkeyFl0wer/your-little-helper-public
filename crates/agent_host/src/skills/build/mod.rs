//! Build Mode Skills - Project creation and spec-driven development
//!
//! Integrates with spec-kit-assistant for structured project workflows.

mod spec_init;
mod spec_check;
mod spec_run;
mod project_scaffold;

pub use spec_init::SpecInitSkill;
pub use spec_check::SpecCheckSkill;
pub use spec_run::SpecRunSkill;
pub use project_scaffold::ProjectScaffoldSkill;

use std::sync::Arc;
use crate::skills::SkillRegistry;

/// Register all build skills
pub fn register_build_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(SpecInitSkill));
    registry.register(Arc::new(SpecCheckSkill));
    registry.register(Arc::new(SpecRunSkill));
    registry.register(Arc::new(ProjectScaffoldSkill));
}
