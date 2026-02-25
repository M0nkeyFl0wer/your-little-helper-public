//! Content mode skills for writing and editing.
//!
//! Content mode relies on the model's native writing capabilities.
//! The model handles text analysis, grammar suggestions, and content
//! formatting directly without dedicated skills.

use crate::skills::SkillRegistry;

/// Register Content mode skills with the registry.
/// Currently empty — the model handles content tasks natively.
pub fn register_skills(_registry: &mut SkillRegistry) {
    // Intentionally empty: the model handles writing, editing,
    // and text analysis natively.
}
