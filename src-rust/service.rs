//! Shared read/shape layer.
//!
//! These functions turn the on-disk config into the JSON shapes the front-ends
//! consume. Both the napi bindings (`lib.rs`, used by the CLI) and the web REST
//! layer (`web.rs`) call into here, so the shaping lives in exactly one place.
//! Mutations live in `ops.rs`; the TUI calls both directly.

use crate::config::{self, load_config, provider_id_for};
use crate::error::{AppError, Result};
use crate::presets::{all_presets, get_preset};
use serde::Serialize;
use serde_json::{json, Value};

// ─── State / profiles ─────────────────────────────────────

/// The full app state: current profile, all profiles, and settings.
pub fn get_state() -> Result<Value> {
    let config = load_config()?;
    Ok(json!({
        "current": config.current,
        "profiles": config.profiles,
        "settings": config.settings,
    }))
}

/// A single profile plus its derived pi provider id.
pub fn get_profile(name: &str) -> Result<Value> {
    let config = load_config()?;
    let profile = config
        .profiles
        .get(name)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", name)))?;
    Ok(json!({
        "name": name,
        "profile": profile,
        "providerId": provider_id_for(&config, name),
    }))
}

/// Absolute paths of every backup file, sorted.
pub fn list_backups() -> Result<Vec<String>> {
    let dir = config::backup_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut entries = std::fs::read_dir(&dir)
        .map_err(|e| AppError::io(&dir, e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path().display().to_string())
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

// ─── Presets ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "websiteUrl")]
    pub website_url: String,
    pub api: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub models: Vec<String>,
}

pub fn presets_info() -> Vec<PresetInfo> {
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

pub fn show_preset(id: &str) -> Result<Value> {
    let preset =
        get_preset(id).ok_or_else(|| AppError::Message(format!("unknown preset '{}'", id)))?;
    serde_json::to_value(&preset).map_err(|e| AppError::Message(e.to_string()))
}

// ─── Doctor ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub ok: bool,
    pub msg: String,
}

/// Health checks over the config + pi models file. Mirrors the CLI `doctor`.
pub fn run_doctor() -> Vec<DoctorCheck> {
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
        Ok(_) => checks.push(DoctorCheck {
            ok: true,
            msg: "config JSON is valid".into(),
        }),
        Err(e) => checks.push(DoctorCheck {
            ok: false,
            msg: e.to_string(),
        }),
    }

    if models_path.exists() {
        match std::fs::read_to_string(&models_path) {
            Ok(text) => {
                let ok = serde_json::from_str::<serde_json::Value>(&text).is_ok();
                checks.push(DoctorCheck {
                    ok,
                    msg: "models.json JSON is valid".into(),
                });
            }
            Err(e) => checks.push(DoctorCheck {
                ok: false,
                msg: e.to_string(),
            }),
        }
    }

    if let Ok(config) = load_config() {
        let count = config.profiles.len();
        checks.push(DoctorCheck {
            ok: count > 0,
            msg: format!("{} profile(s) configured", count),
        });

        for (name, profile) in &config.profiles {
            let base_url = profile
                .get("baseUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("");
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
            let valid_api = matches!(
                api,
                "openai-completions"
                    | "openai-responses"
                    | "anthropic-messages"
                    | "google-generative-ai"
            );
            checks.push(DoctorCheck {
                ok: valid_api,
                msg: format!("{}: api supported ({})", name, api),
            });
        }
    }

    checks
}

// ─── Stats ────────────────────────────────────────────────

/// Usage stats as JSON (request counts, per-provider/model breakdown, circuit state).
pub fn stats_value() -> Value {
    serde_json::to_value(crate::stats::get_stats()).unwrap_or_else(|_| json!({}))
}
