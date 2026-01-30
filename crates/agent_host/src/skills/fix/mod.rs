//! Fix mode skills for troubleshooting and diagnostics.
//!
//! Provides:
//! - System health diagnostics (CPU, memory, disk)
//! - Process monitoring and analysis
//! - Error message explanation
//! - Startup optimization
//! - Log analysis (future)

pub mod error_explainer;
pub mod process_monitor;
pub mod startup_optimizer;
pub mod system_diagnostics;

pub use error_explainer::ErrorExplainer;
pub use process_monitor::ProcessMonitor;
pub use startup_optimizer::StartupOptimizer;
pub use system_diagnostics::SystemDiagnostics;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all Fix mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(SystemDiagnostics::new()));
    registry.register(Arc::new(ProcessMonitor::new()));
    registry.register(Arc::new(ErrorExplainer::new()));
    registry.register(Arc::new(StartupOptimizer::new()));
}
