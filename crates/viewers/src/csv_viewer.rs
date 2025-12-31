//! CSV/TSV viewer with table display, sorting, and filtering

use anyhow::Result;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// CSV viewer state
pub struct CsvViewer {
    path: Option<PathBuf>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    sort_column: Option<usize>,
    sort_ascending: bool,
    filter_text: String,
    filtered_indices: Vec<usize>,
}

impl Default for CsvViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl CsvViewer {
    pub fn new() -> Self {
        Self {
            path: None,
            headers: Vec::new(),
            rows: Vec::new(),
            sort_column: None,
            sort_ascending: true,
            filter_text: String::new(),
            filtered_indices: Vec::new(),
        }
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        // Detect delimiter from extension
        let delimiter = if path.extension().map(|e| e == "tsv").unwrap_or(false) {
            b'\t'
        } else {
            b','
        };

        let mut csv_reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(reader);

        // Read headers
        self.headers = csv_reader
            .headers()?
            .iter()
            .map(|s| s.to_string())
            .collect();

        // Read rows
        self.rows.clear();
        for result in csv_reader.records() {
            let record = result?;
            let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            self.rows.push(row);
        }

        self.path = Some(path.to_path_buf());
        self.sort_column = None;
        self.filter_text.clear();
        self.update_filtered_indices();

        Ok(())
    }

    pub fn load_from_string(&mut self, content: &str, delimiter: u8) -> Result<()> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(content.as_bytes());

        self.headers = csv_reader
            .headers()?
            .iter()
            .map(|s| s.to_string())
            .collect();

        self.rows.clear();
        for result in csv_reader.records() {
            let record = result?;
            let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            self.rows.push(row);
        }

        self.path = None;
        self.update_filtered_indices();
        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_loaded(&self) -> bool {
        !self.headers.is_empty()
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    fn update_filtered_indices(&mut self) {
        let filter_lower = self.filter_text.to_lowercase();

        self.filtered_indices = if filter_lower.is_empty() {
            (0..self.rows.len()).collect()
        } else {
            self.rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row.iter()
                        .any(|cell| cell.to_lowercase().contains(&filter_lower))
                })
                .map(|(i, _)| i)
                .collect()
        };

        // Apply sorting
        if let Some(col) = self.sort_column {
            self.filtered_indices.sort_by(|&a, &b| {
                let val_a = self.rows[a].get(col).map(|s| s.as_str()).unwrap_or("");
                let val_b = self.rows[b].get(col).map(|s| s.as_str()).unwrap_or("");

                // Try numeric comparison first
                let cmp = match (val_a.parse::<f64>(), val_b.parse::<f64>()) {
                    (Ok(num_a), Ok(num_b)) => num_a
                        .partial_cmp(&num_b)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    _ => val_a.cmp(val_b),
                };

                if self.sort_ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
    }

    fn sort_by_column(&mut self, col: usize) {
        if self.sort_column == Some(col) {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = Some(col);
            self.sort_ascending = true;
        }
        self.update_filtered_indices();
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Filter:");
            let filter_response = ui.text_edit_singleline(&mut self.filter_text);
            if filter_response.changed() {
                self.update_filtered_indices();
            }

            ui.separator();
            ui.label(format!(
                "{} / {} rows",
                self.filtered_count(),
                self.row_count()
            ));

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

        // Table
        if self.headers.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No data loaded");
            });
            return;
        }

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("csv_table")
                    .num_columns(self.headers.len())
                    .striped(true)
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        // Header row - collect clicks first to avoid borrow issues
                        let mut clicked_col: Option<usize> = None;
                        for (col, header) in self.headers.iter().enumerate() {
                            let is_sorted = self.sort_column == Some(col);
                            let arrow = if is_sorted {
                                if self.sort_ascending {
                                    " ^"
                                } else {
                                    " v"
                                }
                            } else {
                                ""
                            };

                            if ui.button(format!("{}{}", header, arrow)).clicked() {
                                clicked_col = Some(col);
                            }
                        }
                        ui.end_row();

                        // Handle click after iteration
                        if let Some(col) = clicked_col {
                            self.sort_by_column(col);
                        }

                        // Data rows (limited for performance)
                        let max_display = 1000;
                        for &row_idx in self.filtered_indices.iter().take(max_display) {
                            if let Some(row) = self.rows.get(row_idx) {
                                for cell in row.iter() {
                                    // Truncate long cells
                                    let display = if cell.len() > 50 {
                                        format!("{}...", &cell[..47])
                                    } else {
                                        cell.clone()
                                    };
                                    ui.label(display);
                                }
                                ui.end_row();
                            }
                        }

                        if self.filtered_indices.len() > max_display {
                            ui.label(format!(
                                "... and {} more rows",
                                self.filtered_indices.len() - max_display
                            ));
                            ui.end_row();
                        }
                    });
            });
    }
}
