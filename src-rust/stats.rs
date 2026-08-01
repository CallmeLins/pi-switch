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
            if line.is_empty() { return None; }
            serde_json::from_str(line).ok()
        })
        .collect()
}

fn parse_logs() -> Vec<RequestLogEntry> {
    let path = config_dir().join("requests.log");
    if !path.exists() { return vec![]; }

    parse_entries(&std::fs::read_to_string(&path).unwrap_or_default())
}

fn read_circuit_state() -> HashMap<String, CircuitBreakerEntry> {
    let path = config_dir().join("circuit.json");
    if !path.exists() { return HashMap::new(); }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let state: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();

    state.get("providers")
        .and_then(|p| p.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value(v.clone()).ok().map(|entry| (k.clone(), entry))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn circuit_breaker_status(entry: &CircuitBreakerEntry, cooldown_ms: u64, now_ms: u64) -> CircuitBreakerStatus {
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
            (name.clone(), circuit_breaker_status(entry, cooldown_ms, now_ms))
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
    };

    let mut total_ms: u64 = 0;
    let mut latency_count: u64 = 0;

    for entry in entries {
        stats.total_requests += 1;
        match entry.ok {
            Some(true) => stats.ok_requests += 1,
            _ => stats.failed_requests += 1,
        }
        if entry.retry.unwrap_or(false) { stats.retried_requests += 1; }
        if entry.skipped.unwrap_or(false) { stats.skipped_by_circuit += 1; }

        // Per provider
        let provider = entry.provider.as_deref().unwrap_or("unknown");
        let ps = stats.by_provider.entry(provider.to_string()).or_insert(ProviderStats {
            total: 0, ok: 0, failed: 0, retries: 0, avg_ms: 0, total_ms: 0, last_used: None,
        });
        ps.total += 1;
        if entry.ok.unwrap_or(false) { ps.ok += 1; } else { ps.failed += 1; }
        if entry.retry.unwrap_or(false) { ps.retries += 1; }
        if let Some(ms) = entry.ms {
            ps.total_ms += ms;
            ps.avg_ms = ps.total_ms / ps.total;
        }
        if let Some(ref ts) = entry.ts { ps.last_used = Some(ts.clone()); }

        // Per model
        let model = entry.model.as_deref().unwrap_or("unknown");
        let ms = stats.by_model.entry(model.to_string()).or_insert(ModelStats { total: 0, ok: 0 });
        ms.total += 1;
        if entry.ok.unwrap_or(false) { ms.ok += 1; }

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
        stats.success_rate = format!("{:.1}%",
            (stats.ok_requests as f64 / stats.total_requests as f64) * 100.0);
    }

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

pub fn export_logs_csv() -> crate::error::Result<String> {
    let entries = parse_logs();

    let mut csv = String::from("timestamp,ok,provider,model,status,latency_ms,error,retry,skipped,converted,upstream_url\n");

    for entry in entries {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            entry.ts.as_deref().unwrap_or(""),
            entry.ok.map(|b| if b { "true" } else { "false" }).unwrap_or(""),
            entry.provider.as_deref().unwrap_or(""),
            entry.model.as_deref().unwrap_or(""),
            entry.status.map(|s| s.to_string()).unwrap_or_default(),
            entry.ms.map(|m| m.to_string()).unwrap_or_default(),
            entry.error.as_deref().unwrap_or("").replace(',', ";").replace('\n', " "),
            entry.retry.map(|b| if b { "true" } else { "false" }).unwrap_or(""),
            entry.skipped.map(|b| if b { "true" } else { "false" }).unwrap_or(""),
            entry.converted.as_deref().unwrap_or(""),
            entry.upstream_url.as_deref().unwrap_or(""),
        ));
    }

    Ok(csv)
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
        }
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

        assert_eq!(stats.circuit_breaker["hot"].state, "open", "30s since opened < 60s cooldown");
        assert_eq!(
            stats.circuit_breaker["cooled"].state,
            "half_open",
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
        assert_eq!((fox_ps.total, fox_ps.ok, fox_ps.failed, fox_ps.retries), (1, 0, 1, 1));
        let unknown_ps = &stats.by_provider["unknown"];
        assert_eq!((unknown_ps.total, unknown_ps.ok, unknown_ps.failed), (1, 0, 1));

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
}
