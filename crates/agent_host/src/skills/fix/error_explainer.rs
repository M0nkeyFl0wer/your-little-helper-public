//! Error message explanation skill for Fix mode.
//!
//! Helps users understand cryptic error messages by providing
//! plain-English explanations and suggested fixes.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
    SuggestedAction,
};

/// Error explanation skill.
pub struct ErrorExplainer;

impl ErrorExplainer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze an error message and provide explanation
    fn analyze_error(error_text: &str) -> ErrorAnalysis {
        let error_lower = error_text.to_lowercase();

        // Common error patterns
        let patterns: Vec<ErrorPattern> = vec![
            // File/Permission errors
            ErrorPattern {
                keywords: vec!["permission denied", "access denied", "eacces"],
                category: "Permission Error",
                explanation: "The system or application doesn't have permission to access a file or resource.",
                suggestions: vec![
                    "Run the application as administrator (Windows) or with sudo (Mac/Linux)",
                    "Check file permissions with 'ls -la' (Mac/Linux) or Properties > Security (Windows)",
                    "Make sure the file isn't open in another program",
                ],
            },
            ErrorPattern {
                keywords: vec!["file not found", "no such file", "enoent", "cannot find"],
                category: "File Not Found",
                explanation: "The specified file or directory doesn't exist at the given path.",
                suggestions: vec![
                    "Double-check the file path for typos",
                    "Verify the file exists using File Explorer or terminal",
                    "Check if the path is relative - it might need to be absolute",
                ],
            },
            ErrorPattern {
                keywords: vec!["disk full", "no space left", "enospc"],
                category: "Disk Space Error",
                explanation: "The disk doesn't have enough free space to complete the operation.",
                suggestions: vec![
                    "Delete unnecessary files or move them to external storage",
                    "Empty the Recycle Bin / Trash",
                    "Use disk cleanup tools to free up space",
                ],
            },

            // Network errors
            ErrorPattern {
                keywords: vec!["connection refused", "econnrefused"],
                category: "Connection Refused",
                explanation: "The server or service actively refused the connection. It might not be running.",
                suggestions: vec![
                    "Check if the server/service is running",
                    "Verify the port number is correct",
                    "Check firewall settings",
                ],
            },
            ErrorPattern {
                keywords: vec!["connection timed out", "etimedout", "timeout"],
                category: "Connection Timeout",
                explanation: "The connection took too long to establish. The server might be slow or unreachable.",
                suggestions: vec![
                    "Check your internet connection",
                    "The server might be down - try again later",
                    "Check if a firewall is blocking the connection",
                ],
            },
            ErrorPattern {
                keywords: vec!["dns", "could not resolve", "getaddrinfo", "name resolution"],
                category: "DNS Error",
                explanation: "The domain name couldn't be resolved to an IP address.",
                suggestions: vec![
                    "Check your internet connection",
                    "Try using a different DNS server (like 8.8.8.8)",
                    "The website might be down",
                ],
            },
            ErrorPattern {
                keywords: vec!["ssl", "certificate", "cert", "tls", "handshake"],
                category: "SSL/Certificate Error",
                explanation: "There's a problem with the secure connection certificate.",
                suggestions: vec![
                    "Check if your system date/time is correct",
                    "The website's certificate might have expired",
                    "Try updating your browser or system certificates",
                ],
            },

            // Memory errors
            ErrorPattern {
                keywords: vec!["out of memory", "memory allocation", "enomem", "heap"],
                category: "Memory Error",
                explanation: "The system ran out of available memory (RAM).",
                suggestions: vec![
                    "Close other applications to free up memory",
                    "Restart the application",
                    "The application might have a memory leak - check for updates",
                ],
            },

            // Code/Runtime errors
            ErrorPattern {
                keywords: vec!["null pointer", "nullptr", "nullreferenceexception", "none type"],
                category: "Null Reference Error",
                explanation: "The program tried to use something that doesn't exist (null/None value).",
                suggestions: vec![
                    "If coding: add null checks before using the value",
                    "If using an app: try restarting it or updating to latest version",
                    "Report the bug to the application developer",
                ],
            },
            ErrorPattern {
                keywords: vec!["stack overflow", "recursion", "maximum call stack"],
                category: "Stack Overflow",
                explanation: "The program called itself too many times (infinite recursion).",
                suggestions: vec![
                    "If coding: check for infinite loops or recursive calls",
                    "If using an app: the input might be causing a bug",
                    "Try with simpler/smaller input",
                ],
            },
            ErrorPattern {
                keywords: vec!["syntax error", "unexpected token", "parse error"],
                category: "Syntax Error",
                explanation: "There's a typo or formatting mistake in the code or configuration.",
                suggestions: vec![
                    "Check for missing brackets, quotes, or commas",
                    "Look at the line number mentioned in the error",
                    "Use a code editor with syntax highlighting",
                ],
            },
            ErrorPattern {
                keywords: vec!["module not found", "import error", "cannot find module", "no module named"],
                category: "Missing Module/Package",
                explanation: "A required software library or module isn't installed.",
                suggestions: vec![
                    "Install the missing package (npm install, pip install, etc.)",
                    "Check if you're in the correct project directory",
                    "Verify the package name is spelled correctly",
                ],
            },

            // Database errors
            ErrorPattern {
                keywords: vec!["database", "sql", "query", "constraint", "foreign key"],
                category: "Database Error",
                explanation: "Something went wrong with a database operation.",
                suggestions: vec![
                    "Check if the database server is running",
                    "Verify database connection settings",
                    "The query might violate a database constraint",
                ],
            },

            // Generic
            ErrorPattern {
                keywords: vec!["403", "forbidden"],
                category: "Access Forbidden (403)",
                explanation: "You don't have permission to access this resource.",
                suggestions: vec![
                    "Check if you're logged in",
                    "You might need different credentials",
                    "The resource might require special permissions",
                ],
            },
            ErrorPattern {
                keywords: vec!["404", "not found"],
                category: "Not Found (404)",
                explanation: "The requested resource (page, file, API endpoint) doesn't exist.",
                suggestions: vec![
                    "Check the URL for typos",
                    "The page might have been moved or deleted",
                    "Try searching the website for the content",
                ],
            },
            ErrorPattern {
                keywords: vec!["500", "internal server error"],
                category: "Server Error (500)",
                explanation: "Something went wrong on the server side.",
                suggestions: vec![
                    "This is usually not your fault - the server has a bug",
                    "Try again later",
                    "Contact the website administrator if it persists",
                ],
            },
        ];

        // Find matching patterns
        for pattern in patterns {
            if pattern.keywords.iter().any(|kw| error_lower.contains(kw)) {
                return ErrorAnalysis {
                    category: pattern.category.to_string(),
                    explanation: pattern.explanation.to_string(),
                    suggestions: pattern.suggestions.iter().map(|s| s.to_string()).collect(),
                    original_error: error_text.to_string(),
                    matched: true,
                };
            }
        }

        // Generic fallback
        ErrorAnalysis {
            category: "Unknown Error".to_string(),
            explanation: "I don't recognize this specific error pattern.".to_string(),
            suggestions: vec![
                "Try searching for this error message online".to_string(),
                "Check application logs for more details".to_string(),
                "Restart the application or computer".to_string(),
            ],
            original_error: error_text.to_string(),
            matched: false,
        }
    }

    /// Format the explanation
    fn format_explanation(analysis: &ErrorAnalysis) -> String {
        let mut output = String::new();

        output.push_str(&format!("## {}\n\n", analysis.category));

        output.push_str("### What This Means\n");
        output.push_str(&analysis.explanation);
        output.push_str("\n\n");

        output.push_str("### Suggested Fixes\n");
        for (i, suggestion) in analysis.suggestions.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, suggestion));
        }

        if !analysis.matched {
            output.push_str("\n---\n");
            output.push_str("*I couldn't identify this specific error. Try searching for the exact error message online.*\n");
        }

        output
    }
}

impl Default for ErrorExplainer {
    fn default() -> Self {
        Self::new()
    }
}

struct ErrorPattern {
    keywords: Vec<&'static str>,
    category: &'static str,
    explanation: &'static str,
    suggestions: Vec<&'static str>,
}

struct ErrorAnalysis {
    category: String,
    explanation: String,
    suggestions: Vec<String>,
    original_error: String,
    matched: bool,
}

#[async_trait]
impl Skill for ErrorExplainer {
    fn id(&self) -> &'static str {
        "error_explainer"
    }

    fn name(&self) -> &'static str {
        "Error Explainer"
    }

    fn description(&self) -> &'static str {
        "Explain error messages in plain English with suggested fixes"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        if input.query.trim().is_empty() {
            return Ok(SkillOutput::text(
                "Please paste the error message you'd like me to explain.\n\n\
                 Example: \"What does 'Permission denied' mean?\"",
            ));
        }

        let analysis = Self::analyze_error(&input.query);
        let explanation = Self::format_explanation(&analysis);

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(explanation),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "category": analysis.category,
                "matched": analysis.matched,
                "suggestions_count": analysis.suggestions.len(),
            })),
            citations: Vec::new(),
            suggested_actions: if !analysis.matched {
                vec![SuggestedAction {
                    label: "Search online".to_string(),
                    skill_id: "web_search".to_string(),
                    params: [(
                        "query".to_string(),
                        serde_json::json!(format!("{} fix", analysis.original_error)),
                    )]
                    .into_iter()
                    .collect(),
                }]
            } else {
                Vec::new()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_denied() {
        let analysis =
            ErrorExplainer::analyze_error("Error: Permission denied when opening file.txt");
        assert!(analysis.matched);
        assert_eq!(analysis.category, "Permission Error");
    }

    #[test]
    fn test_file_not_found() {
        let analysis = ErrorExplainer::analyze_error("ENOENT: no such file or directory");
        assert!(analysis.matched);
        assert_eq!(analysis.category, "File Not Found");
    }

    #[test]
    fn test_connection_refused() {
        let analysis = ErrorExplainer::analyze_error("connect ECONNREFUSED 127.0.0.1:3000");
        assert!(analysis.matched);
        assert_eq!(analysis.category, "Connection Refused");
    }

    #[test]
    fn test_unknown_error() {
        let analysis = ErrorExplainer::analyze_error("Some random gibberish xyz123");
        assert!(!analysis.matched);
        assert_eq!(analysis.category, "Unknown Error");
    }
}
