//! Fix mode skills for troubleshooting and diagnostics.
//!
//! Provides:
//! - System health diagnostics (CPU, memory, disk)
//! - Process monitoring and analysis
//! - Error message explanation
//! - Log analysis (future)

pub mod system_diagnostics;
pub mod process_monitor;
pub mod error_explainer;

pub use system_diagnostics::SystemDiagnostics;
pub use process_monitor::ProcessMonitor;
pub use error_explainer::ErrorExplainer;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all Fix mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(SystemDiagnostics::new()));
    registry.register(Arc::new(ProcessMonitor::new()));
    registry.register(Arc::new(ErrorExplainer::new()));
}
