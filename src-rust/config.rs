use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Types matching pi-switch JS config ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_input")]
    pub input: Vec<String>,
    #[serde(default = "default_context_window", rename = "contextWindow", alias = "context_window")]
    pub context_window: u32,
    #[serde(default = "default_max_tokens", rename = "maxTokens", alias = "max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub cost: ModelCost,
}

impl Default for ModelEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: None,
            input: vec!["text".into()],
            context_window: 128000,
            max_tokens: 16384,
            cost: ModelCost::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderProfile {
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(default)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "modelMap")]
    pub model_map: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "exposedModels")]
    pub exposed_models: Vec<String>,
    /// Per-profile User-Agent disguise override (claude-code/codex/gemini). Falls back
    /// to the global settings.proxy.userAgent when unset.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "userAgent")]
    pub spoof: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "userAgent")]
    pub user_agent: Option<String>,
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
                    user_agent: None,
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

pub fn restore_config(backup_path: &str) -> Result<PathBuf> {
    let backup = PathBuf::from(backup_path);
    if !backup.exists() {
        return Err(AppError::Message(format!("Backup file not found: {}", backup_path)));
    }

    // Validate backup is valid JSON
    let content = std::fs::read_to_string(&backup)
        .map_err(|e| AppError::io(&backup, e))?;
    let _: PiSwitchConfig = serde_json::from_str(&content)
        .map_err(|e| AppError::Message(format!("Invalid backup file: {}", e)))?;

    // Create backup of current config before restoring
    let current_backup = backup_config("pre-restore")?;

    // Restore from backup
    let config_path = config_path();
    std::fs::copy(&backup, &config_path)
        .map_err(|e| AppError::io(&config_path, e))?;

    Ok(current_backup.unwrap_or(config_path))
}

pub fn provider_id_for(config: &PiSwitchConfig, name: &str) -> String {
    format!("{}-{}", config.settings.provider_prefix, name)
}

// ─── Validation ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
    pub level: String,
    pub path: String,
    pub message: String,
}

pub fn validate_config() -> Result<Vec<ValidationIssue>> {
    let config = load_config()?;
    let mut issues = Vec::new();

    // Validate each profile
    for (name, value) in &config.profiles {
        let profile: ProviderProfile = match serde_json::from_value(value.clone()) {
            Ok(p) => p,
            Err(e) => {
                issues.push(ValidationIssue {
                    level: "error".into(),
                    path: format!("profiles.{}", name),
                    message: format!("Invalid structure: {}", e),
                });
                continue;
            }
        };

        // Check API field
        if profile.api.is_empty() {
            issues.push(ValidationIssue {
                level: "error".into(),
                path: format!("profiles.{}.api", name),
                message: "API field is empty".into(),
            });
        } else if !matches!(profile.api.as_str(), "openai-completions" | "anthropic-messages") {
            issues.push(ValidationIssue {
                level: "warning".into(),
                path: format!("profiles.{}.api", name),
                message: format!("Unknown API type: {}", profile.api),
            });
        }

        // Check base_url format
        if profile.base_url.is_empty() {
            issues.push(ValidationIssue {
                level: "error".into(),
                path: format!("profiles.{}.baseUrl", name),
                message: "Base URL is empty".into(),
            });
        } else if !profile.base_url.starts_with("http://") && !profile.base_url.starts_with("https://") {
            issues.push(ValidationIssue {
                level: "error".into(),
                path: format!("profiles.{}.baseUrl", name),
                message: format!("Invalid URL format: {}", profile.base_url),
            });
        }

        // Check API key
        if profile.api_key.is_empty() {
            issues.push(ValidationIssue {
                level: "warning".into(),
                path: format!("profiles.{}.apiKey", name),
                message: "API key is empty".into(),
            });
        }

        // Check models
        if profile.models.is_empty() {
            issues.push(ValidationIssue {
                level: "warning".into(),
                path: format!("profiles.{}.models", name),
                message: "No models defined".into(),
            });
        } else {
            for (i, model) in profile.models.iter().enumerate() {
                if model.id.is_empty() {
                    issues.push(ValidationIssue {
                        level: "error".into(),
                        path: format!("profiles.{}.models[{}].id", name, i),
                        message: "Model ID is empty".into(),
                    });
                }
            }
        }
    }

    // Check current setting
    if let Some(ref current) = config.current {
        if !config.profiles.contains_key(current) {
            issues.push(ValidationIssue {
                level: "error".into(),
                path: "current".into(),
                message: format!("Current profile '{}' does not exist", current),
            });
        }
    }

    // Check proxy settings
    if config.settings.proxy.port == 0 {
        issues.push(ValidationIssue {
            level: "error".into(),
            path: "settings.proxy.port".into(),
            message: "Proxy port cannot be 0".into(),
        });
    }

    for (i, name) in config.settings.proxy.failover.iter().enumerate() {
        if !config.profiles.contains_key(name) {
            issues.push(ValidationIssue {
                level: "warning".into(),
                path: format!("settings.proxy.failover[{}]", i),
                message: format!("Failover provider '{}' does not exist", name),
            });
        }
    }

    Ok(issues)
}

// ─── Environment Variable Resolution ──────────────────────

pub fn resolve_env(value: &str) -> String {
    let trimmed = value.trim();
    // Check if it's an env var reference like $VAR or ${VAR}
    if trimmed.starts_with('$') {
        let var_name = trimmed
            .trim_start_matches('$')
            .trim_start_matches('{')
            .trim_end_matches('}');
        if var_name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            return std::env::var(var_name).unwrap_or_else(|_| trimmed.to_string());
        }
    }
    trimmed.to_string()
}
