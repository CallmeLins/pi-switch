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

fn normalize_models(profile: &mut serde_json::Value) {
    if let Some(models) = profile.get_mut("models").and_then(|v| v.as_array_mut()) {
        for m in models {
            if let Some(obj) = m.as_object_mut() {
                if obj.get("contextWindow").or(obj.get("context_window")).and_then(|v| v.as_u64()).unwrap_or(0) == 0 {
                    obj.insert("contextWindow".into(), serde_json::json!(128000));
                }
                if obj.get("maxTokens").or(obj.get("max_tokens")).and_then(|v| v.as_u64()).unwrap_or(0) == 0 {
                    obj.insert("maxTokens".into(), serde_json::json!(16384));
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
    let mut profile = config
        .profiles
        .get(name)
        .ok_or_else(|| AppError::Message(format!("unknown profile '{}'", name)))?
        .clone();

    let mode = mode
        .map(str::to_string)
        .unwrap_or_else(|| config.settings.write_mode.clone());
    let provider_id = provider_id_for(&config, name);

    let models_path = config::models_path();
    let mut models: serde_json::Value = if models_path.exists() {
        let text = std::fs::read_to_string(&models_path).unwrap_or_default();
        serde_json::from_str(&text).unwrap_or(serde_json::json!({ "providers": {} }))
    } else {
        serde_json::json!({ "providers": {} })
    };

    let models_backup = backup_models();

    let providers = models["providers"]
        .as_object_mut()
        .ok_or_else(|| AppError::Message("invalid models.json".into()))?;

    if mode == "exclusive" {
        let prefix = format!("{}-", config.settings.provider_prefix);
        providers.retain(|k, _| !k.starts_with(&prefix));
    }

    normalize_models(&mut profile);
    providers.insert(provider_id.clone(), profile);

    write_models_atomic(&models)?;

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

#[derive(serde::Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(serde::Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

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

    match profile.api.as_str() {
        "openai-completions" => {
            let url = format!("{}/models", profile.base_url.trim_end_matches('/'));
            let resp = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", profile.api_key))
                .send()
                .await
                .map_err(|e| AppError::Message(format!("Request failed: {}", e)))?;

            if !resp.status().is_success() {
                return Err(AppError::Message(format!(
                    "API returned HTTP {}",
                    resp.status().as_u16()
                )));
            }

            let models_resp: OpenAIModelsResponse = resp
                .json()
                .await
                .map_err(|e| AppError::Message(format!("Failed to parse response: {}", e)))?;

            Ok(models_resp.data.into_iter().map(|m| m.id).collect())
        }
        "anthropic-messages" => {
            // Anthropic doesn't provide a models endpoint, return hardcoded list
            Ok(vec![
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-5-haiku-20241022".to_string(),
                "claude-3-opus-20240229".to_string(),
                "claude-3-sonnet-20240229".to_string(),
                "claude-3-haiku-20240307".to_string(),
            ])
        }
        _ => Err(AppError::Message(format!(
            "Unsupported API type: {}. Only openai-completions and anthropic-messages are supported.",
            profile.api
        ))),
    }
}
