//! Content mode skills for writing and editing.
//!
//! Provides:
//! - Text analysis and polishing
//! - Grammar and style suggestions
//! - Content formatting (future)
//! - Brainstorming helpers (future)

pub mod text_polisher;

pub use text_polisher::TextPolisher;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all Content mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(TextPolisher::new()));
}
