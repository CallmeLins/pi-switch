use crate::config::{CircuitBreakerSettings, ProviderProfile};
use crate::error::{AppError, Result};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
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

// ─── Disguise: preset → real client identity ───────────────
//
// Values match real CLI clients. UA whitelists (e.g. Kimi coding) check only the
// name prefix, not the version, so static values stay valid across client upgrades.

/// Resolve the actual User-Agent string from a disguise preset key.
fn resolve_user_agent(preset: &str) -> &str {
    match preset {
        // Real Claude Code CLI sends `claude-cli/<ver> (external, cli)`, not `claude-code/...`.
        "claude-code" => "claude-cli/2.1.161 (external, cli)",
        "codex" => "codex_cli_rs/0.1.0",
        "gemini" => "gemini-cli/0.1.5",
        _ => preset, // raw UA string (legacy / manual)
    }
}

/// Static extra headers a real client of the given preset also sends.
/// (No synthesized session/traceparent — random values never pass deep checks and
/// aren't needed for prefix-only UA whitelists.)
fn disguise_headers(preset: Option<&str>) -> Vec<(&'static str, &'static str)> {
    match preset {
        Some("claude-code") => vec![
            ("anthropic-version", "2023-06-01"),
            ("anthropic-beta", "claude-code-20250219"),
        ],
        Some("gemini") => vec![("x-goog-api-client", "gemini-cli/0.1.5")],
        _ => vec![],
    }
}

/// Build a reqwest client + resolved UA + extra headers for an effective spoof preset.
/// The UA is set on the client builder (reqwest overrides a per-request header with its
/// own default otherwise); the per-request header is applied as a safety net at call sites.
fn build_disguised_client(
    spoof: Option<&str>,
) -> (
    ReqwestClient,
    Option<String>,
    Vec<(&'static str, &'static str)>,
) {
    let ua = spoof.map(|p| resolve_user_agent(p).to_string());
    let mut b = ReqwestClient::builder();
    if let Some(ref u) = ua {
        b = b.user_agent(u);
    }
    let client = b.build().unwrap_or_else(|_| ReqwestClient::new());
    (client, ua, disguise_headers(spoof))
}

// ─── Shared proxy state ───────────────────────────────────

/// Marker state for the axum router. Config is reloaded from disk per request (so live
/// target changes take effect on the running proxy), so no shared config is stored here.
pub struct ProxyState {}

// ─── Request / health types ───────────────────────────────

#[allow(dead_code)]
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

/// Per-process temp dir that proxy tests redirect runtime state (requests.log,
/// circuit.json) to, so unit tests never pollute the real ~/.pi-switch directory.
#[cfg(test)]
pub(crate) fn init_test_state_dir() -> &'static PathBuf {
    use std::sync::OnceLock;
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("pi-switch-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test state dir");
        dir
    })
}

/// Where proxy runtime state (requests.log, circuit.json) lives.
/// Tests redirect this to a per-process temp dir.
fn state_dir() -> PathBuf {
    #[cfg(test)]
    {
        init_test_state_dir().clone()
    }
    #[cfg(not(test))]
    {
        crate::config::config_dir()
    }
}

fn circuit_path() -> PathBuf {
    state_dir().join("circuit.json")
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

fn is_circuit_open(
    state: &CircuitStateStore,
    name: &str,
    settings: &CircuitBreakerSettings,
) -> (bool, bool) {
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
    let entry = state
        .providers
        .entry(name.to_string())
        .or_insert(CircuitEntry {
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

async fn record_failure(
    name: &str,
    settings: &CircuitBreakerSettings,
    reason: &str,
    half_open: bool,
) {
    if !settings.enabled {
        return;
    }
    let mut state = read_circuit_state().await;
    let entry = state
        .providers
        .entry(name.to_string())
        .or_insert(CircuitEntry {
            failures: 0,
            opened_at: None,
            last_failure_at: None,
            last_error: None,
            last_success_at: None,
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
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("claude-sonnet-4-5");
    let max_tokens = body
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(16384);
    let messages = body
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

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
                        Value::Array(arr) => arr
                            .iter()
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
                let new_role = if role == "assistant" {
                    "assistant"
                } else {
                    "user"
                };
                let content = msg
                    .get("content")
                    .cloned()
                    .unwrap_or(Value::String(String::new()));
                let parts = match content {
                    Value::String(s) => vec![json!({ "type": "text", "text": s })],
                    Value::Array(arr) => arr
                        .iter()
                        .map(|c| match c.get("type").and_then(|t| t.as_str()) {
                            Some("text") => {
                                let text = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                json!({ "type": "text", "text": text })
                            }
                            _ => json!({ "type": "text", "text": c.to_string() }),
                        })
                        .collect(),
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
    let model = anthro
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("claude-sonnet-4-5");
    let content_blocks = anthro
        .get("content")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let choices: Vec<Value> = content_blocks
        .iter()
        .enumerate()
        .map(|(i, block)| {
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
        })
        .collect();

    let usage = anthro.get("usage").map(|u| {
        json!({
            "prompt_tokens": u.get("input_tokens").unwrap_or(&json!(0)),
            "completion_tokens": u.get("output_tokens").unwrap_or(&json!(0)),
            "total_tokens": u.get("input_tokens").unwrap_or(&json!(0)).as_u64().unwrap_or(0)
                + u.get("output_tokens").unwrap_or(&json!(0)).as_u64().unwrap_or(0),
        })
    });

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

const DEFAULT_MAX_REQUEST_BODY_MIB: usize = 32;
const MIN_MAX_REQUEST_BODY_MIB: usize = 4;
const MAX_MAX_REQUEST_BODY_MIB: usize = 256;

fn max_request_body_bytes() -> usize {
    std::env::var("PI_SWITCH_MAX_REQUEST_BODY_MIB")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|mib| mib.clamp(MIN_MAX_REQUEST_BODY_MIB, MAX_MAX_REQUEST_BODY_MIB))
        .unwrap_or(DEFAULT_MAX_REQUEST_BODY_MIB)
        * 1024
        * 1024
}

pub fn make_router(state: Arc<ProxyState>) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/v1/models", get(handle_models))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_messages))
        .route("/v1/responses", post(handle_responses))
        .layer(DefaultBodyLimit::max(max_request_body_bytes()))
        .with_state(state)
}

async fn handle_health(State(_state): State<Arc<ProxyState>>) -> impl IntoResponse {
    let config = crate::config::load_config().unwrap_or_default();
    let candidates = exposed_profiles(&config);

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
        "candidates": candidates,
        "supportedApis": supported_apis.into_iter().collect::<Vec<_>>(),
        "failover": &config.settings.proxy.failover,
        "circuitBreaker": &config.settings.proxy.circuit_breaker,
        "circuitState": circuit_state,
    }))
}

async fn handle_models(State(_state): State<Arc<ProxyState>>) -> impl IntoResponse {
    let config = crate::config::load_config().unwrap_or_default();

    let mut seen = HashSet::new();
    let mut data = Vec::new();

    // Advertise the union of every non-proxy profile's exposedModels, namespaced as
    // "profile/realModelId" so pi can pick a model that unambiguously selects an upstream.
    for (name, profile) in &config.profiles {
        if profile
            .get("proxy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        if let Some(exposed) = profile.get("exposedModels").and_then(|v| v.as_array()) {
            for model_id in exposed {
                if let Some(real) = model_id.as_str() {
                    let id = format!("{}/{}", name, real);
                    if seen.insert(id.clone()) {
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
    State(_state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let config = crate::config::load_config().unwrap_or_default();
    let body_value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let body_value = filter_private_params(body_value);

    // Route purely by the model name in the body: "profile/realModel" → that profile
    // (+ same-model failover), and the real model id to send upstream.
    let requested_model = body_value
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let (candidates, real_model) = resolve_route(&config, requested_model);

    if candidates.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": {
                "message": format!("No upstream exposes model '{}'", requested_model),
                "type": "no_route",
            } })),
        )
            .into_response();
    }

    let conversation_id = conversation_id_of(&headers, &body_value);

    let result = forward_with_failover(
        &config,
        &candidates,
        &body_value,
        &real_model,
        "chat/completions",
        &headers,
        conversation_id.as_deref(),
        true,
    )
    .await;

    match result {
        Ok(resp) => resp,
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": { "message": e.to_string(), "type": "failover_exhausted" } })),
        )
            .into_response(),
    }
}

async fn handle_messages(
    State(_state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let config = crate::config::load_config().unwrap_or_default();
    let body_value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let body_value = filter_private_params(body_value);

    let requested_model = body_value
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let (candidates, real_model) = resolve_route(&config, requested_model);

    // Native Anthropic endpoint: only route to anthropic-messages upstreams.
    let candidates: Vec<String> = candidates
        .into_iter()
        .filter(|name| {
            config
                .profiles
                .get(name)
                .and_then(|p| p.get("api").and_then(|v| v.as_str()))
                == Some("anthropic-messages")
        })
        .collect();

    if candidates.is_empty() {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({ "error": { "message": "No Anthropic upstream available for requested model" } })),
        ).into_response();
    }

    let conversation_id = conversation_id_of(&headers, &body_value);

    let result = forward_anthropic_with_failover(
        &config,
        &candidates,
        &body_value,
        &real_model,
        &headers,
        conversation_id.as_deref(),
    )
    .await;

    match result {
        Ok(resp) => resp,
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": { "message": e.to_string() } })),
        )
            .into_response(),
    }
}

// ─── OpenAI Responses API handler ────────────────────────
//
// Pi (via Codex CLI) sends Requests in the Responses API format
// (POST /v1/responses). The proxy converts them to Chat Completions
// for routing, then converts the upstream Chat Completions response
// back to Responses format for Pi.

/// Errors produced while converting between the Responses API and Chat
/// Completions payloads. `NotSupported` maps to the `not_supported` error
/// type; anything else maps to `conversion_error`.
#[derive(Debug, Clone, PartialEq)]
enum ResponsesConversionError {
    NotSupported(String),
    Invalid(String),
}

impl ResponsesConversionError {
    fn message(&self) -> &str {
        match self {
            Self::NotSupported(message) | Self::Invalid(message) => message,
        }
    }
}

/// Convert Responses-format body to Chat Completions body for upstream routing.
fn responses_to_chat(body: &Value) -> std::result::Result<Value, ResponsesConversionError> {
    let messages = match body.get("input") {
        Some(Value::Array(items)) => convert_responses_input(items)?,
        Some(Value::String(s)) => {
            vec![json!({ "role": "user", "content": s })]
        }
        _ => vec![],
    };

    let mut chat_body = json!({
        "model": body.get("model").unwrap_or(&Value::Null),
        "messages": messages,
    });

    // Map common params
    if let Some(v) = body.get("max_output_tokens") {
        chat_body["max_tokens"] = v.clone();
    } else if let Some(v) = body.get("max_tokens") {
        chat_body["max_tokens"] = v.clone();
    }
    if let Some(v) = body.get("temperature") {
        chat_body["temperature"] = v.clone();
    }
    if let Some(v) = body.get("top_p") {
        chat_body["top_p"] = v.clone();
    }
    if let Some(v) = body.get("stream") {
        chat_body["stream"] = v.clone();
    }
    if let Some(v) = body.get("stop") {
        chat_body["stop"] = v.clone();
    }
    // Tools: map Responses tool format (name→function.name, description→function.description).
    // Only function tools can be represented in Chat Completions; anything
    // else (web_search, file_search, computer use, ...) is rejected explicitly.
    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        let mut chat_tools = Vec::new();
        for tool in tools {
            let tool_type = tool
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("function");
            if tool_type != "function" {
                return Err(ResponsesConversionError::NotSupported(format!(
                    "tool type '{tool_type}' is not supported in conversion mode",
                )));
            }
            chat_tools.push(json!({
                "type": "function",
                "function": {
                    "name": tool.get("name").unwrap_or(&Value::Null),
                    "description": tool.get("description").unwrap_or(&Value::Null),
                    "parameters": tool.get("parameters").unwrap_or(&json!({})),
                },
            }));
        }
        chat_body["tools"] = Value::Array(chat_tools);
        if let Some(v) = body.get("tool_choice") {
            chat_body["tool_choice"] = v.clone();
        }
    }
    // Instructions → system message prepended
    if let Some(instructions) = body.get("instructions").and_then(|v| v.as_str()) {
        if !instructions.is_empty() {
            let mut msgs = chat_body["messages"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            msgs.insert(0, json!({ "role": "system", "content": instructions }));
            chat_body["messages"] = Value::Array(msgs);
        }
    }

    Ok(chat_body)
}

/// Convert a Responses `input` array into Chat Completions `messages`,
/// preserving tool-call/tool-result correlation (`call_id` ↔ `tool_call_id`).
fn convert_responses_input(
    items: &[Value],
) -> std::result::Result<Vec<Value>, ResponsesConversionError> {
    let mut messages: Vec<Value> = Vec::new();
    for item in items {
        match item.get("type").and_then(|v| v.as_str()) {
            Some("function_call_output") => {
                let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                let output = item.get("output").cloned().unwrap_or(Value::Null);
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": output,
                }));
            }
            Some("function_call") => {
                // Standalone assistant function-call item: attach to the last
                // assistant message, or create one when none precedes it.
                let call = json!({
                    "id": item.get("call_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "type": "function",
                    "function": {
                        "name": item.get("name").unwrap_or(&Value::Null),
                        "arguments": item
                            .get("arguments")
                            .cloned()
                            .unwrap_or(Value::String(String::new())),
                    },
                });
                if let Some(last) = messages.last_mut() {
                    if last.get("role").and_then(|v| v.as_str()) == Some("assistant") {
                        if last.get("tool_calls").is_some() {
                            last["tool_calls"]
                                .as_array_mut()
                                .expect("tool_calls is an array")
                                .push(call);
                        } else {
                            last["tool_calls"] = json!([call]);
                        }
                        continue;
                    }
                }
                messages.push(json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "tool_calls": [call],
                }));
            }
            _ => {
                let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                let content = item.get("content").cloned().unwrap_or(Value::Null);
                let mut message = json!({ "role": role, "content": content });
                // An assistant message may embed function_call parts in its
                // content array (e.g. when a previous response.output was fed
                // back as input); pull them out into tool_calls.
                if role == "assistant" {
                    if let Some(calls) = extract_embedded_function_calls(&mut message)? {
                        message["tool_calls"] = calls;
                    }
                }
                messages.push(message);
            }
        }
    }
    Ok(messages)
}

/// Pull `{"type":"function_call",...}` parts out of an assistant message's
/// content array into the Chat Completions `tool_calls` shape; remaining text
/// parts become the message's string content.
fn extract_embedded_function_calls(
    message: &mut Value,
) -> std::result::Result<Option<Value>, ResponsesConversionError> {
    let Some(content) = message.get("content").and_then(|v| v.as_array()) else {
        return Ok(None);
    };
    let mut calls = Vec::new();
    let mut texts = Vec::new();
    for part in content {
        match part.get("type").and_then(|v| v.as_str()) {
            Some("function_call") => {
                calls.push(json!({
                    "id": part.get("call_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "type": "function",
                    "function": {
                        "name": part.get("name").unwrap_or(&Value::Null),
                        "arguments": part
                            .get("arguments")
                            .cloned()
                            .unwrap_or(Value::String(String::new())),
                    },
                }));
            }
            Some("output_text") | Some("text") | None => {
                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                    texts.push(text);
                }
            }
            _ => {}
        }
    }
    if calls.is_empty() {
        return Ok(None);
    }
    message["content"] = json!(texts.join(""));
    Ok(Some(Value::Array(calls)))
}

/// Convert a Chat Completions response body back to the Responses API format.
fn chat_response_to_responses(
    chat: Value,
    model: &str,
    created: Option<u64>,
) -> std::result::Result<Value, ResponsesConversionError> {
    let choices = chat
        .get("choices")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ResponsesConversionError::Invalid("chat response has no choices array".to_string())
        })?;

    let mut output = Vec::new();
    for choice in choices {
        let message = choice.get("message").unwrap_or(&Value::Null);
        // Text part becomes a Responses message output item.
        if let Some(content) = message.get("content") {
            if !content.is_null() {
                output.push(json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": match content {
                            Value::String(s) => s.clone(),
                            value => value.to_string(),
                        },
                        "annotations": [],
                    }],
                    "status": "completed",
                }));
            }
        }
        // Tool calls become Responses function_call output items, preserving
        // call_id / name / arguments for correlation with tool results.
        if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            for call in tool_calls {
                let function = call.get("function").unwrap_or(&Value::Null);
                output.push(json!({
                    "type": "function_call",
                    "call_id": call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "name": function.get("name").cloned().unwrap_or(Value::Null),
                    "arguments": function
                        .get("arguments")
                        .cloned()
                        .unwrap_or(Value::String(String::new())),
                    "status": "completed",
                }));
            }
        }
    }

    let usage = chat
        .get("usage")
        .cloned()
        .map(|u| chat_usage_to_responses_usage(&u));

    let mut resp = json!({
        "object": "response",
        "model": model,
        "output": output,
    });
    if let Some(id) = chat.get("id").and_then(|v| v.as_str()) {
        resp["id"] = json!(id);
    }
    if let Some(ts) = created.or_else(|| chat.get("created").and_then(|v| v.as_u64())) {
        resp["created_at"] = json!(ts as f64);
    }
    if let Some(u) = usage {
        resp["usage"] = u;
    }
    resp["status"] = json!("completed");

    Ok(resp)
}

/// Map a Chat Completions `usage` object to the Responses API usage shape.
fn chat_usage_to_responses_usage(usage: &Value) -> Value {
    json!({
        "input_tokens": usage.get("prompt_tokens").unwrap_or(&Value::Null),
        "output_tokens": usage.get("completion_tokens").unwrap_or(&Value::Null),
        "total_tokens": usage.get("total_tokens").unwrap_or(&Value::Null),
        "input_tokens_details": {
            "cached_tokens": usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .unwrap_or(&Value::Null),
        },
        "output_tokens_details": {
            "reasoning_tokens": usage
                .get("completion_tokens_details")
                .and_then(|d| d.get("reasoning_tokens"))
                .unwrap_or(&Value::Null),
        },
    })
}

struct ChatToolCallState {
    index: usize,
    call_id: String,
    name: String,
    arguments: String,
    item_id: String,
    output_index: usize,
}

/// Streaming state machine converting Chat Completions SSE frames into
/// OpenAI Responses SSE events. Feed one parsed `data:` payload per
/// `push_frame`; collected events are returned in emit order. `finish`
/// emits the closing done/completed events once the stream ends.
struct ChatSseToResponses {
    response_id: String,
    model: String,
    created_at: u64,
    /// Next output index to assign.
    output_index: usize,
    /// Whether response.created has been emitted.
    response_started: bool,
    /// Whether the assistant message item (and its content part) is open.
    message_open: bool,
    message_item_id: String,
    message_output_index: usize,
    text: String,
    tool_calls: Vec<ChatToolCallState>,
    usage: Option<Value>,
}

impl ChatSseToResponses {
    fn new(model: &str) -> Self {
        Self {
            response_id: format!("resp_{}", Utc::now().timestamp()),
            model: model.to_string(),
            created_at: Utc::now().timestamp() as u64,
            output_index: 0,
            response_started: false,
            message_open: false,
            message_item_id: String::new(),
            message_output_index: 0,
            text: String::new(),
            tool_calls: Vec::new(),
            usage: None,
        }
    }

    fn push_frame(
        &mut self,
        data: &Value,
    ) -> std::result::Result<Vec<Value>, ResponsesConversionError> {
        let mut events = Vec::new();
        let choices = data.get("choices").and_then(|v| v.as_array());

        // Usage-only tail frame: `{"choices": [], "usage": {...}}`.
        if let Some(usage) = data.get("usage") {
            self.usage = Some(chat_usage_to_responses_usage(usage));
        }
        let Some(choices) = choices else {
            return Ok(events);
        };
        if choices.is_empty() {
            return Ok(events);
        }
        let choice = &choices[0];
        let Some(delta) = choice.get("delta") else {
            return Ok(events);
        };
        if !delta.is_object() {
            return Err(ResponsesConversionError::Invalid(
                "unexpected chat stream delta".to_string(),
            ));
        }

        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
            if !content.is_empty() {
                if !self.response_started {
                    self.response_started = true;
                    events.push(self.emit_created());
                }
                if !self.message_open {
                    self.message_open = true;
                    events.push(self.emit_message_added());
                    events.push(self.emit_content_part_added());
                }
                self.text.push_str(content);
                events.push(self.emit_text_delta(content));
            }
        }

        if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            if !self.response_started {
                self.response_started = true;
                events.push(self.emit_created());
            }
            for call in tool_calls {
                let index = call.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let slot = match self.tool_calls.iter_mut().find(|t| t.index == index) {
                    Some(slot) => slot,
                    None => {
                        let slot = ChatToolCallState {
                            index,
                            call_id: call
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            name: call
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            arguments: String::new(),
                            item_id: format!("fc_{}_{}", self.output_index, index),
                            output_index: self.output_index,
                        };
                        self.output_index += 1;
                        events.push(self.emit_function_call_added(&slot));
                        self.tool_calls.push(slot);
                        self.tool_calls.last_mut().unwrap()
                    }
                };
                if let Some(arguments) = call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|v| v.as_str())
                {
                    if !arguments.is_empty() {
                        slot.arguments.push_str(arguments);
                        events.push(Self::emit_arguments_delta(slot, arguments));
                    }
                }
            }
        }

        Ok(events)
    }

    fn finish(&mut self) -> Vec<Value> {
        let mut events = Vec::new();
        if self.message_open && !self.text.is_empty() {
            events.push(json!({
                "type": "response.output_text.done",
                "item_id": self.message_item_id,
                "output_index": self.message_output_index,
                "content_index": 0,
                "text": self.text,
            }));
            events.push(json!({
                "type": "response.content_part.done",
                "item_id": self.message_item_id,
                "output_index": self.message_output_index,
                "content_index": 0,
                "part": {
                    "type": "output_text",
                    "text": self.text,
                    "annotations": [],
                },
            }));
            events.push(json!({
                "type": "response.output_item.done",
                "output_index": self.message_output_index,
                "item": self.message_item("completed"),
            }));
        }
        for call in &self.tool_calls {
            events.push(json!({
                "type": "response.function_call_arguments.done",
                "item_id": call.item_id,
                "output_index": call.output_index,
                "arguments": call.arguments,
            }));
            events.push(json!({
                "type": "response.output_item.done",
                "output_index": call.output_index,
                "item": Self::tool_call_item(call, "completed"),
            }));
        }

        let mut response = json!({
            "id": self.response_id,
            "object": "response",
            "created_at": self.created_at as f64,
            "status": "completed",
            "model": self.model,
            "output": self.completed_output(),
        });
        if let Some(usage) = &self.usage {
            response["usage"] = usage.clone();
        }
        events.push(json!({
            "type": "response.completed",
            "response": response,
        }));
        events
    }

    /// Structured failure event emitted when a chat frame cannot be converted,
    /// so the client never sees a fake successful completion.
    fn failed_event(&self, message: &str) -> Value {
        json!({
            "type": "response.failed",
            "response": {
                "id": self.response_id,
                "object": "response",
                "created_at": self.created_at as f64,
                "status": "failed",
                "model": self.model,
                "error": {
                    "message": message,
                    "type": "conversion_error",
                },
            },
        })
    }

    fn completed_output(&self) -> Vec<Value> {
        let mut output = Vec::new();
        if self.message_open && !self.text.is_empty() {
            output.push(self.message_item("completed"));
        }
        for call in &self.tool_calls {
            output.push(Self::tool_call_item(call, "completed"));
        }
        output
    }

    fn message_item(&self, status: &str) -> Value {
        json!({
            "id": self.message_item_id,
            "type": "message",
            "role": "assistant",
            "status": status,
            "content": [{
                "type": "output_text",
                "text": self.text,
                "annotations": [],
            }],
        })
    }

    fn tool_call_item(call: &ChatToolCallState, status: &str) -> Value {
        json!({
            "id": call.item_id,
            "type": "function_call",
            "call_id": call.call_id,
            "name": call.name,
            "arguments": call.arguments,
            "status": status,
        })
    }

    fn emit_created(&self) -> Value {
        json!({
            "type": "response.created",
            "response": {
                "id": self.response_id,
                "object": "response",
                "created_at": self.created_at as f64,
                "status": "in_progress",
                "model": self.model,
                "output": [],
            },
        })
    }

    fn emit_message_added(&mut self) -> Value {
        self.message_item_id = format!("msg_{}", self.output_index);
        self.message_output_index = self.output_index;
        self.output_index += 1;
        json!({
            "type": "response.output_item.added",
            "output_index": self.message_output_index,
            "item": {
                "id": self.message_item_id,
                "type": "message",
                "role": "assistant",
                "status": "in_progress",
                "content": [],
            },
        })
    }

    fn emit_content_part_added(&self) -> Value {
        json!({
            "type": "response.content_part.added",
            "item_id": self.message_item_id,
            "output_index": self.message_output_index,
            "content_index": 0,
            "part": { "type": "output_text", "text": "", "annotations": [] },
        })
    }

    fn emit_text_delta(&self, delta: &str) -> Value {
        json!({
            "type": "response.output_text.delta",
            "item_id": self.message_item_id,
            "output_index": self.message_output_index,
            "content_index": 0,
            "delta": delta,
        })
    }

    fn emit_function_call_added(&self, call: &ChatToolCallState) -> Value {
        json!({
            "type": "response.output_item.added",
            "output_index": call.output_index,
            "item": {
                "id": call.item_id,
                "type": "function_call",
                "call_id": call.call_id,
                "name": call.name,
                "arguments": "",
                "status": "in_progress",
            },
        })
    }

    fn emit_arguments_delta(call: &ChatToolCallState, delta: &str) -> Value {
        json!({
            "type": "response.function_call_arguments.delta",
            "item_id": call.item_id,
            "output_index": call.output_index,
            "delta": delta,
        })
    }
}

/// Extract the `data:` payload of one SSE frame.
fn frame_data(frame: &[u8]) -> Option<&str> {
    for line in frame.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if let Some(data) = line.strip_prefix(b"data:") {
            let data = std::str::from_utf8(data).ok()?.trim();
            if !data.is_empty() {
                return Some(data);
            }
        }
    }
    None
}

/// Serialize one Responses event as an SSE frame (`event:` + `data:`).
fn encode_sse_event(event: &Value) -> Vec<u8> {
    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let json = serde_json::to_string(event).unwrap_or_default();
    format!("event: {event_type}\ndata: {json}\n\n").into_bytes()
}

/// Whether the profile should serve Responses streams through the
/// Chat→Responses SSE conversion path.
fn is_chat_completions_convert(profile: &ProviderProfile) -> bool {
    profile.api == "openai-completions"
        && matches!(
            profile.responses_mode,
            crate::config::ResponsesMode::Auto | crate::config::ResponsesMode::Convert
        )
}

/// Wrap an upstream Chat Completions SSE stream, converting each frame into
/// OpenAI Responses SSE events as it flows. Usage is parsed on the side for
/// the request log; the log line is written when the stream ends.
struct ResponsesStreamTransform<S> {
    inner: S,
    converter: ChatSseToResponses,
    buffer: Vec<u8>,
    pending: std::collections::VecDeque<axum::body::Bytes>,
    usage: Option<crate::usage::UsageSummary>,
    fields: Option<StreamLogFields>,
    finished: bool,
    /// Set when a chat frame cannot be converted; the stream is then closed
    /// with a structured `response.failed` event instead of a fake success.
    conversion_error: Option<String>,
}

impl<S, E> futures_util::Stream for ResponsesStreamTransform<S>
where
    S: futures_util::Stream<Item = std::result::Result<axum::body::Bytes, E>> + Unpin,
    E: std::error::Error + Send + Sync + 'static,
{
    type Item = std::result::Result<axum::body::Bytes, Box<dyn std::error::Error + Send + Sync>>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Loop so a chunk that yields no output does not return Pending without
        // a registered waker (which would stall the stream after the last frame).
        loop {
            if let Some(bytes) = self.pending.pop_front() {
                return std::task::Poll::Ready(Some(Ok(bytes)));
            }
            if self.finished {
                return std::task::Poll::Ready(None);
            }
            match std::pin::Pin::new(&mut self.inner).poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(chunk))) => {
                    self.buffer.extend_from_slice(&chunk);
                    self.drain_frames();
                }
                std::task::Poll::Ready(Some(Err(e))) => {
                    self.flush_log(Some(e.to_string()));
                    self.finished = true;
                    return std::task::Poll::Ready(Some(Err(Box::new(e))));
                }
                std::task::Poll::Ready(None) => {
                    if let Some(message) = self.conversion_error.take() {
                        let event = self.converter.failed_event(&message);
                        self.pending
                            .push_back(axum::body::Bytes::from(encode_sse_event(&event)));
                        self.flush_log(Some(message));
                    } else {
                        for event in self.converter.finish() {
                            self.pending
                                .push_back(axum::body::Bytes::from(encode_sse_event(&event)));
                        }
                        self.flush_log(None);
                    }
                    self.finished = true;
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

impl<S> ResponsesStreamTransform<S> {
    fn new(inner: S, converter: ChatSseToResponses, fields: StreamLogFields) -> Self {
        Self {
            inner,
            converter,
            buffer: Vec::new(),
            pending: std::collections::VecDeque::new(),
            usage: None,
            fields: Some(fields),
            finished: false,
            conversion_error: None,
        }
    }

    fn drain_frames(&mut self) {
        while let Some((end, separator_len)) = crate::usage::frame_end(&self.buffer) {
            let frame: Vec<u8> = self.buffer[..end].to_vec();
            self.buffer.drain(..end + separator_len);
            let Some(data) = frame_data(&frame) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(usage) = crate::usage::extract_usage(&value) {
                self.usage = Some(usage);
            }
            match self.converter.push_frame(&value) {
                Ok(events) => {
                    for event in events {
                        self.pending
                            .push_back(axum::body::Bytes::from(encode_sse_event(&event)));
                    }
                }
                Err(error) => {
                    if self.conversion_error.is_none() {
                        self.conversion_error = Some(error.message().to_string());
                    }
                }
            }
        }
    }

    fn flush_log(&mut self, error: Option<String>) {
        if let Some(fields) = self.fields.take() {
            append_log_line(&stream_log_entry(fields, self.usage.as_ref(), error));
        }
    }
}

async fn handle_responses(
    State(_state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let config = crate::config::load_config().unwrap_or_default();
    handle_responses_with_config(&config, headers, body).await
}

async fn handle_responses_with_config(
    config: &crate::config::PiSwitchConfig,
    headers: HeaderMap,
    body: String,
) -> Response {
    let body_value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let body_value = filter_private_params(body_value);
    let is_stream = body_value
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Non-streaming: route through candidates in order, dispatching each by its
    // declared responses mode (native passthrough or Chat conversion), so
    // failover can move across modes.
    if !is_stream {
        let requested_model = body_value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (candidates, real_model) = resolve_route(config, requested_model);
        if candidates.is_empty() {
            return (StatusCode::BAD_GATEWAY,
                Json(json!({ "error": { "message": format!("No upstream exposes model '{}'", requested_model), "type": "no_route" } }))).into_response();
        }
        let conversation_id = conversation_id_of(&headers, &body_value);
        let result = forward_responses_mixed(
            config,
            &candidates,
            &body_value,
            &real_model,
            &headers,
            conversation_id.as_deref(),
        )
        .await;
        match result {
            Ok(response) => response,
            Err(error) => (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": { "message": error.to_string(), "type": "failover_exhausted" } })),
            ).into_response(),
        }
    } else {
        // Streaming: route through candidates in order, dispatching each by its
        // declared responses mode (native passthrough or Chat→Responses SSE
        // conversion), so failover can move across modes before any event is sent.
        let requested_model = body_value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (candidates, real_model) = resolve_route(config, requested_model);
        let conversation_id = conversation_id_of(&headers, &body_value);
        if candidates.is_empty() {
            return (StatusCode::NOT_IMPLEMENTED,
                Json(json!({ "error": { "message": "Responses stream requires an openai-responses or openai-completions upstream", "type": "not_supported" } }))).into_response();
        }
        let result = forward_responses_mixed_stream(
            config,
            &candidates,
            &body_value,
            &real_model,
            &headers,
            conversation_id.as_deref(),
        )
        .await;
        match result {
            Ok(resp) => resp,
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(
                    json!({ "error": { "message": e.to_string(), "type": "failover_exhausted" } }),
                ),
            )
                .into_response(),
        }
    }
}

// ─── Routing ──────────────────────────────────────────────

/// Whether `name` is a known, non-proxy profile.
fn is_non_proxy(config: &crate::config::PiSwitchConfig, name: &str) -> bool {
    config
        .profiles
        .get(name)
        .map(|p| !p.get("proxy").and_then(|v| v.as_bool()).unwrap_or(false))
        .unwrap_or(false)
}

/// Whether profile `name` exposes the (real) model id `model`.
fn exposes(config: &crate::config::PiSwitchConfig, name: &str, model: &str) -> bool {
    config
        .profiles
        .get(name)
        .and_then(|p| p.get("exposedModels"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|m| m.as_str() == Some(model)))
        .unwrap_or(false)
}

/// All non-proxy profiles that expose at least one model.
fn exposed_profiles(config: &crate::config::PiSwitchConfig) -> Vec<String> {
    config
        .profiles
        .iter()
        .filter(|(_, p)| !p.get("proxy").and_then(|v| v.as_bool()).unwrap_or(false))
        .filter(|(_, p)| {
            p.get("exposedModels")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        })
        .map(|(name, _)| name.clone())
        .collect()
}

/// Resolve a (namespaced) requested model into the ordered list of profiles to try and the
/// real upstream model id to send. Stateless — derived entirely from the request + config.
///
/// - `"profile/real"` → primary `profile`, then failover-chain profiles that also expose `real`.
/// - bare `"id"` (defensive fallback) → every non-proxy profile exposing `id`, failover-first.
///
/// Splits on the FIRST `/` only, so real ids that themselves contain `/`
/// (e.g. `openrouter/anthropic/claude-sonnet-4.5`) resolve correctly.
fn resolve_route(config: &crate::config::PiSwitchConfig, requested: &str) -> (Vec<String>, String) {
    if let Some((prefix, rest)) = requested.split_once('/') {
        if is_non_proxy(config, prefix) && exposes(config, prefix, rest) {
            let mut profiles = vec![prefix.to_string()];
            for fo in &config.settings.proxy.failover {
                if fo != prefix
                    && is_non_proxy(config, fo)
                    && exposes(config, fo, rest)
                    && !profiles.contains(fo)
                {
                    profiles.push(fo.clone());
                }
            }
            return (profiles, rest.to_string());
        }
    }

    // Bare / unknown namespacing: any non-proxy profile exposing the whole string,
    // failover-chain order first.
    let mut profiles = Vec::new();
    for fo in &config.settings.proxy.failover {
        if is_non_proxy(config, fo) && exposes(config, fo, requested) && !profiles.contains(fo) {
            profiles.push(fo.clone());
        }
    }
    for name in config.profiles.keys() {
        if is_non_proxy(config, name)
            && exposes(config, name, requested)
            && !profiles.contains(name)
        {
            profiles.push(name.clone());
        }
    }
    (profiles, requested.to_string())
}

// ─── Request body filtering ───────────────────────────────

/// Strip `_`-prefixed private fields recursively before forwarding upstream, so internal
/// tracking params don't leak or trip strict upstream channels. JSON-Schema field names
/// (under properties / patternProperties / definitions / $defs) are user data and kept.
/// Ported from cc-switch's body_filter.
fn filter_private_params(value: Value) -> Value {
    fn recurse(value: Value, parent_key: Option<&str>) -> Value {
        match value {
            Value::Object(map) => {
                let in_schema_names = matches!(
                    parent_key,
                    Some("properties" | "patternProperties" | "definitions" | "$defs")
                );
                let filtered = map
                    .into_iter()
                    .filter_map(|(key, val)| {
                        if key.starts_with('_') && !in_schema_names {
                            None
                        } else {
                            let child = recurse(val, Some(&key));
                            Some((key, child))
                        }
                    })
                    .collect();
                Value::Object(filtered)
            }
            Value::Array(arr) => {
                Value::Array(arr.into_iter().map(|v| recurse(v, parent_key)).collect())
            }
            other => other,
        }
    }
    recurse(value, None)
}

// ─── Conversation id ───────────────────────────────────────

/// The client-supplied conversation identifier for a request: the
/// `x-conversation-id` request header wins, then `x-opencode-session`
/// (sent by pi/open-code clients), and the body `conversation_id`
/// field is the last fallback. Empty or non-string values are ignored.
fn conversation_id_of(headers: &HeaderMap, body: &Value) -> Option<String> {
    for name in ["x-conversation-id", "x-opencode-session"] {
        if let Some(value) = headers.get(name).and_then(|v| v.to_str().ok()) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    body.get("conversation_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// The conversation display name from the `x-conversation-name` request
/// header. Header-only source (no body fallback); empty values are ignored.
/// The client extension percent-encodes non-Latin1 characters (> 0xff) so the
/// header stays HTTP-safe; this decodes them back for a readable name. Only a
/// fully valid UTF-8 decode result is taken — a literal "%AB" that is not
/// valid UTF-8 (e.g. "100%EF") keeps the raw value instead of being
/// mis-decoded. Control characters (tab/newline — legal tab may survive HTTP
/// parsing, and %0A decodes to a newline) are collapsed to spaces so the name
/// stays clean in logs/exports. The name is a display attribute only — it
/// never participates in conversation-boundary detection (ADR-0002).
fn conversation_name_of(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-conversation-name")
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            percent_encoding::percent_decode_str(s)
                .decode_utf8()
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| s.to_string())
        })
        .map(|s| s.replace(['\r', '\n', '\t'], " "))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ─── Response passthrough (streaming + header preservation) ─

/// Wrap an upstream response stream: copy every chunk into an `SseUsageParser`
/// while forwarding it unchanged (no buffering, token-by-token passthrough),
/// then run `on_finish` exactly once — on normal end, on error, or when the
/// stream is dropped mid-flight (client cut the connection).
struct StreamTee<S> {
    inner: S,
    parser: crate::usage::SseUsageParser,
    on_finish: Option<StreamFinish>,
    /// Set when the upstream stream ends with an error; passed to `on_finish`
    /// so an interrupted stream is logged as a failure, not a success.
    error: Option<String>,
}

/// Callback run once when the stream ends: (parsed usage, upstream error).
type StreamFinish = Box<dyn FnOnce(Option<crate::usage::UsageSummary>, Option<String>) + Send>;

impl<S, E> futures_util::Stream for StreamTee<S>
where
    S: futures_util::Stream<Item = std::result::Result<axum::body::Bytes, E>> + Unpin,
    E: std::error::Error + Send + Sync + 'static,
{
    type Item = std::result::Result<axum::body::Bytes, Box<dyn std::error::Error + Send + Sync>>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match std::pin::Pin::new(&mut self.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(bytes))) => {
                self.parser.push(&bytes);
                std::task::Poll::Ready(Some(Ok(bytes)))
            }
            std::task::Poll::Ready(Some(Err(e))) => {
                self.error = Some(e.to_string());
                self.flush_log();
                std::task::Poll::Ready(Some(Err(Box::new(e))))
            }
            std::task::Poll::Ready(None) => {
                self.flush_log();
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl<S> Drop for StreamTee<S> {
    fn drop(&mut self) {
        self.flush_log();
    }
}

impl<S> StreamTee<S> {
    fn new(inner: S, on_finish: StreamFinish) -> Self {
        Self {
            inner,
            parser: crate::usage::SseUsageParser::new(),
            on_finish: Some(on_finish),
            error: None,
        }
    }

    fn flush_log(&mut self) {
        if let Some(cb) = self.on_finish.take() {
            cb(self.parser.finish(), self.error.take());
        }
    }
}

/// Upstream headers to forward to the client, minus per-hop framing headers the
/// server recomputes. Keeps Content-Type / Content-Encoding / SSE headers intact.
fn forward_headers(
    src: &reqwest::header::HeaderMap,
) -> Vec<(reqwest::header::HeaderName, reqwest::header::HeaderValue)> {
    src.iter()
        .filter(|(n, _)| {
            let s = n.as_str();
            !s.eq_ignore_ascii_case("content-length")
                && !s.eq_ignore_ascii_case("transfer-encoding")
                && !s.eq_ignore_ascii_case("connection")
        })
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect()
}

/// Log fields captured at request time, flushed when the response stream ends
/// (usage is parsed from the stream itself).
struct StreamLogFields {
    provider: String,
    ok: bool,
    error: Option<String>,
    status: Option<u16>,
    upstream_url: Option<String>,
    model: Option<String>,
    conversation_id: Option<String>,
    /// Conversation display name (header-only; never part of boundary
    /// detection, ADR-0002). Display attribute only.
    conversation_name: Option<String>,
    /// The model's unit prices at request time (per-request config reload);
    /// `None` means the model has no configured price → cost is unknown.
    cost: Option<crate::config::ModelCost>,
}

impl StreamLogFields {
    /// Fields for a successful passthrough response (the common tee path).
    fn for_success(
        provider: &str,
        status: u16,
        upstream_url: &str,
        model: Option<&str>,
        conversation_id: Option<&str>,
        conversation_name: Option<&str>,
    ) -> Self {
        Self {
            provider: provider.to_string(),
            ok: true,
            error: None,
            status: Some(status),
            upstream_url: Some(upstream_url.to_string()),
            model: model.map(|s| s.to_string()),
            conversation_id: conversation_id.map(|s| s.to_string()),
            conversation_name: conversation_name.map(|s| s.to_string()),
            cost: None,
        }
    }
}

/// Stream an upstream response straight through to the client, preserving status and
/// headers. Enables token-by-token SSE and keeps Content-Type (which the old buffered
/// path dropped). Used for same-format passthrough (not the OpenAI↔Anthropic convert path).
///
/// When `log` is provided, the response stream is teed: every chunk is fed into the
/// usage parser while being forwarded unchanged, and the log line (with token usage
/// and conversation id) is appended once the stream ends — normally, on error, or
/// when the client cuts the connection.
fn stream_response(r: reqwest::Response, log: Option<StreamLogFields>) -> Response {
    let status = r.status().as_u16();
    let headers = forward_headers(r.headers());
    let mut builder = Response::builder().status(status);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }

    let body = match log {
        Some(fields) => {
            let tee = StreamTee::new(
                r.bytes_stream(),
                Box::new(move |usage, error| {
                    append_log_line(&stream_log_entry(fields, usage.as_ref(), error));
                }),
            );
            Body::from_stream(tee)
        }
        None => Body::from_stream(r.bytes_stream()),
    };

    builder.body(body).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::empty())
            .unwrap()
    })
}

/// Build the log entry for a streamed passthrough response; an upstream error
/// mid-stream marks the request as failed so truncated streams are diagnosable.
fn stream_log_entry(
    mut fields: StreamLogFields,
    usage: Option<&crate::usage::UsageSummary>,
    error: Option<String>,
) -> Value {
    if let Some(error) = error {
        fields.ok = false;
        fields.error = Some(error);
    }
    build_log_entry(&fields, usage)
}

fn is_native_responses_passthrough(profile: &ProviderProfile) -> bool {
    profile.api == "openai-responses"
        && matches!(
            profile.responses_mode,
            crate::config::ResponsesMode::Auto | crate::config::ResponsesMode::Passthrough
        )
}

fn is_hop_by_hop_request_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host"
            | "connection"
            | "content-length"
            | "transfer-encoding"
            | "upgrade"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
    )
}

fn build_upstream_headers(
    client_headers: &HeaderMap,
    profile: &ProviderProfile,
    api_key: &str,
    user_agent: Option<&String>,
    disguise: &[(&'static str, &'static str)],
) -> HeaderMap {
    let mut output = HeaderMap::new();
    for (name, value) in client_headers {
        if is_hop_by_hop_request_header(name.as_str())
            || name.as_str().eq_ignore_ascii_case("authorization")
        {
            continue;
        }
        output.append(name.clone(), value.clone());
    }
    if let Some(custom) = &profile.headers {
        for (name, value) in custom {
            let Some(value) = value.as_str() else {
                continue;
            };
            let Ok(name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) else {
                continue;
            };
            let Ok(value) =
                reqwest::header::HeaderValue::from_str(&crate::config::resolve_env(value))
            else {
                continue;
            };
            output.insert(name, value);
        }
    }
    output.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
            .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static("Bearer")),
    );
    output
        .entry(reqwest::header::CONTENT_TYPE)
        .or_insert_with(|| reqwest::header::HeaderValue::from_static("application/json"));
    if let Some(user_agent) = user_agent {
        if let Ok(value) = reqwest::header::HeaderValue::from_str(user_agent) {
            output.insert(reqwest::header::USER_AGENT, value);
        }
    }
    for (name, value) in disguise {
        output.insert(*name, reqwest::header::HeaderValue::from_static(value));
    }
    output
}

fn buffered_response(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: Vec<u8>,
) -> Response {
    let mut builder = Response::builder().status(status.as_u16());
    for (name, value) in forward_headers(headers) {
        builder = builder.header(name, value);
    }
    builder.body(Body::from(body)).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::empty())
            .unwrap()
    })
}

/// Log a failed attempt against one candidate (retryable, non-retryable, or
/// transport error) — shared by the mixed failover loops.
#[allow(clippy::too_many_arguments)]
async fn log_failed_attempt(
    name: &str,
    error: Option<&str>,
    status: Option<u16>,
    url: Option<&str>,
    model: Option<&str>,
    conversation_id: Option<&str>,
    conversation_name: Option<&str>,
) {
    log_request(
        name,
        false,
        error,
        status,
        url,
        None,
        model,
        None,
        conversation_id,
        conversation_name,
        None,
    )
    .await
}

async fn forward_responses_mixed(
    config: &crate::config::PiSwitchConfig,
    candidates: &[String],
    body: &Value,
    real_model: &str,
    headers: &HeaderMap,
    conversation_id: Option<&str>,
) -> Result<Response> {
    let conversation_name = conversation_name_of(headers);
    let circuit_settings = &config.settings.proxy.circuit_breaker;
    let mut circuit_state = read_circuit_state().await;
    let global_spoof = config.settings.proxy.user_agent.as_deref();
    let mut half_open_used = false;
    // Remembered so a conversion failure only surfaces if the whole chain fails.
    let mut conversion_error: Option<ResponsesConversionError> = None;
    let mut out_body = body.clone();
    if !real_model.is_empty() {
        out_body["model"] = json!(real_model);
    }
    let body = &out_body;

    for name in candidates {
        let (is_open, is_half_open) = is_circuit_open(&circuit_state, name, circuit_settings);
        if is_open {
            log_failed_attempt(
                name,
                Some("circuit_open"),
                None,
                None,
                body.get("model").and_then(|v| v.as_str()),
                conversation_id,
                conversation_name.as_deref(),
            )
            .await;
            continue;
        }
        if is_half_open {
            if half_open_used {
                log_failed_attempt(
                    name,
                    Some("half_open_already_probing"),
                    None,
                    None,
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                continue;
            }
            half_open_used = true;
        }
        let Some(profile_value) = config.profiles.get(name) else {
            continue;
        };
        let Ok(profile) = serde_json::from_value::<ProviderProfile>(profile_value.clone()) else {
            continue;
        };
        let is_native = is_native_responses_passthrough(&profile);
        let is_convert = is_chat_completions_convert(&profile);
        if !is_native && !is_convert {
            continue;
        }
        let effective_spoof = profile.spoof.as_deref().or(global_spoof);
        let (client, user_agent, disguise) = build_disguised_client(effective_spoof);
        let api_key = crate::config::resolve_env(&profile.api_key);
        let base = profile.base_url.trim_end_matches('/');
        let url = if is_native {
            format!("{base}/responses")
        } else {
            format!("{base}/chat/completions")
        };
        let request_headers =
            build_upstream_headers(headers, &profile, &api_key, user_agent.as_ref(), &disguise);
        let send_body = if is_native {
            body.clone()
        } else {
            match responses_to_chat(body) {
                Ok(converted) => converted,
                Err(error) => {
                    conversion_error = Some(error);
                    continue;
                }
            }
        };
        let response = client
            .post(&url)
            .headers(request_headers)
            .json(&send_body)
            .send()
            .await;

        match response {
            Ok(upstream) if upstream.status().is_success() => {
                let status = upstream.status();
                let response_headers = upstream.headers().clone();
                let body_bytes = upstream.bytes().await.unwrap_or_default().to_vec();
                record_success(name, is_half_open).await;
                let model = body.get("model").and_then(|v| v.as_str());
                if is_native {
                    let usage = serde_json::from_slice::<Value>(&body_bytes)
                        .ok()
                        .and_then(|value| crate::usage::extract_usage(&value));
                    log_request(
                        name,
                        true,
                        None,
                        Some(status.as_u16()),
                        Some(&url),
                        None,
                        model,
                        usage,
                        conversation_id,
                        conversation_name.as_deref(),
                        lookup_model_cost(&profile, real_model),
                    )
                    .await;
                    return Ok(buffered_response(status, &response_headers, body_bytes));
                }
                // Convert: map the chat body to Responses semantics.
                match serde_json::from_slice::<Value>(&body_bytes) {
                    Ok(chat) => {
                        let usage = crate::usage::extract_usage(&chat);
                        match chat_response_to_responses(
                            chat,
                            real_model,
                            Some(chrono::Utc::now().timestamp() as u64),
                        ) {
                            Ok(responses_body) => {
                                log_request(
                                    name,
                                    true,
                                    None,
                                    Some(status.as_u16()),
                                    Some(&url),
                                    None,
                                    model,
                                    usage,
                                    conversation_id,
                                    conversation_name.as_deref(),
                                    lookup_model_cost(&profile, real_model),
                                )
                                .await;
                                let s = serde_json::to_string(&responses_body).unwrap_or_default();
                                return Ok(Response::builder()
                                    .status(200)
                                    .header("content-type", "application/json")
                                    .body(Body::from(s))
                                    .unwrap());
                            }
                            Err(error) => {
                                log_request(
                                    name,
                                    false,
                                    Some("conversion_error"),
                                    Some(status.as_u16()),
                                    Some(&url),
                                    None,
                                    model,
                                    None,
                                    conversation_id,
                                    conversation_name.as_deref(),
                                    None,
                                )
                                .await;
                                conversion_error = Some(error);
                            }
                        }
                    }
                    Err(_) => {
                        // 2xx but not JSON: pass through unchanged rather than
                        // fabricating a response.
                        log_request(
                            name,
                            true,
                            None,
                            Some(status.as_u16()),
                            Some(&url),
                            None,
                            model,
                            None,
                            conversation_id,
                            conversation_name.as_deref(),
                            None,
                        )
                        .await;
                        return Ok(buffered_response(status, &response_headers, body_bytes));
                    }
                }
            }
            Ok(upstream) if should_retry(upstream.status().as_u16()) => {
                let status = upstream.status().as_u16();
                record_failure(
                    name,
                    circuit_settings,
                    &format!("HTTP {status}"),
                    is_half_open,
                )
                .await;
                log_failed_attempt(
                    name,
                    Some(&format!("HTTP {status}")),
                    Some(status),
                    Some(&url),
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                circuit_state = read_circuit_state().await;
            }
            Ok(upstream) => {
                let status = upstream.status();
                let response_headers = upstream.headers().clone();
                let body_bytes = upstream.bytes().await.unwrap_or_default().to_vec();
                log_failed_attempt(
                    name,
                    None,
                    Some(status.as_u16()),
                    Some(&url),
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                return Ok(buffered_response(status, &response_headers, body_bytes));
            }
            Err(error) => {
                let message = error.to_string();
                record_failure(name, circuit_settings, &message, is_half_open).await;
                log_failed_attempt(
                    name,
                    Some(&message),
                    None,
                    None,
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                circuit_state = read_circuit_state().await;
            }
        }
    }
    if let Some(error) = conversion_error {
        return Ok(conversion_error_response(&error));
    }
    Err(AppError::proxy(
        "All Responses upstream attempts failed".to_string(),
    ))
}

/// Route a Responses streaming request through candidates in order, dispatching
/// each by its declared mode: native providers stream through untouched,
/// Chat Completions providers are translated into Responses SSE events. Failover
/// may move across modes before any header/event reaches the client.
async fn forward_responses_mixed_stream(
    config: &crate::config::PiSwitchConfig,
    candidates: &[String],
    body: &Value,
    real_model: &str,
    headers: &HeaderMap,
    conversation_id: Option<&str>,
) -> Result<Response> {
    let conversation_name = conversation_name_of(headers);
    let circuit_settings = &config.settings.proxy.circuit_breaker;
    let mut circuit_state = read_circuit_state().await;
    let global_spoof = config.settings.proxy.user_agent.as_deref();
    let mut half_open_used = false;
    let mut conversion_error: Option<ResponsesConversionError> = None;
    let mut out_body = body.clone();
    if !real_model.is_empty() {
        out_body["model"] = json!(real_model);
    }
    let body = &out_body;

    for name in candidates {
        let (is_open, is_half_open) = is_circuit_open(&circuit_state, name, circuit_settings);
        if is_open {
            log_failed_attempt(
                name,
                Some("circuit_open"),
                None,
                None,
                body.get("model").and_then(|v| v.as_str()),
                conversation_id,
                conversation_name.as_deref(),
            )
            .await;
            continue;
        }
        if is_half_open {
            if half_open_used {
                log_failed_attempt(
                    name,
                    Some("half_open_already_probing"),
                    None,
                    None,
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                continue;
            }
            half_open_used = true;
        }
        let Some(profile_value) = config.profiles.get(name) else {
            continue;
        };
        let Ok(profile) = serde_json::from_value::<ProviderProfile>(profile_value.clone()) else {
            continue;
        };
        let is_native = is_native_responses_passthrough(&profile);
        let is_convert = is_chat_completions_convert(&profile);
        if !is_native && !is_convert {
            continue;
        }
        let effective_spoof = profile.spoof.as_deref().or(global_spoof);
        let (client, user_agent, disguise) = build_disguised_client(effective_spoof);
        let api_key = crate::config::resolve_env(&profile.api_key);
        let base = profile.base_url.trim_end_matches('/');
        let url = if is_native {
            format!("{base}/responses")
        } else {
            format!("{base}/chat/completions")
        };
        let request_headers =
            build_upstream_headers(headers, &profile, &api_key, user_agent.as_ref(), &disguise);
        let send_body = if is_native {
            body.clone()
        } else {
            match responses_to_chat(body) {
                Ok(mut converted) => {
                    converted["stream"] = json!(true);
                    converted
                }
                Err(error) => {
                    conversion_error = Some(error);
                    continue;
                }
            }
        };
        let response = client
            .post(&url)
            .headers(request_headers)
            .json(&send_body)
            .send()
            .await;

        match response {
            Ok(upstream) if upstream.status().is_success() => {
                let status = upstream.status().as_u16();
                let model = body.get("model").and_then(|v| v.as_str());
                if is_native {
                    record_success(name, is_half_open).await;
                    let mut fields = StreamLogFields::for_success(
                        name,
                        status,
                        &url,
                        model,
                        conversation_id,
                        conversation_name.as_deref(),
                    );
                    fields.cost = lookup_model_cost(&profile, real_model);
                    return Ok(stream_response(upstream, Some(fields)));
                }
                // Convert: the upstream must be an SSE stream; anything else is
                // an upstream error passed through as-is, not a success.
                let is_sse = upstream
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|t| t.contains("event-stream"));
                if !is_sse {
                    let response_status = upstream.status();
                    let response_headers = upstream.headers().clone();
                    let body_bytes = upstream.bytes().await.unwrap_or_default().to_vec();
                    return Ok(buffered_response(
                        response_status,
                        &response_headers,
                        body_bytes,
                    ));
                }
                record_success(name, is_half_open).await;
                let mut builder = Response::builder().status(200);
                for (header_name, value) in forward_headers(upstream.headers()) {
                    builder = builder.header(header_name, value);
                }
                builder = builder.header("content-type", "text/event-stream");
                let mut fields = StreamLogFields::for_success(
                    name,
                    200,
                    &url,
                    model,
                    conversation_id,
                    conversation_name.as_deref(),
                );
                fields.cost = lookup_model_cost(&profile, real_model);
                let converter = ChatSseToResponses::new(real_model);
                let transform =
                    ResponsesStreamTransform::new(upstream.bytes_stream(), converter, fields);
                return Ok(builder.body(Body::from_stream(transform)).unwrap());
            }
            Ok(upstream) if should_retry(upstream.status().as_u16()) => {
                let status = upstream.status().as_u16();
                record_failure(
                    name,
                    circuit_settings,
                    &format!("HTTP {status}"),
                    is_half_open,
                )
                .await;
                log_failed_attempt(
                    name,
                    Some(&format!("HTTP {status}")),
                    Some(status),
                    Some(&url),
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                circuit_state = read_circuit_state().await;
            }
            Ok(upstream) => {
                let status = upstream.status();
                let response_headers = upstream.headers().clone();
                let body_bytes = upstream.bytes().await.unwrap_or_default().to_vec();
                log_failed_attempt(
                    name,
                    None,
                    Some(status.as_u16()),
                    Some(&url),
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                return Ok(buffered_response(status, &response_headers, body_bytes));
            }
            Err(error) => {
                let message = error.to_string();
                record_failure(name, circuit_settings, &message, is_half_open).await;
                log_failed_attempt(
                    name,
                    Some(&message),
                    None,
                    None,
                    body.get("model").and_then(|v| v.as_str()),
                    conversation_id,
                    conversation_name.as_deref(),
                )
                .await;
                circuit_state = read_circuit_state().await;
            }
        }
    }
    if let Some(error) = conversion_error {
        return Ok(conversion_error_response(&error));
    }
    Err(AppError::proxy(
        "All Responses streaming attempts failed".to_string(),
    ))
}

/// Convert a conversion failure into the Responses-style error contract.
fn conversion_error_response(error: &ResponsesConversionError) -> Response {
    let (status, error_type) = match error {
        ResponsesConversionError::NotSupported(_) => (StatusCode::NOT_IMPLEMENTED, "not_supported"),
        ResponsesConversionError::Invalid(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "conversion_error")
        }
    };
    (
        status,
        Json(json!({ "error": { "message": error.message(), "type": error_type } })),
    )
        .into_response()
}

#[allow(clippy::too_many_arguments)]
async fn forward_with_failover(
    config: &crate::config::PiSwitchConfig,
    candidates: &[String],
    body: &Value,
    real_model: &str,
    target_path: &str,
    headers: &HeaderMap,
    conversation_id: Option<&str>,
    log_stream: bool,
) -> Result<Response> {
    let conversation_name = conversation_name_of(headers);
    let circuit_settings = &config.settings.proxy.circuit_breaker;
    let mut circuit_state = read_circuit_state().await;
    let global_spoof = config.settings.proxy.user_agent.as_deref();
    let mut half_open_used = false;

    // Rewrite the namespaced "profile/model" back to the real upstream model id.
    let out_body = {
        let mut b = body.clone();
        if !real_model.is_empty() {
            b["model"] = json!(real_model);
        }
        b
    };
    let body = &out_body;

    for name in candidates {
        let profile_value = match config.profiles.get(name) {
            Some(p) => p,
            None => continue,
        };

        let (is_open, is_half_open) = is_circuit_open(&circuit_state, name, circuit_settings);

        if is_open {
            log_request(
                name,
                false,
                Some("circuit_open"),
                None,
                None,
                None,
                None,
                None,
                conversation_id,
                conversation_name.as_deref(),
                None,
            )
            .await;
            continue;
        }

        // If half-open, only allow one probe request
        if is_half_open {
            if half_open_used {
                log_request(
                    name,
                    false,
                    Some("half_open_already_probing"),
                    None,
                    None,
                    None,
                    None,
                    None,
                    conversation_id,
                    conversation_name.as_deref(),
                    None,
                )
                .await;
                continue;
            }
            half_open_used = true;
        }

        let profile: ProviderProfile = match serde_json::from_value(profile_value.clone()) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let is_anthropic = profile.api == "anthropic-messages";
        let is_responses = profile.api == "openai-responses";
        if profile.api != "openai-completions" && !is_anthropic && !is_responses {
            continue;
        }

        // Effective disguise: per-profile spoof overrides the global setting.
        let effective_spoof = profile.spoof.as_deref().or(global_spoof);
        let (client, user_agent, disguise) = build_disguised_client(effective_spoof);

        let api_key = crate::config::resolve_env(&profile.api_key);

        if is_anthropic {
            // Convert OpenAI -> Anthropic
            let anthro_body = openai_to_anthropic_body(body);
            let url = format!("{}/messages", profile.base_url.trim_end_matches('/'));

            let mut req = client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01");
            if let Some(ref ua) = user_agent {
                req = req.header(reqwest::header::USER_AGENT, ua);
            }
            for (k, v) in &disguise {
                req = req.header(*k, *v);
            }
            let resp = req.json(&anthro_body).send().await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        let anthro_data: Value = r.json().await.unwrap_or(Value::Null);
                        let usage = crate::usage::extract_usage(&anthro_data);
                        let openai_data = anthropic_to_openai_response(&anthro_data);
                        record_success(name, is_half_open).await;
                        log_request(
                            name,
                            true,
                            None,
                            Some(status.as_u16()),
                            Some(&url),
                            None,
                            body.get("model").and_then(|v| v.as_str()),
                            usage,
                            conversation_id,
                            conversation_name.as_deref(),
                            lookup_model_cost(&profile, real_model),
                        )
                        .await;
                        return Ok(Json(openai_data).into_response());
                    } else if should_retry(status.as_u16()) {
                        let status_code = status.as_u16();
                        record_failure(
                            name,
                            circuit_settings,
                            &format!("HTTP {}", status_code),
                            is_half_open,
                        )
                        .await;
                        log_request(
                            name,
                            false,
                            Some(&format!("HTTP {}", status_code)),
                            Some(status_code),
                            Some(&url),
                            None,
                            body.get("model").and_then(|v| v.as_str()),
                            None,
                            conversation_id,
                            conversation_name.as_deref(),
                            None,
                        )
                        .await;
                        circuit_state = read_circuit_state().await;
                        continue;
                    } else {
                        let body_bytes = r.bytes().await.unwrap_or_default();
                        log_request(
                            name,
                            false,
                            None,
                            Some(status.as_u16()),
                            Some(&url),
                            None,
                            body.get("model").and_then(|v| v.as_str()),
                            None,
                            conversation_id,
                            conversation_name.as_deref(),
                            None,
                        )
                        .await;
                        return Ok(Response::builder()
                            .status(status.as_u16())
                            .body(Body::from(body_bytes))
                            .unwrap());
                    }
                }
                Err(e) => {
                    record_failure(name, circuit_settings, &e.to_string(), is_half_open).await;
                    log_request(
                        name,
                        false,
                        Some(&e.to_string()),
                        None,
                        None,
                        None,
                        body.get("model").and_then(|v| v.as_str()),
                        None,
                        conversation_id,
                        conversation_name.as_deref(),
                        None,
                    )
                    .await;
                    circuit_state = read_circuit_state().await;
                    continue;
                }
            }
        } else {
            // OpenAI-compatible
            let url = format!("{}/{}", profile.base_url.trim_end_matches('/'), target_path);

            let mut req = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key));
            if let Some(ref ua) = user_agent {
                req = req.header(reqwest::header::USER_AGENT, ua);
            }
            for (k, v) in &disguise {
                req = req.header(*k, *v);
            }
            let resp = req.json(body).send().await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        record_success(name, is_half_open).await;
                        // Stream straight through (preserves Content-Type + enables SSE).
                        // Unless log_stream is disabled (caller re-streams the body and
                        // writes its own log), the response stream is teed into the usage
                        // parser and the log line is written when it ends.
                        let log = if log_stream {
                            let mut fields = StreamLogFields::for_success(
                                name,
                                status.as_u16(),
                                &url,
                                body.get("model").and_then(|v| v.as_str()),
                                conversation_id,
                                conversation_name.as_deref(),
                            );
                            fields.cost = lookup_model_cost(&profile, real_model);
                            Some(fields)
                        } else {
                            None
                        };
                        return Ok(stream_response(r, log));
                    } else if should_retry(status.as_u16()) {
                        let status_code = status.as_u16();
                        record_failure(
                            name,
                            circuit_settings,
                            &format!("HTTP {}", status_code),
                            is_half_open,
                        )
                        .await;
                        log_request(
                            name,
                            false,
                            Some(&format!("HTTP {}", status_code)),
                            Some(status_code),
                            Some(&url),
                            None,
                            body.get("model").and_then(|v| v.as_str()),
                            None,
                            conversation_id,
                            conversation_name.as_deref(),
                            None,
                        )
                        .await;
                        circuit_state = read_circuit_state().await;
                        continue;
                    } else {
                        // Non-retryable error: pass the upstream response through unchanged.
                        log_request(
                            name,
                            false,
                            None,
                            Some(status.as_u16()),
                            Some(&url),
                            None,
                            body.get("model").and_then(|v| v.as_str()),
                            None,
                            conversation_id,
                            conversation_name.as_deref(),
                            None,
                        )
                        .await;
                        return Ok(stream_response(r, None));
                    }
                }
                Err(e) => {
                    record_failure(name, circuit_settings, &e.to_string(), is_half_open).await;
                    log_request(
                        name,
                        false,
                        Some(&e.to_string()),
                        None,
                        None,
                        None,
                        body.get("model").and_then(|v| v.as_str()),
                        None,
                        conversation_id,
                        conversation_name.as_deref(),
                        None,
                    )
                    .await;
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
    real_model: &str,
    headers: &HeaderMap,
    conversation_id: Option<&str>,
) -> Result<Response> {
    let conversation_name = conversation_name_of(headers);
    let circuit_settings = &config.settings.proxy.circuit_breaker;
    let mut circuit_state = read_circuit_state().await;
    let global_spoof = config.settings.proxy.user_agent.as_deref();
    let mut half_open_used = false;

    // Rewrite the namespaced "profile/model" back to the real upstream model id.
    let out_body = {
        let mut b = body.clone();
        if !real_model.is_empty() {
            b["model"] = json!(real_model);
        }
        b
    };
    let body = &out_body;

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
            Some(p) => p,
            None => continue,
        };
        let profile: ProviderProfile = match serde_json::from_value(profile_value.clone()) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if profile.api != "anthropic-messages" {
            continue;
        }

        // Effective disguise: per-profile spoof overrides the global setting.
        let effective_spoof = profile.spoof.as_deref().or(global_spoof);
        let (client, user_agent, disguise) = build_disguised_client(effective_spoof);

        let api_key = crate::config::resolve_env(&profile.api_key);
        let url = format!("{}/messages", profile.base_url.trim_end_matches('/'));

        let mut req = client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01");
        if let Some(ref ua) = user_agent {
            req = req.header(reqwest::header::USER_AGENT, ua);
        }
        for (k, v) in &disguise {
            req = req.header(*k, *v);
        }
        let resp = req.json(body).send().await;

        match resp {
            Ok(r) if r.status().is_success() || !should_retry(r.status().as_u16()) => {
                let status = r.status();
                if status.is_success() {
                    record_success(name, is_half_open).await;
                    // Anthropic → Anthropic passthrough: stream through, preserve
                    // headers. The stream is teed into the usage parser; the log line
                    // (with token usage + conversation id) is written when it ends.
                    let mut fields = StreamLogFields::for_success(
                        name,
                        status.as_u16(),
                        &url,
                        body.get("model").and_then(|v| v.as_str()),
                        conversation_id,
                        conversation_name.as_deref(),
                    );
                    fields.cost = lookup_model_cost(&profile, real_model);
                    return Ok(stream_response(r, Some(fields)));
                }
                log_request(
                    name,
                    false,
                    None,
                    Some(status.as_u16()),
                    Some(&url),
                    None,
                    body.get("model").and_then(|v| v.as_str()),
                    None,
                    conversation_id,
                    conversation_name.as_deref(),
                    None,
                )
                .await;
                // Non-retryable error: pass the upstream response through unchanged.
                return Ok(stream_response(r, None));
            }
            Ok(r) => {
                let status = r.status().as_u16();
                record_failure(
                    name,
                    circuit_settings,
                    &format!("HTTP {}", status),
                    is_half_open,
                )
                .await;
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

    Err(AppError::proxy(
        "All Anthropic upstream attempts failed".to_string(),
    ))
}

// ─── Request logging ──────────────────────────────────────

/// Model unit prices are per 1M tokens (industry convention); the token
/// product in `compute_cost` is scaled down by this factor.
const COST_PER_MILLION_TOKENS: f64 = 1_000_000.0;

/// Build the JSON object written to `requests.log` for one proxied request.
/// Look up a model's unit prices in its provider profile (already parsed at
/// the call site, so prices are frozen at request time). `None` (unknown
/// model or no `cost` configured) means the request's cost is unknown.
fn lookup_model_cost(profile: &ProviderProfile, model: &str) -> Option<crate::config::ModelCost> {
    profile
        .models
        .iter()
        .find(|m| m.id == model)
        .and_then(|m| m.cost.clone())
}

/// Estimate the cost of a request from its token usage and the model's
/// price; `cache_write` has no token data and never enters. Tiered pricing
/// is handled through `ModelCost::tiers`. Unit prices are per 1M tokens
/// (industry convention), so the token product is scaled down accordingly.
fn compute_cost(usage: &crate::usage::UsageSummary, cost: &crate::config::ModelCost) -> f64 {
    // Pick the highest tier whose input threshold the request's prompt tokens
    // meet; fall back to the base prices otherwise.
    let tier = cost
        .tiers
        .iter()
        .filter(|t| usage.prompt_tokens as f64 >= t.input_tokens_above)
        .max_by(|a, b| a.input_tokens_above.total_cmp(&b.input_tokens_above));
    let (input, output, cache_read) = match tier {
        Some(t) => (t.input, t.output, t.cache_read),
        None => (cost.input, cost.output, cost.cache_read),
    };
    let uncached = usage.prompt_tokens.saturating_sub(usage.cached_tokens) as f64;
    (uncached * input
        + usage.cached_tokens as f64 * cache_read
        + usage.completion_tokens as f64 * output)
        / COST_PER_MILLION_TOKENS
}

/// Build the JSON object written to `requests.log` for one proxied request.
/// Token usage is optional: rows without it (old requests, unavailable usage)
/// get null fields, keeping the format backwards compatible.
fn build_log_entry(fields: &StreamLogFields, usage: Option<&crate::usage::UsageSummary>) -> Value {
    // Cost is the usage priced at the model's request-time unit prices;
    // missing price or missing usage both mean the cost is unknown (null).
    let cost_total = match (usage, &fields.cost) {
        (Some(u), Some(cost)) => Some(compute_cost(u, cost)),
        _ => None,
    };
    json!({
        "ts": Utc::now().to_rfc3339(),
        "ok": fields.ok,
        "provider": fields.provider,
        "error": fields.error,
        "status": fields.status,
        "upstreamUrl": fields.upstream_url,
        "model": fields.model,
        "promptTokens": usage.map(|u| u.prompt_tokens),
        "completionTokens": usage.map(|u| u.completion_tokens),
        "cachedTokens": usage.map(|u| u.cached_tokens),
        "reasoningTokens": usage.map(|u| u.reasoning_tokens),
        "conversationId": fields.conversation_id,
        "conversationName": fields.conversation_name,
        "costTotal": cost_total,
    })
}

/// Serialize `entry` and append it to `requests.log` (creating the file and
/// parent directory as needed). Synchronous: callable from stream teardown
/// paths where awaiting is not possible.
fn append_log_line(entry: &Value) {
    // Concurrent requests append from multiple tasks; serialize the
    // open+write so lines never interleave or lose their trailing newline.
    static LOG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let log_path = state_dir().join("requests.log");
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    if let Ok(json) = serde_json::to_string(entry) {
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

#[allow(clippy::too_many_arguments)]
async fn log_request(
    provider: &str,
    ok: bool,
    error: Option<&str>,
    status: Option<u16>,
    upstream_url: Option<&str>,
    _attempts: Option<&[Value]>,
    model: Option<&str>,
    usage: Option<crate::usage::UsageSummary>,
    conversation_id: Option<&str>,
    conversation_name: Option<&str>,
    cost: Option<crate::config::ModelCost>,
) {
    let fields = StreamLogFields {
        provider: provider.to_string(),
        ok,
        error: error.map(|s| s.to_string()),
        status,
        upstream_url: upstream_url.map(|s| s.to_string()),
        model: model.map(|s| s.to_string()),
        conversation_id: conversation_id.map(|s| s.to_string()),
        conversation_name: conversation_name.map(|s| s.to_string()),
        cost,
    };
    let entry = build_log_entry(&fields, usage.as_ref());
    append_log_line(&entry);
}

#[cfg(test)]
mod tests {
    use super::{
        filter_private_params, handle_responses_with_config, make_router, resolve_route, ProxyState,
    };
    use crate::config::PiSwitchConfig;
    use axum::{
        body::{to_bytes, Body},
        http::{HeaderMap, HeaderValue, Request, StatusCode},
        routing::post,
        Router,
    };
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    fn cfg(profiles: serde_json::Value, failover: Vec<&str>) -> PiSwitchConfig {
        let mut c = PiSwitchConfig::default();
        if let Some(obj) = profiles.as_object() {
            c.profiles = obj.clone();
        }
        c.settings.proxy.failover = failover.into_iter().map(String::from).collect();
        c
    }
    #[tokio::test]
    async fn native_responses_non_streaming_preserves_body_and_response() {
        let seen = Arc::new(Mutex::new(None::<String>));
        let seen_headers = Arc::new(Mutex::new(None::<HeaderMap>));
        let seen_for_upstream = seen.clone();
        let seen_headers_for_upstream = seen_headers.clone();
        let upstream = Router::new().route(
            "/v1/responses",
            post(move |request: Request<Body>| {
                let seen = seen_for_upstream.clone();
                let seen_headers = seen_headers_for_upstream.clone();
                async move {
                    let headers = request.headers().clone();
                    let body = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
                    let body = String::from_utf8(body.to_vec()).unwrap();
                    *seen.lock().unwrap() = Some(body.clone());
                    *seen_headers.lock().unwrap() = Some(headers);
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .header("x-upstream", "preserved")
                        .body(Body::from(body))
                        .unwrap()
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "auto",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "upstream-key",
                    "headers": { "x-provider": "provider-value", "x-client-request-id": "provider-request" },
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let request_body = serde_json::json!({
            "model": "native/model-a",
            "input": [{ "role": "user", "content": "hello" }],
            "metadata": { "keep": true },
            "store": false
        })
        .to_string();
        let mut headers = HeaderMap::new();
        headers.insert("x-client-request-id", HeaderValue::from_static("client-1"));
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer client-key"),
        );

        let response = handle_responses_with_config(&config, headers, request_body).await;
        let status = response.status();
        let upstream_header = response.headers().get("x-upstream").cloned();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let returned: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let sent: serde_json::Value =
            serde_json::from_str(seen.lock().unwrap().as_ref().unwrap()).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(upstream_header.unwrap(), "preserved");
        assert_eq!(sent["model"], "model-a");
        assert_eq!(sent["input"][0]["content"], "hello");
        assert_eq!(sent["metadata"]["keep"], true);
        assert_eq!(returned, sent);
        let upstream_headers = seen_headers.lock().unwrap().clone().unwrap();
        assert_eq!(
            upstream_headers.get("x-provider").unwrap(),
            "provider-value"
        );
        assert_eq!(
            upstream_headers.get("x-client-request-id").unwrap(),
            "provider-request"
        );
        assert_eq!(
            upstream_headers.get("authorization").unwrap(),
            "Bearer upstream-key"
        );
        assert!(!upstream_headers.contains_key("connection"));

        server.abort();
    }

    #[tokio::test]
    async fn native_responses_preserves_error_status_and_body() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = Router::new().route(
            "/v1/responses",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "error": { "message": "invalid request", "type": "invalid_request_error" } }).to_string(),
                    ))
                    .unwrap()
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "upstream-key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "native/model-a" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["type"], "invalid_request_error");
        assert_eq!(value["error"]["message"], "invalid request");
        server.abort();
    }

    #[tokio::test]
    async fn responses_conversion_round_trips_tools_end_to_end() {
        let seen = Arc::new(Mutex::new(None::<String>));
        let seen_for_upstream = seen.clone();
        let upstream = Router::new().route(
            "/v1/chat/completions",
            post(move |body: String| {
                let seen = seen_for_upstream.clone();
                async move {
                    *seen.lock().unwrap() = Some(body.clone());
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::json!({
                                "id": "chatcmpl-200",
                                "choices": [{
                                    "message": {
                                        "role": "assistant",
                                        "content": "checking...",
                                        "tool_calls": [
                                            { "id": "call_1", "type": "function", "function": { "name": "get_weather", "arguments": "{\"city\":\"paris\"}" } },
                                            { "id": "call_2", "type": "function", "function": { "name": "get_time", "arguments": "{}" } }
                                        ]
                                    },
                                    "finish_reason": "tool_calls"
                                }],
                                "usage": {
                                    "prompt_tokens": 100,
                                    "completion_tokens": 30,
                                    "total_tokens": 130,
                                    "prompt_tokens_details": { "cached_tokens": 40 },
                                    "completion_tokens_details": { "reasoning_tokens": 5 }
                                }
                            }).to_string(),
                        ))
                        .unwrap()
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "upstream-key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let request_body = serde_json::json!({
            "model": "chat/model-a",
            "input": [
                { "role": "user", "content": "weather?" },
                { "type": "function_call", "call_id": "call_1", "name": "get_weather", "arguments": "{\"city\":\"paris\"}" },
                { "type": "function_call_output", "call_id": "call_1", "output": "{\"temp\":20}" }
            ],
            "tools": [{ "type": "function", "name": "get_weather", "description": "weather", "parameters": {} }],
            "tool_choice": "auto",
            "instructions": "Be brief."
        })
        .to_string();

        let response = handle_responses_with_config(&config, HeaderMap::new(), request_body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let sent: serde_json::Value =
            serde_json::from_str(seen.lock().unwrap().as_ref().unwrap()).unwrap();
        let messages = sent["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1");
        assert_eq!(
            messages[2]["tool_calls"][0]["function"]["name"],
            "get_weather"
        );
        assert_eq!(messages[3]["role"], "tool");
        assert_eq!(messages[3]["tool_call_id"], "call_1");
        assert_eq!(messages[3]["content"], "{\"temp\":20}");
        assert_eq!(sent["tool_choice"], "auto");
        assert_eq!(sent["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(sent["model"], "model-a");

        let output = value["output"].as_array().unwrap();
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["content"][0]["text"], "checking...");
        assert_eq!(output[1]["type"], "function_call");
        assert_eq!(output[1]["call_id"], "call_1");
        assert_eq!(output[1]["name"], "get_weather");
        assert_eq!(output[2]["type"], "function_call");
        assert_eq!(output[2]["call_id"], "call_2");
        assert_eq!(output[2]["name"], "get_time");
        assert_eq!(value["usage"]["input_tokens"], 100);
        assert_eq!(value["usage"]["output_tokens"], 30);
        assert_eq!(value["usage"]["input_tokens_details"]["cached_tokens"], 40);
        assert_eq!(
            value["usage"]["output_tokens_details"]["reasoning_tokens"],
            5
        );

        server.abort();
    }

    #[tokio::test]
    async fn responses_conversion_rejects_non_function_tools() {
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "auto",
                    "baseUrl": "http://127.0.0.1:1/v1",
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let request_body = serde_json::json!({
            "model": "chat/model-a",
            "input": [{ "role": "user", "content": "hi" }],
            "tools": [{ "type": "web_search" }]
        })
        .to_string();
        let response = handle_responses_with_config(&config, HeaderMap::new(), request_body).await;
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["type"], "not_supported");
    }

    #[tokio::test]
    async fn responses_conversion_unmappable_upstream_returns_conversion_error() {
        let upstream = Router::new().route(
            "/v1/chat/completions",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/json")
                    .body(Body::from("{\"id\":\"x\"}"))
                    .unwrap()
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "chat/model-a", "input": [{ "role": "user", "content": "hi" }] }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["type"], "conversion_error");
        server.abort();
    }

    #[tokio::test]
    async fn responses_conversion_no_route_uses_responses_error_shape() {
        let config = cfg(serde_json::json!({}), vec![]);
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "missing/model", "input": "hi" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["type"], "no_route");
    }

    #[tokio::test]
    async fn responses_mixed_failover_non_streaming_convert_then_native() {
        // Primary candidate is a convert (openai-completions) profile that fails
        // retryably; the failover chain carries a native (openai-responses)
        // candidate. The request must fail over across modes.
        let chat_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let chat_address = chat_listener.local_addr().unwrap();
        let chat_upstream = Router::new().route(
            "/v1/chat/completions",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from("unavailable"))
                    .unwrap()
            }),
        );
        let chat_server = tokio::spawn(async move {
            axum::serve(chat_listener, chat_upstream).await.unwrap();
        });
        let native_body = serde_json::json!({
            "id": "resp-native",
            "object": "response",
            "model": "model-a",
            "output": [{ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "native" }] }],
            "usage": { "input_tokens": 10, "output_tokens": 5, "total_tokens": 15 }
        });
        let native_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let native_address = native_listener.local_addr().unwrap();
        let native_upstream = Router::new().route(
            "/v1/responses",
            post(move || {
                let native_body = native_body.clone();
                async move {
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(Body::from(native_body.to_string()))
                        .unwrap()
                }
            }),
        );
        let native_server = tokio::spawn(async move {
            axum::serve(native_listener, native_upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", chat_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", native_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec!["native"],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "chat/model-a", "input": "hi" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["id"], "resp-native");
        assert_eq!(value["output"][0]["content"][0]["text"], "native");
        chat_server.abort();
        native_server.abort();
    }

    #[tokio::test]
    async fn responses_mixed_failover_non_streaming_native_then_convert() {
        // Primary candidate is a native profile that fails retryably; the
        // failover chain carries a convert candidate. Falls back to conversion.
        let native_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let native_address = native_listener.local_addr().unwrap();
        let native_upstream = Router::new().route(
            "/v1/responses",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from("unavailable"))
                    .unwrap()
            }),
        );
        let native_server = tokio::spawn(async move {
            axum::serve(native_listener, native_upstream).await.unwrap();
        });
        let chat_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let chat_address = chat_listener.local_addr().unwrap();
        let chat_upstream = Router::new().route(
            "/v1/chat/completions",
            post(move || async move {
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "id": "chatcmpl-c",
                            "choices": [{ "message": { "role": "assistant", "content": "converted" }, "finish_reason": "stop" }],
                            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
                        }).to_string(),
                    ))
                    .unwrap()
            }),
        );
        let chat_server = tokio::spawn(async move {
            axum::serve(chat_listener, chat_upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", native_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", chat_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec!["chat"],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "native/model-a", "input": "hi" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["object"], "response");
        assert_eq!(value["output"][0]["content"][0]["text"], "converted");
        native_server.abort();
        chat_server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_preserves_sse_and_records_usage() {
        let upstream_sse = concat!(
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hel\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"usage\":{\"input_tokens\":100,\"output_tokens\":30,\"total_tokens\":130,\"input_tokens_details\":{\"cached_tokens\":40},\"output_tokens_details\":{\"reasoning_tokens\":5}}}}\n\n",
            "data: [DONE]\n\n",
        );
        let seen_body = Arc::new(Mutex::new(None::<String>));
        let seen_for_upstream = seen_body.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = Router::new().route(
            "/v1/responses",
            post(move |body: String| {
                let seen = seen_for_upstream.clone();
                let upstream_sse = upstream_sse;
                async move {
                    *seen.lock().unwrap() = Some(body.clone());
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/event-stream")
                        .body(Body::from(upstream_sse))
                        .unwrap()
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "auto",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "upstream-key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let mut headers = HeaderMap::new();
        headers.insert("x-conversation-id", HeaderValue::from_static("conv-stream"));
        let response = handle_responses_with_config(
            &config,
            headers,
            serde_json::json!({
                "model": "native/model-a",
                "input": "hi",
                "stream": true
            })
            .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream",
        );
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&body), upstream_sse);

        // The stream:true request body must reach the upstream unchanged,
        // apart from the namespaced model id rewrite.
        let sent: serde_json::Value =
            serde_json::from_str(seen_body.lock().unwrap().as_ref().unwrap()).unwrap();
        assert_eq!(sent["stream"], true);
        assert_eq!(sent["input"], "hi");
        assert_eq!(sent["model"], "model-a");
        let log_dir = super::init_test_state_dir();
        let log_text = std::fs::read_to_string(log_dir.join("requests.log")).expect("log written");
        let entry = log_text
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|value| value["conversationId"] == "conv-stream")
            .expect("log entry for stream");
        assert_eq!(entry["ok"], true);
        assert_eq!(entry["promptTokens"], 100);
        assert_eq!(entry["completionTokens"], 30);
        assert_eq!(entry["cachedTokens"], 40);
        assert_eq!(entry["reasoningTokens"], 5);
        server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_fails_over_before_headers() {
        let ok_sse = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n",
            "data: [DONE]\n\n",
        );
        let fail_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fail_address = fail_listener.local_addr().unwrap();
        let fail_upstream = Router::new().route(
            "/v1/responses",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from("unavailable"))
                    .unwrap()
            }),
        );
        let fail_server = tokio::spawn(async move {
            axum::serve(fail_listener, fail_upstream).await.unwrap();
        });
        let ok_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ok_address = ok_listener.local_addr().unwrap();
        let ok_upstream = Router::new().route(
            "/v1/responses",
            post(move || async move {
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/event-stream")
                    .body(Body::from(ok_sse))
                    .unwrap()
            }),
        );
        let ok_server = tokio::spawn(async move {
            axum::serve(ok_listener, ok_upstream).await.unwrap();
        });
        // Two candidates sharing the model; the first returns 503 (retryable
        // before any SSE headers reach the client), the second succeeds.
        let config = cfg(
            serde_json::json!({
                "fail": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", fail_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "ok": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", ok_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec!["ok"],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "fail/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&body), ok_sse);
        fail_server.abort();
        ok_server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_does_not_replay_after_stream_starts() {
        use axum::body::Bytes;
        use futures_util::stream;

        // Primary candidate: sends part of the SSE stream (the response is
        // already streaming to the client), then the stream ends. The proxy
        // must not replay the request against the backup candidate once the
        // upstream response has started.
        let broken_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let broken_address = broken_listener.local_addr().unwrap();
        let broken_upstream = Router::new().route(
            "/v1/responses",
            post(move |body: String| async move {
                assert!(!body.is_empty());
                let body_stream = stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from_static(
                    b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"part\"}\n\n",
                ))]);
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/event-stream")
                    .body(Body::from_stream(body_stream))
                    .unwrap()
            }),
        );
        let broken_server = tokio::spawn(async move {
            axum::serve(broken_listener, broken_upstream).await.unwrap();
        });

        // Backup candidate that must never be contacted.
        let seen = Arc::new(Mutex::new(0usize));
        let seen_for_upstream = seen.clone();
        let backup_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let backup_address = backup_listener.local_addr().unwrap();
        let backup_upstream = Router::new().route(
            "/v1/responses",
            post(move || {
                let seen = seen_for_upstream.clone();
                async move {
                    *seen.lock().unwrap() += 1;
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from("should not be reached"))
                        .unwrap()
                }
            }),
        );
        let backup_server = tokio::spawn(async move {
            axum::serve(backup_listener, backup_upstream).await.unwrap();
        });

        let config = cfg(
            serde_json::json!({
                "broken": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", broken_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "backup": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", backup_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec!["backup"],
        );

        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "broken/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        // Drain whatever the transport delivers; the mid-stream failure may
        // surface as an Err or as a truncated/empty body depending on how the
        // HTTP stack propagates it. The behaviour under test is that the proxy
        // never replays the request against the backup candidate.
        let _ = to_bytes(response.into_body(), 1024 * 1024).await;
        assert_eq!(*seen.lock().unwrap(), 0, "backup must not be contacted");

        let log_dir = super::init_test_state_dir();
        let log_text = std::fs::read_to_string(log_dir.join("requests.log")).expect("log written");
        let entry = log_text
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|value| value["provider"] == "broken")
            .expect("log entry for the broken stream");
        assert_eq!(entry["ok"], true);

        broken_server.abort();
        backup_server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_conversion_emits_responses_events_end_to_end() {
        let chat_sse = concat!(
            "data: {\"id\":\"chatcmpl-9\",\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-9\",\"choices\":[{\"delta\":{\"content\":\"Hel\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-9\",\"choices\":[{\"delta\":{\"content\":\"lo\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-9\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"\"}},{\"index\":1,\"id\":\"call_2\",\"type\":\"function\",\"function\":{\"name\":\"get_time\",\"arguments\":\"\"}}]},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-9\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"city\\\":\\\"par\"}},{\"index\":1,\"function\":{\"arguments\":\"{\\\"tz\\\":\"}}]},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-9\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"is\\\"}\"}},{\"index\":1,\"function\":{\"arguments\":\"\\\"utc\\\"}\"}}]},\"finish_reason\":\"tool_calls\",\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-9\",\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":30,\"total_tokens\":130,\"prompt_tokens_details\":{\"cached_tokens\":40},\"completion_tokens_details\":{\"reasoning_tokens\":5}}}\n\n",
            "data: [DONE]\n\n",
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = Router::new().route(
            "/v1/chat/completions",
            post(move |body: String| {
                let chat_sse = chat_sse;
                async move {
                    let sent: serde_json::Value = serde_json::from_str(&body).unwrap();
                    assert_eq!(sent["stream"], true);
                    assert_eq!(sent["model"], "model-a");
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/event-stream")
                        .body(Body::from(chat_sse))
                        .unwrap()
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-conversation-id",
            HeaderValue::from_static("conv-convert"),
        );
        let response = handle_responses_with_config(
            &config,
            headers,
            serde_json::json!({ "model": "chat/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream",
        );
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        let events: Vec<serde_json::Value> = text
            .split("\n\n")
            .filter_map(|frame| {
                let data = frame.lines().find_map(|l| l.strip_prefix("data: "))?;
                serde_json::from_str(data).ok()
            })
            .collect();
        let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(
            types,
            [
                "response.created",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_item.added",
                "response.output_item.added",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.delta",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.function_call_arguments.done",
                "response.output_item.done",
                "response.function_call_arguments.done",
                "response.output_item.done",
                "response.completed",
            ],
        );
        let completed = events
            .iter()
            .find(|e| e["type"] == "response.completed")
            .unwrap();
        let output = completed["response"]["output"].as_array().unwrap();
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["content"][0]["text"], "Hello");
        assert_eq!(output[1]["type"], "function_call");
        assert_eq!(output[1]["call_id"], "call_1");
        assert_eq!(output[1]["name"], "get_weather");
        assert_eq!(output[1]["arguments"], "{\"city\":\"paris\"}");
        assert_eq!(output[2]["call_id"], "call_2");
        assert_eq!(output[2]["arguments"], "{\"tz\":\"utc\"}");
        assert_eq!(completed["response"]["usage"]["input_tokens"], 100);
        assert_eq!(completed["response"]["usage"]["output_tokens"], 30);
        assert_eq!(
            completed["response"]["usage"]["input_tokens_details"]["cached_tokens"],
            40,
        );
        assert_eq!(
            completed["response"]["usage"]["output_tokens_details"]["reasoning_tokens"],
            5,
        );

        let log_dir = super::init_test_state_dir();
        let log_text = std::fs::read_to_string(log_dir.join("requests.log")).expect("log written");
        let entry = log_text
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|value| value["conversationId"] == "conv-convert")
            .expect("log entry for converted stream");
        assert_eq!(entry["promptTokens"], 100);
        assert_eq!(entry["completionTokens"], 30);
        assert_eq!(entry["cachedTokens"], 40);
        assert_eq!(entry["reasoningTokens"], 5);
        server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_conversion_passes_upstream_errors_unchanged() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = Router::new().route(
            "/v1/chat/completions",
            post(move |body: String| async move {
                assert!(!body.is_empty());
                axum::response::Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "error": { "message": "bad", "type": "invalid_request_error" } }).to_string(),
                    ))
                    .unwrap()
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "chat/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["type"], "invalid_request_error");
        assert_eq!(value["error"]["message"], "bad");
        server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_mixed_failover_convert_then_native() {
        // stream:true, primary convert candidate returns 503; the failover
        // chain carries a native candidate that streams native Responses SSE.
        let chat_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let chat_address = chat_listener.local_addr().unwrap();
        let chat_upstream = Router::new().route(
            "/v1/chat/completions",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from("unavailable"))
                    .unwrap()
            }),
        );
        let chat_server = tokio::spawn(async move {
            axum::serve(chat_listener, chat_upstream).await.unwrap();
        });
        let native_sse = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"native-stream\"}\n\n",
            "data: [DONE]\n\n",
        );
        let native_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let native_address = native_listener.local_addr().unwrap();
        let native_upstream = Router::new().route(
            "/v1/responses",
            post(move || {
                let native_sse = native_sse.clone();
                async move {
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/event-stream")
                        .body(Body::from(native_sse))
                        .unwrap()
                }
            }),
        );
        let native_server = tokio::spawn(async move {
            axum::serve(native_listener, native_upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", chat_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", native_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec!["native"],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "chat/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&body), native_sse);
        chat_server.abort();
        native_server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_mixed_failover_native_then_convert() {
        // stream:true, primary native candidate returns 503; failover chain
        // carries a convert candidate whose Chat SSE is translated to
        // Responses events.
        let native_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let native_address = native_listener.local_addr().unwrap();
        let native_upstream = Router::new().route(
            "/v1/responses",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from("unavailable"))
                    .unwrap()
            }),
        );
        let native_server = tokio::spawn(async move {
            axum::serve(native_listener, native_upstream).await.unwrap();
        });
        let chat_sse = concat!(
            "data: {\"id\":\"chatcmpl-m\",\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-m\",\"choices\":[{\"delta\":{\"content\":\"conv-stream\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-m\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n",
            "data: [DONE]\n\n",
        );
        let chat_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let chat_address = chat_listener.local_addr().unwrap();
        let chat_upstream = Router::new().route(
            "/v1/chat/completions",
            post(move || {
                let chat_sse = chat_sse.clone();
                async move {
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/event-stream")
                        .body(Body::from(chat_sse))
                        .unwrap()
                }
            }),
        );
        let chat_server = tokio::spawn(async move {
            axum::serve(chat_listener, chat_upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", native_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", chat_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec!["chat"],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "native/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("response.output_text.delta"), "body: {text}");
        assert!(text.contains("conv-stream"), "body: {text}");
        assert!(text.contains("response.completed"), "body: {text}");
        assert!(
            !text.contains("chatcmpl-m"),
            "must not leak chat frames: {text}"
        );
        native_server.abort();
        chat_server.abort();
    }

    #[tokio::test]
    async fn responses_mixed_modes_do_not_pollute_each_other() {
        // One config with both a native and a convert profile; a passthrough
        // request must only hit /v1/responses and a convert request only
        // /v1/chat/completions — the modes must not cross-contaminate.
        let native_hits = Arc::new(Mutex::new(0usize));
        let native_hits_up = native_hits.clone();
        let native_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let native_address = native_listener.local_addr().unwrap();
        let native_upstream = Router::new().route(
            "/v1/responses",
            post(move || {
                let hits = native_hits_up.clone();
                async move {
                    *hits.lock().unwrap() += 1;
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::json!({ "id": "resp-native", "object": "response", "model": "model-a", "output": [] })
                                .to_string(),
                        ))
                        .unwrap()
                }
            }),
        );
        let native_server = tokio::spawn(async move {
            axum::serve(native_listener, native_upstream).await.unwrap();
        });
        let chat_hits = Arc::new(Mutex::new(0usize));
        let chat_hits_up = chat_hits.clone();
        let chat_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let chat_address = chat_listener.local_addr().unwrap();
        let chat_upstream = Router::new().route(
            "/v1/chat/completions",
            post(move || {
                let hits = chat_hits_up.clone();
                async move {
                    *hits.lock().unwrap() += 1;
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::json!({
                                "id": "chatcmpl-iso",
                                "choices": [{ "message": { "role": "assistant", "content": "ok" }, "finish_reason": "stop" }],
                                "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
                            })
                            .to_string(),
                        ))
                        .unwrap()
                }
            }),
        );
        let chat_server = tokio::spawn(async move {
            axum::serve(chat_listener, chat_upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", native_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", chat_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );

        // Passthrough request -> only /v1/responses.
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "native/model-a", "input": "hi" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["id"], "resp-native");
        assert_eq!(*native_hits.lock().unwrap(), 1);
        assert_eq!(
            *chat_hits.lock().unwrap(),
            0,
            "convert upstream must not be touched"
        );

        // Convert request -> only /v1/chat/completions.
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "chat/model-a", "input": "hi" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["object"], "response");
        assert_eq!(value["output"][0]["content"][0]["text"], "ok");
        assert_eq!(*chat_hits.lock().unwrap(), 1);
        assert_eq!(
            *native_hits.lock().unwrap(),
            1,
            "passthrough upstream must not be touched by the convert request"
        );

        native_server.abort();
        chat_server.abort();
    }

    #[tokio::test]
    async fn responses_mixed_failover_logs_both_attempts() {
        // Mixed chain (convert 503 then native 200) must leave both a failed
        // attempt and a successful entry in the request log.
        let chat_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let chat_address = chat_listener.local_addr().unwrap();
        let chat_upstream = Router::new().route(
            "/v1/chat/completions",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from("unavailable"))
                    .unwrap()
            }),
        );
        let chat_server = tokio::spawn(async move {
            axum::serve(chat_listener, chat_upstream).await.unwrap();
        });
        let native_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let native_address = native_listener.local_addr().unwrap();
        let native_upstream = Router::new().route(
            "/v1/responses",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "id": "resp-log", "object": "response", "model": "model-a",
                            "output": [],
                            "usage": { "input_tokens": 10, "output_tokens": 5, "total_tokens": 15 }
                        })
                        .to_string(),
                    ))
                    .unwrap()
            }),
        );
        let native_server = tokio::spawn(async move {
            axum::serve(native_listener, native_upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", chat_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                },
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "passthrough",
                    "baseUrl": format!("http://{}/v1", native_address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec!["native"],
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-conversation-id",
            HeaderValue::from_static("conv-mixed-log"),
        );
        let response = handle_responses_with_config(
            &config,
            headers,
            serde_json::json!({ "model": "chat/model-a", "input": "hi" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let log_dir = super::init_test_state_dir();
        let log_text = std::fs::read_to_string(log_dir.join("requests.log")).expect("log written");
        let rows: Vec<serde_json::Value> = log_text
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|value| value["conversationId"] == "conv-mixed-log")
            .collect();
        let chat_row = rows
            .iter()
            .find(|r| r["provider"] == "chat")
            .expect("chat attempt logged");
        assert_eq!(chat_row["ok"], false);
        assert!(chat_row["error"].as_str().unwrap().contains("503"));
        let native_row = rows
            .iter()
            .find(|r| r["provider"] == "native")
            .expect("native success logged");
        assert_eq!(native_row["ok"], true);
        assert_eq!(native_row["promptTokens"], 10);
        assert_eq!(native_row["completionTokens"], 5);
        chat_server.abort();
        native_server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_conversion_fails_open_on_unmappable_frame() {
        // Upstream 200 + SSE content-type, but the only frame cannot be mapped
        // to a chat delta: the stream must end with a structured response.failed
        // event, never a fake completed response.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = Router::new().route(
            "/v1/chat/completions",
            post(move |body: String| async move {
                assert!(!body.is_empty());
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/event-stream")
                    .body(Body::from(
                        "data: {\"choices\": [{\"delta\": 42}]}\n\ndata: [DONE]\n\n",
                    ))
                    .unwrap()
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "chat/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("response.failed"), "body: {text}");
        assert!(text.contains("conversion_error"), "body: {text}");
        assert!(
            !text.contains("response.completed"),
            "must not fake completion: {text}"
        );
        server.abort();
    }

    #[tokio::test]
    async fn responses_streaming_conversion_passes_non_sse_2xx_unchanged() {
        // A 2xx upstream that returns JSON instead of SSE must be passed through
        // untouched, not converted into an empty completed response.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = Router::new().route(
            "/v1/chat/completions",
            post(move |body: String| async move {
                assert!(!body.is_empty());
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "error": { "message": "weird", "type": "upstream_odd" } })
                            .to_string(),
                    ))
                    .unwrap()
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "chat": {
                    "api": "openai-completions",
                    "responsesMode": "convert",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let response = handle_responses_with_config(
            &config,
            HeaderMap::new(),
            serde_json::json!({ "model": "chat/model-a", "input": "hi", "stream": true })
                .to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["type"], "upstream_odd");
        server.abort();
    }

    #[tokio::test]
    async fn native_responses_records_usage_and_conversation_in_log() {
        let log_dir = super::init_test_state_dir().clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let upstream = Router::new().route(
            "/v1/responses",
            post(async || {
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "id": "resp-1",
                            "object": "response",
                            "model": "model-a",
                            "output": [{ "type": "message", "role": "assistant", "content": [{"type":"output_text","text":"hi"}] }],
                            "usage": {
                                "input_tokens": 100,
                                "output_tokens": 30,
                                "total_tokens": 130,
                                "input_tokens_details": { "cached_tokens": 40 },
                                "output_tokens_details": { "reasoning_tokens": 5 }
                            }
                        }).to_string(),
                    ))
                    .unwrap()
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let config = cfg(
            serde_json::json!({
                "native": {
                    "api": "openai-responses",
                    "responsesMode": "auto",
                    "baseUrl": format!("http://{}/v1", address),
                    "apiKey": "upstream-key",
                    "exposedModels": ["model-a"]
                }
            }),
            vec![],
        );
        let mut headers = HeaderMap::new();
        headers.insert("x-conversation-id", HeaderValue::from_static("conv-42"));
        let response = handle_responses_with_config(
            &config,
            headers,
            serde_json::json!({ "model": "native/model-a" }).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        // Drain the body so the request completes before reading the log.
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["usage"]["input_tokens"], 100);
        assert_eq!(value["usage"]["output_tokens"], 30);
        assert_eq!(value["usage"]["input_tokens_details"]["cached_tokens"], 40);
        assert_eq!(
            value["usage"]["output_tokens_details"]["reasoning_tokens"],
            5
        );
        let log_path = log_dir.join("requests.log");
        let log_text = std::fs::read_to_string(&log_path).expect("log file written");
        let entry = log_text
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|value| value["conversationId"] == "conv-42")
            .expect("log entry for this conversation");
        assert_eq!(entry["ok"], true);
        assert_eq!(entry["conversationId"], "conv-42");
        assert_eq!(entry["model"], "model-a");
        assert_eq!(entry["promptTokens"], 100);
        assert_eq!(entry["completionTokens"], 30);
        assert_eq!(entry["cachedTokens"], 40);
        assert_eq!(entry["reasoningTokens"], 5);
        server.abort();
    }

    #[tokio::test]
    async fn accepts_model_requests_larger_than_axum_default_body_limit() {
        let request_body = serde_json::json!({
            "model": "missing/model",
            "messages": [{ "role": "user", "content": "x".repeat(2 * 1024 * 1024) }],
        })
        .to_string();

        let response = make_router(Arc::new(ProxyState {}))
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body = String::from_utf8_lossy(&body);

        assert_eq!(
            status,
            StatusCode::BAD_GATEWAY,
            "unexpected response: {body}"
        );
        assert!(body.contains("No upstream exposes model 'missing/model'"));
    }

    #[test]
    fn append_log_line_writes_to_test_isolated_dir() {
        let dir = super::init_test_state_dir();
        let log_path = dir.join("requests.log");
        super::append_log_line(&serde_json::json!({ "probe": "isolation" }));
        let text = std::fs::read_to_string(&log_path).expect("log written to isolated dir");
        assert!(text.contains("\"probe\":\"isolation\""));
    }

    #[test]
    fn namespaced_routes_to_profile() {
        let c = cfg(
            serde_json::json!({
                "hyb": { "proxy": false, "exposedModels": ["gpt-5.4"] }
            }),
            vec![],
        );
        let (profiles, real) = resolve_route(&c, "hyb/gpt-5.4");
        assert_eq!(profiles, vec!["hyb".to_string()]);
        assert_eq!(real, "gpt-5.4");
    }

    #[test]
    fn namespaced_adds_failover_sharing_model() {
        let c = cfg(
            serde_json::json!({
                "hyb": { "proxy": false, "exposedModels": ["gpt-5.4"] },
                "fox": { "proxy": false, "exposedModels": ["gpt-5.4"] },
            }),
            vec!["fox"],
        );
        let (profiles, real) = resolve_route(&c, "hyb/gpt-5.4");
        assert_eq!(profiles, vec!["hyb".to_string(), "fox".to_string()]);
        assert_eq!(real, "gpt-5.4");
    }

    #[test]
    fn bare_id_failover_first() {
        let c = cfg(
            serde_json::json!({
                "aiapi": { "proxy": false, "exposedModels": ["gpt-5.4"] },
                "hyb": { "proxy": false, "exposedModels": ["gpt-5.4"] },
            }),
            vec!["hyb"],
        );
        let (profiles, real) = resolve_route(&c, "gpt-5.4");
        assert_eq!(profiles.first(), Some(&"hyb".to_string())); // failover-first
        assert!(profiles.contains(&"aiapi".to_string()));
        assert_eq!(real, "gpt-5.4");
    }

    #[test]
    fn splits_on_first_slash_only() {
        let c = cfg(
            serde_json::json!({
                "or": { "proxy": false, "exposedModels": ["anthropic/claude-sonnet-4.5"] }
            }),
            vec![],
        );
        let (profiles, real) = resolve_route(&c, "or/anthropic/claude-sonnet-4.5");
        assert_eq!(profiles, vec!["or".to_string()]);
        assert_eq!(real, "anthropic/claude-sonnet-4.5");
    }

    #[test]
    fn unknown_model_yields_empty() {
        let c = cfg(
            serde_json::json!({
                "hyb": { "proxy": false, "exposedModels": ["gpt-5.4"] }
            }),
            vec![],
        );
        let (profiles, _real) = resolve_route(&c, "hyb/does-not-exist");
        assert!(profiles.is_empty());
    }

    #[test]
    fn filter_strips_top_level_and_nested_private_fields() {
        let input = serde_json::json!({
            "model": "gpt-5.4",
            "_internal_id": "abc",
            "messages": [{ "role": "user", "content": "hi", "_token": "secret" }],
        });
        let out = filter_private_params(input);
        assert!(out.get("model").is_some());
        assert!(out.get("_internal_id").is_none());
        let msg = &out["messages"][0];
        assert!(msg.get("content").is_some());
        assert!(msg.get("_token").is_none());
    }

    #[test]
    fn filter_keeps_underscore_schema_property_names() {
        // A tool's JSON-schema may legitimately define a property named `_foo`.
        let input = serde_json::json!({
            "tools": [{
                "function": {
                    "parameters": {
                        "type": "object",
                        "properties": { "_foo": { "type": "string" }, "bar": { "type": "string" } }
                    }
                }
            }],
            "_private": 1
        });
        let out = filter_private_params(input);
        assert!(out.get("_private").is_none());
        let props = &out["tools"][0]["function"]["parameters"]["properties"];
        assert!(
            props.get("_foo").is_some(),
            "schema property names must be preserved"
        );
        assert!(props.get("bar").is_some());
    }

    #[test]
    fn responses_to_chat_converts_input_to_messages() {
        let responses = serde_json::json!({
            "model": "gpt-5.4",
            "input": [
                { "role": "user", "content": "hello" }
            ],
            "max_output_tokens": 100,
            "temperature": 0.7,
            "stream": false
        });
        let chat = super::responses_to_chat(&responses).expect("convert");
        assert_eq!(chat["model"], "gpt-5.4");
        assert_eq!(chat["messages"][0]["role"], "user");
        assert_eq!(chat["messages"][0]["content"], "hello");
        assert_eq!(chat["max_tokens"], 100);
        assert_eq!(chat["temperature"], 0.7);
        assert!(chat.get("max_output_tokens").is_none());
    }
    #[test]
    fn responses_to_chat_converts_tool_results_to_tool_messages() {
        let responses = serde_json::json!({
            "model": "gpt-5",
            "input": [
                { "role": "user", "content": "weather?" },
                { "type": "function_call_output", "call_id": "call_1", "output": "{\"temp\":20}" }
            ]
        });
        let chat = super::responses_to_chat(&responses).expect("convert");
        let msgs = chat["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "tool");
        assert_eq!(msgs[1]["tool_call_id"], "call_1");
        assert_eq!(msgs[1]["content"], "{\"temp\":20}");
    }

    #[test]
    fn responses_to_chat_merges_assistant_function_calls_into_tool_calls() {
        let responses = serde_json::json!({
            "model": "gpt-5",
            "input": [
                { "role": "user", "content": "weather?" },
                { "type": "function_call", "call_id": "call_1", "name": "get_weather", "arguments": "{\"city\":\"paris\"}" },
                { "type": "function_call_output", "call_id": "call_1", "output": "{\"temp\":20}" }
            ]
        });
        let chat = super::responses_to_chat(&responses).expect("convert");
        let msgs = chat["messages"].as_array().unwrap();
        let assistant = &msgs[1];
        assert_eq!(assistant["role"], "assistant");
        assert_eq!(assistant["tool_calls"][0]["id"], "call_1");
        assert_eq!(assistant["tool_calls"][0]["type"], "function");
        assert_eq!(
            assistant["tool_calls"][0]["function"]["name"],
            "get_weather"
        );
        assert_eq!(
            assistant["tool_calls"][0]["function"]["arguments"],
            "{\"city\":\"paris\"}"
        );
        assert_eq!(msgs[2]["role"], "tool");
        assert_eq!(msgs[2]["tool_call_id"], "call_1");
    }

    #[test]
    fn responses_to_chat_rejects_non_function_tools() {
        let responses = serde_json::json!({
            "model": "gpt-5",
            "input": [{ "role": "user", "content": "hi" }],
            "tools": [{ "type": "web_search", "name": "web" }]
        });
        let err = super::responses_to_chat(&responses).expect_err("must reject");
        assert!(matches!(
            err,
            super::ResponsesConversionError::NotSupported(_)
        ));
    }

    #[test]
    fn responses_to_chat_maps_instructions_to_system_message() {
        let responses = serde_json::json!({
            "model": "gpt-5",
            "input": [{ "role": "user", "content": "hi" }],
            "instructions": "You are helpful."
        });
        let chat = super::responses_to_chat(&responses).expect("convert");
        let msgs = chat["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are helpful.");
        assert_eq!(msgs[1]["role"], "user");
    }

    #[test]
    fn chat_response_to_responses_maps_choices_to_output() {
        let chat = serde_json::json!({
            "id": "chatcmpl-123",
            "choices": [{
                "message": { "role": "assistant", "content": "Hello!" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
        });
        let resp = super::chat_response_to_responses(chat, "gpt-5.4", None).expect("convert");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["model"], "gpt-5.4");
        let output = &resp["output"][0];
        assert_eq!(output["type"], "message");
        assert_eq!(output["content"][0]["type"], "output_text");
        assert_eq!(output["content"][0]["text"], "Hello!");
    }

    #[test]
    fn chat_response_to_responses_preserves_cache_and_reasoning_details() {
        let chat = serde_json::json!({
            "id": "chatcmpl-124",
            "choices": [{
                "message": { "role": "assistant", "content": "Hello!" },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150,
                "prompt_tokens_details": { "cached_tokens": 40 },
                "completion_tokens_details": { "reasoning_tokens": 20 },
            },
        });
        let resp = super::chat_response_to_responses(chat, "gpt-5.4", None).expect("convert");
        let usage = &resp["usage"];
        assert_eq!(usage["input_tokens"], 100);
        assert_eq!(usage["output_tokens"], 50);
        assert_eq!(usage["input_tokens_details"]["cached_tokens"], 40);
        assert_eq!(usage["output_tokens_details"]["reasoning_tokens"], 20);
    }

    #[test]
    fn chat_response_to_responses_omits_details_when_absent() {
        let chat = serde_json::json!({
            "id": "chatcmpl-125",
            "choices": [{
                "message": { "role": "assistant", "content": "Hi" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 },
        });
        let resp = super::chat_response_to_responses(chat, "gpt-5.4", None).expect("convert");
        let usage = &resp["usage"];
        assert_eq!(usage["input_tokens"], 10);
        assert_eq!(usage["output_tokens"], 5);
        assert_eq!(
            usage["input_tokens_details"]["cached_tokens"],
            serde_json::Value::Null,
            "no cached info -> null, not an error"
        );
        assert_eq!(
            usage["output_tokens_details"]["reasoning_tokens"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn chat_response_to_responses_maps_tool_calls_to_function_call_output() {
        let chat = serde_json::json!({
            "id": "chatcmpl-126",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        { "id": "call_1", "type": "function", "function": { "name": "get_weather", "arguments": "{\"city\":\"paris\"}" } },
                        { "id": "call_2", "type": "function", "function": { "name": "get_time", "arguments": "{}" } }
                    ]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
        });
        let resp = super::chat_response_to_responses(chat, "gpt-5.4", None).expect("convert");
        let output = resp["output"].as_array().unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["type"], "function_call");
        assert_eq!(output[0]["call_id"], "call_1");
        assert_eq!(output[0]["name"], "get_weather");
        assert_eq!(output[0]["arguments"], "{\"city\":\"paris\"}");
        assert_eq!(output[0]["status"], "completed");
        assert_eq!(output[1]["call_id"], "call_2");
        assert_eq!(output[1]["name"], "get_time");
    }

    #[test]
    fn chat_response_to_responses_keeps_text_and_tool_calls_together() {
        let chat = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "checking...",
                    "tool_calls": [{ "id": "call_1", "type": "function", "function": { "name": "f", "arguments": "{}" } }]
                }
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
        });
        let resp = super::chat_response_to_responses(chat, "gpt-5.4", None).expect("convert");
        let output = resp["output"].as_array().unwrap();
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["content"][0]["text"], "checking...");
        assert_eq!(output[1]["type"], "function_call");
        assert_eq!(output[1]["call_id"], "call_1");
    }

    #[test]
    fn chat_response_to_responses_rejects_unmappable_choices() {
        let chat = serde_json::json!({ "id": "chatcmpl-127", "usage": {} });
        let err =
            super::chat_response_to_responses(chat, "gpt-5.4", None).expect_err("must reject");
        assert!(matches!(err, super::ResponsesConversionError::Invalid(_)));
    }

    #[test]
    fn conversation_id_prefers_header_over_body() {
        let mut headers = HeaderMap::new();
        headers.insert("x-conversation-id", HeaderValue::from_static("conv-header"));
        let body = serde_json::json!({ "conversation_id": "conv-body" });
        assert_eq!(
            super::conversation_id_of(&headers, &body),
            Some("conv-header".to_string())
        );
    }

    #[test]
    fn conversation_id_falls_back_to_opencode_session_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-opencode-session",
            HeaderValue::from_static("019fc02b-session"),
        );
        let body = serde_json::json!({ "conversation_id": "conv-body" });
        assert_eq!(
            super::conversation_id_of(&headers, &body),
            Some("019fc02b-session".to_string())
        );

        let mut both = HeaderMap::new();
        both.insert("x-conversation-id", HeaderValue::from_static("conv-header"));
        both.insert(
            "x-opencode-session",
            HeaderValue::from_static("019fc02b-session"),
        );
        assert_eq!(
            super::conversation_id_of(&both, &body),
            Some("conv-header".to_string()),
            "x-conversation-id still wins over x-opencode-session"
        );
    }

    #[test]
    fn conversation_id_falls_back_to_body_when_header_absent_or_empty() {
        let body = serde_json::json!({ "conversation_id": "conv-body" });

        let no_header = HeaderMap::new();
        assert_eq!(
            super::conversation_id_of(&no_header, &body),
            Some("conv-body".to_string())
        );

        let mut empty_header = HeaderMap::new();
        empty_header.insert("x-conversation-id", HeaderValue::from_static(""));
        assert_eq!(
            super::conversation_id_of(&empty_header, &body),
            Some("conv-body".to_string())
        );
    }

    #[test]
    fn conversation_name_reads_non_empty_header_only() {
        let mut headers = HeaderMap::new();
        headers.insert("x-conversation-name", HeaderValue::from_static("my-chat"));
        assert_eq!(
            super::conversation_name_of(&headers),
            Some("my-chat".to_string())
        );

        let mut empty = HeaderMap::new();
        empty.insert("x-conversation-name", HeaderValue::from_static(""));
        assert_eq!(
            super::conversation_name_of(&empty),
            None,
            "empty value ignored"
        );

        let absent = HeaderMap::new();
        assert_eq!(
            super::conversation_name_of(&absent),
            None,
            "missing header -> None"
        );
    }

    #[test]
    fn conversation_name_collapses_control_characters() {
        // HTAB is legal in header values; it must not reach the log/display.
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-conversation-name",
            HeaderValue::from_bytes(b"my\tchat").unwrap(),
        );
        assert_eq!(
            super::conversation_name_of(&headers),
            Some("my chat".to_string())
        );

        // Values that are only whitespace/control characters resolve to None.
        let mut blank = HeaderMap::new();
        blank.insert(
            "x-conversation-name",
            HeaderValue::from_bytes(b" \t ").unwrap(),
        );
        assert_eq!(super::conversation_name_of(&blank), None);
    }

    #[test]
    fn conversation_name_percent_decodes_cjk_titles() {
        // The client extension percent-encodes non-Latin1 characters
        // (> 0xff) so the header stays HTTP-safe; the proxy must decode
        // them back so the display name in logs/webui is the readable
        // original, not %-escapes.
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-conversation-name",
            HeaderValue::from_static("%E7%BC%96%E6%8E%92%E8%AE%A1%E5%88%92"),
        );
        assert_eq!(
            super::conversation_name_of(&headers),
            Some("编排计划".to_string())
        );
    }

    #[test]
    fn conversation_name_percent_decode_keeps_ascii_and_mixed() {
        // Spaces and ASCII pass through untouched; only the encoded CJK
        // segment is decoded.
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-conversation-name",
            HeaderValue::from_static("tdd-implement %E7%9A%84%E8%A6%81%E6%B1%82"),
        );
        assert_eq!(
            super::conversation_name_of(&headers),
            Some("tdd-implement 的要求".to_string())
        );
    }

    #[test]
    fn conversation_name_percent_decode_ignores_invalid_utf8_escapes() {
        // "100%EF" is a literal percent sequence, not an encoded name:
        // decoding would yield invalid UTF-8 (0xEF alone), so the raw
        // value is kept instead of being mis-decoded.
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-conversation-name",
            HeaderValue::from_static("100%EF"),
        );
        assert_eq!(
            super::conversation_name_of(&headers),
            Some("100%EF".to_string())
        );
    }

    #[test]
    fn conversation_name_percent_decode_then_collapses_control_characters() {
        // %0A decodes to a newline; the control-character pass must still
        // clean it out of the final name.
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-conversation-name",
            HeaderValue::from_static("line1%0Aline2"),
        );
        assert_eq!(
            super::conversation_name_of(&headers),
            Some("line1 line2".to_string())
        );
    }

    #[test]
    fn conversation_name_does_not_touch_conversation_id_detection() {
        let mut headers = HeaderMap::new();
        headers.insert("x-conversation-name", HeaderValue::from_static("my-chat"));
        headers.insert("x-conversation-id", HeaderValue::from_static("conv-1"));
        assert_eq!(
            super::conversation_name_of(&headers),
            Some("my-chat".to_string())
        );
        assert_eq!(
            super::conversation_id_of(&headers, &serde_json::json!({})),
            Some("conv-1".to_string())
        );
    }

    #[test]
    fn conversation_id_returns_none_when_unavailable_or_malformed() {
        let headers = HeaderMap::new();

        assert_eq!(
            super::conversation_id_of(&headers, &serde_json::json!({})),
            None
        );
        assert_eq!(
            super::conversation_id_of(&headers, &serde_json::json!({ "conversation_id": 123 })),
            None,
            "non-string body field is ignored"
        );
        assert_eq!(
            super::conversation_id_of(&headers, &serde_json::json!({ "conversation_id": null })),
            None
        );
        assert_eq!(
            super::conversation_id_of(&headers, &serde_json::json!({ "conversation_id": "" })),
            None,
            "empty body field is ignored"
        );
    }

    #[test]
    fn log_entry_includes_usage_and_conversation_when_present() {
        let usage = crate::usage::UsageSummary {
            prompt_tokens: 200,
            completion_tokens: 30,
            cached_tokens: 120,
            reasoning_tokens: 20,
        };
        let fields = super::StreamLogFields {
            provider: "hyb".to_string(),
            ok: true,
            error: None,
            status: Some(200),
            upstream_url: Some("http://upstream/chat/completions".to_string()),
            model: Some("gpt-5.4".to_string()),
            conversation_id: Some("conv-1".to_string()),
            conversation_name: None,
            cost: None,
        };
        let entry = super::build_log_entry(&fields, Some(&usage));
        assert!(entry.get("ts").and_then(|v| v.as_str()).is_some());
        assert_eq!(entry["ok"], true);
        assert_eq!(entry["provider"], "hyb");
        assert_eq!(entry["status"], 200);
        assert_eq!(entry["upstreamUrl"], "http://upstream/chat/completions");
        assert_eq!(entry["model"], "gpt-5.4");
        assert_eq!(entry["promptTokens"], 200);
        assert_eq!(entry["completionTokens"], 30);
        assert_eq!(entry["cachedTokens"], 120);
        assert_eq!(entry["reasoningTokens"], 20);
        assert_eq!(entry["conversationId"], "conv-1");
    }

    #[test]
    fn log_entry_includes_conversation_name_when_present() {
        let fields = super::StreamLogFields {
            provider: "hyb".to_string(),
            ok: true,
            error: None,
            status: Some(200),
            upstream_url: Some("http://upstream/chat/completions".to_string()),
            model: Some("gpt-5.4".to_string()),
            conversation_id: Some("conv-1".to_string()),
            conversation_name: Some("my-chat".to_string()),
            cost: None,
        };
        let entry = super::build_log_entry(&fields, None);
        assert_eq!(entry["conversationName"], "my-chat");

        let unnamed = super::StreamLogFields {
            conversation_name: None,
            ..fields
        };
        let entry = super::build_log_entry(&unnamed, None);
        assert_eq!(entry["conversationName"], serde_json::Value::Null);
    }

    #[test]
    fn log_entry_leaves_token_fields_null_without_usage() {
        let fields = super::StreamLogFields {
            provider: "hyb".to_string(),
            ok: false,
            error: Some("boom".to_string()),
            status: None,
            upstream_url: None,
            model: None,
            conversation_id: None,
            conversation_name: None,
            cost: None,
        };
        let entry = super::build_log_entry(&fields, None);
        assert_eq!(entry["ok"], false);
        assert_eq!(entry["error"], "boom");
        assert_eq!(entry["promptTokens"], serde_json::Value::Null);
        assert_eq!(entry["completionTokens"], serde_json::Value::Null);
        assert_eq!(entry["cachedTokens"], serde_json::Value::Null);
        assert_eq!(entry["reasoningTokens"], serde_json::Value::Null);
        assert_eq!(entry["conversationId"], serde_json::Value::Null);
    }

    #[test]
    fn log_entry_roundtrips_through_request_log_entry() {
        let usage = crate::usage::UsageSummary {
            prompt_tokens: 100,
            completion_tokens: 50,
            cached_tokens: 40,
            reasoning_tokens: 0,
        };
        let fields = super::StreamLogFields::for_success(
            "hyb",
            200,
            "http://upstream",
            Some("gpt-5.4"),
            Some("conv-1"),
            None,
        );
        let entry = super::build_log_entry(&fields, Some(&usage));
        let parsed: crate::stats::RequestLogEntry = serde_json::from_value(entry).unwrap();
        assert_eq!(parsed.provider.as_deref(), Some("hyb"));
        assert_eq!(parsed.prompt_tokens, Some(100));
        assert_eq!(parsed.completion_tokens, Some(50));
        assert_eq!(parsed.cached_tokens, Some(40));
        assert_eq!(parsed.conversation_id.as_deref(), Some("conv-1"));
    }

    #[test]
    fn log_entry_writes_cost_total_when_model_has_price() {
        let usage = crate::usage::UsageSummary {
            prompt_tokens: 200,
            completion_tokens: 30,
            cached_tokens: 120,
            reasoning_tokens: 20,
        };
        let cost = crate::config::ModelCost {
            input: 2.0,
            output: 1.0,
            cache_read: 0.5,
            cache_write: 0.0,
            tiers: vec![],
            extra: Default::default(),
        };
        let fields = super::StreamLogFields {
            provider: "hyb".to_string(),
            ok: true,
            error: None,
            status: Some(200),
            upstream_url: Some("http://upstream/chat/completions".to_string()),
            model: Some("gpt-5.4".to_string()),
            conversation_id: Some("conv-1".to_string()),
            conversation_name: None,
            cost: Some(cost),
        };
        let entry = super::build_log_entry(&fields, Some(&usage));
        assert_eq!(
            entry["costTotal"], 0.00025,
            "costTotal is written with per-1M-token prices"
        );
    }

    #[test]
    fn log_entry_leaves_cost_total_null_without_price_or_usage() {
        let usage = crate::usage::UsageSummary {
            prompt_tokens: 200,
            completion_tokens: 30,
            cached_tokens: 120,
            reasoning_tokens: 20,
        };
        let fields = super::StreamLogFields {
            provider: "hyb".to_string(),
            ok: true,
            error: None,
            status: Some(200),
            upstream_url: None,
            model: Some("gpt-5.4".to_string()),
            conversation_id: None,
            conversation_name: None,
            cost: None,
        };
        let entry = super::build_log_entry(&fields, Some(&usage));
        assert_eq!(
            entry["costTotal"],
            serde_json::Value::Null,
            "no price means unknown cost"
        );

        let fields_with_price = super::StreamLogFields {
            cost: Some(crate::config::ModelCost {
                input: 2.0,
                output: 1.0,
                cache_read: 0.5,
                cache_write: 0.0,
                tiers: vec![],
                extra: Default::default(),
            }),
            ..fields
        };
        let entry = super::build_log_entry(&fields_with_price, None);
        assert_eq!(
            entry["costTotal"],
            serde_json::Value::Null,
            "no usage means unknown cost"
        );
    }

    type TeeSlot = (
        std::sync::Arc<
            std::sync::Mutex<Option<(Option<crate::usage::UsageSummary>, Option<String>)>>,
        >,
        Box<dyn FnOnce(Option<crate::usage::UsageSummary>, Option<String>) + Send>,
    );

    fn tee_slot() -> TeeSlot {
        let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        let handle = slot.clone();
        let cb: Box<dyn FnOnce(Option<crate::usage::UsageSummary>, Option<String>) + Send> =
            Box::new(move |summary, error| {
                *handle.lock().unwrap() = Some((summary, error));
            });
        (slot, cb)
    }

    fn openai_stream() -> String {
        concat!(
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"choices\":[],\"usage\":{\"prompt_tokens\":200,\"completion_tokens\":30,\"prompt_tokens_details\":{\"cached_tokens\":120}}}\n\n",
            "data: [DONE]\n\n",
        )
        .to_string()
    }

    #[tokio::test]
    async fn stream_tee_forwards_chunks_unchanged_and_reports_usage() {
        use axum::body::Bytes;
        use futures_util::TryStreamExt;

        let (slot, cb) = tee_slot();
        let chunks: Vec<std::result::Result<Bytes, std::io::Error>> =
            vec![Ok(Bytes::from(openai_stream()))];
        let tee = super::StreamTee::new(futures_util::stream::iter(chunks), cb);

        let out: Vec<Bytes> = tee.try_collect().await.unwrap();
        assert_eq!(
            out,
            vec![Bytes::from(openai_stream())],
            "chunks pass through"
        );

        let summary = slot.lock().unwrap().take().and_then(|(s, _)| s).unwrap();
        assert_eq!(
            (
                summary.prompt_tokens,
                summary.completion_tokens,
                summary.cached_tokens
            ),
            (200, 30, 120)
        );
    }

    #[tokio::test]
    async fn stream_tee_handles_usage_split_across_chunks() {
        use axum::body::Bytes;
        use futures_util::TryStreamExt;

        let stream = openai_stream();
        let cut = stream.find("\"usage\"").unwrap();
        let chunks: Vec<std::result::Result<Bytes, std::io::Error>> = vec![
            Ok(Bytes::from(stream[..cut].to_string())),
            Ok(Bytes::from(stream[cut..].to_string())),
        ];

        let (slot, cb) = tee_slot();
        let tee = super::StreamTee::new(futures_util::stream::iter(chunks), cb);
        let out: Vec<Bytes> = tee.try_collect().await.unwrap();

        let joined: String = out
            .into_iter()
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .collect();
        assert_eq!(joined, stream, "chunks reassemble to the original stream");

        let summary = slot.lock().unwrap().take().and_then(|(s, _)| s).unwrap();
        assert_eq!(
            (
                summary.prompt_tokens,
                summary.completion_tokens,
                summary.cached_tokens
            ),
            (200, 30, 120)
        );
    }

    #[tokio::test]
    async fn stream_tee_reports_none_when_stream_has_no_usage() {
        use axum::body::Bytes;
        use futures_util::TryStreamExt;

        let stream = concat!(
            "data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let (slot, cb) = tee_slot();
        let chunks: Vec<std::result::Result<Bytes, std::io::Error>> = vec![Ok(Bytes::from(stream))];
        let tee = super::StreamTee::new(futures_util::stream::iter(chunks), cb);

        tee.try_collect::<Vec<Bytes>>().await.unwrap();
        assert_eq!(
            *slot.lock().unwrap(),
            Some((None, None)),
            "callback runs with no usage"
        );
    }

    #[tokio::test]
    async fn stream_tee_propagates_error_and_still_reports() {
        use axum::body::Bytes;
        use futures_util::TryStreamExt;

        let (slot, cb) = tee_slot();
        let tee = super::StreamTee::new(
            futures_util::stream::iter(vec![
                Ok(Bytes::from("data: {\"id\":\"1\"}\n\n")),
                Err(std::io::Error::other("upstream died")),
            ]),
            cb,
        );

        let err = tee.try_collect::<Vec<Bytes>>().await.unwrap_err();
        assert!(err.to_string().contains("upstream died"));
        assert_eq!(
            *slot.lock().unwrap(),
            Some((None, Some("upstream died".to_string()))),
            "error end still triggers the callback"
        );
    }

    #[tokio::test]
    async fn stream_tee_drop_mid_stream_still_reports() {
        use axum::body::Bytes;
        use futures_util::TryStreamExt;

        let stream = concat!(
            "data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-3\",\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":10,\"prompt_tokens_details\":{\"cached_tokens\":50}}}\n\n",
        );
        let (slot, cb) = tee_slot();
        let chunks: Vec<std::result::Result<Bytes, std::io::Error>> =
            vec![Ok(Bytes::from(stream)), Ok(Bytes::from("data: [DONE]\n\n"))];
        let mut tee = super::StreamTee::new(futures_util::stream::iter(chunks), cb);

        let first = tee.try_next().await.unwrap().unwrap();
        assert_eq!(first, Bytes::from(stream));
        drop(tee);

        let summary = slot.lock().unwrap().take().and_then(|(s, _)| s).unwrap();
        assert_eq!(
            (
                summary.prompt_tokens,
                summary.completion_tokens,
                summary.cached_tokens
            ),
            (100, 10, 50),
            "client cut still flushes the log line with whatever usage arrived"
        );
    }

    #[test]
    fn stream_tee_drop_mid_stream_without_usage_reports_none() {
        use axum::body::Bytes;

        let (slot, cb) = tee_slot();
        let chunks: Vec<std::result::Result<Bytes, std::io::Error>> =
            vec![Ok(Bytes::from("data: {\"id\":\"1\"}\n\n"))];
        let tee = super::StreamTee::new(futures_util::stream::iter(chunks), cb);

        drop(tee);
        assert_eq!(*slot.lock().unwrap(), Some((None, None)));
    }
    #[test]
    fn stream_log_entry_marks_interrupted_stream_as_failed() {
        let fields = super::StreamLogFields::for_success(
            "native",
            200,
            "http://upstream/v1/responses",
            Some("model-a"),
            None,
            None,
        );
        let entry = super::stream_log_entry(fields, None, Some("upstream died".to_string()));
        assert_eq!(entry["ok"], false);
        assert_eq!(entry["error"], "upstream died");
        assert_eq!(entry["status"], 200);
    }

    #[test]
    fn chat_sse_to_responses_emits_text_events_in_order() {
        let mut converter = super::ChatSseToResponses::new("model-a");
        let mut events = Vec::new();
        for frame in [
            serde_json::json!({ "choices": [{ "delta": { "role": "assistant", "content": "" } }] }),
            serde_json::json!({ "choices": [{ "delta": { "content": "Hel" } }] }),
            serde_json::json!({ "choices": [{ "delta": { "content": "lo" }, "finish_reason": "stop" }] }),
        ] {
            events.extend(converter.push_frame(&frame).unwrap());
        }
        events.extend(converter.finish());

        let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(
            types,
            [
                "response.created",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.completed",
            ],
        );
        assert_eq!(events[0]["response"]["model"], "model-a");
        assert_eq!(events[3]["delta"], "Hel");
        assert_eq!(events[4]["delta"], "lo");
        let completed = &events[8]["response"];
        assert_eq!(completed["status"], "completed");
        assert_eq!(completed["output"][0]["content"][0]["text"], "Hello");
        assert!(completed.get("usage").is_none(), "no usage -> omitted");
    }

    #[test]
    fn chat_sse_to_responses_tracks_parallel_tool_calls() {
        let mut converter = super::ChatSseToResponses::new("model-a");
        let mut events = Vec::new();
        for frame in [
            serde_json::json!({ "choices": [{ "delta": { "role": "assistant", "content": null } }] }),
            serde_json::json!({
                "choices": [{ "delta": { "tool_calls": [
                    { "index": 0, "id": "call_1", "type": "function", "function": { "name": "get_weather", "arguments": "" } },
                    { "index": 1, "id": "call_2", "type": "function", "function": { "name": "get_time", "arguments": "" } },
                ] } }]
            }),
            serde_json::json!({
                "choices": [{ "delta": { "tool_calls": [
                    { "index": 0, "function": { "arguments": "{\"city\":\"par" } },
                    { "index": 1, "function": { "arguments": "{\"tz\":" } },
                ] } }]
            }),
            serde_json::json!({
                "choices": [{ "delta": { "tool_calls": [
                    { "index": 0, "function": { "arguments": "is\"}" } },
                    { "index": 1, "function": { "arguments": "\"utc\"}" } },
                ] }, "finish_reason": "tool_calls" }]
            }),
        ] {
            events.extend(converter.push_frame(&frame).unwrap());
        }
        events.extend(converter.finish());

        let function_call_added: Vec<&serde_json::Value> = events
            .iter()
            .filter(|e| {
                e["type"] == "response.output_item.added" && e["item"]["type"] == "function_call"
            })
            .collect();
        assert_eq!(function_call_added.len(), 2);
        assert_eq!(function_call_added[0]["item"]["type"], "function_call");
        assert_eq!(function_call_added[0]["item"]["call_id"], "call_1");
        assert_eq!(function_call_added[0]["item"]["name"], "get_weather");
        assert_eq!(function_call_added[1]["item"]["call_id"], "call_2");

        let argument_deltas: Vec<&serde_json::Value> = events
            .iter()
            .filter(|e| e["type"] == "response.function_call_arguments.delta")
            .collect();
        assert_eq!(argument_deltas.len(), 4);

        let completed = events
            .iter()
            .find(|e| e["type"] == "response.completed")
            .unwrap();
        let output = completed["response"]["output"].as_array().unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["type"], "function_call");
        assert_eq!(output[0]["call_id"], "call_1");
        assert_eq!(output[0]["name"], "get_weather");
        assert_eq!(output[0]["arguments"], "{\"city\":\"paris\"}");
        assert_eq!(output[1]["call_id"], "call_2");
        assert_eq!(output[1]["arguments"], "{\"tz\":\"utc\"}");
    }

    #[test]
    fn chat_sse_to_responses_maps_usage_and_omits_when_absent() {
        let mut converter = super::ChatSseToResponses::new("model-a");
        let mut events = Vec::new();
        for frame in [
            serde_json::json!({ "choices": [{ "delta": { "role": "assistant", "content": "hi" } }] }),
            serde_json::json!({
                "choices": [],
                "usage": {
                    "prompt_tokens": 100,
                    "completion_tokens": 30,
                    "total_tokens": 130,
                    "prompt_tokens_details": { "cached_tokens": 40 },
                    "completion_tokens_details": { "reasoning_tokens": 5 }
                },
            }),
        ] {
            events.extend(converter.push_frame(&frame).unwrap());
        }
        events.extend(converter.finish());
        let completed = events
            .iter()
            .find(|e| e["type"] == "response.completed")
            .unwrap();
        let usage = &completed["response"]["usage"];
        assert_eq!(usage["input_tokens"], 100);
        assert_eq!(usage["output_tokens"], 30);
        assert_eq!(usage["input_tokens_details"]["cached_tokens"], 40);
        assert_eq!(usage["output_tokens_details"]["reasoning_tokens"], 5);
    }
    #[test]
    fn compute_cost_converts_cached_subset_at_cache_read_price() {
        let usage = crate::usage::UsageSummary {
            prompt_tokens: 200,
            completion_tokens: 30,
            cached_tokens: 120,
            reasoning_tokens: 20,
        };
        let cost = crate::config::ModelCost {
            input: 2.0,
            output: 1.0,
            cache_read: 0.5,
            cache_write: 0.0,
            tiers: vec![],
            extra: Default::default(),
        };
        let total = super::compute_cost(&usage, &cost);
        assert_eq!(
            total, 0.00025,
            "(200-120)*2 + 120*0.5 + 30*1, per 1M tokens"
        );
    }
    #[test]
    fn compute_cost_uses_tier_price_when_input_tokens_reach_threshold() {
        let usage = crate::usage::UsageSummary {
            prompt_tokens: 200,
            completion_tokens: 30,
            cached_tokens: 120,
            reasoning_tokens: 20,
        };
        let cost = crate::config::ModelCost {
            input: 1.0,
            output: 1.0,
            cache_read: 0.5,
            cache_write: 0.0,
            tiers: vec![crate::config::ModelCostTier {
                input_tokens_above: 100.0,
                input: 0.5,
                output: 0.5,
                cache_read: 0.25,
                cache_write: 0.0,
                extra: Default::default(),
            }],
            extra: Default::default(),
        };
        let total = super::compute_cost(&usage, &cost);
        assert_eq!(
            total, 0.000085,
            "tier prices: (200-120)*0.5 + 120*0.25 + 30*0.5, per 1M tokens"
        );
        let total = super::compute_cost(&usage, &cost);
        assert_eq!(
            total, 0.000085,
            "tier prices: (200-120)*0.5 + 120*0.25 + 30*0.5, per 1M tokens"
        );
    }

    #[test]
    fn lookup_model_cost_returns_price_only_when_model_has_cost() {
        let profile: crate::config::ProviderProfile = serde_json::from_value(
            serde_json::json!({
                "models": [
                    { "id": "gpt-5.4", "cost": { "input": 2.0, "output": 1.0, "cacheRead": 0.5, "cacheWrite": 0.0 } },
                    { "id": "free-model" }
                ]
            }),
        )
        .unwrap();
        let priced = super::lookup_model_cost(&profile, "gpt-5.4");
        assert_eq!(priced.as_ref().map(|m| m.input), Some(2.0));
        assert!(
            super::lookup_model_cost(&profile, "free-model").is_none(),
            "no cost config means unknown"
        );
        assert!(
            super::lookup_model_cost(&profile, "missing").is_none(),
            "unknown model means unknown"
        );
    }
}
