//! State management for the Little Helper app
//!
//! This module contains AppState implementations and methods for managing
//! application state, chat history, and async operations.

use crate::types::*;
use crate::utils::*;
use shared::agent_api::{ChatMessage as ApiChatMessage, StreamChunk};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use futures::future::{AbortRegistration, Abortable};

/// Run AI generation in background thread with streaming.
///
/// `status_tx` sends live status strings back to the UI (e.g. "Searching the web…").
/// `stream_tx` sends incremental StreamChunk::Text to the UI for live rendering.
/// The UI polls these channels each frame.
pub fn run_ai_generation(
    messages: Vec<ApiChatMessage>,
    settings: shared::settings::ModelProvider,
    allow_terminal: bool,
    allow_web: bool,
    allowed_dirs: Vec<String>,
    tx: Sender<AiResult>,
    status_tx: Sender<String>,
    stream_tx: Sender<StreamChunk>,
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

    // Check if the active provider is Anthropic (for native tool_use)
    let is_anthropic = router.active_provider() == Some("anthropic");

    // Pre-compile regexes for XML tag parsing (non-Anthropic providers)
    let search_re = regex::Regex::new(r"(?s)<search>(.*?)</search>").unwrap();
    let cmd_re = regex::Regex::new(
        r"(?s)<(?:command|request|cmd|run)>(.*?)</(?:command|request|cmd|run)>",
    )
    .unwrap();

    let result = rt.block_on(Abortable::new(
        async {
            let mut msgs = messages;
            let mut file_to_preview: Option<PathBuf> = None;
            let mut all_executed_commands: Vec<(String, String, bool)> = Vec::new();
            let mut pending_commands: Vec<String> = Vec::new();

            // Decide whether to enable native tool_use
            let enable_tools = is_anthropic;

            // Loop for multi-turn interactions (max 5 iterations)
            for iteration in 0..5 {
                let stage = if iteration == 0 {
                    "Thinking"
                } else {
                    "Thinking again with new info"
                };
                let _ = status_tx.send(stage.to_string());

                // Reset streaming partial for this iteration (signal UI to clear)
                if iteration > 0 {
                    let _ = stream_tx.send(StreamChunk::Done {
                        stop_reason: Some("iteration_reset".to_string()),
                    });
                }

                // --- Stream the response ---
                let (chunk_tx, mut chunk_rx) =
                    tokio::sync::mpsc::unbounded_channel::<StreamChunk>();

                let stream_result = router
                    .generate_stream(msgs.clone(), chunk_tx, enable_tools)
                    .await;

                if let Err(e) = stream_result {
                    return Err(e);
                }

                // Collect the full response from stream chunks
                let mut accumulated_text = String::new();
                let mut tool_uses: Vec<(String, String, serde_json::Value)> = Vec::new(); // (id, name, input)

                loop {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(30),
                        chunk_rx.recv(),
                    )
                    .await
                    {
                        Ok(Some(chunk)) => match chunk {
                            StreamChunk::Text(ref t) => {
                                accumulated_text.push_str(t);
                                // Forward to UI for live rendering
                                let _ = stream_tx.send(StreamChunk::Text(t.clone()));
                            }
                            StreamChunk::ToolUseComplete { id, name, input } => {
                                tool_uses.push((id, name, input));
                            }
                            StreamChunk::Done { .. } => {
                                break;
                            }
                            StreamChunk::Error(e) => {
                                return Err(anyhow::anyhow!("{}", e));
                            }
                            // ToolUseStart and ToolInputDelta are intermediate; skip
                            _ => {}
                        },
                        Ok(None) => {
                            // Channel closed
                            break;
                        }
                        Err(_timeout) => {
                            return Err(anyhow::anyhow!(
                                "AI response timed out (no data for 30 seconds)"
                            ));
                        }
                    }
                }

                let response = accumulated_text;

                // Check for preview tags in text
                for tag in shared::preview_types::parse_preview_tags(&response) {
                    if tag.content_type == "file" {
                        if let Some(path_str) = tag.path {
                            let expanded = expand_user_path(&path_str);
                            if expanded.exists()
                                && is_path_in_allowed_dirs(&expanded, &allowed_dirs)
                            {
                                file_to_preview = Some(expanded);
                            }
                        }
                    }
                }

                // --- Determine actions: native tool_use OR XML tag parsing ---
                let mut searches: Vec<String> = Vec::new();
                let mut commands: Vec<String> = Vec::new();

                if !tool_uses.is_empty() {
                    // Native tool_use path (Anthropic)
                    for (_id, name, input) in &tool_uses {
                        match name.as_str() {
                            "web_search" => {
                                if let Some(q) = input.get("query").and_then(|v| v.as_str()) {
                                    searches.push(q.to_string());
                                }
                            }
                            "bash_execute" => {
                                if let Some(c) = input.get("command").and_then(|v| v.as_str()) {
                                    commands.push(c.to_string());
                                }
                            }
                            "file_preview" => {
                                if let Some(p) = input.get("path").and_then(|v| v.as_str()) {
                                    let expanded = expand_user_path(p);
                                    if expanded.exists()
                                        && is_path_in_allowed_dirs(&expanded, &allowed_dirs)
                                    {
                                        file_to_preview = Some(expanded);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    // XML tag parsing path (all other providers)
                    searches = search_re
                        .captures_iter(&response)
                        .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                        .collect();

                    commands = cmd_re
                        .captures_iter(&response)
                        .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                        .collect();
                }

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
                        results.push(format!("[Command blocked: {}]\n$ {}", reason, cmd));
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
                        let _ =
                            status_tx.send(format!("Running: {}", truncate_for_status(cmd)));
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
                        results
                            .push(format!("[Command '{}' queued for user approval]", cmd));
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
                content:
                    "Summarize what you found so far in plain language. Don't include any command tags."
                        .to_string(),
            });

            // Final summary call — also streamed
            let (chunk_tx, mut chunk_rx) =
                tokio::sync::mpsc::unbounded_channel::<StreamChunk>();
            let _ = stream_tx.send(StreamChunk::Done {
                stop_reason: Some("iteration_reset".to_string()),
            });
            let stream_result = router
                .generate_stream(msgs, chunk_tx, false)
                .await;

            let summary = if stream_result.is_ok() {
                let mut text = String::new();
                loop {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(30),
                        chunk_rx.recv(),
                    )
                    .await
                    {
                        Ok(Some(StreamChunk::Text(t))) => {
                            text.push_str(&t);
                            let _ = stream_tx.send(StreamChunk::Text(t));
                        }
                        Ok(Some(StreamChunk::Done { .. })) | Ok(None) => break,
                        Ok(Some(StreamChunk::Error(_))) => break,
                        Ok(Some(_)) => {}
                        Err(_) => break,
                    }
                }
                text
            } else {
                "I ran several searches but couldn't generate a summary. Check the preview panel for raw results.".to_string()
            };

            Ok((
                summary,
                file_to_preview,
                all_executed_commands,
                pending_commands,
            ))
        },
        abort_reg,
    ));

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
