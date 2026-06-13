use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Types matching pi-switch JS config ───────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_input")]
    pub input: Vec<String>,
    #[serde(default = "default_context_window")]
    pub context_window: u32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub cost: ModelCost,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub api: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "authHeader")]
    pub auth_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat: Option<String>,
    #[serde(default)]
    pub proxy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitBreakerSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_failure_threshold")]
    #[serde(rename = "failureThreshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_cooldown")]
    #[serde(rename = "cooldownSeconds")]
    pub cooldown_seconds: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxySettings {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub failover: Vec<String>,
    #[serde(default, rename = "circuitBreaker")]
    pub circuit_breaker: CircuitBreakerSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_prefix")]
    #[serde(rename = "providerPrefix")]
    pub provider_prefix: String,
    #[serde(default = "default_write_mode")]
    #[serde(rename = "writeMode")]
    pub write_mode: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub proxy: ProxySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiSwitchConfig {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(default)]
    pub profiles: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub settings: Settings,
}

// ─── Defaults ─────────────────────────────────────────────

fn default_true() -> bool { true }
fn default_failure_threshold() -> u32 { 3 }
fn default_cooldown() -> u32 { 60 }
fn default_host() -> String { "127.0.0.1".into() }
fn default_port() -> u16 { 43112 }
fn default_prefix() -> String { "pi-switch".into() }
fn default_write_mode() -> String { "merge".into() }
fn default_input() -> Vec<String> { vec!["text".into()] }
fn default_context_window() -> u32 { 128000 }
fn default_max_tokens() -> u32 { 16384 }

impl Default for PiSwitchConfig {
    fn default() -> Self {
        Self {
            version: 1,
            current: None,
            profiles: Default::default(),
            settings: Settings {
                provider_prefix: default_prefix(),
                write_mode: default_write_mode(),
                language: None,
                proxy: ProxySettings {
                    host: default_host(),
                    port: default_port(),
                    target: None,
                    failover: vec![],
                    circuit_breaker: CircuitBreakerSettings {
                        enabled: true,
                        failure_threshold: 3,
                        cooldown_seconds: 60,
                    },
                },
            },
        }
    }
}

// ─── Paths ────────────────────────────────────────────────

pub fn config_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".pi-switch")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn backup_dir() -> PathBuf {
    config_dir().join("backups")
}

pub fn pi_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".pi").join("agent")
}

pub fn models_path() -> PathBuf {
    pi_dir().join("models.json")
}

// ─── Load / save ──────────────────────────────────────────

pub fn load_config() -> Result<PiSwitchConfig> {
    let path = config_path();
    if !path.exists() {
        return Ok(PiSwitchConfig::default());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| AppError::io(&path, e))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::json(&path, e))
}

pub fn save_config(config: &PiSwitchConfig) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::io(&dir, e))?;
    let path = config_path();
    let tmp = dir.join(format!("config.json.tmp-{}", std::process::id()));
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| AppError::json(&path, e))?;
    std::fs::write(&tmp, json + "\n")
        .map_err(|e| AppError::io(&tmp, e))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| AppError::io(&path, e))?;
    Ok(())
}

pub fn backup_config(label: &str) -> Result<Option<PathBuf>> {
    let path = config_path();
    if !path.exists() {
        return Ok(None);
    }
    let dir = backup_dir();
    std::fs::create_dir_all(&dir).map_err(|e| AppError::io(&dir, e))?;
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S-%3fZ");
    let dest = dir.join(format!("{}-{}.json", label, ts));
    std::fs::copy(&path, &dest).map_err(|e| AppError::io(&dest, e))?;
    Ok(Some(dest))
}

pub fn provider_id_for(config: &PiSwitchConfig, name: &str) -> String {
    format!("{}-{}", config.settings.provider_prefix, name)
}
