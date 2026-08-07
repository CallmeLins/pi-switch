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
    #[serde(rename = "reasoningTokens", default)]
    pub reasoning_tokens: Option<u64>,
    #[serde(rename = "conversationId", default)]
    pub conversation_id: Option<String>,
    /// Conversation display name (header-only, ADR-0002 display attribute).
    #[serde(rename = "conversationName", default)]
    pub conversation_name: Option<String>,
    #[serde(rename = "costTotal", default)]
    pub cost_total: Option<f64>,
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
    /// Summed `costTotal` of countable usage rows; `None` when no row carried
    /// a price (mirrors the global `totalCost` semantics per dimension).
    #[serde(rename = "cost", default)]
    pub cost: Option<f64>,
    #[serde(rename = "cacheRate", default)]
    pub cache_rate: String,
    #[serde(rename = "reasoningTokens")]
    pub reasoning_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct TokenTotals {
    pub input: u64,
    pub output: u64,
    pub total: u64,
    pub cached: u64,
    pub reasoning: u64,
}

#[derive(Debug, Serialize)]
pub struct ConversationStats {
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    /// Display name of the newest named row in the window; `None` when no
    /// row carries a name. Display attribute only — never a grouping key
    /// (ADR-0002).
    pub name: Option<String>,
    pub requests: u64,
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "cachedTokens")]
    pub cached_tokens: u64,
    #[serde(rename = "reasoningTokens")]
    pub reasoning_tokens: u64,
    #[serde(rename = "lastActive")]
    pub last_active: Option<String>,
    #[serde(rename = "cacheRate")]
    pub cache_rate: String,
    #[serde(rename = "cost", default)]
    pub cost: Option<f64>,
}

/// Paged per-conversation stats response for the independent conversation
/// browser (`GET /api/stats/conversations`).
#[derive(Debug, Serialize)]
pub struct ConversationsPage {
    pub conversations: Vec<ConversationStats>,
    pub total: usize,
}

/// Paged request rows for one conversation
/// (`GET /api/stats/conversations/:id/requests`).
#[derive(Debug, Serialize)]
pub struct ConversationRequestsPage {
    pub requests: Vec<RecentRequest>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct RecentRequest {
    pub ts: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub ok: Option<bool>,
    pub status: Option<u16>,
    pub error: Option<String>,
    #[serde(rename = "promptTokens")]
    pub prompt_tokens: Option<u64>,
    #[serde(rename = "completionTokens")]
    pub completion_tokens: Option<u64>,
    #[serde(rename = "cachedTokens")]
    pub cached_tokens: Option<u64>,
    #[serde(rename = "reasoningTokens")]
    pub reasoning_tokens: Option<u64>,
    #[serde(rename = "totalTokens")]
    pub total_tokens: Option<u64>,
    #[serde(rename = "cacheRate")]
    pub cache_rate: String,
    #[serde(rename = "cost", default)]
    pub cost: Option<f64>,
    #[serde(rename = "conversationId", default)]
    pub conversation_id: Option<String>,
    #[serde(rename = "conversationName", default)]
    pub conversation_name: Option<String>,
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
    #[serde(rename = "totalCost", default)]
    pub total_cost: Option<f64>,
    #[serde(rename = "costUnknown", default)]
    pub cost_unknown: u64,
    #[serde(rename = "byConversation")]
    pub by_conversation: Vec<ConversationStats>,
    #[serde(rename = "recentRequests")]
    pub recent_requests: Vec<RecentRequest>,
    #[serde(rename = "recentRequestTotal", default)]
    pub recent_request_total: usize,
}

#[derive(Debug, Serialize)]
pub struct ModelStats {
    pub total: u64,
    pub ok: u64,
    #[serde(rename = "promptTokens")]
    pub prompt_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "cachedTokens")]
    pub cached_tokens: u64,
    #[serde(rename = "reasoningTokens")]
    pub reasoning_tokens: u64,
    /// Summed `costTotal` of countable usage rows; `None` when no row carried
    /// a price (mirrors the global `totalCost` semantics per dimension).
    #[serde(rename = "cost", default)]
    pub cost: Option<f64>,
    #[serde(rename = "cacheRate", default)]
    pub cache_rate: String,
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
    reasoning: u64,
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
            reasoning: entry.reasoning_tokens.unwrap_or(0),
        }),
        _ => None,
    }
}

/// Parse an RFC3339 timestamp into epoch milliseconds. Returns `None` for
/// missing or unparseable input.
fn ts_epoch_ms(ts: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp_millis() as u64)
}

/// Whether an entry falls inside the optional time window (inclusive from,
/// exclusive to). Without a window every entry is inside; with a window,
/// entries whose `ts` is missing or unparseable are outside.
fn in_window(entry: &RequestLogEntry, window: Option<(u64, u64)>) -> bool {
    let Some((from_ms, to_ms)) = window else {
        return true;
    };
    let Some(ts) = entry.ts.as_deref() else {
        return false;
    };
    let Some(ts_ms) = ts_epoch_ms(ts) else {
        return false;
    };
    ts_ms >= from_ms && ts_ms < to_ms
}

/// Format a cache hit rate from aggregated input/cached token counts:
/// no input (or nothing to divide) renders `-`; a measured 0% renders
/// `0.0%`; otherwise one-decimal percent (cached / input).
fn cache_rate_of(input: u64, cached: u64) -> String {
    if input == 0 {
        return "-".into();
    }
    if cached == 0 {
        return "0.0%".into();
    }
    format!("{:.1}%", (cached as f64 / input as f64) * 100.0)
}

/// Descending comparator for optional RFC3339 timestamps: newer first,
/// missing timestamps sorted last.
fn cmp_ts_desc(a: &Option<String>, b: &Option<String>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => y.cmp(x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Aggregate request-log entries into a `UsageStats`. Pure: all inputs are
/// injected (entries, circuit state, cooldown, current time, optional
/// time window), no I/O. The window is `(from_ms, to_ms)` in epoch
/// milliseconds, inclusive of `from`, exclusive of `to`; entries outside
/// (or with missing/unparseable timestamps) are excluded from every
/// aggregate. `None` keeps full-history behaviour.
// The aggregation core is `aggregate_paged`; this convenience wrapper is used
// by the test suite (and kept as the default-view entry point), so allow it
// as dead code in non-test builds.
#[cfg_attr(not(test), allow(dead_code))]
pub fn aggregate(
    entries: &[RequestLogEntry],
    circuit: &HashMap<String, CircuitBreakerEntry>,
    cooldown_ms: u64,
    now_ms: u64,
    window: Option<(u64, u64)>,
) -> UsageStats {
    // Default view: first page, 100 rows (legacy behaviour).
    aggregate_paged(entries, circuit, cooldown_ms, now_ms, window, 0, 100)
}

/// One request-detail row for a log entry: token fields only from countable
/// usage, otherwise null with a "-" rate. Shared by the stats page and the
/// per-conversation request browser so both render identically.
fn request_detail(entry: &RequestLogEntry) -> RecentRequest {
    let mut detail = RecentRequest {
        ts: entry.ts.clone(),
        provider: entry.provider.clone(),
        model: entry.model.clone(),
        ok: entry.ok,
        status: entry.status,
        error: entry.error.clone(),
        prompt_tokens: None,
        completion_tokens: None,
        cached_tokens: None,
        reasoning_tokens: None,
        total_tokens: None,
        cache_rate: "-".into(),
        cost: entry.cost_total,
        conversation_id: entry.conversation_id.clone(),
        conversation_name: entry.conversation_name.clone(),
    };
    if let Some(u) = usage_of(entry) {
        detail.prompt_tokens = Some(u.prompt);
        detail.completion_tokens = Some(u.completion);
        detail.cached_tokens = Some(u.cached);
        detail.reasoning_tokens = Some(u.reasoning);
        detail.total_tokens = Some(u.prompt + u.completion);
        detail.cache_rate = cache_rate_of(u.prompt, u.cached);
    }
    detail
}

/// Aggregate request-log entries into usage stats, paging the recent-request
/// details list. `page` is 0-based, `limit` is the per-page row count; the
/// response always carries the full in-window row count as `recentRequestTotal`.
pub fn aggregate_paged(
    entries: &[RequestLogEntry],
    circuit: &HashMap<String, CircuitBreakerEntry>,
    cooldown_ms: u64,
    now_ms: u64,
    window: Option<(u64, u64)>,
    page: usize,
    limit: usize,
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
            cached: 0,
            reasoning: 0,
        },
        cache_hit_rate: "-".into(),
        total_cost: None,
        cost_unknown: 0,
        by_conversation: Vec::new(),
        recent_requests: Vec::new(),
        recent_request_total: 0,
    };

    let mut total_ms: u64 = 0;
    let mut latency_count: u64 = 0;
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_cached: u64 = 0;
    let mut total_reasoning: u64 = 0;
    let mut total_cost: Option<f64> = None;
    let mut cost_unknown: u64 = 0;
    let mut conversations: HashMap<String, ConversationStats> = HashMap::new();

    for entry in entries {
        if !in_window(entry, window) {
            continue;
        }
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
            total_reasoning += u.reasoning;
            // Cost follows the same scope as token usage: only successful,
            // non-retried rows with usage contribute. Rows without a price are
            // counted as unknown so the UI can hint at incomplete data.
            match entry.cost_total {
                Some(c) => total_cost = Some(total_cost.unwrap_or(0.0) + c),
                None => cost_unknown += 1,
            }
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
                name: None,
                requests: 0,
                input_tokens: 0,
                output_tokens: 0,
                cached_tokens: 0,
                reasoning_tokens: 0,
                last_active: None,
                cache_rate: "-".into(),
                cost: None,
            });
        conv.requests += 1;
        // Name: newest named row in the window wins (log order is
        // chronological). Never a grouping key — ADR-0002.
        if let Some(name) = entry.conversation_name.as_deref().filter(|s| !s.is_empty()) {
            conv.name = Some(name.to_string());
        }
        if let Some(ts) = entry.ts.as_deref() {
            if conv.last_active.as_deref().is_none_or(|last| ts > last) {
                conv.last_active = Some(ts.to_string());
            }
        }
        if let Some(u) = &usage {
            conv.input_tokens += u.prompt;
            conv.output_tokens += u.completion;
            conv.cached_tokens += u.cached;
            conv.reasoning_tokens += u.reasoning;
            if let Some(c) = entry.cost_total {
                conv.cost = Some(conv.cost.unwrap_or(0.0) + c);
            }
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
                reasoning_tokens: 0,
                cost: None,
                cache_rate: "-".into(),
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
            ps.reasoning_tokens += u.reasoning;
            if let Some(c) = entry.cost_total {
                ps.cost = Some(ps.cost.unwrap_or(0.0) + c);
            }
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
            .or_insert(ModelStats {
                total: 0,
                ok: 0,
                prompt_tokens: 0,
                output_tokens: 0,
                cached_tokens: 0,
                reasoning_tokens: 0,
                cost: None,
                cache_rate: "-".into(),
            });
        ms.total += 1;
        if entry.ok.unwrap_or(false) {
            ms.ok += 1;
        }
        if let Some(u) = &usage {
            ms.prompt_tokens += u.prompt;
            ms.output_tokens += u.completion;
            ms.cached_tokens += u.cached;
            ms.reasoning_tokens += u.reasoning;
            if let Some(c) = entry.cost_total {
                ms.cost = Some(ms.cost.unwrap_or(0.0) + c);
            }
        }

        // Latency
        if let Some(ms) = entry.ms {
            total_ms += ms;
            latency_count += 1;
        }

        // Per-request detail rows: every in-window entry gets one row; token
        // fields only from countable usage, otherwise null with a "-" rate.
        let detail = request_detail(entry);
        stats.recent_requests.push(detail);
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
        cached: total_cached,
        reasoning: total_reasoning,
    };
    stats.total_cost = total_cost;
    stats.cost_unknown = cost_unknown;
    stats.cache_hit_rate = if total_cached == 0 {
        "-".into()
    } else {
        format!("{:.1}%", (total_cached as f64 / total_input as f64) * 100.0)
    };

    let mut by_conversation: Vec<ConversationStats> = conversations.into_values().collect();
    for conv in &mut by_conversation {
        conv.cache_rate = cache_rate_of(conv.input_tokens, conv.cached_tokens);
    }
    for ps in stats.by_provider.values_mut() {
        ps.cache_rate = cache_rate_of(ps.prompt_tokens, ps.cached_tokens);
    }
    for ms in stats.by_model.values_mut() {
        ms.cache_rate = cache_rate_of(ms.prompt_tokens, ms.cached_tokens);
    }
    by_conversation.sort_by(|a, b| cmp_ts_desc(&a.last_active, &b.last_active));
    by_conversation.truncate(20);
    stats.by_conversation = by_conversation;

    stats
        .recent_requests
        .sort_by(|a, b| cmp_ts_desc(&a.ts, &b.ts));
    let recent_request_total = stats.recent_requests.len();
    let start = page.saturating_mul(limit);
    stats.recent_requests = stats
        .recent_requests
        .into_iter()
        .skip(start)
        .take(limit)
        .collect();
    stats.recent_request_total = recent_request_total;

    stats
}

/// Parse `/stats` window query parameters.
///
/// Accepts `range=today|last24h|last7d|custom` plus `from`/`to` epoch-millis
/// (left-inclusive, right-exclusive). The backend never computes timezone
/// windows itself — the caller passes the resolved window. Any window request
/// (a `range`, or `from`/`to`) requires both bounds; a bare request with no
/// window parameters at all yields `Ok(None)` (full history).
pub fn parse_window_query(
    range: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Option<(u64, u64)>, String> {
    if range.is_none() && from.is_none() && to.is_none() {
        return Ok(None);
    }
    if let Some(range) = range {
        if !matches!(range, "today" | "last24h" | "last7d" | "custom") {
            return Err(format!("invalid range: {range}"));
        }
    }
    let (Some(from), Some(to)) = (from, to) else {
        return Err("window requires both from and to (epoch millis)".to_string());
    };
    let from_ms = from
        .parse::<u64>()
        .map_err(|_| format!("invalid from: {from}"))?;
    let to_ms = to.parse::<u64>().map_err(|_| format!("invalid to: {to}"))?;
    if from_ms >= to_ms {
        return Err(format!(
            "invalid window: from ({from_ms}) must be < to ({to_ms})"
        ));
    }
    Ok(Some((from_ms, to_ms)))
}

pub fn get_stats(window: Option<(u64, u64)>) -> UsageStats {
    get_stats_paged(window, None, None)
}

/// Usage stats with paged recent-request details: `page` (0-based) and
/// `limit` (rows per page) default to 0 and 100 when omitted.
pub fn get_stats_paged(
    window: Option<(u64, u64)>,
    page: Option<usize>,
    limit: Option<usize>,
) -> UsageStats {
    let entries = parse_logs();
    // Read circuit breaker state
    let circuit_entries = read_circuit_state();
    let cooldown_ms = 60_000; // Default 60 seconds
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    aggregate_paged(
        &entries,
        &circuit_entries,
        cooldown_ms,
        now_ms,
        window,
        page.unwrap_or(0),
        limit.unwrap_or(100),
    )
}

/// Aggregate request-log entries into per-conversation stats, paging the
/// result. Mirrors the conversation grouping inside `aggregate_paged` but
/// returns every conversation in the window (no top-20 cap) together with the
/// full in-window total, so the UI can browse any historical window
/// independently of the main stats. `None` window keeps full history.
pub fn aggregate_conversations_paged(
    entries: &[RequestLogEntry],
    window: Option<(u64, u64)>,
    page: usize,
    limit: usize,
) -> (Vec<ConversationStats>, usize) {
    let mut conversations: HashMap<String, ConversationStats> = HashMap::new();
    for entry in entries {
        if !in_window(entry, window) {
            continue;
        }
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
                name: None,
                requests: 0,
                input_tokens: 0,
                output_tokens: 0,
                cached_tokens: 0,
                reasoning_tokens: 0,
                last_active: None,
                cache_rate: "-".into(),
                cost: None,
            });
        conv.requests += 1;
        // Name: newest named row in the window wins (log order is
        // chronological). Never a grouping key — ADR-0002.
        if let Some(name) = entry.conversation_name.as_deref().filter(|s| !s.is_empty()) {
            conv.name = Some(name.to_string());
        }
        if let Some(ts) = entry.ts.as_deref() {
            if conv.last_active.as_deref().is_none_or(|last| ts > last) {
                conv.last_active = Some(ts.to_string());
            }
        }
        if let Some(u) = usage_of(entry) {
            conv.input_tokens += u.prompt;
            conv.output_tokens += u.completion;
            conv.cached_tokens += u.cached;
            conv.reasoning_tokens += u.reasoning;
            if let Some(c) = entry.cost_total {
                conv.cost = Some(conv.cost.unwrap_or(0.0) + c);
            }
        }
    }

    let mut rows: Vec<ConversationStats> = conversations.into_values().collect();
    for conv in &mut rows {
        conv.cache_rate = cache_rate_of(conv.input_tokens, conv.cached_tokens);
    }
    rows.sort_by(|a, b| cmp_ts_desc(&a.last_active, &b.last_active));
    let total = rows.len();
    let start = page.saturating_mul(limit);
    rows = rows.into_iter().skip(start).take(limit).collect();
    (rows, total)
}

/// Per-conversation stats over the request log, paged: `page` (0-based) and
/// `limit` (rows per page) default to 0 and 100 when omitted. `None` window
/// keeps full history (All-time).
pub fn get_conversations_paged(
    window: Option<(u64, u64)>,
    page: Option<usize>,
    limit: Option<usize>,
) -> (Vec<ConversationStats>, usize) {
    let entries = parse_logs();
    aggregate_conversations_paged(&entries, window, page.unwrap_or(0), limit.unwrap_or(100))
}

/// Whether a log entry belongs to the given conversation key: entries with a
/// non-empty `conversation_id` match that id; `"unlabeled"` matches entries
/// without one (mirrors the conversation-aggregation key semantics).
fn matches_conversation(entry: &RequestLogEntry, conversation_id: &str) -> bool {
    match entry.conversation_id.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => id == conversation_id,
        None => conversation_id == "unlabeled",
    }
}

/// All request-detail rows of one conversation (full history, no window),
/// newest first, paged: `page` (0-based) / `limit` (rows per page). Returns
/// `(rows, total)` where total is the full conversation row count.
pub fn aggregate_conversation_requests(
    entries: &[RequestLogEntry],
    conversation_id: &str,
    page: usize,
    limit: usize,
) -> (Vec<RecentRequest>, usize) {
    let mut rows: Vec<RecentRequest> = entries
        .iter()
        .filter(|e| matches_conversation(e, conversation_id))
        .map(request_detail)
        .collect();
    rows.sort_by(|a, b| cmp_ts_desc(&a.ts, &b.ts));
    let total = rows.len();
    let start = page.saturating_mul(limit);
    rows = rows.into_iter().skip(start).take(limit).collect();
    (rows, total)
}

/// All request-detail rows of one conversation over the request log, paged:
/// `page` (0-based) and `limit` (rows per page) default to 0 and 100.
pub fn get_conversation_requests(
    conversation_id: &str,
    page: Option<usize>,
    limit: Option<usize>,
) -> (Vec<RecentRequest>, usize) {
    let entries = parse_logs();
    aggregate_conversation_requests(
        &entries,
        conversation_id,
        page.unwrap_or(0),
        limit.unwrap_or(100),
    )
}

pub fn export_logs_json() -> crate::error::Result<String> {
    let entries = parse_logs();
    serde_json::to_string_pretty(&entries)
        .map_err(|e| crate::error::AppError::Message(format!("Failed to serialize logs: {}", e)))
}

fn csv_of(entries: &[RequestLogEntry]) -> String {
    let mut csv = String::from(
        "timestamp,ok,provider,model,status,latency_ms,error,retry,skipped,converted,upstream_url,promptTokens,completionTokens,cachedTokens,reasoningTokens,conversationId,costTotal\n",
    );

    for entry in entries {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
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
                .reasoning_tokens
                .map(|t| t.to_string())
                .unwrap_or_default(),
            entry
                .conversation_id
                .as_deref()
                .unwrap_or("")
                .replace(',', ";")
                .replace('\n', " "),
            entry.cost_total.map(|c| c.to_string()).unwrap_or_default(),
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
            reasoning_tokens: None,
            conversation_id: None,
            conversation_name: None,
            cost_total: None,
        }
    }

    fn with_usage(mut e: RequestLogEntry, p: u64, c: u64, cached: u64) -> RequestLogEntry {
        e.prompt_tokens = Some(p);
        e.completion_tokens = Some(c);
        e.cached_tokens = Some(cached);
        e.reasoning_tokens = Some(0);
        e
    }

    fn with_reasoning(mut e: RequestLogEntry, r: u64) -> RequestLogEntry {
        e.reasoning_tokens = Some(r);
        e
    }

    #[test]
    fn aggregate_empty_entries_yields_zero_stats() {
        let stats = aggregate(&[], &HashMap::new(), 60_000, 0, None);
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
        assert!(stats.recent_requests.is_empty());
    }

    #[test]
    fn aggregate_sums_known_cost_and_counts_unknown_rows() {
        let mut known1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        known1.cost_total = Some(0.25);
        let mut known2 = with_usage(
            entry(true, "hyb", "m2", 20, "2026-08-02T10:00:01Z"),
            200,
            20,
            0,
        );
        known2.cost_total = Some(0.5);
        let unknown = with_usage(
            entry(true, "hyb", "m3", 30, "2026-08-02T10:00:02Z"),
            300,
            30,
            0,
        );

        // Failed and retried rows are outside the token/cost scope: their cost
        // (even when present) must not leak into totals.
        let mut failed = entry(false, "hyb", "m1", 40, "2026-08-02T10:00:03Z");
        failed.cost_total = Some(9.9);
        let mut retry = with_usage(
            entry(true, "hyb", "m1", 50, "2026-08-02T10:00:04Z"),
            100,
            10,
            0,
        );
        retry.retry = Some(true);
        retry.cost_total = Some(9.9);

        let stats = aggregate(
            &[known1, known2, unknown, failed, retry],
            &HashMap::new(),
            60_000,
            0,
            None,
        );
        assert_eq!(stats.total_cost, Some(0.75));
        assert_eq!(stats.cost_unknown, 1);
    }

    #[test]
    fn aggregate_total_cost_is_none_when_no_known_cost_rows() {
        let stats = aggregate(
            &[with_usage(
                entry(true, "hyb", "m3", 10, "2026-08-02T10:00:00Z"),
                100,
                10,
                0,
            )],
            &HashMap::new(),
            60_000,
            0,
            None,
        );
        assert_eq!(stats.total_cost, None, "all-unknown window shows no total");
        assert_eq!(stats.cost_unknown, 1);
    }

    #[test]
    fn aggregate_provider_cost_sums_known_rows_and_stays_none_when_all_unknown() {
        let mut known1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            200,
        );
        known1.cost_total = Some(0.25);
        let mut known2 = with_usage(
            entry(true, "hyb", "m2", 20, "2026-08-02T10:00:01Z"),
            200,
            20,
            200,
        );
        known2.cost_total = Some(0.5);
        // 未知 cost 行：计入请求数但不贡献 cost
        let unknown = with_usage(
            entry(true, "hyb", "m3", 30, "2026-08-02T10:00:02Z"),
            300,
            30,
            0,
        );
        // failed 行即使带 cost 也不贡献（usage_of 口径）
        let mut failed = entry(false, "hyb", "m1", 40, "2026-08-02T10:00:03Z");
        failed.cost_total = Some(9.9);
        // 全未知 provider
        let unknown_prov = with_usage(
            entry(true, "other", "m1", 50, "2026-08-02T10:00:04Z"),
            100,
            10,
            0,
        );
        // 只有 failed 行（无 usage）的 provider：token 全零 → 缓存率 `-`
        let failed_only = entry(false, "flaky", "m1", 60, "2026-08-02T10:00:05Z");

        let stats = aggregate(
            &[known1, known2, unknown, failed, unknown_prov, failed_only],
            &HashMap::new(),
            60_000,
            0,
            None,
        );
        let hyb = &stats.by_provider["hyb"];
        assert_eq!(hyb.cost, Some(0.75), "provider sums its known usage rows");
        assert_eq!(hyb.total, 4, "failed row still counts toward requests");
        assert_eq!(
            hyb.cache_rate, "66.7%",
            "provider cache rate = cached / input (400/600)"
        );
        let other = &stats.by_provider["other"];
        assert_eq!(other.cost, None, "all-unknown provider shows no cost");
        assert_eq!(other.cache_rate, "0.0%", "input without cache renders 0.0%");
        assert_eq!(
            stats.by_provider["flaky"].cache_rate, "-",
            "no countable usage renders a dash"
        );
    }

    #[test]
    fn aggregate_model_stats_carry_token_detail_cache_rate_and_cost() {
        let mut m1a = with_usage(
            entry(true, "hyb", "deepseek-r", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            50,
        );
        m1a.cost_total = Some(0.25);
        let mut m1b = with_usage(
            entry(true, "hyb", "deepseek-r", 20, "2026-08-02T10:00:01Z"),
            200,
            20,
            50,
        );
        m1b.cost_total = Some(0.5);
        let m2 = with_usage(
            entry(true, "hyb", "gpt-x", 30, "2026-08-02T10:00:02Z"),
            300,
            30,
            0,
        );
        let failed = entry(false, "hyb", "deepseek-r", 40, "2026-08-02T10:00:03Z");

        let stats = aggregate(&[m1a, m1b, m2, failed], &HashMap::new(), 60_000, 0, None);
        let ms = &stats.by_model["deepseek-r"];
        assert_eq!(ms.total, 3, "failed row counts toward requests");
        assert_eq!(ms.ok, 2);
        assert_eq!(ms.prompt_tokens, 300, "100 + 200");
        assert_eq!(ms.output_tokens, 30, "10 + 20");
        assert_eq!(ms.cached_tokens, 100, "50 + 50");
        assert_eq!(ms.reasoning_tokens, 0);
        assert_eq!(ms.cost, Some(0.75), "model sums its known usage rows");
        assert_eq!(ms.cache_rate, "33.3%", "100 cached / 300 input");
        let g = &stats.by_model["gpt-x"];
        assert_eq!(g.cost, None, "unknown-cost model shows no cost");
        assert_eq!(g.cache_rate, "0.0%", "input without cache");
    }

    #[test]
    fn aggregate_conversation_cost_sums_known_rows_and_stays_none_when_all_unknown() {
        let mut priced1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        priced1.conversation_id = Some("conv-a".into());
        priced1.cost_total = Some(0.25);
        let mut priced2 = with_usage(
            entry(true, "hyb", "m2", 20, "2026-08-02T10:00:01Z"),
            200,
            20,
            0,
        );
        priced2.conversation_id = Some("conv-a".into());
        priced2.cost_total = Some(0.5);
        let mut unknown = with_usage(
            entry(true, "hyb", "m3", 30, "2026-08-02T10:00:02Z"),
            300,
            30,
            0,
        );
        unknown.conversation_id = Some("conv-b".into());

        let stats = aggregate(
            &[priced1, priced2, unknown],
            &HashMap::new(),
            60_000,
            0,
            None,
        );
        let by_id = |id: &str| {
            stats
                .by_conversation
                .iter()
                .find(|c| c.conversation_id == id)
                .unwrap()
        };
        assert_eq!(
            by_id("conv-a").cost,
            Some(0.75),
            "conversation sums its known rows"
        );
        assert_eq!(
            by_id("conv-b").cost,
            None,
            "all-unknown conversation shows no cost"
        );
    }

    #[test]
    fn aggregate_single_success_entry_counts_everywhere() {
        let stats = aggregate(
            &[entry(true, "hyb", "gpt-5.4", 100, "2026-08-02T10:00:00Z")],
            &HashMap::new(),
            60_000,
            0,
            None,
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
        let ms = &stats.by_model["gpt-5.4"];
        assert_eq!((ms.total, ms.ok), (1, 1));
    }

    #[test]
    fn aggregate_recent_requests_carry_cost_or_none() {
        let mut priced = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        priced.cost_total = Some(0.25);
        let unknown = with_usage(
            entry(true, "hyb", "m2", 20, "2026-08-02T10:00:01Z"),
            200,
            20,
            0,
        );

        let stats = aggregate(&[priced, unknown], &HashMap::new(), 60_000, 0, None);
        let by_ts = |ts: &str| {
            stats
                .recent_requests
                .iter()
                .find(|r| r.ts.as_deref() == Some(ts))
                .unwrap()
        };
        assert_eq!(by_ts("2026-08-02T10:00:00Z").cost, Some(0.25));
        assert_eq!(
            by_ts("2026-08-02T10:00:01Z").cost,
            None,
            "unknown row shows no cost"
        );
    }

    #[test]
    fn request_log_entry_deserializes_legacy_rows_without_cost_total() {
        let legacy = r#"{"ts":"2026-08-02T10:00:00Z","ok":true,"provider":"hyb","model":"m1","promptTokens":100,"completionTokens":10}"#;
        let parsed: RequestLogEntry = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed.cost_total, None, "old rows parse with unknown cost");

        let with_cost = r#"{"ok":true,"costTotal":0.25}"#;
        let parsed: RequestLogEntry = serde_json::from_str(with_cost).unwrap();
        assert_eq!(parsed.cost_total, Some(0.25));
        let with_cost = r#"{"ok":true,"costTotal":0.25}"#;
        let parsed: RequestLogEntry = serde_json::from_str(with_cost).unwrap();
        assert_eq!(parsed.cost_total, Some(0.25));
    }

    #[test]
    fn request_log_entry_deserializes_legacy_rows_without_conversation_name() {
        let legacy = r#"{"ts":"2026-08-02T10:00:00Z","ok":true,"provider":"hyb","model":"m1","conversationId":"conv-1"}"#;
        let parsed: RequestLogEntry = serde_json::from_str(legacy).unwrap();
        assert_eq!(
            parsed.conversation_name, None,
            "old rows parse with no name"
        );

        let named = r#"{"ok":true,"conversationName":"my-chat"}"#;
        let parsed: RequestLogEntry = serde_json::from_str(named).unwrap();
        assert_eq!(parsed.conversation_name.as_deref(), Some("my-chat"));
    }

    #[test]
    fn aggregate_conversation_name_comes_from_newest_named_row() {
        let mut old = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        old.conversation_id = Some("conv-a".into());
        old.conversation_name = Some("old-name".into());
        let mut mid = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            100,
            10,
            0,
        );
        mid.conversation_id = Some("conv-a".into());
        mid.conversation_name = Some("new-name".into());
        let mut latest = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:02Z"),
            100,
            10,
            0,
        );
        latest.conversation_id = Some("conv-a".into());

        let stats = aggregate(&[old, mid, latest], &HashMap::new(), 60_000, 0, None);
        let conv_a = stats
            .by_conversation
            .iter()
            .find(|c| c.conversation_id == "conv-a")
            .unwrap();
        assert_eq!(
            conv_a.name.as_deref(),
            Some("new-name"),
            "newest named row wins"
        );

        let mut unnamed = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:03Z"),
            100,
            10,
            0,
        );
        unnamed.conversation_id = Some("conv-b".into());
        let stats = aggregate(&[unnamed], &HashMap::new(), 60_000, 0, None);
        let conv_b = stats
            .by_conversation
            .iter()
            .find(|c| c.conversation_id == "conv-b")
            .unwrap();
        assert_eq!(conv_b.name, None, "no named rows -> null");
    }

    #[test]
    fn aggregate_conversation_grouping_ignores_names() {
        let mut a1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        a1.conversation_id = Some("conv-a".into());
        a1.conversation_name = Some("shared-name".into());
        let mut a2 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            100,
            10,
            0,
        );
        a2.conversation_id = Some("conv-a".into());
        a2.conversation_name = Some("renamed".into());
        let mut b = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:02Z"),
            100,
            10,
            0,
        );
        b.conversation_id = Some("conv-b".into());
        b.conversation_name = Some("shared-name".into());

        let stats = aggregate(&[a1, a2, b], &HashMap::new(), 60_000, 0, None);
        assert_eq!(
            stats.by_conversation.len(),
            2,
            "name never merges or splits groups"
        );
        let conv_a = stats
            .by_conversation
            .iter()
            .find(|c| c.conversation_id == "conv-a")
            .unwrap();
        assert_eq!(conv_a.requests, 2, "same id stays one group");
        assert_eq!(conv_a.name.as_deref(), Some("renamed"));
    }

    #[test]
    fn csv_of_includes_cost_total_column() {
        let mut priced = entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z");
        priced.cost_total = Some(0.25);
        let legacy = entry(true, "hyb", "m2", 20, "2026-08-02T10:00:01Z");

        let csv = csv_of(&[priced, legacy]);
        let header = csv.lines().next().unwrap();
        assert!(header.ends_with("costTotal"), "header has costTotal column");
        let rows: Vec<&str> = csv.lines().skip(1).collect();
        assert_eq!(
            rows[0].split(',').next_back(),
            Some("0.25"),
            "known cost exported"
        );
        assert_eq!(
            rows[1].split(',').next_back(),
            Some(""),
            "legacy row exports empty cost"
        );
    }

    #[test]
    fn export_logs_json_serializes_cost_total() {
        let mut priced = entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z");
        priced.cost_total = Some(0.25);
        let json = serde_json::to_value(&[priced]).unwrap();
        assert_eq!(json[0]["costTotal"], 0.25);

        let legacy = entry(true, "hyb", "m2", 20, "2026-08-02T10:00:01Z");
        let json = serde_json::to_value(&[legacy]).unwrap();
        assert_eq!(
            json[0]["costTotal"],
            serde_json::Value::Null,
            "legacy row has null cost"
        );
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

        let stats = aggregate(&[], &circuit, 60_000, 1_030_000, None);

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
            None,
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
            None,
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

        let stats = aggregate(&[a, b], &HashMap::new(), 60_000, 0, None);

        assert_eq!(stats.cache_hit_rate, "53.3%", "160 cached / 300 input");
    }

    #[test]
    fn aggregate_cache_rate_is_dash_without_any_token_data() {
        let empty = aggregate(&[], &HashMap::new(), 60_000, 0, None);
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
            None,
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

        let stats = aggregate(&[a, b], &HashMap::new(), 60_000, 0, None);

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
        let stats = aggregate(&[], &HashMap::new(), 60_000, 0, None);
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["byConversation"], serde_json::json!([]));
        assert_eq!(json["cacheHitRate"], "-");
        assert_eq!(json["totalTokens"]["total"], 0);
        assert_eq!(json["recentRequests"], serde_json::json!([]));
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
            None,
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
    fn aggregate_sums_reasoning_into_all_dimensions() {
        let a = with_reasoning(
            with_usage(
                entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:00Z"),
                100,
                50,
                40,
            ),
            20,
        );
        let b = with_reasoning(
            with_usage(
                entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:01Z"),
                200,
                60,
                120,
            ),
            30,
        );
        let mut conv_b = with_reasoning(
            with_usage(
                entry(true, "fox", "claude-sonnet", 0, "2026-08-02T10:00:02Z"),
                10,
                20,
                30,
            ),
            5,
        );
        conv_b.conversation_id = Some("conv-b".into());
        let failed = with_reasoning(
            with_usage(
                entry(false, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:03Z"),
                300,
                70,
                30,
            ),
            99,
        );

        let stats = aggregate(&[a, b, conv_b, failed], &HashMap::new(), 60_000, 0, None);

        assert_eq!(
            (
                stats.total_tokens.input,
                stats.total_tokens.output,
                stats.total_tokens.total
            ),
            (310, 130, 440),
            "reasoning is a subset of output, never added to total"
        );
        assert_eq!(stats.total_tokens.cached, 190, "40 + 120 + 30");
        assert_eq!(
            stats.total_tokens.reasoning, 55,
            "20 + 30 + 5; failed row excluded"
        );

        let hyb = &stats.by_provider["hyb"];
        assert_eq!(hyb.reasoning_tokens, 50, "20 + 30");
        let fox = &stats.by_provider["fox"];
        assert_eq!(fox.reasoning_tokens, 5);

        let conv_b = stats
            .by_conversation
            .iter()
            .find(|c| c.conversation_id == "conv-b")
            .expect("conv-b present");
        assert_eq!((conv_b.cached_tokens, conv_b.reasoning_tokens), (30, 5));
        let unlabeled = stats
            .by_conversation
            .iter()
            .find(|c| c.conversation_id == "unlabeled")
            .expect("unlabeled present");
        assert_eq!(
            (
                unlabeled.input_tokens,
                unlabeled.output_tokens,
                unlabeled.cached_tokens,
                unlabeled.reasoning_tokens
            ),
            (300, 110, 160, 50)
        );
    }

    #[test]
    fn aggregate_reasoning_defaults_to_zero_for_old_rows_without_field() {
        let old_row = entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:00Z");
        let stats = aggregate(&[old_row], &HashMap::new(), 60_000, 0, None);
        assert_eq!(stats.total_tokens.reasoning, 0);
        assert_eq!(stats.by_provider["hyb"].reasoning_tokens, 0);
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
            None,
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

        let stats = aggregate(&entries, &HashMap::new(), 60_000, 0, None);

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
        let mut e = with_reasoning(
            with_usage(
                entry(true, "hyb", "gpt-5.4", 12, "2026-08-02T10:00:00Z"),
                100,
                50,
                40,
            ),
            20,
        );
        e.conversation_id = Some("conv-9".into());
        let csv = csv_of(&[e]);
        let mut lines = csv.lines();
        let header = lines.next().unwrap();
        assert!(
            header.ends_with(
                "upstream_url,promptTokens,completionTokens,cachedTokens,reasoningTokens,conversationId,costTotal"
            ),
            "header: {header}"
        );
        let row = lines.next().unwrap();
        assert!(row.ends_with(",100,50,40,20,conv-9,"), "row: {row}");
        assert!(lines.next().is_none(), "exactly one data row");
    }

    #[test]
    fn csv_export_escapes_conversation_id_and_omits_missing_tokens() {
        let mut e = entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:00Z");
        e.conversation_id = Some("a,b\nc".into());
        let csv = csv_of(&[e]);
        let row = csv.lines().nth(1).unwrap();
        assert!(row.ends_with(",,,,a;b c,"), "got: {row}");
    }
    #[test]
    fn json_export_serializes_token_and_conversation_fields() {
        let mut e = with_reasoning(
            with_usage(entry(true, "hyb", "gpt-5.4", 12, "t"), 100, 50, 40),
            20,
        );
        e.conversation_id = Some("conv-9".into());
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"promptTokens\":100"));
        assert!(json.contains("\"completionTokens\":50"));
        assert!(json.contains("\"cachedTokens\":40"));
        assert!(json.contains("\"reasoningTokens\":20"));
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
        assert_eq!(entries[0].reasoning_tokens, None);
        assert_eq!(entries[0].conversation_id, None);
    }

    #[test]
    fn parse_entries_reads_reasoning_tokens_field() {
        let text =
            "{\"ok\":true,\"provider\":\"hyb\",\"promptTokens\":100,\"completionTokens\":50,\"cachedTokens\":40,\"reasoningTokens\":20}\n";
        let entries = parse_entries(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].reasoning_tokens, Some(20));
    }

    #[test]
    fn aggregate_recent_requests_carry_full_usage_fields() {
        let row = with_reasoning(
            with_usage(
                entry(true, "hyb", "gpt-5.4", 100, "2026-08-02T10:00:00Z"),
                1234,
                567,
                890,
            ),
            100,
        );

        let stats = aggregate(&[row], &HashMap::new(), 60_000, 0, None);

        assert_eq!(stats.recent_requests.len(), 1);
        let r = &stats.recent_requests[0];
        assert_eq!(r.ts.as_deref(), Some("2026-08-02T10:00:00Z"));
        assert_eq!(r.provider.as_deref(), Some("hyb"));
        assert_eq!(r.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(r.ok, Some(true));
        assert_eq!(r.status, Some(200));
        assert_eq!(r.error, None);
        assert_eq!(
            (
                r.prompt_tokens,
                r.completion_tokens,
                r.cached_tokens,
                r.reasoning_tokens,
            ),
            (Some(1234), Some(567), Some(890), Some(100))
        );
        assert_eq!(
            r.total_tokens,
            Some(1801),
            "input + output, reasoning not double-counted"
        );
        assert_eq!(r.cache_rate, "72.1%", "890 cached / 1234 input");
    }

    #[test]
    fn aggregate_recent_requests_serialize_with_camel_case_keys() {
        let row = with_usage(
            entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:00Z"),
            100,
            50,
            40,
        );
        let stats = aggregate(&[row], &HashMap::new(), 60_000, 0, None);
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["recentRequests"][0]["promptTokens"], 100);
        assert_eq!(json["recentRequests"][0]["totalTokens"], 150);
        assert_eq!(json["recentRequests"][0]["cacheRate"], "40.0%");
        assert_eq!(json["recentRequests"][0]["ts"], "2026-08-02T10:00:00Z");
    }

    #[test]
    fn aggregate_recent_requests_cache_rate_three_states() {
        let zero_cache = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        let zero_input = with_usage(entry(true, "hyb", "m1", 0, "2026-08-02T10:00:01Z"), 0, 0, 0);
        let normal = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:02Z"),
            1234,
            567,
            890,
        );

        let stats = aggregate(
            &[zero_cache, zero_input, normal],
            &HashMap::new(),
            60_000,
            0,
            None,
        );

        let by_ts = |ts: &str| {
            stats
                .recent_requests
                .iter()
                .find(|r| r.ts.as_deref() == Some(ts))
                .unwrap()
        };
        assert_eq!(
            by_ts("2026-08-02T10:00:00Z").cache_rate,
            "0.0%",
            "cached=0 is a real measurement"
        );
        assert_eq!(
            by_ts("2026-08-02T10:00:01Z").cache_rate,
            "-",
            "input=0 has no rate"
        );
        assert_eq!(by_ts("2026-08-02T10:00:02Z").cache_rate, "72.1%");
    }

    #[test]
    fn aggregate_recent_requests_no_usage_rows_keep_null_token_fields() {
        let mut failed = entry(false, "fox", "claude-sonnet", 0, "2026-08-02T10:00:01Z");
        failed.error = Some("rate limited".into());
        failed.status = Some(429);
        let mut retried = entry(true, "fox", "claude-sonnet", 0, "2026-08-02T10:00:02Z");
        retried.retry = Some(true);
        let old_row = entry(true, "hyb", "gpt-5.4", 0, "2026-08-02T10:00:03Z");

        let stats = aggregate(
            &[failed, retried, old_row],
            &HashMap::new(),
            60_000,
            0,
            None,
        );

        assert_eq!(stats.recent_requests.len(), 3, "no-usage rows still appear");
        for r in &stats.recent_requests {
            assert_eq!(
                (
                    r.prompt_tokens,
                    r.completion_tokens,
                    r.cached_tokens,
                    r.reasoning_tokens,
                    r.total_tokens,
                ),
                (None, None, None, None, None),
                "no usage means null token fields, never 0"
            );
            assert_eq!(r.cache_rate, "-", "no usage means no rate");
        }
        let by_ts = |ts: &str| {
            stats
                .recent_requests
                .iter()
                .find(|r| r.ts.as_deref() == Some(ts))
                .unwrap()
        };
        assert_eq!(by_ts("2026-08-02T10:00:01Z").ok, Some(false));
        assert_eq!(by_ts("2026-08-02T10:00:01Z").status, Some(429));
        assert_eq!(
            by_ts("2026-08-02T10:00:01Z").error.as_deref(),
            Some("rate limited")
        );
    }

    #[test]
    fn aggregate_recent_requests_respect_window() {
        let mut missing_ts = entry(true, "hyb", "m1", 0, "2026-08-02T10:30:00Z");
        missing_ts.ts = None;
        // Window [2026-08-02T10:00:00Z, 2026-08-02T12:00:00Z): from inclusive, to exclusive.
        let window = Some((1_785_664_800_000, 1_785_672_000_000));
        let stats = aggregate(
            &[
                entry(true, "hyb", "m1", 0, "2026-08-02T09:59:59Z"),
                entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z"),
                entry(true, "hyb", "m1", 0, "2026-08-02T11:00:00Z"),
                entry(true, "hyb", "m1", 0, "2026-08-02T12:00:00Z"),
                missing_ts,
            ],
            &HashMap::new(),
            60_000,
            0,
            window,
        );

        let ts: Vec<Option<&str>> = stats
            .recent_requests
            .iter()
            .map(|r| r.ts.as_deref())
            .collect();
        assert_eq!(
            ts,
            vec![Some("2026-08-02T11:00:00Z"), Some("2026-08-02T10:00:00Z")],
            "only in-window rows, missing ts excluded, sorted desc"
        );
    }

    #[test]
    fn aggregate_recent_requests_sort_desc_and_truncate_100() {
        let mut entries: Vec<RequestLogEntry> = (0..100)
            .map(|i| {
                with_usage(
                    entry(
                        true,
                        "hyb",
                        "m1",
                        0,
                        &format!("2026-08-02T10:{:02}:{:02}Z", i / 60, i % 60),
                    ),
                    1,
                    1,
                    0,
                )
            })
            .collect();
        let mut no_ts = entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z");
        no_ts.ts = None;
        entries.push(no_ts);

        let stats = aggregate(&entries, &HashMap::new(), 60_000, 0, None);

        assert_eq!(stats.recent_requests.len(), 100, "capped at 100");
        assert_eq!(
            stats.recent_requests[0].ts.as_deref(),
            Some("2026-08-02T10:01:39Z"),
            "newest ts first"
        );
        assert_eq!(
            stats.recent_requests[99].ts.as_deref(),
            Some("2026-08-02T10:00:00Z"),
            "oldest kept ts last"
        );
        assert!(
            stats.recent_requests.iter().all(|r| r.ts.is_some()),
            "ts-missing row truncated after the 100 kept"
        );
    }

    #[test]
    fn aggregate_recent_requests_window_excluding_everything_yields_empty() {
        let window = Some((1_785_664_800_000, 1_785_672_000_000));
        let stats = aggregate(
            &[
                entry(true, "hyb", "m1", 0, "2026-08-01T00:00:00Z"),
                entry(true, "hyb", "m1", 0, "2026-08-03T00:00:00Z"),
            ],
            &HashMap::new(),
            60_000,
            0,
            window,
        );

        assert!(stats.recent_requests.is_empty(), "window excludes all rows");
    }

    #[test]
    fn aggregate_recent_requests_sort_ts_missing_last() {
        let mut no_ts = entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z");
        no_ts.ts = None;

        let stats = aggregate(
            &[
                entry(true, "hyb", "m1", 0, "2026-08-02T10:00:01Z"),
                no_ts,
                entry(true, "hyb", "m1", 0, "2026-08-02T10:00:02Z"),
            ],
            &HashMap::new(),
            60_000,
            0,
            None,
        );

        assert_eq!(stats.recent_requests.len(), 3);
        assert_eq!(
            stats.recent_requests[0].ts.as_deref(),
            Some("2026-08-02T10:00:02Z")
        );
        assert_eq!(
            stats.recent_requests[1].ts.as_deref(),
            Some("2026-08-02T10:00:01Z")
        );
        assert_eq!(stats.recent_requests[2].ts, None, "missing ts sorted last");
    }

    #[test]
    fn aggregate_conversation_cache_rate_three_states() {
        let mut zero_cached = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        zero_cached.conversation_id = Some("conv-zero-cached".into());
        let mut zero_input =
            with_usage(entry(true, "hyb", "m1", 0, "2026-08-02T10:00:01Z"), 0, 0, 0);
        zero_input.conversation_id = Some("conv-zero-input".into());
        let mut normal = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:02Z"),
            1234,
            567,
            890,
        );
        normal.conversation_id = Some("conv-normal".into());

        let stats = aggregate(
            &[zero_cached, zero_input, normal],
            &HashMap::new(),
            60_000,
            0,
            None,
        );

        let by_id = |id: &str| {
            stats
                .by_conversation
                .iter()
                .find(|c| c.conversation_id == id)
                .unwrap()
        };
        assert_eq!(
            by_id("conv-zero-cached").cache_rate,
            "0.0%",
            "cached=0 measured"
        );
        assert_eq!(by_id("conv-zero-input").cache_rate, "-", "input=0 no rate");
        assert_eq!(by_id("conv-normal").cache_rate, "72.1%");
    }

    #[test]
    fn aggregate_window_keeps_only_entries_between_from_and_to() {
        // Window [2026-08-02T10:00:00Z, 2026-08-02T12:00:00Z):
        // from inclusive, to exclusive.
        let window = Some((1_785_664_800_000, 1_785_672_000_000));
        let stats = aggregate(
            &[
                entry(true, "hyb", "m1", 10, "2026-08-02T09:59:59Z"),
                entry(true, "hyb", "m1", 20, "2026-08-02T10:00:00Z"),
                entry(true, "hyb", "m1", 30, "2026-08-02T11:00:00Z"),
                entry(true, "hyb", "m1", 40, "2026-08-02T12:00:00Z"),
                entry(true, "hyb", "m1", 50, "2026-08-02T12:00:01Z"),
            ],
            &HashMap::new(),
            60_000,
            0,
            window,
        );

        assert_eq!(stats.total_requests, 2, "from inclusive, to exclusive");
        assert_eq!(stats.avg_latency_ms, 25, "(20 + 30) / 2");
        assert_eq!(stats.success_rate, "100.0%");
    }

    #[test]
    fn aggregate_window_excludes_entries_with_missing_or_unparseable_ts() {
        let mut missing_ts = entry(true, "hyb", "m1", 10, "2026-08-02T10:30:00Z");
        missing_ts.ts = None;
        let mut bad_ts = entry(true, "hyb", "m1", 20, "2026-08-02T10:30:00Z");
        bad_ts.ts = Some("not-a-timestamp".into());

        let window = Some((1_785_664_800_000, 1_785_672_000_000));
        let stats = aggregate(
            &[
                entry(true, "hyb", "m1", 30, "2026-08-02T10:30:00Z"),
                missing_ts,
                bad_ts,
            ],
            &HashMap::new(),
            60_000,
            0,
            window,
        );

        assert_eq!(stats.total_requests, 1, "dirty/missing ts rows excluded");
        assert_eq!(stats.avg_latency_ms, 30);
        assert_eq!(stats.success_rate, "100.0%");
    }

    #[test]
    fn aggregate_window_recomputes_all_dimensions_consistently() {
        let mut conv_a = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-02T10:00:00Z"),
            100,
            50,
            40,
        );
        conv_a = with_reasoning(conv_a, 30);
        conv_a.conversation_id = Some("conv-a".into());
        let mut conv_a_late = with_usage(
            entry(true, "hyb", "m1", 0, "2026-08-03T10:00:00Z"),
            1000,
            500,
            400,
        );
        conv_a_late = with_reasoning(conv_a_late, 300);
        conv_a_late.conversation_id = Some("conv-a".into());
        let mut conv_b = with_usage(
            entry(true, "fox", "m2", 0, "2026-08-02T11:00:00Z"),
            10,
            20,
            0,
        );
        conv_b = with_reasoning(conv_b, 5);
        conv_b.conversation_id = Some("conv-b".into());

        let window = Some((1_785_664_800_000, 1_785_672_000_000));
        let stats = aggregate(
            &[conv_a, conv_a_late, conv_b],
            &HashMap::new(),
            60_000,
            0,
            window,
        );

        assert_eq!(stats.total_requests, 2, "late row outside window");
        let hyb = &stats.by_provider["hyb"];
        assert_eq!(
            (hyb.total, hyb.prompt_tokens),
            (1, 100),
            "late row not counted"
        );
        assert_eq!(
            (
                stats.total_tokens.input,
                stats.total_tokens.output,
                stats.total_tokens.cached,
                stats.total_tokens.reasoning,
            ),
            (110, 70, 40, 35),
            "token four dimensions only from in-window rows"
        );
        assert_eq!(stats.by_conversation.len(), 2);
        let a = stats
            .by_conversation
            .iter()
            .find(|c| c.conversation_id == "conv-a")
            .unwrap();
        assert_eq!(
            (a.requests, a.input_tokens, a.reasoning_tokens),
            (1, 100, 30),
            "conversation aggregates recomputed per window"
        );
        let b = stats
            .by_conversation
            .iter()
            .find(|c| c.conversation_id == "conv-b")
            .unwrap();
        assert_eq!((b.requests, b.input_tokens), (1, 10));
    }

    #[test]
    fn aggregate_without_window_keeps_full_history_behaviour() {
        let mut missing_ts = entry(true, "hyb", "m1", 10, "2026-08-02T10:30:00Z");
        missing_ts.ts = None;

        let stats = aggregate(
            &[
                entry(true, "hyb", "m1", 20, "2026-08-02T10:30:00Z"),
                missing_ts,
                entry(false, "hyb", "m1", 30, "bad-timestamp"),
            ],
            &HashMap::new(),
            60_000,
            0,
            None,
        );

        assert_eq!(stats.total_requests, 3, "no window: all rows counted");
        assert_eq!(stats.ok_requests, 2);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.avg_latency_ms, 20, "(10 + 20 + 30) / 3");
    }

    #[test]
    fn parse_window_query_no_params_yields_none() {
        assert_eq!(parse_window_query(None, None, None), Ok(None));
    }
    #[test]
    fn parse_window_query_valid_custom_window() {
        assert_eq!(
            parse_window_query(Some("custom"), Some("1785664800000"), Some("1785672000000")),
            Ok(Some((1_785_664_800_000, 1_785_672_000_000)))
        );
    }

    #[test]
    fn parse_window_query_custom_missing_from_or_to_is_rejected() {
        assert!(parse_window_query(Some("custom"), None, Some("1785672000000")).is_err());
        assert!(parse_window_query(Some("custom"), Some("1785664800000"), None).is_err());
        assert!(parse_window_query(Some("custom"), None, None).is_err());
    }

    #[test]
    fn parse_window_query_preset_ranges_also_require_from_and_to() {
        for range in ["today", "last24h", "last7d"] {
            assert!(
                parse_window_query(Some(range), None, None).is_err(),
                "{range} without window must be rejected"
            );
            assert_eq!(
                parse_window_query(Some(range), Some("1785664800000"), Some("1785672000000")),
                Ok(Some((1_785_664_800_000, 1_785_672_000_000))),
                "{range} with window is accepted"
            );
        }
    }

    #[test]
    fn parse_window_query_rejects_invalid_range_and_non_numeric() {
        assert!(parse_window_query(Some("yesterday"), Some("1"), Some("2")).is_err());
        assert!(parse_window_query(Some("custom"), Some("abc"), Some("2")).is_err());
        assert!(parse_window_query(Some("custom"), Some("1"), Some("2.5")).is_err());
    }

    #[test]
    fn parse_window_query_rejects_inverted_window() {
        assert!(parse_window_query(Some("custom"), Some("2"), Some("1")).is_err());
        assert!(parse_window_query(Some("custom"), Some("5"), Some("5")).is_err());
    }

    #[test]
    fn aggregate_paged_slices_recent_requests_and_reports_total() {
        let entries: Vec<RequestLogEntry> = (0..250)
            .map(|i| {
                let ts = format!("2026-08-02T10:{:02}:{:02}Z", i / 60, i % 60);
                entry(true, "hyb", "m1", 10, &ts)
            })
            .collect();
        let circuit = HashMap::new();

        let first = aggregate_paged(&entries, &circuit, 60_000, 0, None, 0, 50);
        assert_eq!(first.recent_request_total, 250);
        assert_eq!(first.recent_requests.len(), 50);
        // Newest first: i=249 is the latest row.
        assert_eq!(
            first.recent_requests[0].ts.as_deref(),
            Some("2026-08-02T10:04:09Z")
        );

        let last = aggregate_paged(&entries, &circuit, 60_000, 0, None, 4, 50);
        assert_eq!(last.recent_requests.len(), 50);
        assert_eq!(
            last.recent_requests[0].ts.as_deref(),
            Some("2026-08-02T10:00:49Z")
        );

        let beyond = aggregate_paged(&entries, &circuit, 60_000, 0, None, 5, 50);
        assert_eq!(beyond.recent_requests.len(), 0);
        assert_eq!(beyond.recent_request_total, 250);
    }

    #[test]
    fn aggregate_defaults_to_first_hundred_recent_requests() {
        let entries: Vec<RequestLogEntry> = (0..150)
            .map(|i| {
                let ts = format!("2026-08-02T10:{:02}:{:02}Z", i / 60, i % 60);
                entry(true, "hyb", "m1", 10, &ts)
            })
            .collect();
        let stats = aggregate(&entries, &HashMap::new(), 60_000, 0, None);
        assert_eq!(stats.recent_request_total, 150);
        assert_eq!(stats.recent_requests.len(), 100);
    }

    // ─── aggregate_conversations_paged ─────────────────────────

    #[test]
    fn conversations_paged_filters_by_window_and_none_means_all() {
        let mut in_win = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        in_win.conversation_id = Some("conv-a".into());
        let mut out_win = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-03T10:00:00Z"),
            100,
            10,
            0,
        );
        out_win.conversation_id = Some("conv-b".into());

        let window = (
            ts_epoch_ms("2026-08-02T00:00:00Z").unwrap(),
            ts_epoch_ms("2026-08-02T23:59:59Z").unwrap(),
        );
        let (rows, total) = aggregate_conversations_paged(&[in_win, out_win], Some(window), 0, 50);
        assert_eq!(total, 1);
        assert_eq!(
            rows[0].conversation_id, "conv-a",
            "window excludes out-of-window rows"
        );

        let mut in_win2 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        in_win2.conversation_id = Some("conv-a".into());
        let (rows, total) = aggregate_conversations_paged(&[in_win2], None, 0, 50);
        assert_eq!(total, 1, "None window keeps full history");
        assert_eq!(rows[0].conversation_id, "conv-a");
    }

    #[test]
    fn conversations_paged_groups_missing_ids_under_unlabeled() {
        let mut named = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        named.conversation_id = Some("conv-a".into());
        let bare1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            100,
            10,
            0,
        );
        let bare2 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:02Z"),
            100,
            10,
            0,
        );

        let (rows, _) = aggregate_conversations_paged(&[named, bare1, bare2], None, 0, 50);
        assert_eq!(rows.len(), 2);
        let unlabeled = rows
            .iter()
            .find(|c| c.conversation_id == "unlabeled")
            .unwrap();
        assert_eq!(unlabeled.requests, 2, "bare rows share the unlabeled group");
    }

    #[test]
    fn conversations_paged_sorts_by_last_active_desc() {
        let mut old = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T09:00:00Z"),
            100,
            10,
            0,
        );
        old.conversation_id = Some("conv-old".into());
        let mut new = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T11:00:00Z"),
            100,
            10,
            0,
        );
        new.conversation_id = Some("conv-new".into());
        let mut mid = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        mid.conversation_id = Some("conv-mid".into());

        let (rows, _) = aggregate_conversations_paged(&[old, new, mid], None, 0, 50);
        let ids: Vec<&str> = rows.iter().map(|c| c.conversation_id.as_str()).collect();
        assert_eq!(ids, vec!["conv-new", "conv-mid", "conv-old"]);
    }

    #[test]
    fn conversations_paged_slices_pages_and_reports_total() {
        let entries: Vec<RequestLogEntry> = (0..5)
            .map(|i| {
                let mut e = with_usage(
                    entry(true, "hyb", "m1", 10, &format!("2026-08-02T10:00:0{i}Z")),
                    100,
                    10,
                    0,
                );
                e.conversation_id = Some(format!("conv-{i}"));
                e
            })
            .collect();

        let (page1, total) = aggregate_conversations_paged(&entries, None, 0, 2);
        assert_eq!(total, 5, "total always reflects the whole window");
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].conversation_id, "conv-4", "newest first");

        let (page3, _) = aggregate_conversations_paged(&entries, None, 2, 2);
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].conversation_id, "conv-0");

        let (beyond, _) = aggregate_conversations_paged(&entries, None, 5, 2);
        assert!(beyond.is_empty(), "page beyond the end yields no rows");
    }

    #[test]
    fn conversations_paged_cost_sums_known_rows_and_stays_none_when_all_unknown() {
        let mut priced1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        priced1.conversation_id = Some("conv-a".into());
        priced1.cost_total = Some(0.25);
        let mut priced2 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            100,
            10,
            0,
        );
        priced2.conversation_id = Some("conv-a".into());
        priced2.cost_total = Some(0.5);
        // Failed and retried rows carry cost but are outside the countable scope.
        let mut failed = entry(false, "hyb", "m1", 10, "2026-08-02T10:00:02Z");
        failed.conversation_id = Some("conv-a".into());
        failed.cost_total = Some(9.9);
        let mut retry = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:03Z"),
            100,
            10,
            0,
        );
        retry.conversation_id = Some("conv-a".into());
        retry.retry = Some(true);
        retry.cost_total = Some(9.9);

        let (rows, _) =
            aggregate_conversations_paged(&[priced1, priced2, failed, retry], None, 0, 50);
        assert_eq!(rows[0].cost, Some(0.75), "only countable known rows sum");
        assert_eq!(
            rows[0].requests, 4,
            "requests count every row incl. failed/retry"
        );

        let mut unknown = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:04Z"),
            100,
            10,
            0,
        );
        unknown.conversation_id = Some("conv-b".into());
        let (rows, _) = aggregate_conversations_paged(&[unknown], None, 0, 50);
        assert_eq!(rows[0].cost, None, "all-unknown conversation shows no cost");
    }

    #[test]
    fn conversations_paged_name_comes_from_newest_named_row() {
        let mut old = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        old.conversation_id = Some("conv-a".into());
        old.conversation_name = Some("old-name".into());
        let mut latest = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            100,
            10,
            0,
        );
        latest.conversation_id = Some("conv-a".into());
        latest.conversation_name = Some("new-name".into());

        let (rows, _) = aggregate_conversations_paged(&[old, latest], None, 0, 50);
        assert_eq!(
            rows[0].name.as_deref(),
            Some("new-name"),
            "newest named row wins"
        );
    }

    #[test]
    fn conversations_paged_token_scope_excludes_retry_and_failed_rows() {
        let mut ok = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        ok.conversation_id = Some("conv-a".into());
        let mut retry = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            999,
            999,
            0,
        );
        retry.conversation_id = Some("conv-a".into());
        retry.retry = Some(true);
        let mut failed = entry(false, "hyb", "m1", 10, "2026-08-02T10:00:02Z");
        failed.conversation_id = Some("conv-a".into());

        let (rows, _) = aggregate_conversations_paged(&[ok, retry, failed], None, 0, 50);
        assert_eq!(
            rows[0].input_tokens, 100,
            "retry/failed rows do not add tokens"
        );
        assert_eq!(rows[0].output_tokens, 10);
        assert_eq!(rows[0].requests, 3);
        assert_eq!(
            rows[0].last_active.as_deref(),
            Some("2026-08-02T10:00:02Z"),
            "lastActive still tracks every row"
        );
    }

    // ─── conversation_id on request details + conversation requests ───

    #[test]
    fn aggregate_recent_requests_carry_conversation_fields() {
        let mut named = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        named.conversation_id = Some("conv-a".into());
        named.conversation_name = Some("my-chat".into());
        let bare = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            100,
            10,
            0,
        );

        let stats = aggregate(&[named, bare], &HashMap::new(), 60_000, 0, None);
        assert_eq!(stats.recent_requests.len(), 2);
        let named_row = stats
            .recent_requests
            .iter()
            .find(|r| r.ts.as_deref() == Some("2026-08-02T10:00:00Z"))
            .unwrap();
        assert_eq!(named_row.conversation_id.as_deref(), Some("conv-a"));
        assert_eq!(named_row.conversation_name.as_deref(), Some("my-chat"));
        let bare_row = stats
            .recent_requests
            .iter()
            .find(|r| r.ts.as_deref() == Some("2026-08-02T10:00:01Z"))
            .unwrap();
        assert_eq!(
            bare_row.conversation_id, None,
            "bare rows carry no conversation id"
        );
        assert_eq!(bare_row.conversation_name, None);
    }

    #[test]
    fn conversation_requests_filters_by_id_and_keeps_all_rows() {
        let mut a1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        a1.conversation_id = Some("conv-a".into());
        a1.cost_total = Some(0.25);
        let mut a2 = entry(false, "hyb", "m1", 10, "2026-08-02T10:00:01Z");
        a2.conversation_id = Some("conv-a".into());
        a2.retry = Some(true);
        let mut b = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:02Z"),
            50,
            5,
            0,
        );
        b.conversation_id = Some("conv-b".into());

        let (rows, total) = aggregate_conversation_requests(&[a1, a2, b], "conv-a", 0, 50);
        assert_eq!(total, 2, "total counts only matching rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].ts.as_deref(),
            Some("2026-08-02T10:00:01Z"),
            "newest first"
        );
        assert_eq!(
            rows[0].ok,
            Some(false),
            "failed/retry rows included like request details"
        );
        assert_eq!(rows[1].ts.as_deref(), Some("2026-08-02T10:00:00Z"));
        assert_eq!(
            rows[1].prompt_tokens,
            Some(100),
            "token semantics match request details"
        );
        assert_eq!(rows[1].cost, Some(0.25));
    }

    #[test]
    fn conversation_requests_unlabeled_matches_bare_rows() {
        let bare1 = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:00Z"),
            100,
            10,
            0,
        );
        let mut named = with_usage(
            entry(true, "hyb", "m1", 10, "2026-08-02T10:00:01Z"),
            100,
            10,
            0,
        );
        named.conversation_id = Some("conv-a".into());

        let (rows, total) = aggregate_conversation_requests(&[bare1, named], "unlabeled", 0, 50);
        assert_eq!(total, 1);
        assert_eq!(
            rows[0].conversation_id, None,
            "unlabeled matches rows without an id"
        );
    }

    #[test]
    fn conversation_requests_pages_and_sorts_desc() {
        let entries: Vec<RequestLogEntry> = (0..5)
            .map(|i| {
                let mut e = with_usage(
                    entry(true, "hyb", "m1", 10, &format!("2026-08-02T10:00:0{i}Z")),
                    100,
                    10,
                    0,
                );
                e.conversation_id = Some("conv-a".into());
                e
            })
            .collect();

        let (page1, total) = aggregate_conversation_requests(&entries, "conv-a", 0, 2);
        assert_eq!(total, 5);
        assert_eq!(page1.len(), 2);
        assert_eq!(
            page1[0].ts.as_deref(),
            Some("2026-08-02T10:00:04Z"),
            "newest first"
        );

        let (page3, _) = aggregate_conversation_requests(&entries, "conv-a", 2, 2);
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].ts.as_deref(), Some("2026-08-02T10:00:00Z"));

        let (beyond, _) = aggregate_conversation_requests(&entries, "conv-a", 5, 2);
        assert!(beyond.is_empty(), "page beyond the end yields no rows");
    }
}
