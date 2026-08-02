use crate::config::config_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestLogEntry {
    pub ts: Option<String>,
    pub ok: Option<bool>,
    pub provider: Option<String>,
    pub error: Option<String>,
    pub status: Option<u16>,
    #[serde(rename = "upstreamUrl")]
    pub upstream_url: Option<String>,
    pub model: Option<String>,
    pub ms: Option<u64>,
    pub retry: Option<bool>,
    pub skipped: Option<bool>,
    pub converted: Option<String>,
    #[serde(rename = "promptTokens", default)]
    pub prompt_tokens: Option<u64>,
    #[serde(rename = "completionTokens", default)]
    pub completion_tokens: Option<u64>,
    #[serde(rename = "cachedTokens", default)]
    pub cached_tokens: Option<u64>,
    #[serde(rename = "conversationId", default)]
    pub conversation_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderStats {
    pub total: u64,
    pub ok: u64,
    pub failed: u64,
    pub retries: u64,
    #[serde(rename = "avgMs")]
    pub avg_ms: u64,
    #[serde(rename = "totalMs")]
    pub total_ms: u64,
    #[serde(rename = "lastUsed")]
    pub last_used: Option<String>,
    #[serde(rename = "promptTokens")]
    pub prompt_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "cachedTokens")]
    pub cached_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct TokenTotals {
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

#[derive(Debug, Serialize)]
pub struct ConversationStats {
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    pub requests: u64,
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "lastActive")]
    pub last_active: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UsageStats {
    #[serde(rename = "totalRequests")]
    pub total_requests: u64,
    #[serde(rename = "okRequests")]
    pub ok_requests: u64,
    #[serde(rename = "failedRequests")]
    pub failed_requests: u64,
    #[serde(rename = "retriedRequests")]
    pub retried_requests: u64,
    #[serde(rename = "skippedByCircuit")]
    pub skipped_by_circuit: u64,
    #[serde(rename = "successRate")]
    pub success_rate: String,
    #[serde(rename = "avgLatencyMs")]
    pub avg_latency_ms: u64,
    #[serde(rename = "byProvider")]
    pub by_provider: HashMap<String, ProviderStats>,
    #[serde(rename = "byModel")]
    pub by_model: HashMap<String, ModelStats>,
    #[serde(rename = "circuitBreaker")]
    pub circuit_breaker: HashMap<String, CircuitBreakerStatus>,
    #[serde(rename = "totalTokens")]
    pub total_tokens: TokenTotals,
    #[serde(rename = "cacheHitRate")]
    pub cache_hit_rate: String,
    #[serde(rename = "byConversation")]
    pub by_conversation: Vec<ConversationStats>,
}

#[derive(Debug, Serialize)]
pub struct ModelStats {
    pub total: u64,
    pub ok: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CircuitBreakerEntry {
    pub failures: u64,
    #[serde(rename = "openedAt")]
    pub opened_at: Option<u64>,
    #[serde(rename = "lastSuccessAt")]
    pub last_success_at: Option<u64>,
    #[serde(rename = "lastFailureAt")]
    pub last_failure_at: Option<u64>,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CircuitBreakerStatus {
    pub state: String, // "open", "closed", "half_open"
    pub failures: u64,
    #[serde(rename = "openedAt")]
    pub opened_at: Option<u64>,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
}

/// Parse request-log text into entries, skipping empty and malformed lines.
fn parse_entries(text: &str) -> Vec<RequestLogEntry> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str(line).ok()
        })
        .collect()
}

fn parse_logs() -> Vec<RequestLogEntry> {
    let path = config_dir().join("requests.log");
    if !path.exists() {
        return vec![];
    }

    parse_entries(&std::fs::read_to_string(&path).unwrap_or_default())
}

fn read_circuit_state() -> HashMap<String, CircuitBreakerEntry> {
    let path = config_dir().join("circuit.json");
    if !path.exists() {
        return HashMap::new();
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let state: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();

    state
        .get("providers")
        .and_then(|p| p.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value(v.clone())
                        .ok()
                        .map(|entry| (k.clone(), entry))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn circuit_breaker_status(
    entry: &CircuitBreakerEntry,
    cooldown_ms: u64,
    now_ms: u64,
) -> CircuitBreakerStatus {
    let state = if let Some(opened_at) = entry.opened_at {
        if now_ms.saturating_sub(opened_at) < cooldown_ms {
            "open"
        } else {
            "half_open"
        }
    } else {
        "closed"
    };

    CircuitBreakerStatus {
        state: state.to_string(),
        failures: entry.failures,
        opened_at: entry.opened_at,
        last_error: entry.last_error.clone(),
    }
}

/// Token usage of a single countable request-log row.
struct TokenUsage {
    prompt: u64,
    completion: u64,
    cached: u64,
}

/// Token usage of an entry counted into aggregates: only successful,
/// non-retried rows that actually parsed usage data. Failover/retry
/// intermediate rows are excluded so one request is never double-counted.
fn usage_of(entry: &RequestLogEntry) -> Option<TokenUsage> {
    if entry.ok != Some(true) || entry.retry.unwrap_or(false) {
        return None;
    }
    match (entry.prompt_tokens, entry.completion_tokens) {
        (Some(prompt), Some(completion)) => Some(TokenUsage {
            prompt,
            completion,
            cached: entry.cached_tokens.unwrap_or(0),
        }),
        _ => None,
    }
}

/// Aggregate request-log entries into a `UsageStats`. Pure: all inputs are
/// injected (entries, circuit state, cooldown, current time), no I/O.
pub fn aggregate(
    entries: &[RequestLogEntry],
    circuit: &HashMap<String, CircuitBreakerEntry>,
    cooldown_ms: u64,
    now_ms: u64,
) -> UsageStats {
    let circuit_breaker: HashMap<String, CircuitBreakerStatus> = circuit
        .iter()
        .map(|(name, entry)| {
            (
                name.clone(),
                circuit_breaker_status(entry, cooldown_ms, now_ms),
            )
        })
        .collect();

    let mut stats = UsageStats {
        total_requests: 0,
        ok_requests: 0,
        failed_requests: 0,
        retried_requests: 0,
        skipped_by_circuit: 0,
        success_rate: "0%".into(),
        avg_latency_ms: 0,
        by_provider: HashMap::new(),
        by_model: HashMap::new(),
        circuit_breaker,
        total_tokens: TokenTotals {
            input: 0,
            output: 0,
            total: 0,
        },
        cache_hit_rate: "-".into(),
        by_conversation: Vec::new(),
    };

    let mut total_ms: u64 = 0;
    let mut latency_count: u64 = 0;
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_cached: u64 = 0;
    let mut conversations: HashMap<String, ConversationStats> = HashMap::new();

    for entry in entries {
        stats.total_requests += 1;
        match entry.ok {
            Some(true) => stats.ok_requests += 1,
            _ => stats.failed_requests += 1,
        }
        if entry.retry.unwrap_or(false) {
            stats.retried_requests += 1;
        }
        if entry.skipped.unwrap_or(false) {
            stats.skipped_by_circuit += 1;
        }
        let usage = usage_of(entry);
        if let Some(u) = &usage {
            total_input += u.prompt;
            total_output += u.completion;
            total_cached += u.cached;
        }

        // Per conversation: every row counts toward requests/last-active;
        // only countable usage rows contribute tokens.
        let key = entry
            .conversation_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("unlabeled")
            .to_string();
        let conv = conversations
            .entry(key.clone())
            .or_insert_with(|| ConversationStats {
                conversation_id: key.clone(),
                requests: 0,
                input_tokens: 0,
                output_tokens: 0,
                last_active: None,
            });
        conv.requests += 1;
        if let Some(ts) = entry.ts.as_deref() {
            if conv.last_active.as_deref().is_none_or(|last| ts > last) {
                conv.last_active = Some(ts.to_string());
            }
        }
        if let Some(u) = &usage {
            conv.input_tokens += u.prompt;
            conv.output_tokens += u.completion;
        }

        // Per provider
        let provider = entry.provider.as_deref().unwrap_or("unknown");
        let ps = stats
            .by_provider
            .entry(provider.to_string())
            .or_insert(ProviderStats {
                total: 0,
                ok: 0,
                failed: 0,
                retries: 0,
                avg_ms: 0,
                total_ms: 0,
                last_used: None,
                prompt_tokens: 0,
                output_tokens: 0,
                cached_tokens: 0,
            });
        ps.total += 1;
        if entry.ok.unwrap_or(false) {
            ps.ok += 1;
        } else {
            ps.failed += 1;
        }
        if entry.retry.unwrap_or(false) {
            ps.retries += 1;
        }
        if let Some(u) = &usage {
            ps.prompt_tokens += u.prompt;
            ps.output_tokens += u.completion;
            ps.cached_tokens += u.cached;
        }
        if let Some(ms) = entry.ms {
            ps.total_ms += ms;
            ps.avg_ms = ps.total_ms / ps.total;
        }
        if let Some(ref ts) = entry.ts {
            ps.last_used = Some(ts.clone());
        }

        // Per model
        let model = entry.model.as_deref().unwrap_or("unknown");
        let ms = stats
            .by_model
            .entry(model.to_string())
            .or_insert(ModelStats { total: 0, ok: 0 });
        ms.total += 1;
        if entry.ok.unwrap_or(false) {
            ms.ok += 1;
        }

        // Latency
        if let Some(ms) = entry.ms {
            total_ms += ms;
            latency_count += 1;
        }
    }

    if latency_count > 0 {
        stats.avg_latency_ms = total_ms / latency_count;
    }
    if stats.total_requests > 0 {
        stats.success_rate = format!(
            "{:.1}%",
            (stats.ok_requests as f64 / stats.total_requests as f64) * 100.0
        );
    }

    stats.total_tokens = TokenTotals {
        input: total_input,
        output: total_output,
        total: total_input + total_output,
    };
    stats.cache_hit_rate = if total_cached == 0 {
        "-".into()
    } else {
        format!("{:.1}%", (total_cached as f64 / total_input as f64) * 100.0)
    };

    let mut by_conversation: Vec<ConversationStats> = conversations.into_values().collect();
    by_conversation.sort_by(|a, b| match (&a.last_active, &b.last_active) {
        (Some(x), Some(y)) => y.cmp(x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    by_conversation.truncate(20);
    stats.by_conversation = by_conversation;

    stats
}

pub fn get_stats() -> UsageStats {
    let entries = parse_logs();

    // Read circuit breaker state
    let circuit_entries = read_circuit_state();
    let cooldown_ms = 60_000; // Default 60 seconds
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    aggregate(&entries, &circuit_entries, cooldown_ms, now_ms)
}

pub fn export_logs_json() -> crate::error::Result<String> {
    let entries = parse_logs();
    serde_json::to_string_pretty(&entries)
        .map_err(|e| crate::error::AppError::Message(format!("Failed to serialize logs: {}", e)))
}

fn csv_of(entries: &[RequestLogEntry]) -> String {
    let mut csv = String::from(
        "timestamp,ok,provider,model,status,latency_ms,error,retry,skipped,converted,upstream_url,promptTokens,completionTokens,cachedTokens,conversationId\n",
    );

    for entry in entries {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            entry.ts.as_deref().unwrap_or(""),
            entry
                .ok
                .map(|b| if b { "true" } else { "false" })
                .unwrap_or(""),
            entry.provider.as_deref().unwrap_or(""),
            entry.model.as_deref().unwrap_or(""),
            entry.status.map(|s| s.to_string()).unwrap_or_default(),
            entry.ms.map(|m| m.to_string()).unwrap_or_default(),
            entry
                .error
                .as_deref()
                .unwrap_or("")
                .replace(',', ";")
                .replace('\n', " "),
            entry
                .retry
                .map(|b| if b { "true" } else { "false" })
                .unwrap_or(""),
            entry
                .skipped
                .map(|b| if b { "true" } else { "false" })
                .unwrap_or(""),
            entry.converted.as_deref().unwrap_or(""),
            entry.upstream_url.as_deref().unwrap_or(""),
            entry
                .prompt_tokens
                .map(|t| t.to_string())
                .unwrap_or_default(),
            entry
                .completion_tokens
                .map(|t| t.to_string())
                .unwrap_or_default(),
            entry
                .cached_tokens
                .map(|t| t.to_string())
                .unwrap_or_default(),
            entry
                .conversation_id
                .as_deref()
                .unwrap_or("")
                .replace(',', ";")
                .replace('\n', " "),
        ));
    }

    csv
}

pub fn export_logs_csv() -> crate::error::Result<String> {
    Ok(csv_of(&parse_logs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(ok: bool, provider: &str, model: &str, ms: u64, ts: &str) -> RequestLogEntry {
        RequestLogEntry {
            ts: Some(ts.into()),
            ok: Some(ok),
            provider: Some(provider.into()),
            error: None,
            status: Some(200),
            upstream_url: None,
            model: Some(model.into()),
            ms: Some(ms),
            retry: None,
            skipped: None,
            converted: None,
            prompt_tokens: None,
            completion_tokens: None,
            cached_tokens: None,
            conversation_id: None,
        }
    }

    fn with_usage(mut e: RequestLogEntry, p: u64, c: u64, cached: u64) -> RequestLogEntry {
        e.prompt_tokens = Some(p);
        e.completion_tokens = Some(c);
        e.cached_tokens = Some(cached);
        e
    }

    #[test]
    fn aggregate_empty_entries_yields_zero_stats() {
        let stats = aggregate(&[], &HashMap::new(), 60_000, 0);
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.ok_requests, 0);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.retried_requests, 0);
        assert_eq!(stats.skipped_by_circuit, 0);
        assert_eq!(stats.success_rate, "0%");
        assert_eq!(stats.avg_latency_ms, 0);
        assert!(stats.by_provider.is_empty());
        assert!(stats.by_model.is_empty());
        assert!(stats.circuit_breaker.is_empty());
    }

    #[test]
    fn aggregate_single_success_entry_counts_everywhere() {
        let stats = aggregate(
            &[entry(true, "hyb", "gpt-5.4", 100, "2026-08-02T10:00:00Z")],
            &HashMap::new(),
            60_000,
            0,
        );
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.ok_requests, 1);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.retried_requests, 0);
        assert_eq!(stats.success_rate, "100.0%");
        assert_eq!(stats.avg_latency_ms, 100);
        let ps = &stats.by_provider["hyb"];
        assert_eq!((ps.total, ps.ok, ps.failed), (1, 1, 0));
        assert_eq!((ps.total_ms, ps.avg_ms), (100, 100));
        assert_eq!(ps.last_used.as_deref(), Some("2026-08-02T10:00:00Z"));
        let ms = &stats.by_model["gpt-5.4"];
        assert_eq!((ms.total, ms.ok), (1, 1));
    }

    #[test]
    fn aggregate_computes_circuit_breaker_states_from_injected_time() {
        let mut circuit = HashMap::new();
        let circuit_entry = |failures: u64, opened_at: Option<u64>| CircuitBreakerEntry {
            failures,
            opened_at,
            last_success_at: None,
            last_failure_at: None,
            last_error: None,
        };
        // now = 1_030_000; hot opened 30s ago (< 60s cooldown), cooled 90s ago (> cooldown).
        circuit.insert("hot".to_string(), circuit_entry(5, Some(1_000_000)));
        circuit.insert("cooled".to_string(), circuit_entry(2, Some(940_000)));
        circuit.insert("healthy".to_string(), circuit_entry(0, None));

        let stats = aggregate(&[], &circuit, 60_000, 1_030_000);

        assert_eq!(
            stats.circuit_breaker["hot"].state, "open",
            "30s since opened < 60s cooldown"
        );
        assert_eq!(
            stats.circuit_breaker["cooled"].state, "half_open",
            "90s since opened > 60s cooldown"
        );
        assert_eq!(stats.circuit_breaker["healthy"].state, "closed");
        assert_eq!(stats.circuit_breaker["hot"].failures, 5);
        assert_eq!(stats.circuit_breaker["hot"].opened_at, Some(1_000_000));
    }

    #[test]
    fn aggregate_multiple_entries_accumulates_groups() {
        let mut fox = entry(false, "fox", "claude-sonnet", 50, "2026-08-02T10:00:01Z");
        fox.retry = Some(true);
        let mut unlabeled = entry(false, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:02Z");
        unlabeled.provider = None;
        unlabeled.model = None;
        unlabeled.skipped = Some(true);
        unlabeled.ms = None;
        let mut no_ok_flag = entry(true, "hyb", "gpt-5.4", 30, "2026-08-02T10:00:03Z");
        no_ok_flag.ok = None;

        let stats = aggregate(
            &[
                entry(true, "hyb", "gpt-5.4", 100, "2026-08-02T10:00:00Z"),
                fox,
                unlabeled,
                no_ok_flag,
            ],
            &HashMap::new(),
            60_000,
            0,
        );

        assert_eq!(stats.total_requests, 4);
        assert_eq!(stats.ok_requests, 1);
        assert_eq!(stats.failed_requests, 3, "missing ok flag counts as failed");
        assert_eq!(stats.retried_requests, 1);
        assert_eq!(stats.skipped_by_circuit, 1);
        assert_eq!(stats.success_rate, "25.0%");
        assert_eq!(stats.avg_latency_ms, 60, "(100 + 50 + 30) / 3");

        let hyb = &stats.by_provider["hyb"];
        assert_eq!((hyb.total, hyb.ok, hyb.failed), (2, 1, 1));
        assert_eq!((hyb.total_ms, hyb.avg_ms), (130, 65));
        assert_eq!(hyb.last_used.as_deref(), Some("2026-08-02T10:00:03Z"));
        let fox_ps = &stats.by_provider["fox"];
        assert_eq!(
            (fox_ps.total, fox_ps.ok, fox_ps.failed, fox_ps.retries),
            (1, 0, 1, 1)
        );
        let unknown_ps = &stats.by_provider["unknown"];
        assert_eq!(
            (unknown_ps.total, unknown_ps.ok, unknown_ps.failed),
            (1, 0, 1)
        );

        let gpt = &stats.by_model["gpt-5.4"];
        assert_eq!((gpt.total, gpt.ok), (2, 1));
        let claude = &stats.by_model["claude-sonnet"];
        assert_eq!((claude.total, claude.ok), (1, 0));
        let unknown_ms = &stats.by_model["unknown"];
        assert_eq!((unknown_ms.total, unknown_ms.ok), (1, 0));
    }

    #[test]
    fn parse_entries_parses_valid_lines() {
        let text = concat!(
            "{\"ok\":true,\"provider\":\"hyb\",\"ms\":12}\n",
            "{\"ok\":false,\"provider\":\"fox\"}\n",
        );
        let entries = parse_entries(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].ok, Some(true));
        assert_eq!(entries[0].provider.as_deref(), Some("hyb"));
        assert_eq!(entries[0].ms, Some(12));
        assert_eq!(entries[1].ok, Some(false));
        assert_eq!(entries[1].provider.as_deref(), Some("fox"));
    }

    #[test]
    fn parse_entries_skips_empty_and_malformed_lines() {
        let text = concat!(
            "{\"ok\":true}\n",
            "\n",
            "not json at all\n",
            "  \n",
            "{\"broken\n",
            "{\"ok\":false}\n",
        );
        let entries = parse_entries(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].ok, Some(true));
        assert_eq!(entries[1].ok, Some(false));
    }

    #[test]
    fn parse_entries_empty_text_yields_no_entries() {
        assert!(parse_entries("").is_empty());
        assert!(parse_entries("\n\n\n").is_empty());
    }

    #[test]
    fn aggregate_sums_tokens_only_for_successful_non_retry_entries_with_usage() {
        let ok_usage = with_usage(
            entry(true, "hyb", "gpt-5.4", 100, "2026-08-02T10:00:00Z"),
            100,
            50,
            40,
        );
        let failed = with_usage(
            entry(false, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:01Z"),
            200,
            60,
            20,
        );
        let mut retried = with_usage(
            entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:02Z"),
            300,
            70,
            30,
        );
        retried.retry = Some(true);
        let no_usage = entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:03Z");
        let mut unknown_ok = with_usage(
            entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:04Z"),
            0,
            0,
            0,
        );
        unknown_ok.ok = None;

        let stats = aggregate(
            &[ok_usage, failed, retried, no_usage, unknown_ok],
            &HashMap::new(),
            60_000,
            0,
        );

        assert_eq!(
            (
                stats.total_tokens.input,
                stats.total_tokens.output,
                stats.total_tokens.total
            ),
            (100, 50, 150),
            "failed/retried/ok-missing/no-usage rows must not contribute"
        );
    }

    #[test]
    fn aggregate_cache_hit_rate_is_cached_over_total_input() {
        let a = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z"),
            100,
            10,
            40,
        );
        let b = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:01Z"),
            200,
            20,
            120,
        );

        let stats = aggregate(&[a, b], &HashMap::new(), 60_000, 0);

        assert_eq!(stats.cache_hit_rate, "53.3%", "160 cached / 300 input");
    }

    #[test]
    fn aggregate_cache_rate_is_dash_without_any_token_data() {
        let empty = aggregate(&[], &HashMap::new(), 60_000, 0);
        assert_eq!(empty.cache_hit_rate, "-");
        assert_eq!(
            (
                empty.total_tokens.input,
                empty.total_tokens.output,
                empty.total_tokens.total
            ),
            (0, 0, 0)
        );

        let no_usage = aggregate(
            &[
                entry(true, "hyb", "gpt-5.4", 10, "2026-08-02T10:00:00Z"),
                entry(false, "hyb", "gpt-5.4", 10, "2026-08-02T10:00:01Z"),
            ],
            &HashMap::new(),
            60_000,
            0,
        );
        assert_eq!(no_usage.cache_hit_rate, "-");
        assert_eq!(
            (
                no_usage.total_tokens.input,
                no_usage.total_tokens.output,
                no_usage.total_tokens.total
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn aggregate_cache_rate_is_dash_without_cache_data() {
        let a = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        let b = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:01Z"),
            200,
            20,
            0,
        );

        let stats = aggregate(&[a, b], &HashMap::new(), 60_000, 0);

        assert_eq!(
            (stats.total_tokens.input, stats.total_tokens.total),
            (300, 330),
            "usage without cache data still counts toward totals"
        );
        assert_eq!(
            stats.cache_hit_rate, "-",
            "no cache data means no rate, not a fake 0%"
        );
    }

    #[test]
    fn aggregate_no_token_data_serializes_empty_by_conversation() {
        let stats = aggregate(&[], &HashMap::new(), 60_000, 0);
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["byConversation"], serde_json::json!([]));
        assert_eq!(json["cacheHitRate"], "-");
        assert_eq!(json["totalTokens"]["total"], 0);
    }

    #[test]
    fn aggregate_provider_stats_accumulate_token_columns() {
        let hyb_ok = with_usage(
            entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:00Z"),
            100,
            50,
            40,
        );
        let hyb_ok2 = with_usage(
            entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:01Z"),
            200,
            60,
            120,
        );
        let hyb_failed = with_usage(
            entry(false, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:02Z"),
            300,
            70,
            30,
        );
        let fox_ok = with_usage(
            entry(true, "fox", "claude-sonnet", 0, "2026-08-02T10:00:03Z"),
            10,
            20,
            30,
        );
        let fox_no_usage = entry(true, "fox", "claude-sonnet", 0, "2026-08-02T10:00:04Z");

        let stats = aggregate(
            &[hyb_ok, hyb_ok2, hyb_failed, fox_ok, fox_no_usage],
            &HashMap::new(),
            60_000,
            0,
        );

        let hyb = &stats.by_provider["hyb"];
        assert_eq!(
            (hyb.prompt_tokens, hyb.output_tokens, hyb.cached_tokens),
            (300, 110, 160),
            "failed row contributes to counts but not to token columns"
        );
        assert_eq!((hyb.total, hyb.ok, hyb.failed), (3, 2, 1));
        let fox = &stats.by_provider["fox"];
        assert_eq!(
            (fox.prompt_tokens, fox.output_tokens, fox.cached_tokens),
            (10, 20, 30),
            "rows without usage do not contribute token columns"
        );
    }

    #[test]
    fn aggregate_conversations_group_and_merge_unlabeled() {
        let mut conv_a_1 = with_usage(
            entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:00Z"),
            100,
            50,
            40,
        );
        conv_a_1.conversation_id = Some("conv-a".into());
        let mut conv_a_2 = with_usage(
            entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:02Z"),
            200,
            60,
            120,
        );
        conv_a_2.conversation_id = Some("conv-a".into());
        let mut conv_b = with_usage(
            entry(true, "fox", "claude-sonnet", 0, "2026-08-02T10:00:01Z"),
            10,
            20,
            30,
        );
        conv_b.conversation_id = Some("conv-b".into());
        let mut conv_b_failed = with_usage(
            entry(false, "fox", "claude-sonnet", 0, "2026-08-02T10:00:03Z"),
            500,
            90,
            10,
        );
        conv_b_failed.conversation_id = Some("conv-b".into());
        let unlabeled_1 = entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:04Z");
        let mut unlabeled_2 = entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:05Z");
        unlabeled_2.conversation_id = Some("".into());

        let stats = aggregate(
            &[
                conv_a_1,
                conv_a_2,
                conv_b,
                conv_b_failed,
                unlabeled_1,
                unlabeled_2,
            ],
            &HashMap::new(),
            60_000,
            0,
        );

        assert_eq!(stats.by_conversation.len(), 3);

        let unlabeled = &stats.by_conversation[0];
        assert_eq!(unlabeled.conversation_id, "unlabeled");
        assert_eq!(unlabeled.requests, 2);
        assert_eq!((unlabeled.input_tokens, unlabeled.output_tokens), (0, 0));
        assert_eq!(
            unlabeled.last_active.as_deref(),
            Some("2026-08-02T10:00:05Z")
        );

        let conv_b = &stats.by_conversation[1];
        assert_eq!(conv_b.conversation_id, "conv-b");
        assert_eq!(conv_b.requests, 2);
        assert_eq!(
            (conv_b.input_tokens, conv_b.output_tokens),
            (10, 20),
            "failed row not counted"
        );
        assert_eq!(conv_b.last_active.as_deref(), Some("2026-08-02T10:00:03Z"));

        let conv_a = &stats.by_conversation[2];
        assert_eq!(conv_a.conversation_id, "conv-a");
        assert_eq!(conv_a.requests, 2);
        assert_eq!((conv_a.input_tokens, conv_a.output_tokens), (300, 110));
        assert_eq!(conv_a.last_active.as_deref(), Some("2026-08-02T10:00:02Z"));
    }

    #[test]
    fn aggregate_conversations_sort_by_last_active_desc_and_truncate_top_20() {
        let mut entries = Vec::new();
        for i in 0..21u64 {
            let mut e = with_usage(
                entry(
                    true,
                    "hyb",
                    "gpt-5.4",
                    0,
                    &format!("2026-08-02T{:02}:00:00Z", 10 + (i % 12)),
                ),
                1,
                1,
                0,
            );
            e.conversation_id = Some(format!("conv-{i:02}"));
            e.ts = Some(format!("2026-08-02T10:{i:02}:00Z"));
            entries.push(e);
        }

        let stats = aggregate(&entries, &HashMap::new(), 60_000, 0);

        assert_eq!(stats.by_conversation.len(), 20, "top 20 truncated");
        assert_eq!(
            stats.by_conversation[0].conversation_id, "conv-20",
            "most recent activity first"
        );
        assert_eq!(
            stats.by_conversation[19].conversation_id, "conv-01",
            "oldest kept entry is conv-01, conv-00 dropped"
        );
    }

    #[test]
    fn csv_export_includes_token_and_conversation_columns() {
        let mut e = with_usage(
            entry(true, "hyb", "gpt-5.4", 12, "2026-08-02T10:00:00Z"),
            100,
            50,
            40,
        );
        e.conversation_id = Some("conv-9".into());
        let csv = csv_of(&[e]);
        let mut lines = csv.lines();
        let header = lines.next().unwrap();
        assert!(
            header.ends_with(
                "upstream_url,promptTokens,completionTokens,cachedTokens,conversationId"
            ),
            "header: {header}"
        );
        let row = lines.next().unwrap();
        assert!(row.ends_with(",100,50,40,conv-9"), "row: {row}");
        assert!(lines.next().is_none(), "exactly one data row");
    }

    #[test]
    fn csv_export_escapes_conversation_id_and_omits_missing_tokens() {
        let mut e = entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:00Z");
        e.conversation_id = Some("a,b\nc".into());
        let csv = csv_of(&[e]);
        let row = csv.lines().nth(1).unwrap();
        assert!(row.ends_with(",,,a;b c"), "got: {row}");
    }

    #[test]
    fn json_export_serializes_token_and_conversation_fields() {
        let mut e = with_usage(entry(true, "hyb", "gpt-5.4", 12, "t"), 100, 50, 40);
        e.conversation_id = Some("conv-9".into());
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"promptTokens\":100"));
        assert!(json.contains("\"completionTokens\":50"));
        assert!(json.contains("\"cachedTokens\":40"));
        assert!(json.contains("\"conversationId\":\"conv-9\""));
    }

    #[test]
    fn parse_entries_reads_token_and_conversation_fields() {
        let text = "{\"ok\":true,\"provider\":\"hyb\",\"promptTokens\":100,\"completionTokens\":50,\"cachedTokens\":40,\"conversationId\":\"conv-1\"}\n";
        let entries = parse_entries(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].prompt_tokens, Some(100));
        assert_eq!(entries[0].completion_tokens, Some(50));
        assert_eq!(entries[0].cached_tokens, Some(40));
        assert_eq!(entries[0].conversation_id.as_deref(), Some("conv-1"));
    }

    #[test]
    fn parse_entries_defaults_missing_token_fields_to_none() {
        let entries = parse_entries("{\"ok\":true,\"provider\":\"hyb\"}\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].prompt_tokens, None);
        assert_eq!(entries[0].completion_tokens, None);
        assert_eq!(entries[0].cached_tokens, None);
        assert_eq!(entries[0].conversation_id, None);
    }
}
