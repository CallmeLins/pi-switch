mod config;
mod daemon;
mod error;
mod ops;
mod presets;
mod proxy;
mod service;
mod stats;
mod sync;
mod tui;
mod web;

use napi_derive::napi;

use config::ProviderProfile;
use presets::{get_preset, preset_to_profile};

// ─── Init ─────────────────────────────────────────────────

#[napi]
pub fn init_config() -> napi::Result<Vec<String>> {
    ops::init().map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ─── Presets ──────────────────────────────────────────────

#[napi(object)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub website_url: String,
    pub api: String,
    pub base_url: String,
    pub models: Vec<String>,
}

#[napi]
pub fn list_presets() -> Vec<PresetInfo> {
    service::presets_info()
        .into_iter()
        .map(|p| PresetInfo {
            id: p.id,
            name: p.name,
            description: p.description,
            website_url: p.website_url,
            api: p.api,
            base_url: p.base_url,
            models: p.models,
        })
        .collect()
}

#[napi]
pub fn show_preset(id: String) -> napi::Result<String> {
    let preset = get_preset(&id)
        .ok_or_else(|| napi::Error::from_reason(format!("unknown preset '{}'", id)))?;
    serde_json::to_string_pretty(&preset)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ─── Provider CRUD ────────────────────────────────────────

#[napi(object)]
pub struct AddProviderOptions {
    pub name: String,
    pub preset: Option<String>,
    pub api: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub models: Option<Vec<String>>,
}

#[napi(object)]
pub struct AddResult {
    pub name: String,
    pub backup: Option<String>,
}

#[napi]
pub fn add_provider(opts: AddProviderOptions) -> napi::Result<AddResult> {
    let name = opts.name;
    if name.is_empty() {
        return Err(napi::Error::from_reason("profile name required"));
    }

    let profile = if let Some(ref preset_id) = opts.preset {
        let preset = get_preset(preset_id)
            .ok_or_else(|| napi::Error::from_reason(format!("unknown preset '{}'", preset_id)))?;
        let models = opts.models.map(|ids| {
            ids.into_iter()
                .map(|id| config::ModelEntry {
                    id,
                    ..Default::default()
                })
                .collect()
        });
        preset_to_profile(&preset, opts.api_key.as_deref(), models)
    } else {
        let api = opts.api.as_deref().unwrap_or("openai-completions");
        let api = match api {
            "openai" => "openai-completions",
            "anthropic" => "anthropic-messages",
            other => other,
        };
        let base_url = opts.base_url
            .ok_or_else(|| napi::Error::from_reason("base_url required"))?;
        let api_key = opts.api_key
            .ok_or_else(|| napi::Error::from_reason("api_key required"))?;
        let models = opts.models
            .ok_or_else(|| napi::Error::from_reason("at least one model required"))?
            .into_iter()
            .map(|id| config::ModelEntry { id, ..Default::default() })
            .collect();

        ProviderProfile {
            api: api.to_string(),
            base_url,
            api_key,
            models,
            preset: None,
            headers: None,
            auth_header: None,
            compat: None,
            proxy: false,
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
            model_map: None,
            exposed_models: vec![],
            spoof: None,
        }
    };

    let backup = ops::upsert_profile(&name, &profile, None)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .map(|p| p.display().to_string());

    Ok(AddResult { name, backup })
}

#[napi(object)]
pub struct UpsertProviderRawOptions {
    pub name: String,
    pub profile: String, // JSON string
    pub rename_from: Option<String>,
}

#[napi(object)]
pub struct UpsertResult {
    pub name: String,
    pub backup: Option<String>,
}

#[napi]
pub fn upsert_profile_raw(name: String, profile_json: String, rename_from: Option<String>) -> napi::Result<UpsertResult> {
    let profile: ProviderProfile = serde_json::from_str(&profile_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid profile JSON: {}", e)))?;

    let backup = ops::upsert_profile(&name, &profile, rename_from.as_deref())
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .map(|p| p.display().to_string());

    Ok(UpsertResult { name, backup })
}

#[napi]
pub fn list_profiles() -> napi::Result<String> {
    let state = service::get_state().map_err(|e| napi::Error::from_reason(e.to_string()))?;
    serde_json::to_string_pretty(&state).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn show_profile(name: String) -> napi::Result<String> {
    let profile = service::get_profile(&name).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    serde_json::to_string_pretty(&profile).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi(object)]
pub struct UseResult {
    pub name: String,
    pub provider_id: String,
    pub models_backup: Option<String>,
    pub config_backup: Option<String>,
}

#[napi]
pub fn use_profile(name: String, mode: Option<String>) -> napi::Result<UseResult> {
    let outcome = ops::use_profile(&name, mode.as_deref())
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(UseResult {
        name: outcome.name,
        provider_id: outcome.provider_id,
        models_backup: outcome.models_backup.map(|p| p.display().to_string()),
        config_backup: outcome.config_backup.map(|p| p.display().to_string()),
    })
}

#[napi(object)]
pub struct RemoveResult {
    pub name: String,
    pub backup: Option<String>,
}

#[napi]
pub fn remove_profile(name: String) -> napi::Result<RemoveResult> {
    let backup = ops::remove_profile(&name)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .map(|p| p.display().to_string());

    Ok(RemoveResult { name, backup })
}

// ─── Doctor ───────────────────────────────────────────────

#[napi(object)]
pub struct DoctorCheck {
    pub ok: bool,
    pub msg: String,
}

#[napi]
pub fn doctor() -> napi::Result<Vec<DoctorCheck>> {
    Ok(service::run_doctor()
        .into_iter()
        .map(|c| DoctorCheck { ok: c.ok, msg: c.msg })
        .collect())
}

// ─── Backup list ──────────────────────────────────────────

#[napi]
pub fn list_backups() -> napi::Result<Vec<String>> {
    service::list_backups().map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ─── TUI ──────────────────────────────────────────────────

#[napi]
pub fn run_native_tui() -> napi::Result<()> {
    tui::run_tui().map_err(napi::Error::from_reason)
}

// ─── Proxy ────────────────────────────────────────────────
// NOTE: Full proxy logic (failover, circuit breaker, OpenAI↔Anthropic conversion)
// is implemented in src-rust/proxy.rs. It needs axum 0.7 serve API compatibility.
// The JS proxy.js currently serves as the HTTP layer. Coming in next iteration.

// ─── Proxy Server ─────────────────────────────────────────

#[napi]
pub async fn run_proxy_server(host: String, port: u16) -> napi::Result<()> {
    use std::sync::Arc;

    // Ensure pi's models.json has a fresh gateway provider before serving.
    if let Err(e) = ops::sync_gateway_to_pi() {
        eprintln!("Warning: failed to sync gateway provider: {}", e);
    }

    // Config is loaded per request inside the handlers, so the running proxy always
    // reflects the latest target/failover without needing a restart.
    let state = Arc::new(proxy::ProxyState {});

    let app = proxy::make_router(state);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| napi::Error::from_reason(format!("Failed to bind to {}: {}", addr, e)))?;

    eprintln!("Proxy server listening on http://{}", addr);

    let result = axum::serve(listener, app).await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(napi::Error::from_reason(format!("Server error: {}", e))),
    }
}

// ─── Web UI Server ────────────────────────────────────────

#[napi]
pub async fn run_web_server(host: String, port: u16, project_dir: Option<String>) -> napi::Result<()> {
    use std::sync::Arc;

    let password = web::resolve_password(&host);
    let state = Arc::new(web::WebState { project_dir, password: password.clone() });

    let app = web::make_web_router(state);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| napi::Error::from_reason(format!("Failed to bind to {}: {}", addr, e)))?;

    eprintln!("WebUI server listening on http://{}", addr);
    if let Some(pw) = password {
        eprintln!("Basic auth enabled (non-loopback bind). Username: admin  Password: {}", pw);
        eprintln!("(password also stored in ~/.pi-switch/webui_password)");
    }

    match axum::serve(listener, app).await {
        Ok(_) => Ok(()),
        Err(e) => Err(napi::Error::from_reason(format!("Server error: {}", e))),
    }
}

fn resolve_service(name: &str) -> napi::Result<daemon::Service> {
    daemon::service_by_name(name)
        .ok_or_else(|| napi::Error::from_reason(format!("unknown service '{}'", name)))
}

#[napi]
pub fn daemon_start_native(service: String, host: Option<String>, port: Option<u16>, project_dir: Option<String>) -> napi::Result<String> {
    let svc = resolve_service(&service)?;
    let result = daemon::daemon_start(&svc, host, port, project_dir)
        .map_err(napi::Error::from_reason)?;
    serde_json::to_string_pretty(&result)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn daemon_stop_native(service: String) -> napi::Result<String> {
    let svc = resolve_service(&service)?;
    let result = daemon::daemon_stop(&svc)
        .map_err(napi::Error::from_reason)?;
    serde_json::to_string_pretty(&result)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn daemon_status_native(service: String) -> napi::Result<String> {
    let svc = resolve_service(&service)?;
    let result = daemon::daemon_status(&svc)
        .map_err(napi::Error::from_reason)?;
    serde_json::to_string_pretty(&result)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ─── Stats ────────────────────────────────────────────────

#[napi]
pub fn get_usage_stats() -> napi::Result<String> {
    let stats = stats::get_stats();
    serde_json::to_string_pretty(&stats)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn export_logs_json() -> napi::Result<String> {
    stats::export_logs_json()
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn export_logs_csv() -> napi::Result<String> {
    stats::export_logs_csv()
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

// ─── Sync ─────────────────────────────────────────────────

#[napi]
pub fn export_config(passphrase: String) -> napi::Result<String> {
    sync::encrypt_config(&passphrase)
        .map_err(napi::Error::from_reason)
}

#[napi]
pub fn import_config(file_path: String, passphrase: String) -> napi::Result<String> {
    sync::import_config(&file_path, &passphrase)
        .map_err(napi::Error::from_reason)
}

#[napi]
pub fn export_dir() -> String {
    sync::export_dir()
}

// ─── Validation ───────────────────────────────────────────

#[napi(object)]
pub struct ValidationIssue {
    pub level: String,
    pub path: String,
    pub message: String,
}

#[napi]
pub fn validate_config() -> napi::Result<Vec<ValidationIssue>> {
    let issues = config::validate_config()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(issues.into_iter().map(|i| ValidationIssue {
        level: i.level,
        path: i.path,
        message: i.message,
    }).collect())
}

#[napi(object)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub response_time_ms: Option<u32>,
}

#[napi]
pub async fn test_provider(name: String) -> napi::Result<TestResult> {
    let result = ops::test_provider(&name)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(TestResult {
        success: result.success,
        message: result.message,
        response_time_ms: result.response_time_ms.map(|ms| ms as u32),
    })
}

#[napi]
pub async fn fetch_models(name: String) -> napi::Result<Vec<String>> {
    ops::fetch_models(&name)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn restore_backup(backup_path: String) -> napi::Result<String> {
    let current_backup = config::restore_config(&backup_path)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(format!("Config restored from backup. Current config backed up to: {}", current_backup.display()))
}

#[napi]
pub fn duplicate_provider(src_name: String, dst_name: String) -> napi::Result<String> {
    let backup = ops::duplicate_profile(&src_name, &dst_name)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    if let Some(path) = backup {
        Ok(format!("Provider '{}' duplicated as '{}'. Backup: {}", src_name, dst_name, path.display()))
    } else {
        Ok(format!("Provider '{}' duplicated as '{}'", src_name, dst_name))
    }
}

#[napi]
pub fn update_exposed_models(name: String, model_ids: Vec<String>) -> napi::Result<String> {
    let backup = ops::update_exposed_models(&name, model_ids)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    if let Some(path) = backup {
        Ok(format!("Exposed models updated. Backup: {}", path.display()))
    } else {
        Ok("Exposed models updated".to_string())
    }
}

#[napi(object)]
pub struct ModelEntryInput {
    pub id: String,
    pub name: Option<String>,
    pub input: Option<Vec<String>>,
    pub context_window: Option<u32>,
    pub max_tokens: Option<u32>,
}

#[napi]
pub fn update_provider_models(name: String, models: Vec<ModelEntryInput>) -> napi::Result<String> {
    let model_entries: Vec<config::ModelEntry> = models
        .into_iter()
        .map(|m| config::ModelEntry {
            id: m.id,
            name: m.name,
            input: m.input.unwrap_or_else(|| vec!["text".to_string()]),
            context_window: m.context_window.unwrap_or(128000),
            max_tokens: m.max_tokens.unwrap_or(16384),
            cost: config::ModelCost::default(),
        })
        .collect();

    let backup = ops::update_provider_models(&name, model_entries)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    if let Some(path) = backup {
        Ok(format!("Provider models updated. Backup: {}", path.display()))
    } else {
        Ok("Provider models updated".to_string())
    }
}

// ─── Proxy Configuration ──────────────────────────────────────────────────────

#[napi]
pub fn set_proxy_target(target: String) -> napi::Result<String> {
    // Deprecated: gateway mode routes by the model name in the request body, so there is no
    // single target. Kept for back-compat — records the field and refreshes the gateway.
    ops::set_proxy_target(Some(&target))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(format!(
        "Note: 'proxy target' is deprecated. The gateway now routes by model name (profile/model). \
         Recorded '{}' for back-compat.",
        target
    ))
}

#[napi]
pub fn set_proxy_failover(failover_profiles: Vec<String>) -> napi::Result<String> {
    let joined = failover_profiles.join(" → ");
    let empty = failover_profiles.is_empty();

    let backup = ops::set_failover(failover_profiles)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let mut msg = if empty {
        "Failover chain cleared".to_string()
    } else {
        format!("Failover chain set: {}", joined)
    };

    if let Some(path) = backup {
        msg.push_str(&format!("\nBackup: {}", path.display()));
    }
    Ok(msg)
}

