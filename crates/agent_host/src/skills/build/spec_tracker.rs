//! Tracks the status of a Spec-Driven Build.
//!
//! Persists the build plan and current progress to `specs/status.json`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub output_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecStatus {
    pub spec_file: String,
    pub current_task_index: usize,
    pub tasks: Vec<BuildTask>,
    pub completed: bool,
}

impl SpecStatus {
    pub fn new(spec_file: String, tasks: Vec<BuildTask>) -> Self {
        Self {
            spec_file,
            current_task_index: 0,
            tasks,
            completed: false,
        }
    }

    /// Load from `specs/status.json` in the given project root
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join("specs/status.json");
        let content = fs::read_to_string(path)?;
        let status = serde_json::from_str(&content)?;
        Ok(status)
    }

    /// Save to `specs/status.json`
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let path = project_root.join("specs/status.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the next pending task
    pub fn next_pending_task(&self) -> Option<&BuildTask> {
        if self.completed {
            return None;
        }
        self.tasks.get(self.current_task_index)
    }

    /// Mark current task as complete and advance
    pub fn complete_current_task(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.current_task_index) {
            task.status = TaskStatus::Completed;
        }

        self.current_task_index += 1;
        if self.current_task_index >= self.tasks.len() {
            self.completed = true;
        }
    }
}
