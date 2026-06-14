mod config;
mod daemon;
mod error;
mod ops;
mod presets;
mod proxy;
mod stats;
mod sync;
mod tui;

use napi_derive::napi;

use config::{load_config, provider_id_for, save_config, ProviderProfile};
use presets::{all_presets, get_preset, preset_to_profile};

// ─── Init ─────────────────────────────────────────────────

#[napi]
pub fn init_config() -> napi::Result<Vec<String>> {
    let dir = config::config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| napi::Error::from_reason(format!("Failed to create config dir: {}", e)))?;

    let pi_dir = config::pi_dir();
    std::fs::create_dir_all(&pi_dir)
        .map_err(|e| napi::Error::from_reason(format!("Failed to create pi dir: {}", e)))?;

    let mut messages = Vec::new();
    let config_path = config::config_path();
    if !config_path.exists() {
        save_config(&config::PiSwitchConfig::default())
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        messages.push(format!("Created {}", config_path.display()));
    } else {
        messages.push(format!("Already exists: {}", config_path.display()));
    }

    let models_path = config::models_path();
    if !models_path.exists() {
        let default_models = serde_json::json!({ "providers": {} });
        let tmp = config::config_dir().join("models.json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&default_models).unwrap() + "\n")
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        std::fs::rename(&tmp, &models_path)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        messages.push(format!("Created {}", models_path.display()));
    } else {
        messages.push(format!("Already exists: {}", models_path.display()));
    }

    Ok(messages)
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
    all_presets()
        .into_iter()
        .map(|p| PresetInfo {
            id: p.id,
            name: p.name,
            description: p.description,
            website_url: p.website_url,
            api: p.api,
            base_url: p.base_url,
            models: p.models.into_iter().map(|m| m.id).collect(),
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
        }
    };

    let backup = ops::upsert_profile(&name, &profile, None)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .map(|p| p.display().to_string());

    Ok(AddResult { name, backup })
}

#[napi]
pub fn list_profiles() -> napi::Result<String> {
    let config = load_config().map_err(|e| napi::Error::from_reason(e.to_string()))?;
    serde_json::to_string_pretty(&serde_json::json!({
        "current": config.current,
        "profiles": config.profiles,
    }))
    .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn show_profile(name: String) -> napi::Result<String> {
    let config = load_config().map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let profile = config.profiles.get(&name)
        .ok_or_else(|| napi::Error::from_reason(format!("unknown profile '{}'", name)))?;

    serde_json::to_string_pretty(&serde_json::json!({
        "name": name,
        "profile": profile,
        "providerId": provider_id_for(&config, &name),
    }))
    .map_err(|e| napi::Error::from_reason(e.to_string()))
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
    let mut checks = Vec::new();
    let config_path = config::config_path();
    let models_path = config::models_path();

    checks.push(DoctorCheck {
        ok: config_path.exists(),
        msg: format!("config file: {}", config_path.display()),
    });
    checks.push(DoctorCheck {
        ok: models_path.exists(),
        msg: format!("pi models file: {}", models_path.display()),
    });

    match load_config() {
        Ok(_) => checks.push(DoctorCheck { ok: true, msg: "config JSON is valid".into() }),
        Err(e) => checks.push(DoctorCheck { ok: false, msg: e.to_string() }),
    }

    if models_path.exists() {
        match std::fs::read_to_string(&models_path) {
            Ok(text) => {
                let ok = serde_json::from_str::<serde_json::Value>(&text).is_ok();
                checks.push(DoctorCheck { ok, msg: "models.json JSON is valid".into() });
            }
            Err(e) => checks.push(DoctorCheck { ok: false, msg: e.to_string() }),
        }
    }

    if let Ok(config) = load_config() {
        let count = config.profiles.len();
        checks.push(DoctorCheck {
            ok: count > 0,
            msg: format!("{} profile(s) configured", count),
        });

        for (name, profile) in &config.profiles {
            let base_url = profile.get("baseUrl").and_then(|v| v.as_str()).unwrap_or("");
            let api_key = profile.get("apiKey").and_then(|v| v.as_str()).unwrap_or("");
            let api = profile.get("api").and_then(|v| v.as_str()).unwrap_or("");

            checks.push(DoctorCheck {
                ok: !base_url.is_empty(),
                msg: format!("{}: baseUrl set", name),
            });
            checks.push(DoctorCheck {
                ok: !api_key.is_empty(),
                msg: format!("{}: apiKey set", name),
            });
            let valid_api = matches!(api, "openai-completions" | "anthropic-messages" | "google-generative-ai");
            checks.push(DoctorCheck {
                ok: valid_api,
                msg: format!("{}: api supported ({})", name, api),
            });
        }
    }

    Ok(checks)
}

// ─── Backup list ──────────────────────────────────────────

#[napi]
pub fn list_backups() -> napi::Result<Vec<String>> {
    let dir = config::backup_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut entries = std::fs::read_dir(&dir)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path().display().to_string())
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

// ─── TUI ──────────────────────────────────────────────────

#[napi]
pub fn run_native_tui() -> napi::Result<()> {
    tui::run_tui().map_err(|e| napi::Error::from_reason(e))
}

// ─── Proxy ────────────────────────────────────────────────
// NOTE: Full proxy logic (failover, circuit breaker, OpenAI↔Anthropic conversion)
// is implemented in src-rust/proxy.rs. It needs axum 0.7 serve API compatibility.
// The JS proxy.js currently serves as the HTTP layer. Coming in next iteration.

// ─── Proxy Server ─────────────────────────────────────────

#[napi]
pub async fn run_proxy_server(host: String, port: u16) -> napi::Result<()> {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config = load_config().map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let state = Arc::new(proxy::ProxyState {
        config: Arc::new(RwLock::new(config)),
    });

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

// ─── Daemon ───────────────────────────────────────────────

#[napi]
pub fn daemon_start_native(host: Option<String>, port: Option<u16>) -> napi::Result<String> {
    let result = daemon::daemon_start(host, port)
        .map_err(|e| napi::Error::from_reason(e))?;
    serde_json::to_string_pretty(&result)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn daemon_stop_native() -> napi::Result<String> {
    let result = daemon::daemon_stop()
        .map_err(|e| napi::Error::from_reason(e))?;
    serde_json::to_string_pretty(&result)
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn daemon_status_native() -> napi::Result<String> {
    let result = daemon::daemon_status()
        .map_err(|e| napi::Error::from_reason(e))?;
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
        .map_err(|e| napi::Error::from_reason(e))
}

#[napi]
pub fn import_config(file_path: String, passphrase: String) -> napi::Result<String> {
    sync::import_config(&file_path, &passphrase)
        .map_err(|e| napi::Error::from_reason(e))
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
