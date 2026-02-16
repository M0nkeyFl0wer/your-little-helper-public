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

static SHARED_HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(2)
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

/// Anthropic message — `content` is `serde_json::Value` so it can be either a plain
/// string (for simple text) or an array of content blocks (for tool_use / tool_result).
#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

/// Native tool definition for Anthropic tool_use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// SSE event payload types for Anthropic streaming.
#[derive(Debug, Deserialize)]
struct AnthropicSseEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    #[allow(dead_code)]
    index: Option<usize>,
    #[serde(default)]
    content_block: Option<AnthropicContentBlock>,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
}

pub struct AnthropicClient {
    http: Client,
    auth_token: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(model: &str) -> Result<Self> {
        let key =
            env::var("ANTHROPIC_API_KEY").map_err(|_| anyhow!("ANTHROPIC_API_KEY not set"))?;
        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token: key,
            model: model.to_string(),
        })
    }

    pub fn from_auth(model: &str, auth: &ProviderAuth) -> Result<Self> {
        let auth_token = if let Some(api_key) = &auth.api_key {
            api_key.clone()
        } else if let Some(oauth) = &auth.oauth {
            oauth.access_token.clone()
        } else {
            // Try environment variable as fallback
            env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow!("No Anthropic authentication configured"))?
        };

        Ok(Self {
            http: SHARED_HTTP.clone(),
            auth_token,
            model: model.to_string(),
        })
    }

    pub async fn generate(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = "https://api.anthropic.com/v1/messages";

        let mut system_prompt = String::new();
        let mut anthropic_messages: Vec<AnthropicMessage> = Vec::new();
        for m in messages {
            if m.role == "system" {
                if !system_prompt.is_empty() {
                    system_prompt.push_str("\n\n");
                }
                system_prompt.push_str(&m.content);
            } else {
                let content = if let Some(parts) = &m.content_parts {
                    serde_json::Value::Array(parts.clone())
                } else {
                    serde_json::Value::String(m.content.clone())
                };
                anthropic_messages.push(AnthropicMessage {
                    role: m.role.clone(),
                    content,
                });
            }
        }

        let system = if system_prompt.trim().is_empty() {
            None
        } else {
            Some(system_prompt)
        };

        let req = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system,
            messages: anthropic_messages,
            stream: None,
            tools: None,
        };

        let resp = self
            .http
            .post(url)
            .header("x-api-key", &self.auth_token)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let detail: String = body.chars().take(800).collect();
            if detail.trim().is_empty() {
                return Err(anyhow!("anthropic error: {}", status));
            }
            return Err(anyhow!("anthropic error: {}\n{}", status, detail));
        }

        let body: AnthropicResponse = resp.json().await?;
        let text = body
            .content
            .get(0)
            .map(|c| c.text.clone())
            .unwrap_or_default();
        Ok(text)
    }

    /// Helper to build the Anthropic request body from messages.
    fn build_request(
        &self,
        messages: &[ChatMessage],
        stream: bool,
        tools: Option<Vec<AnthropicTool>>,
    ) -> AnthropicRequest {
        let mut system_prompt = String::new();
        let mut anthropic_messages: Vec<AnthropicMessage> = Vec::new();
        for m in messages {
            if m.role == "system" {
                if !system_prompt.is_empty() {
                    system_prompt.push_str("\n\n");
                }
                system_prompt.push_str(&m.content);
            } else {
                let content = if let Some(parts) = &m.content_parts {
                    serde_json::Value::Array(parts.clone())
                } else {
                    serde_json::Value::String(m.content.clone())
                };
                anthropic_messages.push(AnthropicMessage {
                    role: m.role.clone(),
                    content,
                });
            }
        }

        let system = if system_prompt.trim().is_empty() {
            None
        } else {
            Some(system_prompt)
        };

        AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system,
            messages: anthropic_messages,
            stream: if stream { Some(true) } else { None },
            tools,
        }
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
        let tools_opt = if tools.is_empty() { None } else { Some(tools) };
        self.generate_stream_inner(&messages, tools_opt, &tx).await
    }

    async fn generate_stream_inner(
        &self,
        messages: &[ChatMessage],
        tools: Option<Vec<AnthropicTool>>,
        tx: &UnboundedSender<StreamChunk>,
    ) -> Result<()> {
        let url = "https://api.anthropic.com/v1/messages";
        let req = self.build_request(messages, true, tools);

        let resp = self
            .http
            .post(url)
            .header("x-api-key", &self.auth_token)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let detail: String = body.chars().take(800).collect();
            if detail.trim().is_empty() {
                return Err(anyhow!("anthropic error: {}", status));
            }
            return Err(anyhow!("anthropic error: {}\n{}", status, detail));
        }

        let mut parser = crate::sse::SseParser::new();
        let mut stream = resp.bytes_stream();

        // Track active tool_use blocks for assembly
        let mut active_tool_id: Option<String> = None;
        let mut active_tool_name: Option<String> = None;
        let mut active_tool_json = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| anyhow!("stream read error: {}", e))?;
            for event in parser.feed(&bytes) {
                let data = &event.data;
                if let Ok(sse) = serde_json::from_str::<AnthropicSseEvent>(data) {
                    match sse.event_type.as_str() {
                        "content_block_start" => {
                            if let Some(block) = &sse.content_block {
                                if block.block_type == "tool_use" {
                                    let id = block.id.clone().unwrap_or_default();
                                    let name = block.name.clone().unwrap_or_default();
                                    let _ = tx.send(StreamChunk::ToolUseStart {
                                        id: id.clone(),
                                        name: name.clone(),
                                    });
                                    active_tool_id = Some(id);
                                    active_tool_name = Some(name);
                                    active_tool_json.clear();
                                }
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = &sse.delta {
                                match delta.delta_type.as_str() {
                                    "text_delta" => {
                                        if let Some(text) = &delta.text {
                                            if !text.is_empty() {
                                                let _ =
                                                    tx.send(StreamChunk::Text(text.clone()));
                                            }
                                        }
                                    }
                                    "input_json_delta" => {
                                        if let Some(json) = &delta.partial_json {
                                            active_tool_json.push_str(json);
                                            let _ = tx.send(StreamChunk::ToolInputDelta(
                                                json.clone(),
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        "content_block_stop" => {
                            // If we were accumulating a tool_use, emit ToolUseComplete
                            if let (Some(id), Some(name)) =
                                (active_tool_id.take(), active_tool_name.take())
                            {
                                let input: serde_json::Value =
                                    serde_json::from_str(&active_tool_json)
                                        .unwrap_or(serde_json::Value::Object(Default::default()));
                                active_tool_json.clear();
                                let _ = tx.send(StreamChunk::ToolUseComplete { id, name, input });
                            }
                        }
                        "message_stop" => {
                            let _ = tx.send(StreamChunk::Done { stop_reason: None });
                            return Ok(());
                        }
                        "message_delta" => {
                            // Could extract stop_reason here if needed
                        }
                        _ => {}
                    }
                }
            }
        }

        let _ = tx.send(StreamChunk::Done { stop_reason: None });
        Ok(())
    }

    /// Build tool definitions for Anthropic native tool_use.
    pub fn build_tool_definitions() -> Vec<AnthropicTool> {
        vec![
            AnthropicTool {
                name: "web_search".to_string(),
                description: "Search the web for information. Returns search results.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        }
                    },
                    "required": ["query"]
                }),
            },
            AnthropicTool {
                name: "bash_execute".to_string(),
                description: "Execute a terminal command and return the output.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
            AnthropicTool {
                name: "file_preview".to_string(),
                description: "Open a file in the preview panel for the user to see.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute path to the file to preview"
                        }
                    },
                    "required": ["path"]
                }),
            },
        ]
    }
}
