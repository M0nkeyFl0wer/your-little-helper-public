//! Build Mode Skills - Project creation and spec-driven development
//!
//! Integrates with spec-kit-assistant for structured project workflows.

mod rust_spec;
pub mod spec_tracker;

pub use rust_spec::{SpecInitSkill, SpecScaffoldSkill, SpecImplementSkill, SpecNextTaskSkill};

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all build skills
pub fn register_build_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(SpecInitSkill));
    registry.register(Arc::new(SpecScaffoldSkill));
    registry.register(Arc::new(SpecImplementSkill));
    registry.register(Arc::new(SpecNextTaskSkill));
}
