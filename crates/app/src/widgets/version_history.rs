//! Version history display widget for the preview panel.
//!
//! Displays file version history in a user-friendly format with
//! restore options. No git terminology is exposed to users.

use egui;
use shared::version::FileVersion;
use std::path::PathBuf;

/// Action triggered from version history widget
#[derive(Clone, Debug)]
pub enum VersionAction {
    /// Restore to a specific version
    Restore { version_number: u32 },
    /// Preview a specific version
    Preview { version_number: u32 },
    /// Open the current file
    OpenFile,
    /// Show in folder
    RevealInFolder,
}

/// Version history display widget
pub struct VersionHistoryWidget {
    /// File path being displayed
    file_path: PathBuf,
    /// File name for display
    file_name: String,
    /// All versions
    versions: Vec<FileVersion>,
    /// Currently selected version (for highlighting)
    selected_version: Option<u32>,
    /// Pending action from user interaction
    pending_action: Option<VersionAction>,
}

impl VersionHistoryWidget {
    /// Create a new version history widget
    pub fn new(file_path: PathBuf, versions: Vec<FileVersion>) -> Self {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string_lossy().to_string());

        Self {
            file_path,
            file_name,
            versions,
            selected_version: None,
            pending_action: None,
        }
    }

    /// Get and clear any pending action
    pub fn take_action(&mut self) -> Option<VersionAction> {
        self.pending_action.take()
    }

    /// Get the file path
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// Render the widget
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let is_dark_mode = ui.visuals().dark_mode;
        let accent_color = if is_dark_mode {
            egui::Color32::from_rgb(100, 180, 255)
        } else {
            egui::Color32::from_rgb(50, 100, 200)
        };
        let text_color = if is_dark_mode {
            egui::Color32::from_rgb(200, 200, 200)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        };

        ui.vertical(|ui| {
            // Header
            ui.horizontal(|ui| {
                ui.colored_label(accent_color, "");
                ui.label(
                    egui::RichText::new(format!("Version History: {}", self.file_name))
                        .strong()
                        .size(14.0)
                );
            });

            ui.add_space(4.0);

            // Summary line
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} saved version{}",
                        self.versions.len(),
                        if self.versions.len() == 1 { "" } else { "s" }
                    ))
                    .small()
                    .weak()
                );
            });

            ui.add_space(12.0);

            if self.versions.is_empty() {
                self.render_empty_state(ui, text_color);
            } else {
                self.render_version_list(ui, is_dark_mode, accent_color, text_color);
            }

            ui.add_space(16.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("Open File").clicked() {
                    self.pending_action = Some(VersionAction::OpenFile);
                }
                if ui.button("Show in Folder").clicked() {
                    self.pending_action = Some(VersionAction::RevealInFolder);
                }
            });
        });
    }

    fn render_empty_state(&mut self, ui: &mut egui::Ui, text_color: egui::Color32) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("No saved versions")
                    .color(text_color)
                    .size(16.0)
            );
            ui.add_space(8.0);
            ui.label("Versions are automatically saved when files are modified through Little Helper.");
        });
    }

    fn render_version_list(
        &mut self,
        ui: &mut egui::Ui,
        is_dark_mode: bool,
        accent_color: egui::Color32,
        text_color: egui::Color32,
    ) {
        // Display versions in reverse order (newest first)
        for version in self.versions.iter().rev() {
            let is_selected = self.selected_version == Some(version.version_number);
            let is_current = version.is_current;

            let bg_color = if is_selected {
                if is_dark_mode {
                    egui::Color32::from_rgb(50, 60, 80)
                } else {
                    egui::Color32::from_rgb(220, 230, 250)
                }
            } else if is_dark_mode {
                egui::Color32::from_rgb(35, 35, 42)
            } else {
                egui::Color32::from_rgb(250, 250, 252)
            };

            let response = egui::Frame::none()
                .fill(bg_color)
                .rounding(egui::Rounding::same(4.0))
                .inner_margin(egui::Margin::same(10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Version number badge
                        let badge_color = if is_current {
                            egui::Color32::from_rgb(100, 180, 100)
                        } else {
                            egui::Color32::from_rgb(120, 120, 140)
                        };

                        egui::Frame::none()
                            .fill(badge_color)
                            .rounding(egui::Rounding::same(3.0))
                            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!("v{}", version.version_number))
                                        .small()
                                        .color(egui::Color32::WHITE)
                                );
                            });

                        if is_current {
                            ui.label(
                                egui::RichText::new("current")
                                    .small()
                                    .color(egui::Color32::from_rgb(100, 180, 100))
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Restore button (not shown for current version)
                            if !is_current {
                                if ui.small_button("Restore").clicked() {
                                    self.pending_action = Some(VersionAction::Restore {
                                        version_number: version.version_number,
                                    });
                                }
                            }

                            // Size
                            ui.label(
                                egui::RichText::new(version.formatted_size())
                                    .small()
                                    .weak()
                            );
                        });
                    });

                    ui.add_space(4.0);

                    // Description and time
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&version.description)
                                .color(text_color)
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(version.relative_time())
                                .small()
                                .weak()
                        );
                        ui.label(
                            egui::RichText::new(format!(" ({})", version.formatted_time()))
                                .small()
                                .weak()
                        );
                    });
                });

            // Handle click to select
            if response.response.clicked() {
                self.selected_version = Some(version.version_number);
            }

            ui.add_space(6.0);
        }
    }
}

/// Add VersionHistory variant to PreviewContent if needed
/// This should be done in shared/preview_types.rs
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_version(num: u32, is_current: bool) -> FileVersion {
        let mut v = FileVersion::new(
            num,
            Utc::now(),
            format!("Test version {}", num),
            1024 * num as u64,
            format!("ref{}", num),
        );
        if is_current {
            v = v.mark_current();
        }
        v
    }

    #[test]
    fn test_widget_creation() {
        let versions = vec![
            create_test_version(1, false),
            create_test_version(2, true),
        ];
        let widget = VersionHistoryWidget::new(PathBuf::from("/test/file.txt"), versions);

        assert_eq!(widget.file_name, "file.txt");
        assert_eq!(widget.versions.len(), 2);
    }
}
