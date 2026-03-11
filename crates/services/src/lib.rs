//! External service integrations for Little Helper.
//!
//! Each module provides a self-contained service the app can call:
//! - [`file_index`] -- SQLite FTS5-backed file indexing and fuzzy search.
//! - [`file_search`] -- Lightweight in-memory file finder using `ignore` crate walkers.
//! - [`version_control`] -- Hidden git-based file versioning (no git terminology in UI).
//! - [`web_preview`] -- Web page metadata extraction and screenshot capture.
//! - [`slack`] -- Incoming webhook notifications for draft-ready and content events.
//! - [`organizer`] -- File move/rename plan builder with safe apply (no deletes).
//! - [`mini_swarm`] -- Stub for future multi-agent research pipeline.
//! - [`support`] -- Basic network diagnostics (DNS, TCP connectivity checks).

pub mod file_index;
pub mod file_search;
pub mod mini_swarm;
pub mod organizer;
pub mod slack;
pub mod support;
pub mod version_control;
pub mod web_preview;
