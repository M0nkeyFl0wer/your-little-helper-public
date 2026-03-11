//! Common infrastructure and cross-mode skills.
//!
//! Every agent mode shares these foundational components:
//!
//! - **`SafeFileOps`** -- all file mutations route through here to enforce
//!   the NO DELETE policy.  There is intentionally no `delete_file` method;
//!   unwanted files are *archived* instead.
//! - **`AuditLogger`** -- JSONL audit trail with automatic 10 MB rotation.
//! - **`VersionHistory` / `VersionRestore`** -- user-facing version control
//!   that hides git terminology behind friendly natural-language prompts.
//! - **`WriteFileSkill`** -- skill wrapper around `SafeFileOps::write_file`
//!   that auto-versions and emits a `<preview>` tag.
//! - **`GitHelper`** -- exposes real git operations (status, add, commit,
//!   log) as a Sensitive skill for the Build and Fix modes.

mod git_helper;
pub use git_helper::GitHelper;

pub mod audit;
pub mod safe_file_ops;
pub mod version_history;
pub mod version_restore;
pub mod write_file;

pub use audit::{AuditLogger, AuditStats};
pub use safe_file_ops::SafeFileOps;
pub use version_history::VersionHistory;
pub use version_restore::VersionRestore;
pub use write_file::WriteFileSkill;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Initialize common skill infrastructure.
///
/// Creates the SafeFileOps and AuditLogger instances with the given data directory.
pub fn init_common_infrastructure(data_dir: &Path) -> Result<CommonInfrastructure> {
    let archive_dir = data_dir.join("archive");
    let log_dir = data_dir.join("audit");

    let safe_file_ops = SafeFileOps::new(archive_dir);
    let audit_logger = AuditLogger::new(log_dir)?;

    let security_context = Arc::new(SecurityContext::new(15)); // 15 min timeout

    Ok(CommonInfrastructure {
        safe_file_ops: Arc::new(safe_file_ops),
        audit_logger: Arc::new(audit_logger),
        data_dir: data_dir.to_path_buf(),
        security_context,
    })
}

/// Shared services injected into every skill via `Arc`.
///
/// Constructed once during `init_common_infrastructure()` and threaded
/// through the registry so individual skills never need to manage their
/// own file-safety or audit plumbing.
use crate::security::SecurityContext;

pub struct CommonInfrastructure {
    pub safe_file_ops: Arc<SafeFileOps>,
    pub audit_logger: Arc<AuditLogger>,
    pub data_dir: PathBuf,
    /// Used by the security skill and the executor's 2FA gate.
    pub security_context: Arc<SecurityContext>,
}

use crate::skills::SkillRegistry;

/// Register common skills available in all modes.
pub fn register_common_skills(
    registry: &mut SkillRegistry,
    infra: &std::sync::Arc<CommonInfrastructure>,
) {
    registry.register(std::sync::Arc::new(VersionHistory::new()));
    registry.register(std::sync::Arc::new(VersionRestore::new()));
    registry.register(std::sync::Arc::new(WriteFileSkill::new(infra.clone())));
    registry.register(std::sync::Arc::new(GitHelper));
}
