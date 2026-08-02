//! Token usage extraction: pure functions that turn upstream responses into a
//! normalized `UsageSummary` (prompt / completion / cached input tokens).
//!
//! No filesystem or network access — everything is a function of the JSON or
//! SSE text fed in, so it is unit-testable in isolation.

/// Normalized token usage for one request.
///
/// `cached_tokens` is the count of *cache-hit* input tokens only (the
/// "cached input ÷ total input" cache-rate denominator never includes output).
#[derive(Debug, Clone, PartialEq)]
pub struct UsageSummary {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: u64,
}

/// Extract a `UsageSummary` from a complete response JSON.
///
/// Cache fields are probed in order Anthropic > OpenAI standard > DeepSeek
/// variant, taking the first that exists. Returns `None` when the response
/// carries no usage object at all.
pub fn extract_usage(value: &serde_json::Value) -> Option<UsageSummary> {
    let usage = value.get("usage")?;
    if !usage.is_object() {
        return None;
    }
    let first_u64 = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| usage.get(*k).and_then(serde_json::Value::as_u64))
            .unwrap_or(0)
    };
    let cached_tokens = cache_read_of(usage)
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(serde_json::Value::as_u64)
        })
        .or_else(|| {
            usage
                .get("prompt_cache_hit_tokens")
                .and_then(serde_json::Value::as_u64)
        })
        .unwrap_or(0);
    Some(UsageSummary {
        prompt_tokens: first_u64(&["input_tokens", "prompt_tokens"]),
        completion_tokens: first_u64(&["output_tokens", "completion_tokens"]),
        cached_tokens,
    })
}

/// Position of the next SSE frame separator in `buf`, with its length.
/// Accepts both `\n\n` (LF) and `\r\n\r\n` (CRLF) framing; CRLF wins when
/// both are present so mixed streams still split on complete frames.
fn frame_end(buf: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some((pos, 4));
    }
    buf.windows(2)
        .position(|w| w == b"\n\n")
        .map(|pos| (pos, 2))
}

/// The Anthropic cache-hit input count in a usage object, if reported.
fn cache_read_of(usage: &serde_json::Value) -> Option<u64> {
    usage
        .get("cache_read_input_tokens")
        .and_then(serde_json::Value::as_u64)
}

/// Incrementally parses an SSE response stream, accumulating a `UsageSummary`.
///
/// Feed it raw bytes via `push` (any chunk boundary is fine); call `finish`
/// once the stream closes for the summary. OpenAI streams carry the usage
/// frame in the chunk before `[DONE]`; Anthropic streams carry input/cache
/// counts on `message_start` and the cumulative output count on
/// `message_delta`.
pub struct SseUsageParser {
    buffer: Vec<u8>,
    summary: Option<UsageSummary>,
    anthropic_input: Option<u64>,
    anthropic_cached: Option<u64>,
    anthropic_completion: Option<u64>,
}

impl SseUsageParser {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            summary: None,
            anthropic_input: None,
            anthropic_cached: None,
            anthropic_completion: None,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
        while let Some((end, separator_len)) = frame_end(&self.buffer) {
            let frame: Vec<u8> = self.buffer[..end].to_vec();
            self.buffer.drain(..end + separator_len);
            self.handle_frame(&frame);
        }
    }

    fn handle_frame(&mut self, frame: &[u8]) {
        for line in frame.split(|&b| b == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let Some(data) = line.strip_prefix(b"data:") else {
                continue;
            };
            let data = match std::str::from_utf8(data) {
                Ok(s) => s.trim(),
                Err(_) => continue,
            };
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            match value.get("type").and_then(serde_json::Value::as_str) {
                Some("message_start") => {
                    if let Some(usage) = value.get("message").and_then(|m| m.get("usage")) {
                        if let Some(v) = usage.get("input_tokens") {
                            self.anthropic_input = Some(v.as_u64().unwrap_or(0));
                        }
                        if let Some(n) = cache_read_of(usage) {
                            self.anthropic_cached = Some(n);
                        }
                    }
                }
                Some("message_delta") => {
                    if let Some(usage) = value.get("usage") {
                        if let Some(v) = usage.get("output_tokens") {
                            self.anthropic_completion = Some(v.as_u64().unwrap_or(0));
                        }
                        if let Some(n) = cache_read_of(usage) {
                            self.anthropic_cached = Some(n);
                        }
                    }
                }
                Some(_) => {}
                None => {
                    if let Some(summary) = extract_usage(&value) {
                        self.summary = Some(summary);
                    }
                }
            }
        }
    }

    pub fn finish(&self) -> Option<UsageSummary> {
        if let Some(summary) = &self.summary {
            return Some(summary.clone());
        }
        match (self.anthropic_input, self.anthropic_completion) {
            (Some(prompt_tokens), Some(completion_tokens)) => Some(UsageSummary {
                prompt_tokens,
                completion_tokens,
                cached_tokens: self.anthropic_cached.unwrap_or(0),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_usage_reads_all_three_field_styles() {
        let anthropic = json!({
            "id": "msg_01",
            "type": "message",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_read_input_tokens": 40,
                "cache_creation_input_tokens": 60,
            },
        });
        let openai = json!({
            "usage": {
                "prompt_tokens": 200,
                "completion_tokens": 30,
                "prompt_tokens_details": { "cached_tokens": 120 },
            },
        });
        let deepseek = json!({
            "usage": {
                "prompt_tokens": 300,
                "completion_tokens": 40,
                "prompt_cache_hit_tokens": 150,
                "prompt_cache_miss_tokens": 150,
            },
        });

        let a = extract_usage(&anthropic).expect("anthropic style");
        assert_eq!(
            (a.prompt_tokens, a.completion_tokens, a.cached_tokens),
            (100, 50, 40)
        );

        let o = extract_usage(&openai).expect("openai style");
        assert_eq!(
            (o.prompt_tokens, o.completion_tokens, o.cached_tokens),
            (200, 30, 120)
        );

        let d = extract_usage(&deepseek).expect("deepseek style");
        assert_eq!(
            (d.prompt_tokens, d.completion_tokens, d.cached_tokens),
            (300, 40, 150)
        );
    }

    #[test]
    fn extract_usage_probes_cache_fields_in_order_and_takes_first() {
        let all_three = json!({
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "cache_read_input_tokens": 7,
                "prompt_tokens_details": { "cached_tokens": 8 },
                "prompt_cache_hit_tokens": 9,
            },
        });
        let openai_plus_deepseek = json!({
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "prompt_tokens_details": { "cached_tokens": 8 },
                "prompt_cache_hit_tokens": 9,
            },
        });
        let deepseek_only = json!({
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "prompt_cache_hit_tokens": 9,
            },
        });

        let a = extract_usage(&all_three).unwrap();
        assert_eq!(a.cached_tokens, 7, "anthropic field wins");
        let o = extract_usage(&openai_plus_deepseek).unwrap();
        assert_eq!(o.cached_tokens, 8, "openai field wins over deepseek");
        let d = extract_usage(&deepseek_only).unwrap();
        assert_eq!(d.cached_tokens, 9, "deepseek field as last resort");
    }

    #[test]
    fn extract_usage_handles_missing_or_malformed_usage() {
        assert_eq!(extract_usage(&json!({ "content": "no usage here" })), None);
        assert_eq!(extract_usage(&json!({ "usage": null })), None);
        assert_eq!(extract_usage(&json!({ "usage": "nope" })), None);

        let partial = extract_usage(&json!({
            "usage": { "prompt_tokens": 200 }
        }))
        .unwrap();
        assert_eq!(
            (
                partial.prompt_tokens,
                partial.completion_tokens,
                partial.cached_tokens
            ),
            (200, 0, 0),
            "missing fields default to zero"
        );
    }

    #[test]
    fn sse_parser_extracts_openai_usage_before_done() {
        let stream = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[],\"usage\":{\"prompt_tokens\":200,\"completion_tokens\":30,\"prompt_tokens_details\":{\"cached_tokens\":120}}}\n\n",
            "data: [DONE]\n\n",
        );
        let mut parser = SseUsageParser::new();
        parser.push(stream.as_bytes());
        assert_eq!(
            parser.finish(),
            Some(UsageSummary {
                prompt_tokens: 200,
                completion_tokens: 30,
                cached_tokens: 120,
            })
        );
    }

    #[test]
    fn sse_parser_returns_none_for_openai_stream_without_usage() {
        let stream = concat!(
            "data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let mut parser = SseUsageParser::new();
        parser.push(stream.as_bytes());
        assert_eq!(parser.finish(), None);
    }

    #[test]
    fn sse_parser_keeps_usage_when_stream_cut_after_usage_frame() {
        let stream = concat!(
            "data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-3\",\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":10,\"prompt_tokens_details\":{\"cached_tokens\":50}}}\n\n",
        );
        let mut parser = SseUsageParser::new();
        parser.push(stream.as_bytes());
        assert_eq!(
            parser.finish(),
            Some(UsageSummary {
                prompt_tokens: 100,
                completion_tokens: 10,
                cached_tokens: 50,
            })
        );
    }

    fn anthropic_stream(start_usage: &str, delta_cache: &str) -> String {
        format!(
            concat!(
                "event: message_start\n",
                "data: {{\"type\":\"message_start\",\"message\":{{\"usage\":{{{start_usage}}}}}}}\n\n",
                "event: content_block_delta\n",
                "data: {{\"type\":\"content_block_delta\",\"delta\":{{\"type\":\"text_delta\",\"text\":\"Hello\"}}}}\n\n",
                "event: message_delta\n",
                "data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"output_tokens\":13,{delta_cache}}}}}\n\n",
                "event: message_stop\n",
                "data: {{\"type\":\"message_stop\"}}\n\n",
            ),
            start_usage = start_usage,
            delta_cache = delta_cache,
        )
    }

    #[test]
    fn sse_parser_extracts_anthropic_usage_from_start_and_delta() {
        let stream = anthropic_stream(
            r#""input_tokens":25,"cache_creation_input_tokens":0,"cache_read_input_tokens":40"#,
            r#""cache_creation_input_tokens":null,"cache_read_input_tokens":null"#,
        );
        let mut parser = SseUsageParser::new();
        parser.push(stream.as_bytes());
        assert_eq!(
            parser.finish(),
            Some(UsageSummary {
                prompt_tokens: 25,
                completion_tokens: 13,
                cached_tokens: 40,
            })
        );
    }

    #[test]
    fn sse_parser_returns_none_for_anthropic_stream_without_message_start() {
        let stream = anthropic_stream(
            "",
            r#""cache_creation_input_tokens":null,"cache_read_input_tokens":null"#,
        );
        let mut parser = SseUsageParser::new();
        parser.push(stream.as_bytes());
        assert_eq!(parser.finish(), None);
    }

    #[test]
    fn sse_parser_returns_none_for_anthropic_stream_without_message_delta() {
        let mut parser = SseUsageParser::new();
        parser.push(
            concat!(
                "event: message_start\n",
                "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":40}}}\n\n",
            )
            .as_bytes(),
        );
        assert_eq!(parser.finish(), None);
    }

    #[test]
    fn sse_parser_takes_later_anthropic_cache_update_from_message_delta() {
        let stream = anthropic_stream(
            r#""input_tokens":25,"cache_creation_input_tokens":0,"cache_read_input_tokens":0"#,
            r#""cache_creation_input_tokens":null,"cache_read_input_tokens":55"#,
        );
        let mut parser = SseUsageParser::new();
        parser.push(stream.as_bytes());
        assert_eq!(
            parser.finish(),
            Some(UsageSummary {
                prompt_tokens: 25,
                completion_tokens: 13,
                cached_tokens: 55,
            })
        );
    }

    #[test]
    fn sse_parser_matches_whole_stream_result_across_arbitrary_chunk_boundaries() {
        let openai_stream = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[],\"usage\":{\"prompt_tokens\":200,\"completion_tokens\":30,\"prompt_tokens_details\":{\"cached_tokens\":120}}}\n\n",
            "data: [DONE]\n\n",
        );
        let expected = Some(UsageSummary {
            prompt_tokens: 200,
            completion_tokens: 30,
            cached_tokens: 120,
        });

        let mut one_byte_at_a_time = SseUsageParser::new();
        for byte in openai_stream.as_bytes() {
            one_byte_at_a_time.push(&[*byte]);
        }
        assert_eq!(one_byte_at_a_time.finish(), expected);

        for offset in 0..openai_stream.len() {
            let mut split = SseUsageParser::new();
            split.push(&openai_stream.as_bytes()[..offset]);
            split.push(&openai_stream.as_bytes()[offset..]);
            assert_eq!(
                split.finish(),
                expected,
                "two-chunk split at byte offset {offset} diverges"
            );
        }
    }

    #[test]
    fn sse_parser_tolerates_garbage_and_empty_input() {
        let mut parser = SseUsageParser::new();
        assert_eq!(parser.finish(), None, "empty stream");
        parser.push(b"\xff\xfe\x00 not utf-8 garbage \xc3");
        parser.push(b"data: this is not json\n\n");
        parser.push(b"event: unknown_event\n");
        parser.push(b"data: {\"type\":\"ping\"}\n\n");
        parser.push(b"data: [DONE]\n\n");
        assert_eq!(
            parser.finish(),
            None,
            "garbage must not panic or invent usage"
        );
    }

    #[test]
    fn sse_parser_handles_crlf_frame_separators() {
        let stream = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\r\n\r\n",
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[],\"usage\":{\"prompt_tokens\":200,\"completion_tokens\":30,\"prompt_tokens_details\":{\"cached_tokens\":120}}}\r\n\r\n",
            "data: [DONE]\r\n\r\n",
        );
        let mut parser = SseUsageParser::new();
        parser.push(stream.as_bytes());
        assert_eq!(
            parser.finish(),
            Some(UsageSummary {
                prompt_tokens: 200,
                completion_tokens: 30,
                cached_tokens: 120,
            })
        );
    }

    #[test]
    fn sse_parser_defaults_null_anthropic_field_to_zero() {
        let mut parser = SseUsageParser::new();
        parser.push(
            concat!(
                "event: message_start\n",
                "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"cache_read_input_tokens\":40}}}\n\n",
                "event: message_delta\n",
                "data: {\"type\":\"message_delta\",\"delta\":{},\"usage\":{\"output_tokens\":null}}\n\n",
            )
            .as_bytes(),
        );
        assert_eq!(
            parser.finish(),
            Some(UsageSummary {
                prompt_tokens: 25,
                completion_tokens: 0,
                cached_tokens: 40,
            })
        );
    }
}
