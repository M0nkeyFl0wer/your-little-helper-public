/// Shared SSE (Server-Sent Events) parser for streaming AI provider responses.
///
/// SSE format: events separated by `\n\n`, each containing optional `event:` and `data:` lines.

/// A single parsed SSE event.
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// The `event:` field, if present (e.g., "message_start", "content_block_delta").
    pub event: Option<String>,
    /// The `data:` field content.
    pub data: String,
}

/// Incremental SSE parser that buffers incomplete lines across chunk boundaries.
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Feed raw bytes from the HTTP response. Returns any complete SSE events found.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let text = String::from_utf8_lossy(chunk);
        self.buffer.push_str(&text);

        let mut events = Vec::new();

        // Split on double newline (SSE event boundary)
        while let Some(boundary) = self.buffer.find("\n\n") {
            let block = self.buffer[..boundary].to_string();
            self.buffer = self.buffer[boundary + 2..].to_string();

            let mut event_type: Option<String> = None;
            let mut data_lines: Vec<String> = Vec::new();

            for line in block.lines() {
                if let Some(val) = line.strip_prefix("event:") {
                    event_type = Some(val.trim().to_string());
                } else if let Some(val) = line.strip_prefix("data:") {
                    data_lines.push(val.trim_start_matches(' ').to_string());
                }
                // Ignore other fields (id:, retry:, comments starting with :)
            }

            if !data_lines.is_empty() {
                events.push(SseEvent {
                    event: event_type,
                    data: data_lines.join("\n"),
                });
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sse() {
        let mut parser = SseParser::new();
        let events = parser.feed(b"data: hello\n\ndata: world\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[1].data, "world");
    }

    #[test]
    fn test_event_types() {
        let mut parser = SseParser::new();
        let events = parser.feed(b"event: message_start\ndata: {\"type\":\"message\"}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("message_start"));
        assert_eq!(events[0].data, "{\"type\":\"message\"}");
    }

    #[test]
    fn test_split_across_chunks() {
        let mut parser = SseParser::new();
        let events1 = parser.feed(b"data: hel");
        assert_eq!(events1.len(), 0);
        let events2 = parser.feed(b"lo\n\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "hello");
    }
}
