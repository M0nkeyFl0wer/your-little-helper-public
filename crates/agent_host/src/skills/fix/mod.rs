//! Fix mode skills for troubleshooting and diagnostics.
//!
//! Provides:
//! - System health diagnostics (CPU, memory, disk)
//! - Process monitoring and analysis
//! - Error message explanation
//! - Startup optimization
//! - Privacy auditing
//! - Storage cleaning and organization
//! - Device capability detection
//! - Log analysis (future)

pub mod device_capability;
pub mod error_explainer;
pub mod privacy_auditor;
pub mod process_monitor;
pub mod startup_optimizer;
pub mod storage_cleaner;
pub mod system_diagnostics;

pub use device_capability::DeviceCapabilityDetector;
pub use error_explainer::ErrorExplainer;
pub use privacy_auditor::PrivacyAuditor;
pub use process_monitor::ProcessMonitor;
pub use startup_optimizer::StartupOptimizer;
pub use storage_cleaner::StorageCleaner;
pub use system_diagnostics::SystemDiagnostics;

use crate::skills::SkillRegistry;
use std::sync::Arc;

/// Register all Fix mode skills with the registry
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(Arc::new(SystemDiagnostics::new()));
    registry.register(Arc::new(ProcessMonitor::new()));
    registry.register(Arc::new(ErrorExplainer::new()));
    registry.register(Arc::new(StartupOptimizer::new()));
    registry.register(Arc::new(PrivacyAuditor::new()));
    registry.register(Arc::new(DeviceCapabilityDetector::new()));
    registry.register(Arc::new(StorageCleaner::new()));
}
