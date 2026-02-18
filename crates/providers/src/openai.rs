use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::agent_api::{ChatMessage, StreamChunk};
use shared::settings::ProviderAuth;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

use crate::anthropic::AnthropicTool;

static SHARED_HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(2)
        .build()
        .expect("failed to build HTTP client")
});

// ── Request types ────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
}

/// OpenAI function-calling tool definition.
#[derive(Debug, Clone, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Clone, Serialize)]
struct OpenAIFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

// ── Non-streaming response types ─────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIResponseMessage,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIToolCall {
    id: String,
    function: OpenAIToolCallFunction,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIToolCallFunction {
    name: String,
    arguments: String,
}

// ── Streaming response types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAIStreamResponse {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIStreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamToolCall {
    /// Present on the first chunk for this tool call.
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<OpenAIStreamToolCallFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamToolCallFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

// ── Conversion ───────────────────────────────────────────────────────

/// Convert an AnthropicTool to OpenAI function-calling format.
fn to_openai_tool(tool: &AnthropicTool) -> OpenAITool {
    OpenAITool {
        tool_type: "function".to_string(),
        function: OpenAIFunction {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.input_schema.clone(),
        },
    }
}

/// Convert ChatMessages to OpenAI message format.
///
/// Handles:
/// - system / user / assistant messages as simple {role, content}
/// - Messages with `content_parts` containing tool_result blocks →
///   individual `{role: "tool", tool_call_id, content}` messages
/// - Assistant messages with tool_use content_parts → assistant message
///   with `tool_calls` array
fn to_openai_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    let mut out = Vec::with_capacity(messages.len());

    for m in messages {
        if let Some(parts) = &m.content_parts {
            // Check if these are tool_result blocks (user role with tool results)
            let has_tool_results = parts
                .iter()
                .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_result"));

            if has_tool_results {
                // Emit one "tool" message per tool_result block
                for part in parts {
                    if part.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        let tool_call_id = part
                            .get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let content = part
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        out.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": tool_call_id,
                            "content": content
                        }));
                    }
                }
                continue;
            }

            // Check if these are tool_use blocks (assistant with tool calls)
            let has_tool_use = parts
                .iter()
                .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_use"));

            if has_tool_use {
                let text = parts
                    .iter()
                    .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("");

                let tool_calls: Vec<serde_json::Value> = parts
                    .iter()
                    .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                    .map(|p| {
                        serde_json::json!({
                            "id": p.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                            "type": "function",
                            "function": {
                                "name": p.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                "arguments": p.get("input")
                                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                                    .unwrap_or_default()
                            }
                        })
                    })
                    .collect();

                let mut msg = serde_json::json!({
                    "role": "assistant",
                    "tool_calls": tool_calls
                });
                if !text.is_empty() {
                    msg["content"] = serde_json::Value::String(text);
                }
                out.push(msg);
                continue;
            }
        }

        // Simple text message
        out.push(serde_json::json!({
            "role": m.role,
            "content": m.content
        }));
    }

    out
}

// ── Client ───────────────────────────────────────────────────────────

pub struct OpenAIClient {
    http: Client,
    auth_token: String,
    model: String,
    base_url: String,
}

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

impl OpenAIClient {
    pub fn new(model: &str) -> Result<Self> {
        let key = env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))?;
        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token: key,
            model: model.to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
        })
    }

    pub fn from_auth(model: &str, auth: &ProviderAuth, base_url: Option<&str>) -> Result<Self> {
        let auth_token = if let Some(api_key) = &auth.api_key {
            api_key.clone()
        } else if let Some(oauth) = &auth.oauth {
            oauth.access_token.clone()
        } else {
            // Try environment variable as fallback
            env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow!("No OpenAI authentication configured"))?
        };

        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token,
            model: model.to_string(),
            base_url: base_url
                .unwrap_or(DEFAULT_BASE_URL)
                .trim_end_matches('/')
                .to_string(),
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let openai_messages = to_openai_messages(&messages);
        let req = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
            stream: None,
            tools: None,
        };
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let detail: String = body.chars().take(800).collect();
            if detail.trim().is_empty() {
                return Err(anyhow!("openai error: {}", status));
            }
            return Err(anyhow!("openai error: {}\n{}", status, detail));
        }
        let body: OpenAIResponse = resp.json().await?;
        let text = body
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();
        Ok(text)
    }

    pub async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: UnboundedSender<StreamChunk>,
    ) -> Result<()> {
        self.generate_stream_inner(&messages, None, &tx).await
    }

    pub async fn generate_stream_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<AnthropicTool>,
        tx: UnboundedSender<StreamChunk>,
    ) -> Result<()> {
        let openai_tools: Vec<OpenAITool> = tools.iter().map(to_openai_tool).collect();
        let tools_opt = if openai_tools.is_empty() {
            None
        } else {
            Some(openai_tools)
        };
        self.generate_stream_inner(&messages, tools_opt, &tx).await
    }

    async fn generate_stream_inner(
        &self,
        messages: &[ChatMessage],
        tools: Option<Vec<OpenAITool>>,
        tx: &UnboundedSender<StreamChunk>,
    ) -> Result<()> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let openai_messages = to_openai_messages(messages);
        let req = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
            stream: Some(true),
            tools,
        };
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let detail: String = body.chars().take(800).collect();
            if detail.trim().is_empty() {
                return Err(anyhow!("openai error: {}", status));
            }
            return Err(anyhow!("openai error: {}\n{}", status, detail));
        }

        let mut parser = crate::sse::SseParser::new();
        let mut stream = resp.bytes_stream();

        // Track active tool calls being assembled (indexed by position)
        let mut active_tools: Vec<(String, String, String)> = Vec::new(); // (id, name, arguments_json)

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| anyhow!("stream read error: {}", e))?;
            for event in parser.feed(&bytes) {
                if event.data == "[DONE]" {
                    // Emit any completed tool calls before Done
                    self.flush_tool_calls(&mut active_tools, tx);
                    let _ = tx.send(StreamChunk::Done { stop_reason: None });
                    return Ok(());
                }
                match serde_json::from_str::<OpenAIStreamResponse>(&event.data) {
                    Ok(resp) => {
                        if let Some(choice) = resp.choices.first() {
                            // Handle text content
                            if let Some(content) = &choice.delta.content {
                                if !content.is_empty() {
                                    let _ = tx.send(StreamChunk::Text(content.clone()));
                                }
                            }

                            // Handle tool_calls deltas
                            if let Some(tool_calls) = &choice.delta.tool_calls {
                                for tc in tool_calls {
                                    let idx = tc.index;

                                    // Grow the active_tools vec if needed
                                    while active_tools.len() <= idx {
                                        active_tools.push((String::new(), String::new(), String::new()));
                                    }

                                    // First chunk for this tool call has id + name
                                    if let Some(id) = &tc.id {
                                        active_tools[idx].0 = id.clone();
                                    }
                                    if let Some(func) = &tc.function {
                                        if let Some(name) = &func.name {
                                            active_tools[idx].1 = name.clone();
                                            // Emit ToolUseStart
                                            let _ = tx.send(StreamChunk::ToolUseStart {
                                                id: active_tools[idx].0.clone(),
                                                name: name.clone(),
                                            });
                                        }
                                        if let Some(args) = &func.arguments {
                                            active_tools[idx].2.push_str(args);
                                            let _ = tx.send(StreamChunk::ToolInputDelta(
                                                args.clone(),
                                            ));
                                        }
                                    }
                                }
                            }

                            // Check finish_reason
                            if let Some(reason) = &choice.finish_reason {
                                self.flush_tool_calls(&mut active_tools, tx);
                                let _ = tx.send(StreamChunk::Done {
                                    stop_reason: Some(reason.clone()),
                                });
                                return Ok(());
                            }
                        }
                    }
                    Err(_) => {
                        // Skip unparseable SSE lines (e.g. comments)
                    }
                }
            }
        }

        self.flush_tool_calls(&mut active_tools, tx);
        let _ = tx.send(StreamChunk::Done { stop_reason: None });
        Ok(())
    }

    /// Emit ToolUseComplete for all accumulated tool calls and clear the buffer.
    fn flush_tool_calls(
        &self,
        active_tools: &mut Vec<(String, String, String)>,
        tx: &UnboundedSender<StreamChunk>,
    ) {
        for (id, name, args_json) in active_tools.drain(..) {
            if name.is_empty() {
                continue;
            }
            let input: serde_json::Value = serde_json::from_str(&args_json)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            let _ = tx.send(StreamChunk::ToolUseComplete { id, name, input });
        }
    }
}
