//! Fix mode skills for troubleshooting and diagnostics.
//!
//! Fix mode relies on core tools (bash_execute, web_search) rather than
//! dedicated skills. The model handles system diagnostics, process monitoring,
//! error explanation, and other troubleshooting tasks natively via shell commands.

use crate::skills::SkillRegistry;

/// Register Fix mode skills with the registry.
/// Currently empty — Fix mode uses bash_execute + web_search directly.
pub fn register_skills(_registry: &mut SkillRegistry) {
    // Intentionally empty: the model handles Fix mode tasks via
    // bash_execute (system commands) and web_search (research).
}
