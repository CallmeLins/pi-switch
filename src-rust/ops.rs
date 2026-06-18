use crate::config::{
    self, backup_config, load_config, provider_id_for, save_config, ProviderProfile,
};
use crate::error::{AppError, Result};
use std::path::PathBuf;

pub struct UseOutcome {
    pub name: String,
    pub provider_id: String,
    pub models_backup: Option<PathBuf>,
    pub config_backup: Option<PathBuf>,
}

#[allow(dead_code)]
fn normalize_models(profile: &mut serde_json::Value) {
    if let Some(models) = profile.get_mut("models").and_then(|v| v.as_array_mut()) {
        for m in models {
            if let Some(obj) = m.as_object_mut() {
                if obj.get("contextWindow").or(obj.get("context_window")).and_then(|v| v.as_u64()).unwrap_or(0) == 0 {
                    obj.insert("contextWindow".into(), serde_json::json!(1000000));
                }
                if obj.get("maxTokens").or(obj.get("max_tokens")).and_then(|v| v.as_u64()).unwrap_or(0) == 0 {
                    obj.insert("maxTokens".into(), serde_json::json!(128000));
                }
                if obj.get("input").and_then(|v| v.as_array()).map(|a| a.is_empty()).unwrap_or(true) {
                    obj.insert("input".into(), serde_json::json!(["text"]));
                }
            }
        }
    }
}

fn write_models_atomic(models: &serde_json::Value) -> Result<()> {
    let models_path = config::models_path();
    let tmp = config::config_dir().join("models.json.tmp");
    let json = serde_json::to_string_pretty(models).map_err(|e| AppError::json(&tmp, e))?;
    std::fs::write(&tmp, json + "\n").map_err(|e| AppError::io(&tmp, e))?;
    std::fs::rename(&tmp, &models_path).map_err(|e| AppError::io(&models_path, e))?;
    Ok(())
}

pub fn update_exposed_models(name: &str, model_ids: Vec<String>) -> Result<Option<PathBuf>> {
    let mut config = load_config()?;
    let backup = backup_config("config")?;

    let profile_value = config
        .profiles
        .get_mut(name)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", name)))?;

    let mut profile: ProviderProfile = serde_json::from_value(profile_value.clone())
        .map_err(|e| AppError::Message(format!("invalid profile: {}", e)))?;

    profile.exposed_models = model_ids;
    profile.updated_at = Some(chrono::Utc::now().to_rfc3339());

    *profile_value = serde_json::to_value(&profile)
        .map_err(|e| AppError::json(config::config_path(), e))?;

    save_config(&config)?;

    // Sync all profiles to pi config to keep models.json consistent
    sync_all_profiles_to_pi()?;

    Ok(backup)
}

pub fn update_provider_models(name: &str, models: Vec<config::ModelEntry>) -> Result<Option<PathBuf>> {
    let mut config = load_config()?;
    let backup = backup_config("config")?;

    let profile_value = config
        .profiles
        .get_mut(name)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", name)))?;

    let mut profile: ProviderProfile = serde_json::from_value(profile_value.clone())
        .map_err(|e| AppError::Message(format!("invalid profile: {}", e)))?;

    profile.models = models;
    profile.updated_at = Some(chrono::Utc::now().to_rfc3339());

    *profile_value = serde_json::to_value(&profile)
        .map_err(|e| AppError::json(config::config_path(), e))?;

    save_config(&config)?;

    Ok(backup)
}
fn backup_models() -> Option<PathBuf> {
    let models_path = config::models_path();
    if !models_path.exists() {
        return None;
    }
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S-%3fZ");
    let backup_path = config::backup_dir().join(format!("models-{}.json", ts));
    std::fs::create_dir_all(config::backup_dir()).ok();
    std::fs::copy(&models_path, &backup_path).ok()?;
    Some(backup_path)
}

pub fn use_profile(name: &str, mode: Option<&str>) -> Result<UseOutcome> {
    let mut config = load_config()?;

    let mode = mode
        .map(str::to_string)
        .unwrap_or_else(|| config.settings.write_mode.clone());
    let provider_id = provider_id_for(&config, name);

    let models_path = config::models_path();
    let models_backup = backup_models();

    // Handle exclusive mode
    if mode == "exclusive" {
        let mut models: serde_json::Value = if models_path.exists() {
            let text = std::fs::read_to_string(&models_path).unwrap_or_default();
            serde_json::from_str(&text).unwrap_or(serde_json::json!({ "providers": {} }))
        } else {
            serde_json::json!({ "providers": {} })
        };

        if let Some(providers) = models["providers"].as_object_mut() {
            let prefix = format!("{}-", config.settings.provider_prefix);
            providers.retain(|k, _| !k.starts_with(&prefix));
            write_models_atomic(&models)?;
        }
    }

    // Sync exposed models to pi config
    sync_exposed_models_to_pi(name)?;

    let config_backup = backup_config("config")?;

    config.current = Some(name.to_string());
    save_config(&config)?;

    Ok(UseOutcome {
        name: name.to_string(),
        provider_id,
        models_backup,
        config_backup,
    })
}

pub fn upsert_profile(
    name: &str,
    profile: &ProviderProfile,
    rename_from: Option<&str>,
) -> Result<Option<PathBuf>> {
    if name.is_empty() {
        return Err(AppError::InvalidInput("profile name required".into()));
    }

    let mut config = load_config()?;
    let backup = backup_config("config")?;

    if let Some(old) = rename_from {
        if old != name {
            config.profiles.remove(old);
            if config.current.as_deref() == Some(old) {
                config.current = Some(name.to_string());
            }
        }
    }

    config.profiles.insert(
        name.to_string(),
        serde_json::to_value(profile).map_err(|e| AppError::json(config::config_path(), e))?,
    );
    if config.current.is_none() {
        config.current = Some(name.to_string());
    }
    save_config(&config)?;

    Ok(backup)
}

pub fn remove_profile(name: &str) -> Result<Option<PathBuf>> {
    let mut config = load_config()?;
    if !config.profiles.contains_key(name) {
        return Err(AppError::Message(format!("unknown profile '{}'", name)));
    }

    let backup = backup_config("config")?;

    // Remove from models.json
    let provider_id = provider_id_for(&config, name);
    let models_path = config::models_path();
    if models_path.exists() {
        let mut models: serde_json::Value = {
            let text = std::fs::read_to_string(&models_path)
                .map_err(|e| AppError::io(&models_path, e))?;
            serde_json::from_str(&text).unwrap_or(serde_json::json!({ "providers": {} }))
        };

        if let Some(providers) = models["providers"].as_object_mut() {
            providers.remove(&provider_id);
            write_models_atomic(&models)?;
        }
    }

    config.profiles.remove(name);
    if config.current.as_deref() == Some(name) {
        config.current = config.profiles.keys().next().cloned();
    }
    save_config(&config)?;

    Ok(backup)
}

pub fn duplicate_profile(src: &str, dst: &str) -> Result<Option<PathBuf>> {
    let mut config = load_config()?;
    let profile = config
        .profiles
        .get(src)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", src)))?
        .clone();
    if config.profiles.contains_key(dst) {
        return Err(AppError::Message(format!("profile '{}' already exists", dst)));
    }

    let backup = backup_config("config")?;
    config.profiles.insert(dst.to_string(), profile);
    save_config(&config)?;

    Ok(backup)
}

// ─── Provider Testing ─────────────────────────────────────

#[derive(serde::Serialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub response_time_ms: Option<u64>,
}

pub async fn test_provider(name: &str) -> Result<TestResult> {
    let config = load_config()?;
    let profile_value = config
        .profiles
        .get(name)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", name)))?;

    let profile: ProviderProfile = serde_json::from_value(profile_value.clone())
        .map_err(|e| AppError::Message(format!("invalid profile: {}", e)))?;

    let start = std::time::Instant::now();

    // Build test request based on API type
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Message(format!("HTTP client error: {}", e)))?;

    let test_body = match profile.api.as_str() {
        "openai-completions" => serde_json::json!({
            "model": profile.models.first().map(|m| &m.id).unwrap_or(&"gpt-3.5-turbo".to_string()),
            "messages": [{"role": "user", "content": "test"}],
            "max_tokens": 5
        }),
        "anthropic-messages" => serde_json::json!({
            "model": profile.models.first().map(|m| &m.id).unwrap_or(&"claude-3-haiku-20240307".to_string()),
            "messages": [{"role": "user", "content": "test"}],
            "max_tokens": 5
        }),
        _ => {
            return Ok(TestResult {
                success: false,
                message: format!("Unsupported API type: {}", profile.api),
                response_time_ms: None,
            });
        }
    };

    let url = format!("{}/chat/completions", profile.base_url.trim_end_matches('/'));
    let mut req = client.post(&url).json(&test_body);

    // Add authorization header
    if profile.api == "anthropic-messages" {
        req = req.header("x-api-key", &profile.api_key)
            .header("anthropic-version", "2023-06-01");
    } else {
        req = req.header("Authorization", format!("Bearer {}", profile.api_key));
    }

    match req.send().await {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let status = resp.status();

            if status.is_success() {
                Ok(TestResult {
                    success: true,
                    message: format!("✓ Connected successfully (HTTP {})", status.as_u16()),
                    response_time_ms: Some(elapsed),
                })
            } else {
                let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".into());
                Ok(TestResult {
                    success: false,
                    message: format!("✗ HTTP {} - {}", status.as_u16(), error_text.chars().take(100).collect::<String>()),
                    response_time_ms: Some(elapsed),
                })
            }
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(TestResult {
                success: false,
                message: format!("✗ Connection failed: {}", e),
                response_time_ms: Some(elapsed),
            })
        }
    }
}

// ─── Fetch Models ─────────────────────────────────────────

pub async fn fetch_models(name: &str) -> Result<Vec<String>> {
    let config = load_config()?;
    let profile_value = config
        .profiles
        .get(name)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", name)))?;

    let profile: ProviderProfile = serde_json::from_value(profile_value.clone())
        .map_err(|e| AppError::Message(format!("invalid profile: {}", e)))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Message(format!("HTTP client error: {}", e)))?;

    let api_key = crate::config::resolve_env(&profile.api_key);

    // Build candidate URLs (try multiple common endpoints)
    let candidate_urls = build_model_fetch_urls(&profile.base_url, &profile.api);
    let mut last_error = String::from("No candidate URLs");

    for url in candidate_urls {
        let mut req = client.get(&url);

        // Set auth headers based on API type
        req = match profile.api.as_str() {
            "openai-completions" => req.header("Authorization", format!("Bearer {}", api_key)),
            "anthropic-messages" => req
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01"),
            _ => req.header("Authorization", format!("Bearer {}", api_key)),
        };

        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    last_error = format!("HTTP {} ({})", status.as_u16(), url);
                    // Skip 404/405 and try next URL
                    if status == reqwest::StatusCode::NOT_FOUND
                        || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
                    {
                        continue;
                    }
                    return Err(AppError::Message(last_error));
                }

                match resp.json::<serde_json::Value>().await {
                    Ok(payload) => {
                        let models = parse_model_ids(&payload);
                        if models.is_empty() {
                            last_error = format!("No models found in response ({})", url);
                            continue;
                        }
                        return Ok(models);
                    }
                    Err(e) => {
                        last_error = format!("Invalid JSON ({}): {}", url, e);
                    }
                }
            }
            Err(e) => {
                last_error = format!("Request failed ({}): {}", url, e);
            }
        }
    }

    Err(AppError::Message(last_error))
}


// ─── Sync Exposed Models to Pi Config ────────────────────

pub fn sync_exposed_models_to_pi(name: &str) -> Result<()> {
    let config = load_config()?;
    let profile_value = config
        .profiles
        .get(name)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", name)))?;

    let profile: ProviderProfile = serde_json::from_value(profile_value.clone())
        .map_err(|e| AppError::Message(format!("invalid profile: {}", e)))?;

    let provider_id = provider_id_for(&config, name);
    let models_path = config::models_path();

    // Load existing models.json
    let mut models: serde_json::Value = if models_path.exists() {
        let text = std::fs::read_to_string(&models_path)
            .map_err(|e| AppError::io(&models_path, e))?;
        serde_json::from_str(&text).unwrap_or(serde_json::json!({ "providers": {} }))
    } else {
        serde_json::json!({ "providers": {} })
    };

    let providers = models["providers"]
        .as_object_mut()
        .ok_or_else(|| AppError::Message("invalid models.json".into()))?;

    // Filter models to only exposed ones
    let exposed_models: Vec<serde_json::Value> = profile.models
        .into_iter()
        .filter(|m| profile.exposed_models.contains(&m.id))
        .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null))
        .collect();

    // Determine baseUrl: use proxy if enabled and target is set
    let base_url = if let Some(proxy_target) = config.settings.proxy.target.as_ref() {
        if proxy_target == name {
            // This is the proxy target, route through proxy server
            let host = &config.settings.proxy.host;
            let port = config.settings.proxy.port;
            format!("http://{}:{}/v1", host, port)
        } else {
            // Not the target, use original baseUrl
            profile.base_url.clone()
        }
    } else {
        // No proxy target configured, use original baseUrl
        profile.base_url.clone()
    };

    // Build provider entry
    let mut provider_entry = serde_json::json!({
        "api": profile.api,
        "baseUrl": base_url,
        "apiKey": profile.api_key,
        "models": exposed_models,
        "proxy": profile.proxy,
    });

    if let Some(preset) = profile.preset {
        provider_entry["preset"] = serde_json::json!(preset);
    }
    if let Some(headers) = profile.headers {
        provider_entry["headers"] = headers;
    }
    if let Some(auth_header) = profile.auth_header {
        provider_entry["authHeader"] = serde_json::json!(auth_header);
    }
    if let Some(compat) = profile.compat {
        provider_entry["compat"] = serde_json::json!(compat);
    }
    if let Some(updated_at) = profile.updated_at {
        provider_entry["updatedAt"] = serde_json::json!(updated_at);
    }

    providers.insert(provider_id, provider_entry);

    // Write atomically
    let tmp = config::config_dir().join("models.json.tmp");
    let json = serde_json::to_string_pretty(&models)
        .map_err(|e| AppError::json(&models_path, e))?;
    std::fs::write(&tmp, json + "\n")
        .map_err(|e| AppError::io(&tmp, e))?;
    std::fs::rename(&tmp, &models_path)
        .map_err(|e| AppError::io(&models_path, e))?;

    Ok(())
}

pub fn sync_all_profiles_to_pi() -> Result<()> {
    let config = load_config()?;
    let models_path = config::models_path();

    // Load existing models.json
    let mut models: serde_json::Value = if models_path.exists() {
        let text = std::fs::read_to_string(&models_path)
            .map_err(|e| AppError::io(&models_path, e))?;
        serde_json::from_str(&text).unwrap_or(serde_json::json!({ "providers": {} }))
    } else {
        serde_json::json!({ "providers": {} })
    };

    let providers = models["providers"]
        .as_object_mut()
        .ok_or_else(|| AppError::Message("invalid models.json".into()))?;

    // Sync all non-proxy profiles
    for (name, profile_value) in &config.profiles {
        let profile: ProviderProfile = match serde_json::from_value(profile_value.clone()) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if profile.proxy {
            continue;
        }

        let provider_id = provider_id_for(&config, name);

        // Filter models to only exposed ones
        let exposed_models: Vec<serde_json::Value> = profile.models
            .into_iter()
            .filter(|m| profile.exposed_models.contains(&m.id))
            .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null))
            .collect();

        // Determine baseUrl: use proxy if enabled and target is set
        let base_url = if let Some(proxy_target) = config.settings.proxy.target.as_ref() {
            if proxy_target == name {
                // This is the proxy target, route through proxy server
                let host = &config.settings.proxy.host;
                let port = config.settings.proxy.port;
                format!("http://{}:{}/v1", host, port)
            } else {
                // Not the target, use original baseUrl
                profile.base_url.clone()
            }
        } else {
            // No proxy target configured, use original baseUrl
            profile.base_url.clone()
        };

        // Build provider entry
        let mut provider_entry = serde_json::json!({
            "api": profile.api,
            "baseUrl": base_url,
            "apiKey": profile.api_key,
            "models": exposed_models,
            "proxy": profile.proxy,
        });

        if let Some(preset) = profile.preset {
            provider_entry["preset"] = serde_json::json!(preset);
        }
        if let Some(headers) = profile.headers {
            provider_entry["headers"] = headers;
        }
        if let Some(auth_header) = profile.auth_header {
            provider_entry["authHeader"] = serde_json::json!(auth_header);
        }
        if let Some(compat) = profile.compat {
            provider_entry["compat"] = serde_json::json!(compat);
        }
        if let Some(updated_at) = profile.updated_at {
            provider_entry["updatedAt"] = serde_json::json!(updated_at);
        }

        providers.insert(provider_id, provider_entry);
    }

    // Write atomically
    let tmp = config::config_dir().join("models.json.tmp");
    let json = serde_json::to_string_pretty(&models)
        .map_err(|e| AppError::json(&models_path, e))?;
    std::fs::write(&tmp, json + "\n")
        .map_err(|e| AppError::io(&tmp, e))?;
    std::fs::rename(&tmp, &models_path)
        .map_err(|e| AppError::io(&models_path, e))?;

    Ok(())
}

// Build multiple candidate URLs to try (following cc-switch logic)
pub fn build_model_fetch_urls(base_url: &str, api_type: &str) -> Vec<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Vec::new();
    }

    // If already ends with /models, use it directly
    if base.ends_with("/models") {
        return vec![base.to_string()];
    }

    let mut urls = Vec::new();
    let append_models = format!("{}/models", base);
    let has_version_suffix = base.ends_with("/v1") || base.ends_with("/v1beta");

    match api_type {
        "anthropic-messages" => {
            // Try /v1/models first for Anthropic-compatible endpoints
            if !has_version_suffix {
                urls.push(format!("{}/v1/models", base));
            } else {
                urls.push(append_models.clone());
            }

            // Try stripping known compatibility suffixes
            if let Some(stripped) = strip_compat_suffix(base) {
                let root = stripped.trim_end_matches('/');
                if !root.is_empty() && root.contains("://") {
                    urls.push(format!("{}/v1/models", root));
                    urls.push(format!("{}/models", root));
                }
            } else if !has_version_suffix {
                urls.push(append_models);
            }
        }
        _ => {
            // OpenAI and others: try /models, then /v1/models
            urls.push(append_models);
            if !has_version_suffix {
                urls.push(format!("{}/v1/models", base));
            }
        }
    }

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    urls.retain(|url| seen.insert(url.clone()));
    urls
}

// Strip known compatibility path suffixes (e.g., /api/anthropic, /claudecode)
fn strip_compat_suffix(base: &str) -> Option<&str> {
    const KNOWN_SUFFIXES: &[&str] = &[
        "/api/claudecode",
        "/api/anthropic",
        "/apps/anthropic",
        "/api/coding",
        "/claudecode",
        "/anthropic",
        "/step_plan",
        "/coding",
        "/claude",
    ];

    let lower = base.to_ascii_lowercase();
    KNOWN_SUFFIXES.iter().find_map(|suffix| {
        lower
            .ends_with(suffix)
            .then(|| &base[..base.len() - suffix.len()])
    })
}

// Parse model IDs from various response formats
pub fn parse_model_ids(payload: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // Try OpenAI format: { "data": [{"id": "..."}, ...] }
    if let Some(data) = payload.get("data").and_then(|v| v.as_array()) {
        for item in data {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                out.push(id.to_string());
            }
        }
    }

    // Try Google format: { "models": [{"name": "models/..."}, ...] }
    if out.is_empty() {
        if let Some(models) = payload.get("models").and_then(|v| v.as_array()) {
            for item in models {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    out.push(name.strip_prefix("models/").unwrap_or(name).to_string());
                }
            }
        }
    }

    // Try direct array: [{"id": "..."}, ...]
    if out.is_empty() {
        if let Some(arr) = payload.as_array() {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    out.push(id.to_string());
                }
            }
        }
    }

    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    out.retain(|model| seen.insert(model.clone()));
    out
}

pub fn set_proxy_target(target: Option<&str>) -> Result<()> {
    let mut config = load_config()?;

    if let Some(name) = target {
        if !config.profiles.contains_key(name) {
            return Err(AppError::Message(format!("Profile '{}' not found", name)));
        }
        config.settings.proxy.target = Some(name.to_string());
    } else {
        config.settings.proxy.target = None;
    }

    save_config(&config)?;
    sync_all_profiles_to_pi()?;
    Ok(())
}

