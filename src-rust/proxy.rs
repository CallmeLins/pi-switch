use crate::config::{CircuitBreakerSettings, ProviderProfile, config_dir};
use crate::error::{AppError, Result};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

// ─── Shared proxy state ───────────────────────────────────

pub struct ProxyState {
    pub config: Arc<RwLock<crate::config::PiSwitchConfig>>,
}

// ─── Request / health types ───────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyHealth {
    pub ok: bool,
    pub target: Option<String>,
    pub candidates: Vec<String>,
    pub api: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "supportedApis")]
    pub supported_apis: Vec<String>,
    pub failover: Vec<String>,
    #[serde(rename = "circuitBreaker")]
    pub circuit_breaker: CircuitBreakerSettings,
    #[serde(rename = "circuitState")]
    pub circuit_state: CircuitStateStore,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CircuitEntry {
    pub failures: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "openedAt")]
    pub opened_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lastFailureAt")]
    pub last_failure_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lastSuccessAt")]
    pub last_success_at: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CircuitStateStore {
    pub providers: std::collections::HashMap<String, CircuitEntry>,
}

// ─── Circuit breaker ──────────────────────────────────────

fn circuit_path() -> PathBuf {
    config_dir().join("circuit.json")
}

pub async fn read_circuit_state() -> CircuitStateStore {
    let path = circuit_path();
    if !path.exists() {
        return CircuitStateStore::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub async fn write_circuit_state(state: &CircuitStateStore) {
    let path = circuit_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        std::fs::write(&path, json).ok();
    }
}

fn is_circuit_open(state: &CircuitStateStore, name: &str, settings: &CircuitBreakerSettings) -> (bool, bool) {
    if !settings.enabled {
        return (false, false);
    }

    let entry = match state.providers.get(name) {
        Some(e) => e,
        None => return (false, false),
    };

    match entry.opened_at {
        Some(opened) => {
            let cooldown_ms = (settings.cooldown_seconds as u64) * 1000;
            let now = now_ms();
            let elapsed = now.saturating_sub(opened);

            if elapsed < cooldown_ms {
                // Still in cooldown, circuit is open
                (true, false)
            } else {
                // Cooldown expired, enter half-open
                (false, true)
            }
        }
        None => (false, false),
    }
}

async fn record_success(name: &str, half_open: bool) {
    let mut state = read_circuit_state().await;
    let entry = state.providers.entry(name.to_string()).or_insert(CircuitEntry {
        failures: 0,
        opened_at: None,
        last_failure_at: None,
        last_error: None,
        last_success_at: None,
    });

    entry.failures = 0;
    entry.last_success_at = Some(now_ms());

    // If in half-open state and success, transition to closed
    if half_open {
        entry.opened_at = None;
    }

    write_circuit_state(&state).await;
}

async fn record_failure(name: &str, settings: &CircuitBreakerSettings, reason: &str, half_open: bool) {
    if !settings.enabled { return; }
    let mut state = read_circuit_state().await;
    let entry = state.providers.entry(name.to_string()).or_insert(CircuitEntry {
        failures: 0, opened_at: None, last_failure_at: None, last_error: None, last_success_at: None,
    });

    entry.failures += 1;
    entry.last_failure_at = Some(now_ms());
    entry.last_error = Some(reason.to_string());

    // If half-open and failed, immediately reopen
    // If closed and reached threshold, open
    if half_open || entry.failures >= settings.failure_threshold {
        entry.opened_at = Some(now_ms());
    }

    write_circuit_state(&state).await;
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Retry statuses ───────────────────────────────────────

fn should_retry(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

// ─── OpenAI <-> Anthropic conversion ──────────────────────

fn openai_to_anthropic_body(body: &Value) -> Value {
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or("claude-sonnet-4-5");
    let max_tokens = body.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(16384);
    let messages = body.get("messages").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    // Extract system messages
    let mut system_parts = Vec::new();
    let mut anthropic_msgs = Vec::new();

    for msg in &messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        match role {
            "system" => {
                if let Some(content) = msg.get("content") {
                    let text = match content {
                        Value::String(s) => s.clone(),
                        Value::Array(arr) => arr.iter()
                            .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        _ => content.to_string(),
                    };
                    if !text.is_empty() {
                        system_parts.push(json!({ "type": "text", "text": text }));
                    }
                }
            }
            _ => {
                let new_role = if role == "assistant" { "assistant" } else { "user" };
                let content = msg.get("content").cloned().unwrap_or(Value::String(String::new()));
                let parts = match content {
                    Value::String(s) => vec![json!({ "type": "text", "text": s })],
                    Value::Array(arr) => {
                        arr.iter().filter_map(|c| {
                            match c.get("type").and_then(|t| t.as_str()) {
                                Some("text") => {
                                    let text = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                    Some(json!({ "type": "text", "text": text }))
                                }
                                _ => Some(json!({ "type": "text", "text": c.to_string() })),
                            }
                        }).collect()
                    }
                    _ => vec![json!({ "type": "text", "text": content.to_string() })],
                };
                anthropic_msgs.push(json!({ "role": new_role, "content": parts }));
            }
        }
    }

    let mut anthro_body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": anthropic_msgs,
    });

    if !system_parts.is_empty() {
        anthro_body["system"] = Value::Array(system_parts);
    }
    if let Some(temp) = body.get("temperature") {
        anthro_body["temperature"] = temp.clone();
    }
    if let Some(stop) = body.get("stop") {
        anthro_body["stop_sequences"] = match stop {
            Value::Array(a) => Value::Array(a.clone()),
            s => json!([s.clone()]),
        };
    }

    anthro_body
}

fn anthropic_to_openai_response(anthro: &Value) -> Value {
    let model = anthro.get("model").and_then(|v| v.as_str()).unwrap_or("claude-sonnet-4-5");
    let content_blocks = anthro.get("content").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let choices: Vec<Value> = content_blocks.iter().enumerate().map(|(i, block)| {
        let text = block.get("text").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "index": i,
            "message": { "role": "assistant", "content": text },
            "finish_reason": match anthro.get("stop_reason").and_then(|v| v.as_str()) {
                Some("end_turn") => "stop",
                Some("max_tokens") => "length",
                Some(r) => r,
                None => "stop",
            }
        })
    }).collect();

    let usage = anthro.get("usage").map(|u| json!({
        "prompt_tokens": u.get("input_tokens").unwrap_or(&json!(0)),
        "completion_tokens": u.get("output_tokens").unwrap_or(&json!(0)),
        "total_tokens": u.get("input_tokens").unwrap_or(&json!(0)).as_u64().unwrap_or(0)
            + u.get("output_tokens").unwrap_or(&json!(0)).as_u64().unwrap_or(0),
    }));

    let mut resp = json!({
        "id": anthro.get("id").unwrap_or(&json!(format!("chatcmpl-{}", now_ms()))),
        "object": "chat.completion",
        "created": now_ms() / 1000,
        "model": model,
        "choices": choices,
    });

    if let Some(u) = usage {
        resp["usage"] = u;
    }

    resp
}

// ─── Proxy router ─────────────────────────────────────────

pub fn make_router(state: Arc<ProxyState>) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/v1/models", get(handle_models))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_messages))
        .with_state(state)
}

async fn handle_health(State(state): State<Arc<ProxyState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let target = config.settings.proxy.target.clone();
    let candidates = build_candidates(&config, None);
    let profile = target.as_ref()
        .and_then(|t| config.profiles.get(t))
        .and_then(|v| serde_json::from_value::<ProviderProfile>(v.clone()).ok());

    let mut supported_apis = HashSet::new();
    for name in &candidates {
        if let Some(p) = config.profiles.get(name) {
            if let Some(api) = p.get("api").and_then(|v| v.as_str()) {
                supported_apis.insert(api.to_string());
            }
        }
    }

    let circuit_state = read_circuit_state().await;

    Json(json!({
        "ok": true,
        "target": target,
        "candidates": candidates,
        "api": profile.as_ref().map_or("", |p| &p.api),
        "baseUrl": profile.as_ref().map_or("", |p| &p.base_url),
        "supportedApis": supported_apis.into_iter().collect::<Vec<_>>(),
        "failover": &config.settings.proxy.failover,
        "circuitBreaker": &config.settings.proxy.circuit_breaker,
        "circuitState": circuit_state,
    }))
}

async fn handle_models(State(state): State<Arc<ProxyState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let candidates = build_candidates(&config, None);
    let mut seen = HashSet::new();
    let mut data = Vec::new();

    for name in &candidates {
        if let Some(profile) = config.profiles.get(name) {
            if let Some(models) = profile.get("models").and_then(|v| v.as_array()) {
                for model in models {
                    let id = model.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if seen.insert(id.to_string()) {
                        data.push(json!({
                            "id": id,
                            "object": "model",
                            "owned_by": name,
                        }));
                    }
                }
            }
        }
    }

    Json(json!({ "object": "list", "data": data }))
}

// ─── Chat completions with failover ───────────────────────

async fn handle_chat_completions(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let config = state.config.read().await;
    let candidates = build_candidates(&config, None);
    let body_value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);

    let result = forward_with_failover(&config, &candidates, &body_value, "chat/completions", &headers).await;

    match result {
        Ok(resp) => resp,
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": { "message": e.to_string(), "type": "failover_exhausted" } })),
        ).into_response(),
    }
}

async fn handle_messages(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let config = state.config.read().await;
    let candidates: Vec<String> = build_candidates(&config, None)
        .into_iter()
        .filter(|name| {
            config.profiles.get(name)
                .and_then(|p| p.get("api").and_then(|v| v.as_str()))
                .map_or(false, |api| api == "anthropic-messages")
        })
        .collect();

    if candidates.is_empty() {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({ "error": { "message": "No Anthropic upstream available" } })),
        ).into_response();
    }

    let body_value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let result = forward_anthropic_with_failover(&config, &candidates, &body_value, &headers).await;

    match result {
        Ok(resp) => resp,
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": { "message": e.to_string() } })),
        ).into_response(),
    }
}

// ─── Failover logic ───────────────────────────────────────

fn build_candidates(config: &crate::config::PiSwitchConfig, explicit: Option<&str>) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();

    let mut add = |name: &str| {
        if name.is_empty() { return; }
        if seen.contains(name) { return; }
        if let Some(p) = config.profiles.get(name) {
            if p.get("proxy").and_then(|v| v.as_bool()).unwrap_or(false) { return; }
        }
        seen.insert(name.to_string());
        names.push(name.to_string());
    };

    if let Some(e) = explicit { add(e); }
    if let Some(ref t) = config.settings.proxy.target { add(t); }
    for name in &config.settings.proxy.failover { add(name); }

    // Pick first non-proxy profile that hasn't been added yet
    let fallback = config.profiles.keys()
        .filter(|name| {
            config.profiles.get(*name)
                .and_then(|p| p.get("proxy"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false) == false
        })
        .find(|name| !seen.contains(name.as_str()));

    if let Some(fb) = fallback {
        names.push(fb.clone());
    }

    names
}

async fn forward_with_failover(
    config: &crate::config::PiSwitchConfig,
    candidates: &[String],
    body: &Value,
    target_path: &str,
    _headers: &HeaderMap,
) -> Result<Response> {
    let circuit_settings = &config.settings.proxy.circuit_breaker;
    let mut circuit_state = read_circuit_state().await;
    let client = ReqwestClient::new();
    let mut half_open_used = false;

    for name in candidates {
        let profile_value = match config.profiles.get(name) {
            Some(p) => p,
            None => continue,
        };

        let (is_open, is_half_open) = is_circuit_open(&circuit_state, name, circuit_settings);

        if is_open {
            log_request(name, false, Some("circuit_open"), None, None, None, None).await;
            continue;
        }

        // If half-open, only allow one probe request
        if is_half_open {
            if half_open_used {
                log_request(name, false, Some("half_open_already_probing"), None, None, None, None).await;
                continue;
            }
            half_open_used = true;
        }

        let profile: ProviderProfile = match serde_json::from_value(profile_value.clone()) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let is_anthropic = profile.api == "anthropic-messages";
        if profile.api != "openai-completions" && !is_anthropic {
            continue;
        }

        let api_key = resolve_env(&profile.api_key);

        if is_anthropic {
            // Convert OpenAI -> Anthropic
            let anthro_body = openai_to_anthropic_body(body);
            let url = format!("{}/messages", profile.base_url.trim_end_matches('/'));

            let resp = client.post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&anthro_body)
                .send()
                .await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        let anthro_data: Value = r.json().await.unwrap_or(Value::Null);
                        let openai_data = anthropic_to_openai_response(&anthro_data);
                        record_success(name, is_half_open).await;
                        log_request(name, true, None, Some(status.as_u16()), Some(&url), None, body.get("model").and_then(|v| v.as_str())).await;
                        return Ok(Json(openai_data).into_response());
                    } else if should_retry(status.as_u16()) {
                        let status_code = status.as_u16();
                        record_failure(name, circuit_settings, &format!("HTTP {}", status_code), is_half_open).await;
                        log_request(name, false, Some(&format!("HTTP {}", status_code)), Some(status_code), Some(&url), None, body.get("model").and_then(|v| v.as_str())).await;
                        circuit_state = read_circuit_state().await;
                        continue;
                    } else {
                        let body_bytes = r.bytes().await.unwrap_or_default();
                        log_request(name, false, None, Some(status.as_u16()), Some(&url), None, body.get("model").and_then(|v| v.as_str())).await;
                        return Ok(Response::builder()
                            .status(status.as_u16())
                            .body(Body::from(body_bytes))
                            .unwrap());
                    }
                }
                Err(e) => {
                    record_failure(name, circuit_settings, &e.to_string(), is_half_open).await;
                    log_request(name, false, Some(&e.to_string()), None, None, None, body.get("model").and_then(|v| v.as_str())).await;
                    circuit_state = read_circuit_state().await;
                    continue;
                }
            }
        } else {
            // OpenAI-compatible
            let url = format!("{}/{}", profile.base_url.trim_end_matches('/'), target_path);

            let resp = client.post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .json(body)
                .send()
                .await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        let body_bytes = r.bytes().await.unwrap_or_default();
                        record_success(name, is_half_open).await;
                        log_request(name, true, None, Some(status.as_u16()), Some(&url), None, body.get("model").and_then(|v| v.as_str())).await;
                        return Ok(Response::builder()
                            .status(status.as_u16())
                            .body(Body::from(body_bytes))
                            .unwrap());
                    } else if should_retry(status.as_u16()) {
                        let status_code = status.as_u16();
                        record_failure(name, circuit_settings, &format!("HTTP {}", status_code), is_half_open).await;
                        log_request(name, false, Some(&format!("HTTP {}", status_code)), Some(status_code), Some(&url), None, body.get("model").and_then(|v| v.as_str())).await;
                        circuit_state = read_circuit_state().await;
                        continue;
                    } else {
                        let body_bytes = r.bytes().await.unwrap_or_default();
                        log_request(name, false, None, Some(status.as_u16()), Some(&url), None, body.get("model").and_then(|v| v.as_str())).await;
                        return Ok(Response::builder()
                            .status(status.as_u16())
                            .body(Body::from(body_bytes))
                            .unwrap());
                    }
                }
                Err(e) => {
                    record_failure(name, circuit_settings, &e.to_string(), is_half_open).await;
                    log_request(name, false, Some(&e.to_string()), None, None, None, body.get("model").and_then(|v| v.as_str())).await;
                    circuit_state = read_circuit_state().await;
                    continue;
                }
            }
        }
    }

    Err(AppError::proxy("All upstream attempts failed".to_string()))
}

async fn forward_anthropic_with_failover(
    config: &crate::config::PiSwitchConfig,
    candidates: &[String],
    body: &Value,
    _headers: &HeaderMap,
) -> Result<Response> {
    let circuit_settings = &config.settings.proxy.circuit_breaker;
    let mut circuit_state = read_circuit_state().await;
    let client = ReqwestClient::new();
    let mut half_open_used = false;

    for name in candidates {
        let (is_open, is_half_open) = is_circuit_open(&circuit_state, name, circuit_settings);

        if is_open {
            continue;
        }

        if is_half_open {
            if half_open_used {
                continue;
            }
            half_open_used = true;
        }

        let profile_value = match config.profiles.get(name) {
            Some(p) => p, None => continue,
        };
        let profile: ProviderProfile = match serde_json::from_value(profile_value.clone()) {
            Ok(p) => p, Err(_) => continue,
        };
        if profile.api != "anthropic-messages" { continue; }

        let api_key = resolve_env(&profile.api_key);
        let url = format!("{}/messages", profile.base_url.trim_end_matches('/'));

        let resp = client.post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() || !should_retry(r.status().as_u16()) => {
                let status = r.status();
                let body_bytes = r.bytes().await.unwrap_or_default();
                if status.is_success() {
                    record_success(name, is_half_open).await;
                }
                return Ok(Response::builder()
                    .status(status.as_u16())
                    .body(Body::from(body_bytes))
                    .unwrap());
            }
            Ok(r) => {
                let status = r.status().as_u16();
                record_failure(name, circuit_settings, &format!("HTTP {}", status), is_half_open).await;
                circuit_state = read_circuit_state().await;
                continue;
            }
            Err(e) => {
                record_failure(name, circuit_settings, &e.to_string(), is_half_open).await;
                circuit_state = read_circuit_state().await;
                continue;
            }
        }
    }

    Err(AppError::proxy("All Anthropic upstream attempts failed".to_string()))
}

// ─── Env resolution ───────────────────────────────────────

fn resolve_env(value: &str) -> String {
    let trimmed = value.trim();
    // Check if it's an env var reference like $VAR or ${VAR}
    if trimmed.starts_with('$') {
        let var_name = trimmed.trim_start_matches('$').trim_start_matches('{').trim_end_matches('}');
        if var_name.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()) {
            return std::env::var(var_name).unwrap_or_else(|_| trimmed.to_string());
        }
    }
    trimmed.to_string()
}

// ─── Request logging ──────────────────────────────────────

async fn log_request(
    provider: &str,
    ok: bool,
    error: Option<&str>,
    status: Option<u16>,
    upstream_url: Option<&str>,
    _attempts: Option<&[Value]>,
    model: Option<&str>,
) {
    let log_path = config_dir().join("requests.log");
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let entry = json!({
        "ts": Utc::now().to_rfc3339(),
        "ok": ok,
        "provider": provider,
        "error": error,
        "status": status,
        "upstreamUrl": upstream_url,
        "model": model,
    });

    if let Ok(json) = serde_json::to_string(&entry) {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            use std::io::Write;
            let _ = writeln!(file, "{}", json);
        }
    }
}
