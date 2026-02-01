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
pub fn run_ai_generation(
    messages: Vec<ApiChatMessage>,
    settings: shared::settings::ModelProvider,
    allow_terminal: bool,
    allow_web: bool,
    allowed_dirs: Vec<String>,
    tx: Sender<AiResult>,
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
    let cmd_re = regex::Regex::new(r"(?s)<command>(.*?)</command>").unwrap();

    let result = rt.block_on(Abortable::new(async {
        let mut msgs = messages;
        let mut file_to_preview: Option<PathBuf> = None;
        let mut all_executed_commands: Vec<(String, String, bool)> = Vec::new();
        let mut pending_commands: Vec<String> = Vec::new();

        // Loop for multi-turn interactions (max 5 iterations)
        for _iteration in 0..5 {
            // Get AI response
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

        Ok((
            "I've done several steps of research. Let me know if you need more details!"
                .to_string(),
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
