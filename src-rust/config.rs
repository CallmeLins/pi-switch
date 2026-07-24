use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::PathBuf;

// ─── Types matching pi-switch JS config ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "thinkingLevelMap"
    )]
    pub thinking_level_map: Option<Value>,
    #[serde(default = "default_input")]
    pub input: Vec<String>,
    #[serde(
        default = "default_context_window",
        rename = "contextWindow",
        alias = "context_window"
    )]
    pub context_window: u32,
    #[serde(
        default = "default_max_tokens",
        rename = "maxTokens",
        alias = "max_tokens"
    )]
    pub max_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<ModelCost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for ModelEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: None,
            api: None,
            base_url: None,
            reasoning: None,
            thinking_level_map: None,
            input: vec!["text".into()],
            context_window: 128000,
            max_tokens: 16384,
            cost: None,
            headers: None,
            compat: None,
            extra: Map::new(),
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiers: Vec<ModelCostTier>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelCostTier {
    pub input_tokens_above: f64,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ModelEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "authHeader")]
    pub auth_header: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "modelOverrides"
    )]
    pub model_overrides: Option<Map<String, Value>>,
    #[serde(default)]
    pub proxy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "modelMap")]
    pub model_map: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "exposedModels")]
    pub exposed_models: Vec<String>,
    /// Per-profile User-Agent disguise override (claude-code/codex/gemini). Falls back
    /// to the global settings.proxy.userAgent when unset.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "userAgent")]
    pub spoof: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSettings {
    #[serde(default = "default_web_host")]
    pub host: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            host: default_web_host(),
            port: default_web_port(),
        }
    }
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
    #[serde(default)]
    pub web: WebSettings,
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

fn default_true() -> bool {
    true
}
fn default_failure_threshold() -> u32 {
    3
}
fn default_cooldown() -> u32 {
    60
}
fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    43112
}
fn default_web_host() -> String {
    "127.0.0.1".into()
}
fn default_web_port() -> u16 {
    43110
}
fn default_prefix() -> String {
    "pi-switch".into()
}
fn default_write_mode() -> String {
    "merge".into()
}
fn default_input() -> Vec<String> {
    vec!["text".into()]
}
fn default_context_window() -> u32 {
    128000
}
fn default_max_tokens() -> u32 {
    16384
}

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
                web: WebSettings {
                    host: default_web_host(),
                    port: default_web_port(),
                },
            },
        }
    }
}

// ─── Paths ────────────────────────────────────────────────

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi-switch")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn backup_dir() -> PathBuf {
    config_dir().join("backups")
}

pub fn pi_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("agent")
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
    let text = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    serde_json::from_str(&text).map_err(|e| AppError::json(&path, e))
}

pub fn save_config(config: &PiSwitchConfig) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| AppError::io(&dir, e))?;
    let path = config_path();
    let tmp = dir.join(format!("config.json.tmp-{}", std::process::id()));
    let json = serde_json::to_string_pretty(config).map_err(|e| AppError::json(&path, e))?;
    std::fs::write(&tmp, json + "\n").map_err(|e| AppError::io(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| AppError::io(&path, e))?;
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
        return Err(AppError::Message(format!(
            "Backup file not found: {}",
            backup_path
        )));
    }

    // Validate backup is valid JSON
    let content = std::fs::read_to_string(&backup).map_err(|e| AppError::io(&backup, e))?;
    let _: PiSwitchConfig = serde_json::from_str(&content)
        .map_err(|e| AppError::Message(format!("Invalid backup file: {}", e)))?;

    // Create backup of current config before restoring
    let current_backup = backup_config("pre-restore")?;

    // Restore from backup
    let config_path = config_path();
    std::fs::copy(&backup, &config_path).map_err(|e| AppError::io(&config_path, e))?;

    Ok(current_backup.unwrap_or(config_path))
}

pub fn provider_id_for(config: &PiSwitchConfig, name: &str) -> String {
    format!("{}-{}", config.settings.provider_prefix, name)
}

// ─── Validation ───────────────────────────────────────────

pub const SUPPORTED_APIS: [&str; 4] = [
    "openai-completions",
    "openai-responses",
    "anthropic-messages",
    "google-generative-ai",
];

fn validate_string_map(value: &Value, path: &str) -> std::result::Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;
    for (key, value) in object {
        if !value.is_string() {
            return Err(format!("{path}.{key} must be a string"));
        }
    }
    Ok(())
}

fn validate_thinking_level_map(value: &Value, path: &str) -> std::result::Result<(), String> {
    const LEVELS: [&str; 7] = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;
    for (key, value) in object {
        if !LEVELS.contains(&key.as_str()) {
            return Err(format!("{path}.{key} is not a supported thinking level"));
        }
        if !value.is_string() && !value.is_null() {
            return Err(format!("{path}.{key} must be a string or null"));
        }
    }
    Ok(())
}

fn validate_compat(value: &Value, path: &str) -> std::result::Result<(), String> {
    const BOOLEAN_FIELDS: [&str; 19] = [
        "supportsStore",
        "supportsDeveloperRole",
        "supportsReasoningEffort",
        "supportsUsageInStreaming",
        "requiresToolResultName",
        "requiresAssistantAfterToolResult",
        "requiresThinkingAsText",
        "requiresReasoningContentOnAssistantMessages",
        "supportsStrictMode",
        "sendSessionAffinityHeaders",
        "supportsLongCacheRetention",
        "supportsToolSearch",
        "supportsEagerToolInputStreaming",
        "supportsCacheControlOnTools",
        "forceAdaptiveThinking",
        "supportsToolReferences",
        "allowEmptySignature",
        "supportsStrictTools",
        "supportsOpenAIGrammarTools",
    ];
    const THINKING_FORMATS: [&str; 10] = [
        "openai",
        "openrouter",
        "together",
        "deepseek",
        "zai",
        "qwen",
        "chat-template",
        "qwen-chat-template",
        "string-thinking",
        "ant-ling",
    ];
    const SESSION_FORMATS: [&str; 3] = ["openai", "openai-nosession", "openrouter"];

    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;
    for (key, value) in object {
        if BOOLEAN_FIELDS.contains(&key.as_str()) && !value.is_boolean() {
            return Err(format!("{path}.{key} must be a boolean"));
        }
        match key.as_str() {
            "maxTokensField" => {
                if !matches!(value.as_str(), Some("max_completion_tokens" | "max_tokens")) {
                    return Err(format!(
                        "{path}.{key} must be max_completion_tokens or max_tokens"
                    ));
                }
            }
            "thinkingFormat" => {
                if !value
                    .as_str()
                    .is_some_and(|item| THINKING_FORMATS.contains(&item))
                {
                    return Err(format!("{path}.{key} is not supported"));
                }
            }
            "cacheControlFormat" => {
                if value.as_str() != Some("anthropic") {
                    return Err(format!("{path}.{key} must be anthropic"));
                }
            }
            "sessionAffinityFormat" => {
                if !value
                    .as_str()
                    .is_some_and(|item| SESSION_FORMATS.contains(&item))
                {
                    return Err(format!("{path}.{key} is not supported"));
                }
            }
            "deferredToolsMode" => {
                if value.as_str() != Some("kimi") {
                    return Err(format!("{path}.{key} must be kimi"));
                }
            }
            "chatTemplateKwargs" | "openRouterRouting" | "vercelGatewayRouting"
                if !value.is_object() =>
            {
                return Err(format!("{path}.{key} must be an object"));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_model(model: &ModelEntry, path: &str) -> std::result::Result<(), String> {
    if model.id.trim().is_empty() {
        return Err(format!("{path}.id must not be empty"));
    }
    if let Some(api) = model.api.as_deref() {
        if !SUPPORTED_APIS.contains(&api) {
            return Err(format!("{path}.api is not supported: {api}"));
        }
    }
    if let Some(base_url) = model.base_url.as_deref() {
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(format!(
                "{path}.baseUrl must start with http:// or https://"
            ));
        }
    }
    if model.input.is_empty()
        || model
            .input
            .iter()
            .any(|input| !matches!(input.as_str(), "text" | "image"))
    {
        return Err(format!("{path}.input must contain only text or image"));
    }
    if model.context_window == 0 {
        return Err(format!("{path}.contextWindow must be greater than 0"));
    }
    if model.max_tokens == 0 {
        return Err(format!("{path}.maxTokens must be greater than 0"));
    }
    if let Some(cost) = &model.cost {
        validate_cost(cost, &format!("{path}.cost"))?;
    }
    if let Some(headers) = &model.headers {
        validate_string_map(&Value::Object(headers.clone()), &format!("{path}.headers"))?;
    }
    if let Some(map) = &model.thinking_level_map {
        validate_thinking_level_map(map, &format!("{path}.thinkingLevelMap"))?;
    }
    if let Some(compat) = &model.compat {
        validate_compat(compat, &format!("{path}.compat"))?;
    }
    Ok(())
}

fn validate_cost(cost: &ModelCost, path: &str) -> std::result::Result<(), String> {
    let rates = [cost.input, cost.output, cost.cache_read, cost.cache_write];
    if rates.iter().any(|rate| !rate.is_finite() || *rate < 0.0) {
        return Err(format!("{path} rates must be finite non-negative numbers"));
    }
    for (index, tier) in cost.tiers.iter().enumerate() {
        let tier_rates = [tier.input, tier.output, tier.cache_read, tier.cache_write];
        if !tier.input_tokens_above.is_finite()
            || tier.input_tokens_above < 0.0
            || tier_rates
                .iter()
                .any(|rate| !rate.is_finite() || *rate < 0.0)
        {
            return Err(format!(
                "{path}.tiers[{index}] must contain finite non-negative numbers"
            ));
        }
    }
    Ok(())
}

fn validate_model_override(value: &Value, path: &str) -> std::result::Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;
    if object
        .get("name")
        .is_some_and(|value| value.as_str().is_none_or(str::is_empty))
    {
        return Err(format!("{path}.name must be a non-empty string"));
    }
    if object
        .get("reasoning")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err(format!("{path}.reasoning must be a boolean"));
    }
    if let Some(input) = object.get("input") {
        let valid = input.as_array().is_some_and(|items| {
            items
                .iter()
                .all(|item| matches!(item.as_str(), Some("text" | "image")))
        });
        if !valid {
            return Err(format!("{path}.input must contain only text or image"));
        }
    }
    if let Some(map) = object.get("thinkingLevelMap") {
        validate_thinking_level_map(map, &format!("{path}.thinkingLevelMap"))?;
    }
    if let Some(headers) = object.get("headers") {
        validate_string_map(headers, &format!("{path}.headers"))?;
    }
    if let Some(compat) = object.get("compat") {
        validate_compat(compat, &format!("{path}.compat"))?;
    }
    if let Some(cost) = object.get("cost") {
        let cost_object = cost
            .as_object()
            .ok_or_else(|| format!("{path}.cost must be an object"))?;
        for (key, value) in cost_object {
            if key == "tiers" {
                let tiers = value
                    .as_array()
                    .ok_or_else(|| format!("{path}.cost.tiers must be an array"))?;
                for (index, tier) in tiers.iter().enumerate() {
                    let tier = tier
                        .as_object()
                        .ok_or_else(|| format!("{path}.cost.tiers[{index}] must be an object"))?;
                    for field in [
                        "inputTokensAbove",
                        "input",
                        "output",
                        "cacheRead",
                        "cacheWrite",
                    ] {
                        if !tier
                            .get(field)
                            .and_then(Value::as_f64)
                            .is_some_and(|number| number.is_finite() && number >= 0.0)
                        {
                            return Err(format!(
                                "{path}.cost.tiers[{index}].{field} must be a non-negative number"
                            ));
                        }
                    }
                }
            } else if !matches!(
                key.as_str(),
                "input" | "output" | "cacheRead" | "cacheWrite"
            ) || !value
                .as_f64()
                .is_some_and(|rate| rate.is_finite() && rate >= 0.0)
            {
                return Err(format!(
                    "{path}.cost.{key} is not a valid non-negative rate"
                ));
            }
        }
    }
    for key in ["contextWindow", "maxTokens"] {
        if object.get(key).is_some_and(|value| {
            !value
                .as_f64()
                .is_some_and(|number| number.is_finite() && number > 0.0)
        }) {
            return Err(format!("{path}.{key} must be greater than 0"));
        }
    }
    Ok(())
}

pub fn validate_provider_profile(
    name: &str,
    profile: &ProviderProfile,
) -> std::result::Result<(), String> {
    if name.trim().is_empty() {
        return Err("profile name must not be empty".into());
    }
    if let Some(oauth) = profile.oauth.as_deref() {
        if oauth != "radius" {
            return Err("oauth must be radius".into());
        }
    }
    if !profile.api.is_empty() && !SUPPORTED_APIS.contains(&profile.api.as_str()) {
        return Err(format!("api is not supported: {}", profile.api));
    }
    if !profile.base_url.is_empty()
        && !profile.base_url.starts_with("http://")
        && !profile.base_url.starts_with("https://")
    {
        return Err("baseUrl must start with http:// or https://".into());
    }
    if !profile.models.is_empty() && profile.base_url.is_empty() {
        return Err("baseUrl is required when models are defined".into());
    }
    if !profile.models.is_empty()
        && profile.api.is_empty()
        && profile.models.iter().any(|model| model.api.is_none())
    {
        return Err("api is required at provider or model level when models are defined".into());
    }
    if let Some(headers) = &profile.headers {
        validate_string_map(&Value::Object(headers.clone()), "headers")?;
    }
    if let Some(compat) = &profile.compat {
        validate_compat(compat, "compat")?;
    }
    if let Some(overrides) = &profile.model_overrides {
        for (model_id, value) in overrides {
            if model_id.is_empty() {
                return Err("modelOverrides keys must not be empty".into());
            }
            validate_model_override(value, &format!("modelOverrides.{model_id}"))?;
        }
    }

    let mut ids = std::collections::HashSet::new();
    for (index, model) in profile.models.iter().enumerate() {
        validate_model(model, &format!("models[{index}]"))?;
        if !ids.insert(model.id.as_str()) {
            return Err(format!("duplicate model id: {}", model.id));
        }
    }
    for exposed in &profile.exposed_models {
        if !ids.contains(exposed.as_str()) {
            return Err(format!("exposedModels references unknown model: {exposed}"));
        }
    }
    Ok(())
}

pub fn parse_provider_wrapper(
    input: &str,
) -> std::result::Result<(String, ProviderProfile), String> {
    let parsed: Value = serde_json::from_str(input).map_err(|error| {
        format!(
            "JSON syntax error at line {}, column {}: {}",
            error.line(),
            error.column(),
            error
        )
    })?;
    let object = parsed
        .as_object()
        .ok_or_else(|| "JSON must be an object".to_string())?;
    if object.len() != 1 {
        return Err("JSON must contain exactly one profile".into());
    }
    let (name, value) = object.iter().next().expect("checked one entry");
    let profile: ProviderProfile = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid profile structure: {error}"))?;
    validate_provider_profile(name, &profile)?;
    Ok((name.clone(), profile))
}

pub fn format_provider_wrapper(input: &str) -> std::result::Result<String, String> {
    let (name, profile) = parse_provider_wrapper(input)?;
    let mut wrapper = Map::new();
    wrapper.insert(
        name,
        serde_json::to_value(profile).map_err(|error| error.to_string())?,
    );
    serde_json::to_string_pretty(&Value::Object(wrapper)).map_err(|error| error.to_string())
}

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

        if let Err(message) = validate_provider_profile(name, &profile) {
            issues.push(ValidationIssue {
                level: "error".into(),
                path: format!("profiles.{}", name),
                message,
            });
            continue;
        }

        // Check API key
        if profile.api_key.is_empty() {
            issues.push(ValidationIssue {
                level: "warning".into(),
                path: format!("profiles.{}.apiKey", name),
                message: "API key is empty".into(),
            });
        }

        if profile.models.is_empty() {
            issues.push(ValidationIssue {
                level: "warning".into(),
                path: format!("profiles.{}.models", name),
                message: "No models defined".into(),
            });
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

#[cfg(test)]
mod tests {
    use super::{format_provider_wrapper, parse_provider_wrapper};

    #[test]
    fn parses_full_pi_provider_wrapper_without_losing_fields() {
        let input = r#"{
          "custom": {
            "name": "Custom",
            "baseUrl": "https://example.com/v1",
            "api": "openai-responses",
            "apiKey": "$CUSTOM_KEY",
            "authHeader": true,
            "headers": { "x-extra": "$EXTRA" },
            "models": [{
              "id": "model-a",
              "api": "openai-completions",
              "baseUrl": "https://model.example.com/v1",
              "reasoning": true,
              "thinkingLevelMap": { "off": null, "high": "high" },
              "input": ["text", "image"],
              "contextWindow": 200000,
              "maxTokens": 32000,
              "headers": { "x-model": "value" },
              "cost": {
                "input": 1,
                "output": 2,
                "cacheRead": 0.1,
                "cacheWrite": 0.2,
                "tiers": [{
                  "inputTokensAbove": 100000,
                  "input": 2,
                  "output": 4,
                  "cacheRead": 0.2,
                  "cacheWrite": 0.4
                }]
              },
              "compat": { "supportsDeveloperRole": false },
              "futureModelField": { "preserved": true }
            }],
            "modelOverrides": {
              "builtin-model": { "contextWindow": 1050000 }
            },
            "futureProviderField": ["preserved"]
          }
        }"#;

        let (name, profile) = parse_provider_wrapper(input).expect("valid provider wrapper");
        assert_eq!(name, "custom");
        assert_eq!(profile.api, "openai-responses");
        assert_eq!(profile.auth_header, Some(true));
        assert!(profile.model_overrides.is_some());
        assert!(profile.extra.contains_key("futureProviderField"));
        assert!(profile.models[0].extra.contains_key("futureModelField"));

        let formatted = format_provider_wrapper(input).expect("format provider wrapper");
        let (_, reparsed) =
            parse_provider_wrapper(&formatted).expect("formatted wrapper remains valid");
        assert!(reparsed.extra.contains_key("futureProviderField"));
        assert!(reparsed.models[0].extra.contains_key("futureModelField"));
    }

    #[test]
    fn rejects_multiple_profiles() {
        let error = parse_provider_wrapper(r#"{"one": {}, "two": {}}"#).unwrap_err();
        assert!(error.contains("exactly one profile"));
    }

    #[test]
    fn reports_json_line_and_column() {
        let error = parse_provider_wrapper("{\n  \"broken\": {,\n}").unwrap_err();
        assert!(error.contains("line"));
        assert!(error.contains("column"));
    }

    #[test]
    fn rejects_unknown_api_and_invalid_input_type() {
        let unknown_api = r#"{
          "custom": {
            "baseUrl": "https://example.com/v1",
            "api": "unknown-api",
            "models": [{ "id": "model-a" }]
          }
        }"#;
        assert!(parse_provider_wrapper(unknown_api)
            .unwrap_err()
            .contains("api is not supported"));

        let invalid_input = r#"{
          "custom": {
            "baseUrl": "https://example.com/v1",
            "api": "openai-completions",
            "models": [{ "id": "model-a", "input": ["audio"] }]
          }
        }"#;
        assert!(parse_provider_wrapper(invalid_input)
            .unwrap_err()
            .contains("text or image"));
    }

    #[test]
    fn rejects_partial_explicit_cost() {
        let input = r#"{
          "custom": {
            "baseUrl": "https://example.com/v1",
            "api": "openai-completions",
            "models": [{ "id": "model-a", "cost": { "input": 1 } }]
          }
        }"#;
        assert!(parse_provider_wrapper(input)
            .unwrap_err()
            .contains("invalid profile structure"));
    }
}
