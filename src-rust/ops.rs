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
    let profile = config
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
