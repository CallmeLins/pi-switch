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
}

#[derive(Debug, Serialize)]
pub struct ModelStats {
    pub total: u64,
    pub ok: u64,
}

fn parse_logs() -> Vec<RequestLogEntry> {
    let path = config_dir().join("requests.log");
    if !path.exists() { return vec![]; }

    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() { return None; }
            serde_json::from_str(line).ok()
        })
        .collect()
}

pub fn get_stats() -> UsageStats {
    let entries = parse_logs();

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
    };

    let mut total_ms: u64 = 0;
    let mut latency_count: u64 = 0;

    for entry in &entries {
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
