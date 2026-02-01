//! JSON viewer with tree view and raw mode

use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// JSON viewer state
pub struct JsonViewer {
    path: Option<PathBuf>,
    value: Option<Value>,
    raw_content: String,
    show_raw: bool,
    expanded_paths: HashSet<String>,
}

impl Default for JsonViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonViewer {
    pub fn new() -> Self {
        Self {
            path: None,
            value: None,
            raw_content: String::new(),
            show_raw: false,
            expanded_paths: HashSet::new(),
        }
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)?;
        self.load_string(&content)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn load_string(&mut self, content: &str) -> Result<()> {
        self.raw_content = content.to_string();
        self.value = Some(serde_json::from_str(content)?);
        self.expanded_paths.clear();
        // Auto-expand root
        self.expanded_paths.insert("$".to_string());
        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_loaded(&self) -> bool {
        self.value.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.show_raw, false, "Tree");
            ui.selectable_value(&mut self.show_raw, true, "Raw");

            if ui.button("Expand All").clicked() {
                self.expand_all();
            }
            if ui.button("Collapse All").clicked() {
                self.expanded_paths.clear();
                self.expanded_paths.insert("$".to_string());
            }

            if let Some(path) = &self.path {
                ui.separator();
                ui.label(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        });

        ui.separator();

        if self.show_raw {
            // Raw JSON view
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.raw_content.as_str())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
        } else {
            // Tree view
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(value) = &self.value.clone() {
                        self.render_value(ui, value, "$", 0);
                    }
                });
        }
    }

    fn expand_all(&mut self) {
        if let Some(value) = self.value.clone() {
            self.collect_paths(&value, "$");
        }
    }

    fn collect_paths(&mut self, value: &Value, path: &str) {
        self.expanded_paths.insert(path.to_string());

        match value {
            Value::Object(map) => {
                for (key, val) in map {
                    let child_path = format!("{}.{}", path, key);
                    self.collect_paths(val, &child_path);
                }
            }
            Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let child_path = format!("{}[{}]", path, i);
                    self.collect_paths(val, &child_path);
                }
            }
            _ => {}
        }
    }

    fn render_value(&mut self, ui: &mut egui::Ui, value: &Value, path: &str, indent: usize) {
        let indent_str = "  ".repeat(indent);

        match value {
            Value::Null => {
                ui.label(
                    egui::RichText::new(format!("{}null", indent_str))
                        .monospace()
                        .color(egui::Color32::GRAY),
                );
            }
            Value::Bool(b) => {
                let color = if *b {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                };
                ui.label(
                    egui::RichText::new(format!("{}{}", indent_str, b))
                        .monospace()
                        .color(color),
                );
            }
            Value::Number(n) => {
                ui.label(
                    egui::RichText::new(format!("{}{}", indent_str, n))
                        .monospace()
                        .color(egui::Color32::from_rgb(86, 156, 214)),
                );
            }
            Value::String(s) => {
                // Truncate long strings
                let display = if s.len() > 100 {
                    format!("\"{}...\"", &s[..97])
                } else {
                    format!("\"{}\"", s)
                };
                ui.label(
                    egui::RichText::new(format!("{}{}", indent_str, display))
                        .monospace()
                        .color(egui::Color32::from_rgb(206, 145, 120)),
                );
            }
            Value::Array(arr) => {
                let is_expanded = self.expanded_paths.contains(path);
                let header = if is_expanded { "[-]" } else { "[+]" };

                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new(format!(
                                "{}{} Array[{}]",
                                indent_str,
                                header,
                                arr.len()
                            ))
                            .monospace(),
                        )
                        .clicked()
                    {
                        if is_expanded {
                            self.expanded_paths.remove(path);
                        } else {
                            self.expanded_paths.insert(path.to_string());
                        }
                    }
                });

                if is_expanded {
                    for (i, item) in arr.iter().enumerate() {
                        let child_path = format!("{}[{}]", path, i);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{}  [{}]:", indent_str, i))
                                    .monospace()
                                    .weak(),
                            );
                        });
                        self.render_value(ui, item, &child_path, indent + 2);
                    }
                }
            }
            Value::Object(map) => {
                let is_expanded = self.expanded_paths.contains(path);
                let header = if is_expanded { "[-]" } else { "[+]" };

                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new(format!(
                                "{}{} Object{{{}}}",
                                indent_str,
                                header,
                                map.len()
                            ))
                            .monospace(),
                        )
                        .clicked()
                    {
                        if is_expanded {
                            self.expanded_paths.remove(path);
                        } else {
                            self.expanded_paths.insert(path.to_string());
                        }
                    }
                });

                if is_expanded {
                    for (key, val) in map {
                        let child_path = format!("{}.{}", path, key);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{}  \"{}\":", indent_str, key))
                                    .monospace()
                                    .color(egui::Color32::from_rgb(156, 220, 254)),
                            );
                        });
                        self.render_value(ui, val, &child_path, indent + 2);
                    }
                }
            }
        }
    }
}
