use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

/// Manager for Daily Logs (Long-term archival memory)
pub struct DailyLogManager {
    log_dir: PathBuf,
}

impl DailyLogManager {
    /// Create a new DailyLogManager
    pub fn new(data_dir: &Path) -> Result<Self> {
        let log_dir = data_dir.join("memory").join("logs");
        fs::create_dir_all(&log_dir).with_context(|| "Failed to create daily log directory")?;
        Ok(Self { log_dir })
    }

    /// Create a new log entry for the current day
    ///
    /// # Arguments
    /// * `slug` - A descriptive slug for the entry (e.g., "auth-refactor")
    /// * `content` - The information to archive
    ///
    /// # Returns
    /// Path to the created log file
    pub fn create_entry(&self, slug: &str, content: &str) -> Result<PathBuf> {
        let now = Local::now();
        let date_str = now.format("%Y-%m-%d").to_string();
        let timestamp = now.format("%H:%M:%S").to_string();

        // Sanitize slug
        let safe_slug = slug
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();

        let filename = format!("{}-{}.md", date_str, safe_slug);
        let path = self.log_dir.join(&filename);

        let entry = format!(
            "# {}\n\n**Time:** {}\n\n{}\n\n---\n",
            slug.replace("-", " "),
            timestamp,
            content
        );

        // Append if file exists, create if not
        if path.exists() {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(&path)?;
            use std::io::Write;
            writeln!(file, "\n{}", entry)?;
        } else {
            fs::write(&path, entry)?;
        }

        Ok(path)
    }

    /// List logs for a specific date (optional)
    pub fn list_logs(&self) -> Result<Vec<PathBuf>> {
        let mut logs = Vec::new();
        if self.log_dir.exists() {
            for entry in fs::read_dir(&self.log_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "md") {
                    logs.push(path);
                }
            }
        }
        logs.sort();
        logs.reverse(); // Newest first
        Ok(logs)
    }
}
