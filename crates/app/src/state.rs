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

/// Core tools that are always available regardless of mode.
/// These are handled with dedicated logic (not via SkillRegistry).
const CORE_TOOL_IDS: &[&str] = &["web_search", "bash_execute", "file_preview", "file_search"];

/// Build Anthropic tool definitions: core tools + mode-specific skills from registry.
fn build_tool_definitions(
    registry: &agent_host::skills::SkillRegistry,
    mode: &str,
) -> Vec<providers::anthropic::AnthropicTool> {
    // Start with the 4 core tools
    let mut tools = providers::anthropic::AnthropicClient::build_tool_definitions();

    // Map mode string to Mode enum
    let skill_mode = match mode {
        "find" => Some(shared::skill::Mode::Find),
        "fix" => Some(shared::skill::Mode::Fix),
        "research" => Some(shared::skill::Mode::Research),
        "data" => Some(shared::skill::Mode::Data),
        "content" => Some(shared::skill::Mode::Content),
        "build" => Some(shared::skill::Mode::Build),
        _ => None,
    };

    if let Some(mode_enum) = skill_mode {
        for skill in registry.for_mode(mode_enum) {
            let id = skill.id();
            // Skip skills that overlap with core tools
            if CORE_TOOL_IDS.contains(&id)
                || id == "fuzzy_file_search"  // covered by file_search
                || id == "web_search"         // already a core tool
            {
                continue;
            }

            tools.push(providers::anthropic::AnthropicTool {
                name: id.to_string(),
                description: skill.description().to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The request or query for this skill"
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional file or directory path relevant to this request"
                        }
                    },
                    "required": ["query"]
                }),
            });
        }
    }

    tools
}

/// Execute a registered skill via the SkillRegistry.
async fn execute_skill(
    registry: &agent_host::skills::SkillRegistry,
    skill_id: &str,
    input: &serde_json::Value,
    mode: &str,
    status_tx: &Sender<String>,
) -> String {
    let query = input
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let skill_name = registry
        .get(skill_id)
        .map(|s| s.name())
        .unwrap_or(skill_id);
    let _ = status_tx.send(format!("Running: {}", skill_name));

    // Build SkillInput from the tool call
    let mut skill_input = shared::skill::SkillInput::from_query(&query);
    // Pass any extra params from the tool input
    if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
        skill_input = skill_input.with_param("path", serde_json::json!(path));
    }

    let mode_enum = match mode {
        "find" => shared::skill::Mode::Find,
        "fix" => shared::skill::Mode::Fix,
        "research" => shared::skill::Mode::Research,
        "data" => shared::skill::Mode::Data,
        "content" => shared::skill::Mode::Content,
        "build" => shared::skill::Mode::Build,
        _ => shared::skill::Mode::Find,
    };

    let data_dir = dirs::config_dir()
        .map(|p| p.join("little_helper"))
        .unwrap_or_else(|| PathBuf::from("./data"));
    let ctx = shared::skill::SkillContext::new(mode_enum, data_dir);

    match registry.invoke(skill_id, skill_input, &ctx).await {
        Ok(execution) => {
            if let Some(output) = &execution.output {
                if let Some(text) = &output.text {
                    format!("[{} result]\n{}", skill_name, text)
                } else if let Some(data) = &output.data {
                    format!(
                        "[{} result]\n{}",
                        skill_name,
                        serde_json::to_string_pretty(data).unwrap_or_default()
                    )
                } else {
                    format!("[{} completed successfully]", skill_name)
                }
            } else if let Some(err) = &execution.error {
                format!("[{} error]: {}", skill_name, err)
            } else {
                format!("[{} completed]", skill_name)
            }
        }
        Err(e) => format!("[{} failed]: {}", skill_id, e),
    }
}

/// Get or compute a query embedding, using the LRU cache on FileIndexService.
/// Returns None if Ollama is unavailable or the embedding fails (graceful fallback).
async fn get_query_embedding(
    fi: &services::file_index::FileIndexService,
    query: &str,
) -> Option<Vec<f32>> {
    // Check cache first
    if let Some(cached) = fi.get_cached_query_embedding(query) {
        return Some(cached);
    }
    // Compute via Ollama
    let client = services::embedding_client::EmbeddingClient::default_ollama();
    match client.embed_single(query).await {
        Ok(embedding) => {
            fi.cache_query_embedding(query, embedding.clone());
            Some(embedding)
        }
        Err(_) => None,
    }
}

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
    file_index: Option<std::sync::Arc<services::file_index::FileIndexService>>,
    skill_registry: std::sync::Arc<agent_host::skills::SkillRegistry>,
    mode: String,
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

    // Check if the active provider supports native tool_use
    let active_provider = router.active_provider().unwrap_or("").to_string();
    let supports_tools = matches!(active_provider.as_str(), "anthropic" | "openai");

    // Build tool definitions dynamically: core tools + mode-specific skills
    let anthropic_tools = if supports_tools {
        Some(build_tool_definitions(&skill_registry, &mode))
    } else {
        None
    };

    // Pre-compile regexes for XML tag parsing (non-tool-calling providers)
    let search_re = regex::Regex::new(r"(?s)<search>(.*?)</search>").unwrap();
    let cmd_re = regex::Regex::new(
        r"(?s)<(?:command|request|cmd|run)>(.*?)</(?:command|request|cmd|run)>",
    )
    .unwrap();
    let file_search_re =
        regex::Regex::new(r#"(?s)<file_search(?:\s[^>]*)?>([^<]*)</file_search>"#).unwrap();
    let preview_re =
        regex::Regex::new(r#"<preview\s+type="file"\s+path="([^"]+)"[^>]*>"#).unwrap();
    // Generic skill tag: <tool name="skill_id">query</tool>
    let skill_tag_re =
        regex::Regex::new(r#"(?s)<tool\s+name="([^"]+)"[^>]*>(.*?)</tool>"#).unwrap();

    let result = rt.block_on(Abortable::new(
        async {
            let mut msgs = messages;
            let mut file_to_preview: Option<PathBuf> = None;
            let mut all_executed_commands: Vec<(String, String, bool)> = Vec::new();
            let mut pending_commands: Vec<String> = Vec::new();

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
                    .generate_stream(msgs.clone(), chunk_tx, anthropic_tools.clone())
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
                let mut file_searches: Vec<(String, usize)> = Vec::new();
                // Skill invocations from XML tags: (skill_id, query)
                let mut xml_skill_invocations: Vec<(String, String)> = Vec::new();
                let has_tool_uses = !tool_uses.is_empty();

                if has_tool_uses {
                    // Native tool_use path — categorize core tools;
                    // skill tools will be dispatched in the execution phase.
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
                            "file_search" => {
                                if let Some(q) = input.get("query").and_then(|v| v.as_str()) {
                                    let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                                    file_searches.push((q.to_string(), limit));
                                }
                            }
                            _ => {
                                // Non-core tool — will be dispatched via SkillRegistry
                            }
                        }
                    }
                } else {
                    // XML tag parsing path (providers without native tool calling)
                    searches = search_re
                        .captures_iter(&response)
                        .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                        .collect();

                    commands = cmd_re
                        .captures_iter(&response)
                        .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
                        .collect();

                    // <file_search>query</file_search> tags
                    for cap in file_search_re.captures_iter(&response) {
                        if let Some(q) = cap.get(1) {
                            let query = q.as_str().trim();
                            if !query.is_empty() {
                                file_searches.push((query.to_string(), 20));
                            }
                        }
                    }

                    // <preview type="file" path="..."> tags
                    for cap in preview_re.captures_iter(&response) {
                        if let Some(p) = cap.get(1) {
                            let expanded = expand_user_path(p.as_str());
                            if expanded.exists()
                                && is_path_in_allowed_dirs(&expanded, &allowed_dirs)
                            {
                                file_to_preview = Some(expanded);
                            }
                        }
                    }

                    // <tool name="skill_id">query</tool> — generic skill invocation
                    for cap in skill_tag_re.captures_iter(&response) {
                        if let (Some(name), Some(query)) = (cap.get(1), cap.get(2)) {
                            let skill_id = name.as_str().trim().to_string();
                            let query_text = query.as_str().trim().to_string();
                            if !skill_id.is_empty()
                                && !CORE_TOOL_IDS.contains(&skill_id.as_str())
                                && skill_registry.get(&skill_id).is_some()
                            {
                                xml_skill_invocations.push((skill_id, query_text));
                            }
                        }
                    }
                }

                // Check if any non-core skill tools were invoked (native or XML)
                let has_skill_tools = tool_uses.iter().any(|(_, name, _)| {
                    !CORE_TOOL_IDS.contains(&name.as_str())
                }) || !xml_skill_invocations.is_empty();

                // If no actions needed, return the response
                if searches.is_empty()
                    && commands.is_empty()
                    && file_searches.is_empty()
                    && !has_skill_tools
                {
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

                // Add assistant response to conversation.
                // For providers with native tool_use, include structured content blocks.
                if has_tool_uses {
                    let mut parts: Vec<serde_json::Value> = Vec::new();
                    if !response.is_empty() {
                        parts.push(serde_json::json!({
                            "type": "text",
                            "text": response
                        }));
                    }
                    for (id, name, input) in &tool_uses {
                        parts.push(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }));
                    }
                    msgs.push(ApiChatMessage {
                        role: "assistant".to_string(),
                        content: response.clone(),
                        content_parts: Some(parts),
                    });
                } else {
                    msgs.push(ApiChatMessage {
                        role: "assistant".to_string(),
                        content: response.clone(),
                        content_parts: None,
                    });
                }

                // --- Execute tools and collect results ---
                let mut tool_result_parts: Vec<serde_json::Value> = Vec::new();
                let mut plain_results: Vec<String> = Vec::new();

                // Helper: execute a search query
                async fn do_search(
                    query: &str,
                    allow_web: bool,
                ) -> String {
                    if !allow_web {
                        return format!(
                            "[Search blocked: Internet access disabled]\nQuery: {}",
                            query
                        );
                    }
                    match web_search(query).await {
                        Ok(result) => {
                            format!("[Search Results for '{}']\n{}", query, result.output)
                        }
                        Err(e) => {
                            format!("[Search failed for '{}']: {}", query, e)
                        }
                    }
                }

                // Helper: execute a command
                async fn do_command(
                    cmd: &str,
                    allow_terminal: bool,
                    allowed_dirs: &[String],
                    all_executed: &mut Vec<(String, String, bool)>,
                    pending: &mut Vec<String>,
                    status_tx: &Sender<String>,
                ) -> String {
                    if !allow_terminal {
                        all_executed.push((
                            cmd.to_string(),
                            "Terminal access disabled in settings".to_string(),
                            false,
                        ));
                        return format!(
                            "[Command blocked: terminal access disabled]\n$ {}",
                            cmd
                        );
                    }
                    if let Err(reason) =
                        validate_command_against_allowed(cmd, allowed_dirs)
                    {
                        return format!("[Command blocked: {}]\n$ {}", reason, cmd);
                    }
                    let danger = classify_command(cmd);
                    if danger == DangerLevel::Blocked {
                        all_executed.push((
                            cmd.to_string(),
                            "Blocked for safety".to_string(),
                            false,
                        ));
                        return format!("[Command blocked for safety: {}]", cmd);
                    }
                    if danger == DangerLevel::Safe {
                        let _ = status_tx
                            .send(format!("Running: {}", truncate_for_status(cmd)));
                        match agent_host::execute_command(cmd, 60).await {
                            Ok(r) => {
                                all_executed.push((
                                    cmd.to_string(),
                                    r.output.clone(),
                                    r.success,
                                ));
                                format!(
                                    "[Command output]\n$ {}\n{}",
                                    cmd,
                                    if r.output.trim().is_empty() {
                                        "(no output)".to_string()
                                    } else {
                                        r.output
                                    }
                                )
                            }
                            Err(e) => {
                                all_executed
                                    .push((cmd.to_string(), e.to_string(), false));
                                format!("[Command failed]\n$ {}\n{}", cmd, e)
                            }
                        }
                    } else {
                        let _ =
                            status_tx.send("Waiting for your approval".to_string());
                        if !pending.iter().any(|c| c == cmd) {
                            pending.push(cmd.to_string());
                        }
                        format!("[Command '{}' queued for user approval]", cmd)
                    }
                }

                if has_tool_uses {
                    // Anthropic native tool_use: execute each tool and build
                    // tool_result content blocks keyed by tool_use_id.
                    for (id, name, input) in &tool_uses {
                        let result_text = match name.as_str() {
                            "web_search" => {
                                let q = input
                                    .get("query")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let _ =
                                    status_tx.send(format!("Searching: {}", q));
                                do_search(q, allow_web).await
                            }
                            "bash_execute" => {
                                let c = input
                                    .get("command")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                do_command(
                                    c,
                                    allow_terminal,
                                    &allowed_dirs,
                                    &mut all_executed_commands,
                                    &mut pending_commands,
                                    &status_tx,
                                )
                                .await
                            }
                            "file_preview" => {
                                // file_preview was already handled above
                                "File opened in preview panel.".to_string()
                            }
                            "file_search" => {
                                let q = input
                                    .get("query")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let limit = input
                                    .get("limit")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(20) as usize;
                                let _ = status_tx.send(format!("Searching files: {}", q));
                                if let Some(ref fi) = file_index {
                                    let qe = get_query_embedding(fi, q).await;
                                    match fi.semantic_search(q, qe.as_deref(), limit) {
                                        Ok(results) if !results.is_empty() => {
                                            let mut out = format!("[File search results for '{}']\n", q);
                                            for (i, r) in results.iter().enumerate() {
                                                out.push_str(&format!(
                                                    "{}. {} ({:.0}%)\n   {}\n",
                                                    i + 1,
                                                    r.name,
                                                    r.score * 100.0,
                                                    r.path.display()
                                                ));
                                            }
                                            out
                                        }
                                        Ok(_) => format!("[No files found matching '{}']\nThe file index may still be building. Try again shortly.", q),
                                        Err(e) => format!("[File search error]: {}", e),
                                    }
                                } else {
                                    format!("[File search unavailable — index not initialized]\nQuery: {}", q)
                                }
                            }
                            // --- Skill dispatch: any non-core tool goes to the registry ---
                            other => {
                                execute_skill(
                                    &skill_registry,
                                    other,
                                    input,
                                    &mode,
                                    &status_tx,
                                )
                                .await
                            }
                        };
                        tool_result_parts.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": id,
                            "content": result_text
                        }));
                    }
                } else {
                    // XML tag path: execute searches and commands as plain text
                    for query in &searches {
                        let _ = status_tx.send(format!("Searching: {}", query));
                        plain_results.push(do_search(query, allow_web).await);
                    }
                    for cmd in &commands {
                        plain_results.push(
                            do_command(
                                cmd,
                                allow_terminal,
                                &allowed_dirs,
                                &mut all_executed_commands,
                                &mut pending_commands,
                                &status_tx,
                            )
                            .await,
                        );
                    }
                    // Execute file search queries from XML tags
                    for (query, limit) in &file_searches {
                        let _ = status_tx.send(format!("Searching files: {}", query));
                        if let Some(ref fi) = file_index {
                            let qe = get_query_embedding(fi, query).await;
                            match fi.semantic_search(query, qe.as_deref(), *limit) {
                                Ok(results) if !results.is_empty() => {
                                    let mut out =
                                        format!("[File search results for '{}']\n", query);
                                    for (i, r) in results.iter().enumerate() {
                                        out.push_str(&format!(
                                            "{}. {} ({:.0}%)\n   {}\n",
                                            i + 1,
                                            r.name,
                                            r.score * 100.0,
                                            r.path.display()
                                        ));
                                    }
                                    plain_results.push(out);
                                }
                                Ok(_) => {
                                    plain_results.push(format!(
                                        "[No files found matching '{}']\nThe file index may still be building.",
                                        query
                                    ));
                                }
                                Err(e) => {
                                    plain_results
                                        .push(format!("[File search error]: {}", e));
                                }
                            }
                        }
                    }
                    // Execute XML skill invocations
                    for (skill_id, query_text) in &xml_skill_invocations {
                        let _ = status_tx.send(format!("Running {}", skill_id));
                        let input = serde_json::json!({"query": query_text});
                        let result = execute_skill(
                            &skill_registry,
                            skill_id,
                            &input,
                            &mode,
                            &status_tx,
                        )
                        .await;
                        plain_results.push(result);
                    }
                }

                // Add results back to conversation
                if !tool_result_parts.is_empty() {
                    // Structured tool_result content blocks
                    let content_text = tool_result_parts
                        .iter()
                        .filter_map(|p| p.get("content").and_then(|c| c.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    // Show tool activity in the streaming partial so the user sees what happened
                    let mut activity = String::from("\n\n---\n");
                    for (_id, name, input) in &tool_uses {
                        let summary = match name.as_str() {
                            "web_search" => {
                                let q = input.get("query").and_then(|v| v.as_str()).unwrap_or("?");
                                format!("**Searched web:** {}", q)
                            }
                            "file_search" => {
                                let q = input.get("query").and_then(|v| v.as_str()).unwrap_or("?");
                                format!("**Searched files:** {}", q)
                            }
                            "bash_execute" => {
                                let c = input.get("command").and_then(|v| v.as_str()).unwrap_or("?");
                                format!("**Ran command:** `{}`", c)
                            }
                            "file_preview" => {
                                let p = input.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                                format!("**Opened file:** {}", p)
                            }
                            other => {
                                let q = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
                                format!("**{}:** {}", other, q)
                            }
                        };
                        activity.push_str(&format!("{}\n", summary));
                    }
                    let _ = stream_tx.send(StreamChunk::Text(activity));

                    msgs.push(ApiChatMessage {
                        role: "user".to_string(),
                        content: content_text,
                        content_parts: Some(tool_result_parts),
                    });
                } else if !plain_results.is_empty() {
                    // Show XML tag tool activity
                    let mut activity = String::from("\n\n---\n");
                    for query in &searches {
                        activity.push_str(&format!("**Searched web:** {}\n", query));
                    }
                    for cmd in &commands {
                        activity.push_str(&format!("**Ran command:** `{}`\n", cmd));
                    }
                    for (query, _) in &file_searches {
                        activity.push_str(&format!("**Searched files:** {}\n", query));
                    }
                    for (skill_id, query) in &xml_skill_invocations {
                        activity.push_str(&format!("**{}:** {}\n", skill_id, query));
                    }
                    let _ = stream_tx.send(StreamChunk::Text(activity));

                    msgs.push(ApiChatMessage {
                        role: "user".to_string(),
                        content: plain_results.join("\n\n"),
                        content_parts: None,
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
                content_parts: None,
            });

            // Final summary call — also streamed, no tools
            let (chunk_tx, mut chunk_rx) =
                tokio::sync::mpsc::unbounded_channel::<StreamChunk>();
            let _ = stream_tx.send(StreamChunk::Done {
                stop_reason: Some("iteration_reset".to_string()),
            });
            let stream_result = router
                .generate_stream(msgs, chunk_tx, None)
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
