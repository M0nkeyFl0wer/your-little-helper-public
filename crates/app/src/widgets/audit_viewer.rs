//! Audit log viewer widget for settings screen.
//!
//! Displays file operations and skill executions to the primary user.
//! Helps users understand what changes Little Helper has made.

use chrono::{DateTime, Local, Utc};
use egui;
use shared::events::{AuditEntry, EventType};

/// Filter options for audit viewer
#[derive(Clone, Debug, Default)]
pub struct AuditViewerFilter {
    /// Show only file operations
    pub file_ops_only: bool,
    /// Show only errors
    pub errors_only: bool,
    /// Filter by skill ID
    pub skill_filter: Option<String>,
}

/// Audit log viewer widget
pub struct AuditViewer {
    /// Entries to display
    entries: Vec<AuditEntry>,
    /// Current filter
    filter: AuditViewerFilter,
    /// Selected entry index for detail view
    selected: Option<usize>,
    /// Search text
    search_text: String,
}

impl AuditViewer {
    /// Create a new audit viewer with the given entries
    pub fn new(entries: Vec<AuditEntry>) -> Self {
        Self {
            entries,
            filter: AuditViewerFilter::default(),
            selected: None,
            search_text: String::new(),
        }
    }

    /// Update entries
    pub fn update_entries(&mut self, entries: Vec<AuditEntry>) {
        self.entries = entries;
        // Reset selection if out of bounds
        if let Some(idx) = self.selected {
            if idx >= self.entries.len() {
                self.selected = None;
            }
        }
    }

    /// Get filtered entries
    fn filtered_entries(&self) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| {
                // Apply type filters
                if self.filter.file_ops_only && !matches!(e.event_type, EventType::FileOp) {
                    return false;
                }
                if self.filter.errors_only && !matches!(e.event_type, EventType::Error) {
                    return false;
                }
                // Apply skill filter
                if let Some(ref skill) = self.filter.skill_filter {
                    if e.skill_id.as_deref() != Some(skill.as_str()) {
                        return false;
                    }
                }
                // Apply search filter
                if !self.search_text.is_empty() {
                    let search = self.search_text.to_lowercase();
                    let matches = e.action.to_lowercase().contains(&search)
                        || e.skill_id.as_ref().map_or(false, |s| s.to_lowercase().contains(&search))
                        || e.file_path.as_ref().map_or(false, |p| p.to_string_lossy().to_lowercase().contains(&search));
                    if !matches {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Render the widget
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let is_dark_mode = ui.visuals().dark_mode;
        let accent_color = if is_dark_mode {
            egui::Color32::from_rgb(100, 180, 255)
        } else {
            egui::Color32::from_rgb(50, 100, 200)
        };

        ui.vertical(|ui| {
            // Header
            ui.horizontal(|ui| {
                ui.heading("Activity Log");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{} entries", self.entries.len()));
                });
            });

            ui.add_space(8.0);

            // Filter bar
            ui.horizontal(|ui| {
                // Search box
                ui.label("Search:");
                ui.add(egui::TextEdit::singleline(&mut self.search_text)
                    .hint_text("Filter entries...")
                    .desired_width(150.0));

                ui.separator();

                // Type filters
                ui.checkbox(&mut self.filter.file_ops_only, "File ops only");
                ui.checkbox(&mut self.filter.errors_only, "Errors only");
            });

            ui.add_space(12.0);

            // Entry list
            let filtered = self.filtered_entries();

            if filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("No matching entries")
                            .weak()
                            .size(16.0)
                    );
                    if !self.search_text.is_empty() || self.filter.file_ops_only || self.filter.errors_only {
                        ui.add_space(8.0);
                        if ui.button("Clear filters").clicked() {
                            self.search_text.clear();
                            self.filter = AuditViewerFilter::default();
                        }
                    }
                });
            } else {
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        for (idx, entry) in filtered.iter().enumerate() {
                            let is_selected = self.selected == Some(idx);
                            self.render_entry(ui, entry, idx, is_selected, is_dark_mode, accent_color);
                        }
                    });
            }
        });
    }

    fn render_entry(
        &mut self,
        ui: &mut egui::Ui,
        entry: &AuditEntry,
        idx: usize,
        is_selected: bool,
        is_dark_mode: bool,
        accent_color: egui::Color32,
    ) {
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
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Event type icon
                    let (icon, icon_color) = match entry.event_type {
                        EventType::SkillExec => ("", egui::Color32::from_rgb(100, 180, 100)),
                        EventType::FileOp => ("", egui::Color32::from_rgb(100, 150, 200)),
                        EventType::PermChange => ("", egui::Color32::from_rgb(200, 180, 100)),
                        EventType::Error => ("", egui::Color32::from_rgb(200, 100, 100)),
                    };
                    ui.colored_label(icon_color, icon);

                    ui.vertical(|ui| {
                        // Action description
                        ui.label(
                            egui::RichText::new(&entry.action)
                                .color(if is_dark_mode {
                                    egui::Color32::from_rgb(220, 220, 220)
                                } else {
                                    egui::Color32::from_rgb(40, 40, 40)
                                })
                        );

                        // Details line
                        ui.horizontal(|ui| {
                            // Skill name
                            if let Some(ref skill) = entry.skill_id {
                                ui.label(
                                    egui::RichText::new(skill)
                                        .small()
                                        .color(accent_color)
                                );
                                ui.label(egui::RichText::new("•").small().weak());
                            }

                            // File path (truncated)
                            if let Some(ref path) = entry.file_path {
                                let path_str = path.to_string_lossy();
                                let display_path = if path_str.len() > 40 {
                                    format!("...{}", &path_str[path_str.len()-37..])
                                } else {
                                    path_str.to_string()
                                };
                                ui.label(egui::RichText::new(display_path).small().weak());
                                ui.label(egui::RichText::new("•").small().weak());
                            }

                            // Timestamp
                            let local_time: DateTime<Local> = entry.timestamp.into();
                            ui.label(
                                egui::RichText::new(local_time.format("%Y-%m-%d %H:%M").to_string())
                                    .small()
                                    .weak()
                            );
                        });
                    });
                });
            });

        if response.response.clicked() {
            self.selected = if is_selected { None } else { Some(idx) };
        }

        ui.add_space(4.0);
    }
}

impl Default for AuditViewer {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::events::AuditEntry;

    #[test]
    fn test_viewer_creation() {
        let entries = vec![
            AuditEntry::skill_execution("test_skill", "Test action", None),
        ];
        let viewer = AuditViewer::new(entries);
        assert_eq!(viewer.entries.len(), 1);
    }

    #[test]
    fn test_filter() {
        let entries = vec![
            AuditEntry::skill_execution("skill_a", "Action A", None),
            AuditEntry::error("Error", None, None),
        ];
        let mut viewer = AuditViewer::new(entries);

        viewer.filter.errors_only = true;
        let filtered = viewer.filtered_entries();
        assert_eq!(filtered.len(), 1);
    }
}
