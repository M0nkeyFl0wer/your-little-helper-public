//! File picker widget using rfd (rust file dialog).
//!
//! Provides native file dialogs for selecting files to add to context.

use std::path::PathBuf;

/// File picker widget for selecting files.
pub struct FilePickerWidget {
    /// Last selected files
    selected_files: Vec<PathBuf>,
    /// Whether a dialog is currently open
    dialog_open: bool,
    /// File type filter (e.g., ["txt", "md"])
    filters: Vec<FileFilter>,
    /// Starting directory
    start_dir: Option<PathBuf>,
    /// Dialog title
    title: String,
    /// Allow multiple file selection
    multiple: bool,
}

/// Filter for file types.
#[derive(Clone)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileFilter {
    pub fn new(name: impl Into<String>, extensions: &[&str]) -> Self {
        Self {
            name: name.into(),
            extensions: extensions.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Common filters
    pub fn all_files() -> Self {
        Self::new("All Files", &["*"])
    }

    pub fn text_files() -> Self {
        Self::new("Text Files", &["txt", "md", "json", "yaml", "yml", "toml"])
    }

    pub fn images() -> Self {
        Self::new("Images", &["png", "jpg", "jpeg", "gif", "webp", "svg"])
    }

    pub fn documents() -> Self {
        Self::new("Documents", &["pdf", "doc", "docx", "xls", "xlsx", "csv"])
    }

    pub fn code() -> Self {
        Self::new("Code", &["rs", "py", "js", "ts", "html", "css", "json"])
    }
}

impl Default for FilePickerWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePickerWidget {
    /// Create a new file picker.
    pub fn new() -> Self {
        Self {
            selected_files: Vec::new(),
            dialog_open: false,
            filters: Vec::new(),
            start_dir: None,
            title: "Select File".to_string(),
            multiple: false,
        }
    }

    /// Set the dialog title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Add a file filter.
    pub fn with_filter(mut self, filter: FileFilter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Set the starting directory.
    pub fn with_start_dir(mut self, dir: PathBuf) -> Self {
        self.start_dir = Some(dir);
        self
    }

    /// Allow multiple file selection.
    pub fn multiple(mut self, allow: bool) -> Self {
        self.multiple = allow;
        self
    }

    /// Open the file picker dialog (async).
    ///
    /// Returns immediately - check `take_files()` for results.
    pub fn open(&mut self) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        self.selected_files.clear();
    }

    /// Blocking file picker (opens native dialog and waits).
    pub fn pick_files(&mut self) -> Vec<PathBuf> {
        let mut dialog = rfd::FileDialog::new().set_title(&self.title);

        // Add filters
        for filter in &self.filters {
            let ext_refs: Vec<&str> = filter.extensions.iter().map(|s| s.as_str()).collect();
            dialog = dialog.add_filter(&filter.name, &ext_refs);
        }

        // Set start directory
        if let Some(ref dir) = self.start_dir {
            dialog = dialog.set_directory(dir);
        }

        // Open dialog
        let result = if self.multiple {
            dialog.pick_files().unwrap_or_default()
        } else {
            dialog.pick_file().map(|f| vec![f]).unwrap_or_default()
        };

        self.selected_files = result.clone();
        self.dialog_open = false;
        result
    }

    /// Pick a directory instead of files.
    pub fn pick_directory(&mut self) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new().set_title(&self.title);

        if let Some(ref dir) = self.start_dir {
            dialog = dialog.set_directory(dir);
        }

        let result = dialog.pick_folder();
        if let Some(ref path) = result {
            self.selected_files = vec![path.clone()];
        }
        self.dialog_open = false;
        result
    }

    /// Get and clear selected files.
    pub fn take_files(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.selected_files)
    }

    /// Get selected files without clearing.
    pub fn selected_files(&self) -> &[PathBuf] {
        &self.selected_files
    }

    /// Check if dialog is currently open.
    pub fn is_open(&self) -> bool {
        self.dialog_open
    }
}

/// Async file picker using rfd's async API.
pub struct AsyncFilePicker {
    /// Receiver for dialog results
    result_rx: Option<std::sync::mpsc::Receiver<Vec<PathBuf>>>,
}

impl AsyncFilePicker {
    pub fn new() -> Self {
        Self { result_rx: None }
    }

    /// Start an async file pick operation.
    #[cfg(feature = "async")]
    pub fn pick_files_async(
        &mut self,
        title: String,
        filters: Vec<FileFilter>,
        start_dir: Option<PathBuf>,
        multiple: bool,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.result_rx = Some(rx);

        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new().set_title(&title);

            for filter in &filters {
                let ext_refs: Vec<&str> = filter.extensions.iter().map(|s| s.as_str()).collect();
                dialog = dialog.add_filter(&filter.name, &ext_refs);
            }

            if let Some(dir) = start_dir {
                dialog = dialog.set_directory(dir);
            }

            let result = if multiple {
                dialog.pick_files().unwrap_or_default()
            } else {
                dialog.pick_file().map(|f| vec![f]).unwrap_or_default()
            };

            let _ = tx.send(result);
        });
    }

    /// Check if results are ready.
    pub fn try_get_result(&mut self) -> Option<Vec<PathBuf>> {
        if let Some(ref rx) = self.result_rx {
            match rx.try_recv() {
                Ok(files) => {
                    self.result_rx = None;
                    Some(files)
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.result_rx = None;
                    Some(Vec::new())
                }
            }
        } else {
            None
        }
    }

    /// Check if a pick operation is in progress.
    pub fn is_picking(&self) -> bool {
        self.result_rx.is_some()
    }
}

impl Default for AsyncFilePicker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_picker_creation() {
        let picker = FilePickerWidget::new()
            .with_title("Test Picker")
            .with_filter(FileFilter::text_files())
            .multiple(true);

        assert_eq!(picker.title, "Test Picker");
        assert!(picker.multiple);
        assert!(!picker.is_open());
    }

    #[test]
    fn test_file_filter_presets() {
        let filter = FileFilter::code();
        assert!(filter.extensions.contains(&"rs".to_string()));
        assert!(filter.extensions.contains(&"py".to_string()));
    }
}
