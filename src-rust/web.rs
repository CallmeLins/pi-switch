//! Web UI backend: an axum server that exposes the same operations as the CLI/TUI
//! over `REST /api/*` and serves the embedded React frontend.
//!
//! Every handler is a thin adapter: parse input → call `ops`/`service`/`daemon`/`sync`
//! → serialize output. All business logic stays in those shared modules, so adding a
//! capability to the web UI means wiring one route here — not reimplementing anything.

use crate::error::AppError;
use crate::{config, daemon, ops, service, stats, sync};
use axum::{
    body::Body,
    extract::{Path, Query, Request, State},
    http::{header, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Json, Response},
    routing::{get, post, put},
    Router,
};
use base64::Engine;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

// ─── Embedded frontend ────────────────────────────────────
//
// `webui/dist` is produced by `npm run build:webui`. In release builds rust-embed
// bakes the files into the .node; in debug builds it reads them from disk at runtime
// (so `vite build` + re-run picks up changes without recompiling).

#[derive(RustEmbed)]
#[folder = "webui/dist"]
struct WebAssets;

// ─── Shared state ─────────────────────────────────────────

pub struct WebState {
    /// The JS project dir (parent of bin/), threaded through so proxy start/stop
    /// launched from the web UI can locate bin/pi-switch.js.
    pub project_dir: Option<String>,
    /// When `Some`, HTTP Basic auth (user `admin`) is required — enabled automatically
    /// for non-loopback binds. `None` for localhost.
    pub password: Option<String>,
}

// ─── Error type ───────────────────────────────────────────

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}
impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        ApiError(StatusCode::BAD_REQUEST, e.to_string())
    }
}
impl From<String> for ApiError {
    fn from(e: String) -> Self {
        ApiError(StatusCode::BAD_REQUEST, e)
    }
}

type ApiJson = std::result::Result<Json<Value>, ApiError>;

// ─── Router ───────────────────────────────────────────────

pub fn make_web_router(state: Arc<WebState>) -> Router {
    let api = Router::new()
        // reads
        .route("/state", get(get_state))
        .route("/presets", get(get_presets))
        .route("/presets/:id", get(get_preset))
        .route(
            "/profiles/:name",
            get(get_profile).put(put_profile).delete(delete_profile),
        )
        .route("/doctor", get(get_doctor))
        .route("/config/validate", get(get_validate))
        .route("/backups", get(get_backups))
        .route("/stats", get(get_stats))
        .route("/stats/conversations", get(get_stats_conversations))
        .route("/stats/conversations/:id/requests", get(get_conversation_requests))
        .route("/proxy/status", get(get_proxy_status))
        .route("/webui/info", get(get_webui_info))
        .route("/logs/export", get(get_logs_export))
        // package management
        .route("/packages", get(get_packages).post(post_package))
        .route("/packages/import", post(post_package_import))
        .route("/packages/:id", get(get_package).delete(delete_package))
        .route("/packages/:id/toggle", post(post_package_toggle))
        // profile mutations
        .route("/init", post(post_init))
        .route("/profiles", post(post_profile))
        .route("/profiles/:name/duplicate", post(post_duplicate))
        .route("/profiles/:name/use", post(post_use))
        .route("/profiles/:name/test", post(post_test))
        .route("/profiles/:name/fetch-models", post(post_fetch_models))
        .route("/profiles/:name/models", put(put_models))
        .route("/profiles/:name/expose", put(put_expose))
        .route("/profiles/:name/spoof", put(put_spoof))
        // proxy + settings + config
        .route("/proxy/start", post(post_proxy_start))
        .route("/proxy/stop", post(post_proxy_stop))
        .route("/proxy/failover", put(put_failover))
        .route("/settings", put(put_settings))
        .route("/config/export", post(post_config_export))
        .route("/config/import", post(post_config_import))
        .route("/config/restore", post(post_config_restore))
        .fallback(api_not_found)
        .with_state(state.clone());

    Router::new()
        .nest("/api", api)
        .fallback(static_handler)
        .layer(axum::middleware::from_fn_with_state(state, auth_mw))
}

// ─── Auth (Basic, only when password set) ─────────────────

async fn auth_mw(State(state): State<Arc<WebState>>, req: Request, next: Next) -> Response {
    if let Some(ref pw) = state.password {
        let expected = format!("admin:{}", pw);
        let ok = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Basic "))
            .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|creds| creds == expected)
            .unwrap_or(false);
        if !ok {
            return (
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, "Basic realm=\"pi-switch\"")],
                "Unauthorized",
            )
                .into_response();
        }
    }
    next.run(req).await
}

/// Decide whether the server needs auth: loopback binds run open; anything else
/// requires Basic auth with an auto-generated password stored under ~/.pi-switch/.
pub fn resolve_password(host: &str) -> Option<String> {
    if matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return None;
    }
    let path = config::config_dir().join("webui_password");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    // Generate a fresh 24-hex-char password and persist it.
    use rand::RngCore;
    let mut buf = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut buf);
    let pw = buf.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    std::fs::create_dir_all(config::config_dir()).ok();
    std::fs::write(&path, &pw).ok();
    Some(pw)
}

// ─── Read handlers ────────────────────────────────────────

async fn get_state() -> ApiJson {
    Ok(Json(service::get_state()?))
}

async fn get_presets() -> Json<Value> {
    Json(json!(service::presets_info()))
}

async fn get_preset(Path(id): Path<String>) -> ApiJson {
    Ok(Json(service::show_preset(&id)?))
}

async fn get_profile(Path(name): Path<String>) -> ApiJson {
    Ok(Json(service::get_profile(&name)?))
}

async fn get_doctor() -> Json<Value> {
    Json(json!(service::run_doctor()))
}

async fn get_validate() -> ApiJson {
    let issues = config::validate_config()?;
    Ok(Json(json!(issues)))
}

async fn get_backups() -> ApiJson {
    Ok(Json(json!(service::list_backups()?)))
}

async fn get_stats(Query(q): Query<HashMap<String, String>>) -> ApiJson {
    let window = crate::stats::parse_window_query(
        q.get("range").map(String::as_str),
        q.get("from").map(String::as_str),
        q.get("to").map(String::as_str),
    )?;
    // Optional 0-based page / per-page limit for the recent-request details
    // list; omitted values fall back to page 0 / limit 100 so old callers
    // keep the previous behaviour.
    let page = q.get("page").and_then(|s| s.parse::<usize>().ok());
    let limit = q.get("limit").and_then(|s| s.parse::<usize>().ok());
    Ok(Json(service::stats_value(window, page, limit)))
}

async fn get_stats_conversations(Query(q): Query<HashMap<String, String>>) -> ApiJson {
    // Same window semantics as `/stats`: no range/from/to at all means full
    // history (the All-time preset); a partial or invalid window is a 400.
    let window = crate::stats::parse_window_query(
        q.get("range").map(String::as_str),
        q.get("from").map(String::as_str),
        q.get("to").map(String::as_str),
    )?;
    let page = q.get("page").and_then(|s| s.parse::<usize>().ok());
    let limit = q.get("limit").and_then(|s| s.parse::<usize>().ok());
    Ok(Json(service::conversations_value(window, page, limit)))
}

async fn get_conversation_requests(
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiJson {
    if id.trim().is_empty() {
        return Err("conversation id must not be empty".to_string().into());
    }
    // Same optional paging as `/stats`: omitted or unparseable values fall
    // back to page 0 / limit 100.
    let page = q.get("page").and_then(|s| s.parse::<usize>().ok());
    let limit = q.get("limit").and_then(|s| s.parse::<usize>().ok());
    Ok(Json(service::conversation_requests_value(&id, page, limit)))
}

async fn get_proxy_status() -> ApiJson {
    let result = daemon::daemon_status(&daemon::PROXY)?;
    Ok(Json(serde_json::to_value(result).unwrap_or_else(|_| json!({}))))
}

async fn get_webui_info(State(state): State<Arc<WebState>>) -> Json<Value> {
    Json(json!({
        "authRequired": state.password.is_some(),
    }))
}

async fn get_logs_export(Query(q): Query<HashMap<String, String>>) -> Response {
    let format = q.get("format").map(|s| s.as_str()).unwrap_or("json");
    let (body, content_type, filename) = match format {
        "csv" => match stats::export_logs_csv() {
            Ok(text) => (text, "text/csv", "pi-switch-logs.csv"),
            Err(e) => return ApiError::from(e).into_response(),
        },
        _ => match stats::export_logs_json() {
            Ok(text) => (text, "application/json", "pi-switch-logs.json"),
            Err(e) => return ApiError::from(e).into_response(),
        },
    };
    (
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        body,
    )
        .into_response()
}

// ─── Package handlers ─────────────────────────────────────

async fn get_packages() -> ApiJson {
    let packages = crate::package_ops::list_packages()?;
    Ok(Json(json!({ "packages": packages })))
}

async fn get_package(Path(id): Path<String>) -> ApiJson {
    let package = crate::package_ops::get_package(&id)?;
    Ok(Json(serde_json::to_value(package).unwrap_or_else(|_| json!({}))))
}

#[derive(Deserialize)]
struct PackageBody {
    name: String,
    version: String,
}

async fn post_package(Json(body): Json<PackageBody>) -> ApiJson {
    // Create spec from the body (for now, just use id as spec)
    let spec = format!("{}@{}", body.name, body.version);
    let package = crate::package_ops::add_package(&spec)?;
    Ok(Json(json!({ "ok": true, "package": package.name })))
}

async fn post_package_toggle(Path(id): Path<String>) -> ApiJson {
    let package = crate::package_ops::toggle_package(&id)?;
    Ok(Json(json!({ "ok": true, "enabled": package.enabled })))
}

async fn delete_package(Path(id): Path<String>) -> ApiJson {
    crate::package_ops::remove_package(&id)?;
    Ok(Json(json!({ "ok": true })))
}

async fn post_package_import() -> ApiJson {
    let imported = crate::package_ops::import_from_pi()?;
    Ok(Json(json!({
        "ok": true,
        "count": imported.len(),
        "message": format!("Imported {} packages from Pi Agent", imported.len())
    })))
}

// ─── Mutation handlers ────────────────────────────────────

fn ok(value: Value) -> ApiJson {
    Ok(Json(value))
}

fn backup_msg(backup: Option<std::path::PathBuf>) -> Value {
    json!({ "ok": true, "backup": backup.map(|p| p.display().to_string()) })
}

async fn post_init() -> ApiJson {
    let messages = ops::init()?;
    ok(json!({ "messages": messages }))
}

#[derive(Deserialize)]
struct UpsertBody {
    name: String,
    profile: Value,
}

async fn post_profile(Json(body): Json<UpsertBody>) -> ApiJson {
    let profile: config::ProviderProfile = serde_json::from_value(body.profile)
        .map_err(|e| AppError::Message(format!("invalid profile: {}", e)))?;
    let backup = ops::upsert_profile(&body.name, &profile, None)?;
    ok(backup_msg(backup))
}

#[derive(Deserialize)]
struct PutProfileBody {
    profile: Value,
    #[serde(rename = "renameFrom")]
    rename_from: Option<String>,
}

async fn put_profile(Path(name): Path<String>, Json(body): Json<PutProfileBody>) -> ApiJson {
    let profile: config::ProviderProfile = serde_json::from_value(body.profile)
        .map_err(|e| AppError::Message(format!("invalid profile: {}", e)))?;
    let backup = ops::upsert_profile(&name, &profile, body.rename_from.as_deref())?;
    ok(backup_msg(backup))
}

async fn delete_profile(Path(name): Path<String>) -> ApiJson {
    let backup = ops::remove_profile(&name)?;
    ok(backup_msg(backup))
}

#[derive(Deserialize)]
struct DuplicateBody {
    #[serde(rename = "as")]
    as_name: String,
}

async fn post_duplicate(Path(name): Path<String>, Json(body): Json<DuplicateBody>) -> ApiJson {
    let backup = ops::duplicate_profile(&name, &body.as_name)?;
    ok(backup_msg(backup))
}

#[derive(Deserialize)]
struct UseBody {
    mode: Option<String>,
}

async fn post_use(Path(name): Path<String>, Json(body): Json<UseBody>) -> ApiJson {
    let outcome = ops::use_profile(&name, body.mode.as_deref())?;
    ok(json!({
        "ok": true,
        "name": outcome.name,
        "providerId": outcome.provider_id,
        "modelsBackup": outcome.models_backup.map(|p| p.display().to_string()),
        "configBackup": outcome.config_backup.map(|p| p.display().to_string()),
    }))
}

async fn post_test(Path(name): Path<String>) -> ApiJson {
    let result = ops::test_provider(&name).await?;
    ok(json!({
        "success": result.success,
        "message": result.message,
        "responseTimeMs": result.response_time_ms,
    }))
}

async fn post_fetch_models(Path(name): Path<String>) -> ApiJson {
    let models = ops::fetch_models(&name).await?;
    ok(json!({ "models": models }))
}

#[derive(Deserialize)]
struct ModelsBody {
    models: Vec<config::ModelEntry>,
}

async fn put_models(Path(name): Path<String>, Json(body): Json<ModelsBody>) -> ApiJson {
    let backup = ops::update_provider_models(&name, body.models)?;
    ok(backup_msg(backup))
}

#[derive(Deserialize)]
struct ExposeBody {
    #[serde(rename = "modelIds")]
    model_ids: Vec<String>,
}

async fn put_expose(Path(name): Path<String>, Json(body): Json<ExposeBody>) -> ApiJson {
    let backup = ops::update_exposed_models(&name, body.model_ids)?;
    ok(backup_msg(backup))
}

#[derive(Deserialize)]
struct SpoofBody {
    spoof: Option<String>,
}

async fn put_spoof(Path(name): Path<String>, Json(body): Json<SpoofBody>) -> ApiJson {
    let backup = ops::set_profile_spoof(&name, body.spoof)?;
    ok(backup_msg(backup))
}

#[derive(Deserialize)]
struct ProxyStartBody {
    host: Option<String>,
    port: Option<u16>,
}

async fn post_proxy_start(
    State(state): State<Arc<WebState>>,
    Json(body): Json<ProxyStartBody>,
) -> ApiJson {
    let result = daemon::daemon_start(&daemon::PROXY, body.host, body.port, state.project_dir.clone())?;
    ok(serde_json::to_value(result).unwrap_or_else(|_| json!({})))
}

async fn post_proxy_stop() -> ApiJson {
    let result = daemon::daemon_stop(&daemon::PROXY)?;
    ok(serde_json::to_value(result).unwrap_or_else(|_| json!({})))
}

#[derive(Deserialize)]
struct FailoverBody {
    failover: Vec<String>,
}

async fn put_failover(Json(body): Json<FailoverBody>) -> ApiJson {
    let backup = ops::set_failover(body.failover)?;
    ok(backup_msg(backup))
}

async fn put_settings(Json(settings): Json<Value>) -> ApiJson {
    let backup = ops::update_settings(&settings)?;
    ok(backup_msg(backup))
}

#[derive(Deserialize)]
struct ExportBody {
    passphrase: String,
}

async fn post_config_export(Json(body): Json<ExportBody>) -> ApiJson {
    let path = sync::encrypt_config(&body.passphrase)?;
    ok(json!({ "ok": true, "path": path }))
}

#[derive(Deserialize)]
struct ImportBody {
    #[serde(rename = "filePath")]
    file_path: String,
    passphrase: String,
}

async fn post_config_import(Json(body): Json<ImportBody>) -> ApiJson {
    let msg = sync::import_config(&body.file_path, &body.passphrase)?;
    ok(json!({ "ok": true, "message": msg }))
}

#[derive(Deserialize)]
struct RestoreBody {
    #[serde(rename = "backupPath")]
    backup_path: String,
}

async fn post_config_restore(Json(body): Json<RestoreBody>) -> ApiJson {
    let current_backup = config::restore_config(&body.backup_path)?;
    ok(json!({ "ok": true, "backup": current_backup.display().to_string() }))
}

async fn api_not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "unknown API endpoint".into())
}

// ─── Static file serving (SPA) ────────────────────────────

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = WebAssets::get(path) {
        let mime = content.metadata.mimetype();
        return ([(header::CONTENT_TYPE, mime.to_string())], content.data.into_owned())
            .into_response();
    }

    // SPA history fallback: serve index.html for unknown non-asset routes.
    match WebAssets::get("index.html") {
        Some(content) => (
            [(header::CONTENT_TYPE, "text/html".to_string())],
            content.data.into_owned(),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/html".to_string())],
            Body::from(PLACEHOLDER_HTML),
        )
            .into_response(),
    }
}

const PLACEHOLDER_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<title>pi-switch WebUI</title></head><body style="font-family:system-ui;max-width:40rem;margin:4rem auto;line-height:1.6">
<h1>pi-switch WebUI</h1>
<p>The frontend has not been built yet. Run:</p>
<pre style="background:#f4f4f5;padding:1rem;border-radius:.5rem">npm run build:webui
npm run build:native</pre>
<p>then restart the server. The REST API under <code>/api</code> is already live.</p>
</body></html>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tower::ServiceExt;

    fn router() -> Router {
        make_web_router(Arc::new(WebState {
            project_dir: None,
            password: None,
        }))
    }

    async fn get(uri: &str) -> Response {
        router()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn stats_without_window_params_returns_200() {
        let res = get("/api/stats").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn stats_custom_without_bounds_is_rejected_not_500() {
        for uri in [
            "/api/stats?range=custom",
            "/api/stats?range=custom&from=1785664800000",
            "/api/stats?range=custom&to=1785672000000",
            "/api/stats?range=today",
            "/api/stats?range=today&from=1785664800000",
        ] {
            let res = get(uri).await;
            assert_eq!(
                res.status(),
                StatusCode::BAD_REQUEST,
                "{uri} should be 400, not 500"
            );
        }
    }

    #[tokio::test]
    async fn stats_invalid_range_or_inverted_window_is_rejected_not_500() {
        for uri in [
            "/api/stats?range=week",
            "/api/stats?range=custom&from=abc&to=1785672000000",
            "/api/stats?range=custom&from=1785672000000&to=1785664800000",
            "/api/stats?range=custom&from=1785672000000&to=1785672000000",
        ] {
            let res = get(uri).await;
            assert_eq!(
                res.status(),
                StatusCode::BAD_REQUEST,
                "{uri} should be 400, not 500"
            );
        }
    }

    #[tokio::test]
    async fn stats_with_valid_window_returns_200() {
        for uri in [
            "/api/stats?range=custom&from=1785664800000&to=1785672000000",
            "/api/stats?range=today&from=1785664800000&to=1785672000000",
            "/api/stats?range=last24h&from=1785664800000&to=1785672000000",
            "/api/stats?range=last7d&from=1785664800000&to=1785672000000",
            "/api/stats?from=1785664800000&to=1785672000000",
        ] {
            let res = get(uri).await;
            assert_eq!(res.status(), StatusCode::OK, "{uri} should be 200");
        }
    }

    #[tokio::test]
    async fn stats_response_includes_recent_request_total_field() {
        let res = get("/api/stats?range=today&from=1785664800000&to=1785672000000&page=0&limit=50")
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let total = body.get("recentRequestTotal").expect("stats body must include recentRequestTotal");
        assert!(total.is_u64() || total.is_i64(), "recentRequestTotal must be a number");
    }

    #[tokio::test]
    async fn proxy_status_route_still_registered() {
        // Regression guard: /proxy/status must never be dropped when adding
        // sibling routes (it feeds the ProxyPanel on mount).
        let res = get("/api/proxy/status").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn conversations_without_window_params_returns_200_all_history() {
        let res = get("/api/stats/conversations").await;
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let convs = body.get("conversations").expect("body must include conversations");
        assert!(convs.is_array(), "conversations must be an array");
        let total = body.get("total").expect("body must include total");
        assert!(total.is_u64() || total.is_i64(), "total must be a number");
    }

    #[tokio::test]
    async fn conversations_with_valid_window_returns_200() {
        for uri in [
            "/api/stats/conversations?range=custom&from=1785664800000&to=1785672000000",
            "/api/stats/conversations?range=today&from=1785664800000&to=1785672000000",
            "/api/stats/conversations?range=last24h&from=1785664800000&to=1785672000000",
            "/api/stats/conversations?range=last7d&from=1785664800000&to=1785672000000",
            "/api/stats/conversations?from=1785664800000&to=1785672000000",
            "/api/stats/conversations?page=0&limit=50",
        ] {
            let res = get(uri).await;
            assert_eq!(res.status(), StatusCode::OK, "{uri} should be 200");
        }
    }

    #[tokio::test]
    async fn conversations_invalid_window_is_rejected_not_500() {
        for uri in [
            "/api/stats/conversations?range=week",
            "/api/stats/conversations?range=custom",
            "/api/stats/conversations?range=today&from=1785664800000",
            "/api/stats/conversations?range=custom&from=abc&to=1785672000000",
            "/api/stats/conversations?range=custom&from=1785672000000&to=1785664800000",
        ] {
            let res = get(uri).await;
            assert_eq!(
                res.status(),
                StatusCode::BAD_REQUEST,
                "{uri} should be 400, not 500"
            );
        }
    }

    #[tokio::test]
    async fn conversation_requests_with_valid_id_returns_200() {
        let res = get("/api/stats/conversations/conv-a/requests?page=0&limit=5").await;
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let requests = body.get("requests").expect("body must include requests");
        assert!(requests.is_array(), "requests must be an array");
        let total = body.get("total").expect("body must include total");
        assert!(total.is_u64() || total.is_i64(), "total must be a number");
    }

    #[tokio::test]
    async fn conversation_requests_empty_id_is_rejected() {
        let res = get("/api/stats/conversations//requests").await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "empty id should be 400");
    }

    #[tokio::test]
    async fn conversation_requests_coexists_with_conversations_list() {
        // The precise /stats/conversations route must still resolve after the
        // deeper :id/requests route is registered.
        let list = get("/api/stats/conversations").await;
        assert_eq!(list.status(), StatusCode::OK, "list route unaffected");
        let detail = get("/api/stats/conversations/conv-a/requests").await;
        assert_eq!(detail.status(), StatusCode::OK, "detail route resolves too");
    }
}
