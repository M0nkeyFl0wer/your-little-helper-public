//! State management for the Little Helper app
//!
//! This module contains AppState implementations and methods for managing
//! application state, chat history, and async operations.

use crate::types::*;
use crate::utils::*;
use agent_host::executor::SessionState;
use shared::agent_api::ChatMessage as ApiChatMessage;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use agent_host::skills::SkillRegistry;
use futures::future::{AbortRegistration, Abortable};
use shared::skill::{Mode, SkillContext, SkillInput};
use std::sync::Arc;

/// Run AI generation in background thread (non-blocking)
///
/// `status_tx` sends live status strings back to the UI (e.g. "Searching the web…").
/// The UI polls this channel each frame to update the thinking indicator.
#[allow(clippy::too_many_arguments)]
pub fn run_ai_generation(
    messages: Vec<ApiChatMessage>,
    settings: shared::settings::ModelProvider,
    allow_terminal: bool,
    allow_web: bool,
    research_depth: ResearchDepth,
    current_mode: Mode,
    allowed_dirs: Vec<String>,
    skill_registry: Arc<SkillRegistry>,
    tx: Sender<AiResult>,
    status_tx: Sender<String>,
    abort_reg: AbortRegistration,
) {
    use agent_host::{classify_command, web_search, DangerLevel};
    use providers::router::ProviderRouter;
    use tracing::info;

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.send(AiResult {
                response: String::new(),
                preview_file: None,
                preview_web_url: None,
                preview_web_title: None,
                preview_web_snippet: None,
                preview_web_query: None,
                preview_web_source: None,
                preview_web_search_time_ms: None,
                preview_web_results: None,
                error: Some(format!("Failed to start async runtime: {}", e)),
                provider: None,
                model: None,
                fallback: None,
                llm_calls: 0,
                llm_duration_ms: 0,
                executed_commands: Vec::new(),
                pending_commands: Vec::new(),
            });
            return;
        }
    };

    // Fast Mode Logic: Use faster models for Find/Fix or simple queries
    let mut model_settings = settings.clone();
    if matches!(current_mode, Mode::Find | Mode::Fix | Mode::Content) {
        model_settings.openai_model = model_settings.openai_fast_model.clone();
        model_settings.anthropic_model = model_settings.anthropic_fast_model.clone();
        model_settings.gemini_model = model_settings.gemini_fast_model.clone();
    }

    let router = ProviderRouter::new(model_settings);
    let mut session_state = SessionState::new();

    // Pre-compile regexes
    let search_re = regex::Regex::new(r"(?s)<search>(.*?)</search>").unwrap();
    let cmd_re =
        regex::Regex::new(r"(?s)<(?:command|request|cmd|run)>(.*?)</(?:command|request|cmd|run)>")
            .unwrap();
    // Also catch markdown code blocks as commands (AI keeps outputting these instead of tags)
    let md_cmd_re = regex::Regex::new(r"(?s)```(?:bash|sh|shell|zsh)?\n(.*?)```").unwrap();
    let skill_re = regex::Regex::new(r"(?s)<skill id=[\x22'](.*?)[\x22']>(.*?)</skill>").unwrap();

    // Heuristic: detect when the model wrote command-looking text but didn't use tags.
    // Used for a one-shot "format repair" retry.
    let commandish_re = regex::Regex::new(
        r"(?mi)^(?:\$\s*)?(?:cd|ls|rg|grep|find|cat|head|tail|git|cargo|npm|bun|pnpm|yarn|python|python3|pip|pip3)\b.*$",
    )
    .unwrap();

    let result = rt.block_on(Abortable::new(async {
        let mut msgs = messages;
        let mut file_to_preview: Option<PathBuf> = None;
        let mut preview_web_url: Option<String> = None;
        let mut preview_web_title: Option<String> = None;
        let mut preview_web_snippet: Option<String> = None;
        let mut preview_web_query: Option<String> = None;
        let mut preview_web_source: Option<String> = None;
        let mut preview_web_search_time_ms: Option<u64> = None;
        let mut preview_web_results: Option<Vec<shared::preview_types::WebSearchResultItem>> =
            None;
        let mut all_executed_commands: Vec<(String, String, bool)> = Vec::new();
        let mut pending_commands: Vec<String> = Vec::new();

        let mut last_provider: Option<String> = None;
        let mut last_model: Option<String> = None;
        let mut last_fallback: Option<String> = None;
        let mut llm_calls: u32 = 0;
        let mut llm_duration_ms: u64 = 0;

        // Loop for multi-turn interactions.
        // Research defaults to a shorter "quick" loop unless the user opts into Deep.
        let max_iterations: u32 = match (current_mode, research_depth) {
            (Mode::Research, ResearchDepth::Quick) => 2,
            _ => 5,
        };

        for iteration in 0..max_iterations {
            // Get AI response
            let stage = if iteration == 0 { "Thinking" } else { "Thinking again with new info" };
            let _ = status_tx.send(stage.to_string());
            let gen = router.generate_with_meta(msgs.clone()).await?;
            let mut response = gen.text;
            llm_calls = llm_calls.saturating_add(1);
            llm_duration_ms = llm_duration_ms.saturating_add(gen.meta.duration_ms);
            last_provider = Some(gen.meta.provider.clone());
            last_model = Some(gen.meta.model.clone());
            if let (Some(from), Some(err)) = (gen.meta.fallback_from.clone(), gen.meta.fallback_error.clone()) {
                last_fallback = Some(format!("{} -> {}", from, err));
            }
            info!(
                provider = %gen.meta.provider,
                model = %gen.meta.model,
                duration_ms = gen.meta.duration_ms,
                iteration,
                "llm.generate"
            );

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
            let mut searches: Vec<String> = search_re
                .captures_iter(&response)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();

            let mut commands: Vec<String> = cmd_re
                .captures_iter(&response)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();

            // Also extract commands from markdown code blocks (fallback for AI not using tags)
            let md_commands: Vec<String> = md_cmd_re
                .captures_iter(&response)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();
            commands.extend(md_commands);

            // Parse skills
            let mut skill_calls = Vec::new();
            for cap in skill_re.captures_iter(&response) {
                let id = cap[1].trim().to_string();
                let params_str = cap[2].trim();
                let params: Option<serde_json::Value> = serde_json::from_str(params_str).ok();
                skill_calls.push((id, params.unwrap_or(serde_json::Value::Null)));
            }

            // If the model is describing commands instead of emitting tags, do a one-shot repair.
            if searches.is_empty()
                && commands.is_empty()
                && skill_calls.is_empty()
                && (allow_terminal || allow_web)
                && !response.contains("<command>")
                && !response.contains("<search>")
                && commandish_re.is_match(&response)
            {
                let _ = status_tx.send("Fixing tool format…".to_string());
                let mut repair_msgs = msgs.clone();
                repair_msgs.push(ApiChatMessage {
                    role: "assistant".to_string(),
                    content: response.clone(),
                });
                repair_msgs.push(ApiChatMessage {
                    role: "user".to_string(),
                    content: "Re-send ONLY tool tags. Use <command>...</command> and/or <search>...</search>. No prose, no code blocks. If you cannot run anything, reply with: BLOCKED: <short reason>.".to_string(),
                });

                let repair = router.generate_with_meta(repair_msgs).await?;
                llm_calls = llm_calls.saturating_add(1);
                llm_duration_ms = llm_duration_ms.saturating_add(repair.meta.duration_ms);
                last_provider = Some(repair.meta.provider.clone());
                last_model = Some(repair.meta.model.clone());
                info!(
                    provider = %repair.meta.provider,
                    model = %repair.meta.model,
                    duration_ms = repair.meta.duration_ms,
                    iteration,
                    "llm.generate.repair"
                );

                response = repair.text;

                // Re-parse after repair.
                searches = search_re
                    .captures_iter(&response)
                    .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                    .collect();

                commands = cmd_re
                    .captures_iter(&response)
                    .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                    .collect();

                let md_commands: Vec<String> = md_cmd_re
                    .captures_iter(&response)
                    .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                    .collect();
                commands.extend(md_commands);

                skill_calls.clear();
                for cap in skill_re.captures_iter(&response) {
                    let id = cap[1].trim().to_string();
                    let params_str = cap[2].trim();
                    let params: Option<serde_json::Value> = serde_json::from_str(params_str).ok();
                    skill_calls.push((id, params.unwrap_or(serde_json::Value::Null)));
                }

                // Re-check preview tags after repair.
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
            }

            // If no actions needed, return the response
            if searches.is_empty() && commands.is_empty() && skill_calls.is_empty() {
                return Ok::<
                    (
                        String,
                        Option<PathBuf>,
                        Vec<(String, String, bool)>,
                        Vec<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<u64>,
                        Option<Vec<shared::preview_types::WebSearchResultItem>>,
                        u32,
                        u64,
                    ),
                    anyhow::Error,
                >((
                    response, // Was display_response
                    file_to_preview,
                    all_executed_commands,
                    pending_commands,
                    last_provider,
                    last_model,
                    last_fallback,
                    preview_web_url,
                    preview_web_title,
                    preview_web_snippet,
                    preview_web_query,
                    preview_web_source,
                    preview_web_search_time_ms,
                    preview_web_results,
                    llm_calls,
                    llm_duration_ms,
                ));
            }



            // Add assistant response to conversation
            msgs.push(ApiChatMessage {
                role: "assistant".to_string(),
                content: response.clone(),
            });

            let mut results = Vec::new();

            // Execute searches
            let search_limit = match (current_mode, research_depth) {
                (Mode::Research, ResearchDepth::Quick) => 1,
                _ => usize::MAX,
            };
            for query in searches.iter().take(search_limit) {
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
                        if preview_web_url.is_none() {
                            if let Some(parsed) = parse_first_web_result(&result.output) {
                                preview_web_url = Some(parsed.url);
                                preview_web_title = parsed.title;
                                preview_web_snippet = parsed.snippet;
                            }
                        }

                        if preview_web_results.is_none() {
                            let items = parse_web_result_items(&result.output);
                            if !items.is_empty() {
                                preview_web_results = Some(items);
                                preview_web_query = Some(query.clone());
                                preview_web_source = Some(infer_web_source(&result.output));
                                preview_web_search_time_ms = Some(result.duration_ms);
                            }
                        }
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
            let mut safe_command_executed = false;
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
                eprintln!("COMMAND CLASSIFY: {} -> {:?}", cmd, danger);
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
                    match agent_host::executor::execute_command(cmd, 60, &mut session_state).await {
                        Ok(r) => {
                            all_executed_commands.push((
                                cmd.clone(),
                                r.output.clone(),
                                r.success,
                            ));
                            results.push(format!(
                                "[Command completed]\n{}",
                                if r.output.trim().is_empty() {
                                    "(no output)".to_string()
                                } else {
                                    r.output
                                }
                            ));
                            safe_command_executed = true;
                        }
                        Err(e) => {
                            all_executed_commands
                                .push((cmd.clone(), e.to_string(), false));
                            results.push(format!("[Command failed]\n$ {}\n{}", cmd, e));
                            safe_command_executed = true;
                        }
                    }
                } else {
                    let _ = status_tx.send("Waiting for your approval".to_string());
                    results.push(format!("[Command '{}' queued for user approval]", cmd));
                    if !pending_commands.iter().any(|c| c == cmd) {
                        pending_commands.push(cmd.clone());
                    }
                }

                if safe_command_executed {
                    break;
                }
            }

            // Execute skills
            for (id, params) in skill_calls {
                let _ = status_tx.send(format!("Running skill: {}", id));
                let mut input = SkillInput::from_query("");
                if let serde_json::Value::Object(map) = params {
                    for (k, v) in map {
                        input = input.with_param(k, v);
                    }
                }

                let ctx = SkillContext::new(current_mode, PathBuf::from("."));

                match skill_registry.invoke(&id, input, &ctx).await {
                    Ok(execution) => {
                        match execution.output {
                            Some(output) => {
                                 results.push(format!("[Skill {} completed]\n{:?}", id, output));
                            }
                            None => {
                                results.push(format!("[Skill {} completed (no output)]", id));
                            }
                        }
                    }
                    Err(e) => {
                        results.push(format!("[Skill {} failed]: {}", id, e));
                    }
                }
            }

            if !searches.is_empty() || !commands.is_empty() || !results.is_empty() {
                // response cleared
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
        let gen = router.generate_with_meta(msgs).await;
        let summary = match gen {
            Ok(gen) => {
                llm_calls = llm_calls.saturating_add(1);
                llm_duration_ms = llm_duration_ms.saturating_add(gen.meta.duration_ms);
                last_provider = Some(gen.meta.provider.clone());
                last_model = Some(gen.meta.model.clone());
                if let (Some(from), Some(err)) = (gen.meta.fallback_from.clone(), gen.meta.fallback_error.clone()) {
                    last_fallback = Some(format!("{} -> {}", from, err));
                }
                info!(
                    provider = %gen.meta.provider,
                    model = %gen.meta.model,
                    duration_ms = gen.meta.duration_ms,
                    iteration = 999u32,
                    "llm.generate.summary"
                );
                gen.text
            }
            Err(_) => {
                "I ran several searches but couldn't generate a summary. Check the preview panel for raw results.".to_string()
            }
        };
        Ok((
            summary,
            file_to_preview,
            all_executed_commands,
            pending_commands,
            last_provider,
            last_model,
            last_fallback,
            preview_web_url,
            preview_web_title,
            preview_web_snippet,
            preview_web_query,
            preview_web_source,
            preview_web_search_time_ms,
            preview_web_results,
            llm_calls,
            llm_duration_ms,
        ))
    }, abort_reg));

    // Send result back to UI
    let ai_result = match result {
        Ok(Ok((
            response,
            preview_file,
            executed_commands,
            pending_commands,
            provider,
            model,
            fallback,
            preview_web_url,
            preview_web_title,
            preview_web_snippet,
            preview_web_query,
            preview_web_source,
            preview_web_search_time_ms,
            preview_web_results,
            llm_calls,
            llm_duration_ms,
        ))) => AiResult {
            response,
            preview_file,
            preview_web_url,
            preview_web_title,
            preview_web_snippet,
            preview_web_query,
            preview_web_source,
            preview_web_search_time_ms,
            preview_web_results,
            error: None,
            provider,
            model,
            fallback,
            llm_calls,
            llm_duration_ms,
            executed_commands,
            pending_commands,
        },
        Ok(Err(e)) => AiResult {
            response: String::new(),
            preview_file: None,
            preview_web_url: None,
            preview_web_title: None,
            preview_web_snippet: None,
            preview_web_query: None,
            preview_web_source: None,
            preview_web_search_time_ms: None,
            preview_web_results: None,
            error: Some(e.to_string()),
            provider: None,
            model: None,
            fallback: None,
            llm_calls: 0,
            llm_duration_ms: 0,
            executed_commands: Vec::new(),
            pending_commands: Vec::new(),
        },
        Err(_aborted) => AiResult {
            response: String::new(),
            preview_file: None,
            preview_web_url: None,
            preview_web_title: None,
            preview_web_snippet: None,
            preview_web_query: None,
            preview_web_source: None,
            preview_web_search_time_ms: None,
            preview_web_results: None,
            error: Some("Cancelled".to_string()),
            provider: None,
            model: None,
            fallback: None,
            llm_calls: 0,
            llm_duration_ms: 0,
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

struct FirstWebResult {
    url: String,
    title: Option<String>,
    snippet: Option<String>,
}

fn parse_first_web_result(output: &str) -> Option<FirstWebResult> {
    // Executor formats results like:
    // 1. Title
    //    Description
    //    URL: https://...
    let mut last_title: Option<String> = None;
    let mut last_desc: Option<String> = None;

    for line in output.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }

        if let Some(rest) = l.strip_prefix("URL:") {
            let url = rest.trim().to_string();
            if url.starts_with("http://") || url.starts_with("https://") {
                return Some(FirstWebResult {
                    url,
                    title: last_title.take(),
                    snippet: last_desc.take(),
                });
            }
        }

        // Brave: "1. <title>"
        if let Some(dot) = l.find('.') {
            if dot <= 2 {
                let after = l[dot + 1..].trim();
                if !after.is_empty() {
                    last_title = Some(after.to_string());
                    continue;
                }
            }
        }

        // Indented description line
        if !l.starts_with('[')
            && !l.starts_with('$')
            && !l.starts_with("Results from")
            && last_desc.is_none()
        {
            last_desc = Some(l.to_string());
        }
    }

    None
}

/// Parse all web search result items from executor output.
/// Expects the format produced by the web search executor:
///   1. Title
///      Description text
///      URL: https://...
fn parse_web_result_items(output: &str) -> Vec<shared::preview_types::WebSearchResultItem> {
    let mut items = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_snippet: Option<String> = None;

    for line in output.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }

        if let Some(rest) = l.strip_prefix("URL:") {
            let url = rest.trim().to_string();
            if url.starts_with("http://") || url.starts_with("https://") {
                items.push(shared::preview_types::WebSearchResultItem {
                    title: current_title.take().unwrap_or_default(),
                    snippet: current_snippet.take().unwrap_or_default(),
                    url,
                });
            }
            continue;
        }

        // Numbered result title: "1. Title text"
        if let Some(dot) = l.find('.') {
            if dot <= 2 && l[..dot].chars().all(|c| c.is_ascii_digit()) {
                let after = l[dot + 1..].trim();
                if !after.is_empty() {
                    current_title = Some(after.to_string());
                    current_snippet = None;
                    continue;
                }
            }
        }

        // Description line (between title and URL)
        if current_title.is_some()
            && current_snippet.is_none()
            && !l.starts_with('[')
            && !l.starts_with('$')
            && !l.starts_with("Results from")
        {
            current_snippet = Some(l.to_string());
        }
    }

    items
}

/// Infer the search source (e.g. "Brave", "Wikipedia") from executor output.
fn infer_web_source(output: &str) -> String {
    let lower = output.to_lowercase();
    if lower.contains("brave") || lower.contains("brave search") {
        "Brave".to_string()
    } else if lower.contains("wikipedia") {
        "Wikipedia".to_string()
    } else if lower.contains("duckduckgo") {
        "DuckDuckGo".to_string()
    } else {
        "Web".to_string()
    }
}
