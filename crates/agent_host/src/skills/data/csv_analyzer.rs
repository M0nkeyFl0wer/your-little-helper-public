//! CSV analysis skill for Data mode.
//!
//! Analyzes CSV files to provide statistics, summaries, and insights.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// CSV analysis skill.
pub struct CsvAnalyzer;

impl CsvAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze a CSV file
    fn analyze_csv(path: &PathBuf) -> Result<CsvAnalysis> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Read header
        let header_line = lines
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty CSV file"))??;

        let headers: Vec<String> = parse_csv_line(&header_line);
        let column_count = headers.len();

        // Analyze rows
        let mut row_count = 0;
        let mut column_stats: Vec<ColumnStats> = headers
            .iter()
            .map(|name| ColumnStats {
                name: name.clone(),
                non_empty: 0,
                numeric_count: 0,
                unique_values: HashMap::new(),
                min_numeric: f64::MAX,
                max_numeric: f64::MIN,
                sum_numeric: 0.0,
            })
            .collect();

        for line_result in lines.take(10000) {
            // Limit for performance
            let line = line_result?;
            let values = parse_csv_line(&line);
            row_count += 1;

            for (i, value) in values.iter().enumerate() {
                if i >= column_stats.len() {
                    continue;
                }

                let stat = &mut column_stats[i];
                let trimmed = value.trim();

                if !trimmed.is_empty() {
                    stat.non_empty += 1;

                    // Track unique values (limit to prevent memory issues)
                    if stat.unique_values.len() < 100 {
                        *stat.unique_values.entry(trimmed.to_string()).or_insert(0) += 1;
                    }

                    // Check if numeric
                    if let Ok(num) = trimmed.parse::<f64>() {
                        stat.numeric_count += 1;
                        stat.sum_numeric += num;
                        if num < stat.min_numeric {
                            stat.min_numeric = num;
                        }
                        if num > stat.max_numeric {
                            stat.max_numeric = num;
                        }
                    }
                }
            }
        }

        Ok(CsvAnalysis {
            path: path.clone(),
            row_count,
            column_count,
            headers,
            column_stats,
        })
    }

    /// Format the analysis for display
    fn format_analysis(analysis: &CsvAnalysis) -> String {
        let mut output = String::new();

        let file_name = analysis
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("data.csv");

        output.push_str(&format!("## CSV Analysis: {}\n\n", file_name));
        output.push_str(&format!("- **Rows**: {}\n", analysis.row_count));
        output.push_str(&format!("- **Columns**: {}\n\n", analysis.column_count));

        // Column details
        output.push_str("### Column Summary\n\n");
        output.push_str("| Column | Type | Non-Empty | Unique | Min | Max | Avg |\n");
        output.push_str("|--------|------|-----------|--------|-----|-----|-----|\n");

        for stat in &analysis.column_stats {
            let data_type = if stat.numeric_count > stat.non_empty / 2 {
                "Numeric"
            } else if stat.unique_values.len() <= 10 {
                "Categorical"
            } else {
                "Text"
            };

            let unique_display = if stat.unique_values.len() >= 100 {
                "100+".to_string()
            } else {
                stat.unique_values.len().to_string()
            };

            let (min, max, avg) = if stat.numeric_count > 0 {
                let avg = stat.sum_numeric / stat.numeric_count as f64;
                (
                    format!("{:.2}", stat.min_numeric),
                    format!("{:.2}", stat.max_numeric),
                    format!("{:.2}", avg),
                )
            } else {
                ("-".to_string(), "-".to_string(), "-".to_string())
            };

            // Truncate long column names
            let name_display = if stat.name.len() > 20 {
                format!("{}...", &stat.name[..17])
            } else {
                stat.name.clone()
            };

            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                name_display, data_type, stat.non_empty, unique_display, min, max, avg
            ));
        }

        // Sample categorical values
        let categoricals: Vec<&ColumnStats> = analysis
            .column_stats
            .iter()
            .filter(|s| s.unique_values.len() > 0 && s.unique_values.len() <= 10)
            .take(3)
            .collect();

        if !categoricals.is_empty() {
            output.push_str("\n### Categorical Values\n\n");
            for stat in categoricals {
                let values: Vec<String> = stat
                    .unique_values
                    .keys()
                    .take(5)
                    .map(|v| format!("\"{}\"", v))
                    .collect();
                output.push_str(&format!("- **{}**: {}\n", stat.name, values.join(", ")));
            }
        }

        output.push_str("\n### Suggestions\n\n");
        output.push_str("- I can help you filter, sort, or summarize this data\n");
        output.push_str("- Ask me about specific columns or patterns\n");
        output.push_str("- I can suggest visualizations based on the data types\n");

        output
    }
}

impl Default for CsvAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a CSV line (simple implementation)
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in line.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                result.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    result.push(current.trim().to_string());
    result
}

struct CsvAnalysis {
    path: PathBuf,
    row_count: usize,
    column_count: usize,
    headers: Vec<String>,
    column_stats: Vec<ColumnStats>,
}

struct ColumnStats {
    name: String,
    non_empty: usize,
    numeric_count: usize,
    unique_values: HashMap<String, usize>,
    min_numeric: f64,
    max_numeric: f64,
    sum_numeric: f64,
}

#[async_trait]
impl Skill for CsvAnalyzer {
    fn id(&self) -> &'static str {
        "csv_analyzer"
    }

    fn name(&self) -> &'static str {
        "CSV Analyzer"
    }

    fn description(&self) -> &'static str {
        "Analyze CSV files to discover structure, statistics, and patterns"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Data]
    }

    async fn execute(&self, input: SkillInput, ctx: &SkillContext) -> Result<SkillOutput> {
        // Get file path from params or query
        let path_str = input
            .params
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                // Look for .csv file in query
                for word in input.query.split_whitespace() {
                    if word.ends_with(".csv") {
                        return Some(word.to_string());
                    }
                }
                None
            });

        let path = match path_str {
            Some(p) => {
                let path = PathBuf::from(&p);
                if path.is_absolute() {
                    path
                } else {
                    ctx.working_dir.join(path)
                }
            }
            None => {
                return Ok(SkillOutput::text(
                    "Please specify a CSV file to analyze.\n\n\
                     Example: \"Analyze sales_data.csv\"\n\n\
                     I'll show you the structure, statistics, and patterns in the data.",
                ));
            }
        };

        if !path.exists() {
            return Ok(SkillOutput::text(format!(
                "File not found: {}\n\nPlease check the path and try again.",
                path.display()
            )));
        }

        let analysis = Self::analyze_csv(&path)?;
        let formatted = Self::format_analysis(&analysis);

        Ok(SkillOutput {
            result_type: ResultType::Data,
            text: Some(formatted),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "file": path.to_string_lossy(),
                "row_count": analysis.row_count,
                "column_count": analysis.column_count,
                "columns": analysis.headers,
            })),
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_line() {
        let line = "name,age,\"city, state\"";
        let result = parse_csv_line(line);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "name");
        assert_eq!(result[2], "city, state");
    }
}
