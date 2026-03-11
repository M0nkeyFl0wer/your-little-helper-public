//! Build Mode Skills -- spec-driven project creation and implementation.
//!
//! Integrates with the spec-kit-assistant workflow:
//! `scaffold -> init -> implement -> next_task`, guiding users from a
//! rough idea through specification, planning, and incremental
//! task-by-task implementation.

mod rust_spec;
pub mod spec_tracker;

pub use rust_spec::{SpecImplementSkill, SpecInitSkill, SpecNextTaskSkill, SpecScaffoldSkill};

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all build skills
pub fn register_build_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(SpecInitSkill));
    registry.register(Arc::new(SpecScaffoldSkill));
    registry.register(Arc::new(SpecImplementSkill));
    registry.register(Arc::new(SpecNextTaskSkill));
}
