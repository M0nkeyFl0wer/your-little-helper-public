//! State management for the Little Helper app
//!
//! This module contains AppState implementations and methods for managing
//! application state, chat history, and async operations.

use crate::types::*;
use crate::utils::*;
use shared::agent_api::ChatMessage as ApiChatMessage;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use futures::future::{AbortRegistration, Abortable};
/// Run AI generation in background thread (non-blocking)
///
/// `status_tx` sends live status strings back to the UI (e.g. "Searching the web…").
/// The UI polls this channel each frame to update the thinking indicator.
pub fn run_ai_generation(
    messages: Vec<ApiChatMessage>,
    settings: shared::settings::ModelProvider,
    allow_terminal: bool,
    allow_web: bool,
    allowed_dirs: Vec<String>,
    tx: Sender<AiResult>,
    status_tx: Sender<String>,
    abort_reg: AbortRegistration,
) {
    use agent_host::{classify_command, web_search, DangerLevel};
    use providers::router::ProviderRouter;

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(AiResult {
                response: String::new(),
                preview_file: None,
                error: Some(format!("Failed to start async runtime: {}", e)),
                executed_commands: Vec::new(),
                pending_commands: Vec::new(),
            });
            return;
        }
    };

    let router = ProviderRouter::new(settings);

    // Pre-compile regexes
    let search_re = regex::Regex::new(r"(?s)<search>(.*?)</search>").unwrap();
    let cmd_re = regex::Regex::new(r"(?s)<(?:command|request|cmd|run)>(.*?)</(?:command|request|cmd|run)>").unwrap();

    let result = rt.block_on(Abortable::new(async {
        let mut msgs = messages;
        let mut file_to_preview: Option<PathBuf> = None;
        let mut all_executed_commands: Vec<(String, String, bool)> = Vec::new();
        let mut pending_commands: Vec<String> = Vec::new();

        // Loop for multi-turn interactions (max 5 iterations)
        for iteration in 0..5 {
            // Get AI response
            let stage = if iteration == 0 { "Thinking" } else { "Thinking again with new info" };
            let _ = status_tx.send(stage.to_string());
            let response = router.generate(msgs.clone()).await?;

            // Check for preview tags
            for tag in shared::preview_types::parse_preview_tags(&response) {
                if tag.content_type == "file" {
                    if let Some(path_str) = tag.path {
                        let expanded = expand_user_path(&path_str);
                        if expanded.exists() && is_path_in_allowed_dirs(&expanded, &allowed_dirs) {
                            file_to_preview = Some(expanded);
                        }
                    }
                }
            }

            // Check for search and command tags
            let searches: Vec<String> = search_re
                .captures_iter(&response)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();

            let commands: Vec<String> = cmd_re
                .captures_iter(&response)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();

            // If no actions needed, return the response
            if searches.is_empty() && commands.is_empty() {
                return Ok::<
                    (
                        String,
                        Option<PathBuf>,
                        Vec<(String, String, bool)>,
                        Vec<String>,
                    ),
                    anyhow::Error,
                >((
                    response,
                    file_to_preview,
                    all_executed_commands,
                    pending_commands,
                ));
            }

            // Add assistant response to conversation
            msgs.push(ApiChatMessage {
                role: "assistant".to_string(),
                content: response.clone(),
            });

            let mut results = Vec::new();

            // Execute searches
            for query in &searches {
                let _ = status_tx.send(format!("Searching: {}", query));
                if !allow_web {
                    results.push(format!(
                        "[Search blocked: Internet access disabled]\nQuery: {}",
                        query
                    ));
                    continue;
                }
                match web_search(query).await {
                    Ok(result) => {
                        results.push(format!(
                            "[Search Results for '{}']\n{}",
                            query, result.output
                        ));
                    }
                    Err(e) => {
                        results.push(format!("[Search failed for '{}']: {}", query, e));
                    }
                }
            }

            // Run safe commands automatically; queue the rest for approval.
            for cmd in &commands {
                if !allow_terminal {
                    all_executed_commands.push((
                        cmd.clone(),
                        "Terminal access disabled in settings".to_string(),
                        false,
                    ));
                    results.push(format!(
                        "[Command blocked: terminal access disabled]\n$ {}",
                        cmd
                    ));
                    continue;
                }

                // Apply folder and safety policy before showing to user.
                if let Err(reason) = validate_command_against_allowed(cmd, &allowed_dirs) {
                    results.push(format!(
                        "[Command blocked: {}]\n$ {}",
                        reason, cmd
                    ));
                    continue;
                }
                let danger = classify_command(cmd);
                if danger == DangerLevel::Blocked {
                    all_executed_commands.push((
                        cmd.clone(),
                        "Blocked for safety".to_string(),
                        false,
                    ));
                    results.push(format!("[Command blocked for safety: {}]", cmd));
                    continue;
                }

                if danger == DangerLevel::Safe {
                    let _ = status_tx.send(format!("Running: {}", truncate_for_status(cmd)));
                    match agent_host::execute_command(cmd, 60).await {
                        Ok(r) => {
                            all_executed_commands.push((
                                cmd.clone(),
                                r.output.clone(),
                                r.success,
                            ));
                            results.push(format!(
                                "[Command output]\n$ {}\n{}",
                                cmd,
                                if r.output.trim().is_empty() {
                                    "(no output)".to_string()
                                } else {
                                    r.output
                                }
                            ));
                        }
                        Err(e) => {
                            all_executed_commands
                                .push((cmd.clone(), e.to_string(), false));
                            results.push(format!("[Command failed]\n$ {}\n{}", cmd, e));
                        }
                    }
                } else {
                    let _ = status_tx.send("Waiting for your approval".to_string());
                    results.push(format!("[Command '{}' queued for user approval]", cmd));
                    if !pending_commands.iter().any(|c| c == cmd) {
                        pending_commands.push(cmd.clone());
                    }
                }
            }

            // Add results back to conversation
            if !results.is_empty() {
                msgs.push(ApiChatMessage {
                    role: "user".to_string(),
                    content: results.join("\n\n"),
                });
            }
        }

        // Ran out of iterations — ask the model to summarize what it found
        let _ = status_tx.send("Summarizing results...".to_string());
        msgs.push(ApiChatMessage {
            role: "user".to_string(),
            content: "Summarize what you found so far in plain language. Don't include any command tags.".to_string(),
        });
        let summary = router.generate(msgs).await
            .unwrap_or_else(|_| "I ran several searches but couldn't generate a summary. Check the preview panel for raw results.".to_string());
        Ok((
            summary,
            file_to_preview,
            all_executed_commands,
            pending_commands,
        ))
    }, abort_reg));

    // Send result back to UI
    let ai_result = match result {
        Ok(Ok((response, preview_file, executed_commands, pending_commands))) => AiResult {
            response,
            preview_file,
            error: None,
            executed_commands,
            pending_commands,
        },
        Ok(Err(e)) => AiResult {
            response: String::new(),
            preview_file: None,
            error: Some(e.to_string()),
            executed_commands: Vec::new(),
            pending_commands: Vec::new(),
        },
        Err(_aborted) => AiResult {
            response: String::new(),
            preview_file: None,
            error: Some("Cancelled".to_string()),
            executed_commands: Vec::new(),
            pending_commands: Vec::new(),
        },
    };

    let _ = tx.send(ai_result);
}

/// Truncate a command string for display in the status indicator.
fn truncate_for_status(s: &str) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > 60 {
        format!("{}…", &first_line[..57])
    } else {
        first_line.to_string()
    }
}
