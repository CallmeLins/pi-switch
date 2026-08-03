//! cc-switch provider import.
//!
//! Reads `~/.cc-switch/cc-switch.db` (SQLite, read-only) and maps the providers
//! that cc-switch manages for Claude Code / Codex / Gemini into pi-switch
//! provider profiles. Only the three common client types are mapped; the
//! official presets (`category = 'official'`) are skipped. Dedup is by base
//! URL: a cc-switch provider whose `base_url` already exists in pi-switch is
//! reported as `exists` and skipped on import unless forced.

use crate::config::{load_config, ModelEntry, ProviderProfile};
use crate::error::{AppError, Result};
use crate::ops;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;

/// Default location of the cc-switch database.
pub fn default_db_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".cc-switch").join("cc-switch.db"))
        .unwrap_or_else(|| PathBuf::from(".cc-switch/cc-switch.db"))
}

/// A provider discovered in the cc-switch database, mapped to pi-switch terms.
#[derive(Debug, Clone, Serialize)]
pub struct CcsProvider {
    pub id: String,
    pub name: String,
    /// cc-switch client type: claude | codex | gemini
    #[serde(rename = "appType")]
    pub app_type: String,
    /// pi-switch api id (anthropic-messages | openai-responses | google-generative-ai)
    pub api: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub models: Vec<String>,
    /// true when a pi-switch profile already uses the same base URL
    pub exists: bool,
}

/// One selection for import (by cc-switch provider id).
#[derive(Debug, Clone)]
pub struct CcsImportSelection {
    pub id: String,
    pub force: bool,
}

/// Result of importing one provider.
#[derive(Debug, Clone, Serialize)]
pub struct CcsImportResult {
    pub name: String,
    pub imported: bool,
    pub message: String,
}

/// List importable cc-switch providers. `path` defaults to `~/.cc-switch/cc-switch.db`.
pub fn list_ccswitch_providers(path: Option<&str>) -> Result<Vec<CcsProvider>> {
    let path = path.map(PathBuf::from).unwrap_or_else(default_db_path);
    if !path.exists() {
        return Err(AppError::Message(format!(
            "cc-switch database not found at {}",
            path.display()
        )));
    }

    let conn = Connection::open(&path)
        .map_err(|e| AppError::Message(format!("Failed to open cc-switch db: {}", e)))?;

    let mut stmt = conn
        .prepare("SELECT id, app_type, name, settings_config FROM providers")
        .map_err(|e| AppError::Message(format!("Failed to read cc-switch db: {}", e)))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| AppError::Message(format!("Failed to query cc-switch db: {}", e)))?;

    // Existing base URLs in pi-switch config (dedup key).
    let existing_urls: std::collections::HashSet<String> = load_config()
        .map(|c| {
            c.profiles
                .values()
                .filter_map(|p| p.get("baseUrl").and_then(|v| v.as_str()).map(normalize_url))
                .collect()
        })
        .unwrap_or_default();

    let mut providers = Vec::new();
    for row in rows {
        let (id, app_type, name, settings_config) =
            row.map_err(|e| AppError::Message(format!("Failed to read row: {}", e)))?;

        // Skip cc-switch's official presets (claude-official etc.).
        let is_official =
            matches!(app_type.as_str(), "claude" | "codex" | "gemini") && id.contains("official");
        if is_official {
            continue;
        }

        if let Some(p) = parse_cc_provider(&id, &app_type, &name, &settings_config, &existing_urls)
        {
            providers.push(p);
        }
    }

    Ok(providers)
}

/// Import selected providers. Skipped (unless `force`) when the base URL
/// already exists in pi-switch. Returns per-selection results.
pub fn import_ccswitch_providers(
    selections: &[CcsImportSelection],
    path: Option<&str>,
) -> Result<Vec<CcsImportResult>> {
    let providers = list_ccswitch_providers(path)?;
    let mut results = Vec::new();

    for sel in selections {
        let Some(p) = providers.iter().find(|p| p.id == sel.id) else {
            results.push(CcsImportResult {
                name: sel.id.clone(),
                imported: false,
                message: "provider not found in cc-switch".to_string(),
            });
            continue;
        };

        if p.exists && !sel.force {
            results.push(CcsImportResult {
                name: p.name.clone(),
                imported: false,
                message: "already exists in pi-switch (same base URL)".to_string(),
            });
            continue;
        }

        // Resolve a unique profile name: keep the cc-switch name unless a
        // pi-switch profile with the same name points elsewhere.
        let name = unique_profile_name(&p.name);

        let models = p
            .models
            .iter()
            .map(|id| ModelEntry {
                id: id.clone(),
                ..Default::default()
            })
            .collect::<Vec<_>>();

        let profile = ProviderProfile {
            api: p.api.clone(),
            base_url: p.base_url.clone(),
            api_key: p.api_key.clone(),
            models,
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        };

        match ops::upsert_profile(&name, &profile, None) {
            Ok(_) => results.push(CcsImportResult {
                name: name.clone(),
                imported: true,
                message: "imported".to_string(),
            }),
            Err(e) => results.push(CcsImportResult {
                name: name.clone(),
                imported: false,
                message: format!("import failed: {}", e),
            }),
        }
    }

    Ok(results)
}

/// Map one cc-switch provider row to a pi-switch representation.
fn parse_cc_provider(
    id: &str,
    app_type: &str,
    name: &str,
    settings_config: &str,
    existing_urls: &std::collections::HashSet<String>,
) -> Option<CcsProvider> {
    let v: serde_json::Value = serde_json::from_str(settings_config).ok()?;

    let (api, base_url, api_key, models) = match app_type {
        "claude" => {
            let env = v.get("env")?;
            let base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|x| x.as_str())?
                .to_string();
            let api_key = env
                .get("ANTHROPIC_AUTH_TOKEN")
                .and_then(|x| x.as_str())?
                .to_string();
            let mut models = Vec::new();
            for key in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            ] {
                if let Some(m) = env.get(key).and_then(|x| x.as_str()) {
                    if !models.contains(&m.to_string()) {
                        models.push(m.to_string());
                    }
                }
            }
            if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                if !models.contains(&m.to_string()) {
                    models.push(m.to_string());
                }
            }
            ("anthropic-messages".to_string(), base_url, api_key, models)
        }
        "codex" => {
            let auth = v.get("auth")?;
            let api_key = auth
                .get("OPENAI_API_KEY")
                .and_then(|x| x.as_str())?
                .to_string();
            let config_str = v.get("config").and_then(|x| x.as_str())?;
            let toml_val: toml::Value = toml::from_str(config_str).ok()?;
            let base_url = toml_val
                .get("model_providers")?
                .get("custom")?
                .get("base_url")?
                .as_str()?
                .to_string();
            let mut models = Vec::new();
            if let Some(catalog) = v
                .get("modelCatalog")
                .and_then(|x| x.get("models"))
                .and_then(|x| x.as_array())
            {
                for m in catalog {
                    if let Some(id) = m.get("model").and_then(|x| x.as_str()) {
                        if !models.contains(&id.to_string()) {
                            models.push(id.to_string());
                        }
                    }
                }
            }
            ("openai-responses".to_string(), base_url, api_key, models)
        }
        "gemini" => {
            let env = v.get("env")?;
            let base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|x| x.as_str())?
                .to_string();
            let api_key = env
                .get("GEMINI_API_KEY")
                .and_then(|x| x.as_str())?
                .to_string();
            let mut models = Vec::new();
            if let Some(m) = env.get("GEMINI_MODEL").and_then(|x| x.as_str()) {
                models.push(m.to_string());
            }
            (
                "google-generative-ai".to_string(),
                base_url,
                api_key,
                models,
            )
        }
        _ => return None,
    };

    if base_url.is_empty() || api_key.is_empty() {
        return None;
    }

    let exists = existing_urls.contains(&normalize_url(&base_url));
    Some(CcsProvider {
        id: id.to_string(),
        name: name.to_string(),
        app_type: app_type.to_string(),
        api,
        base_url,
        api_key,
        models,
        exists,
    })
}

/// Trim trailing slashes so `https://x/v1` and `https://x/v1/` dedup together.
fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// Pick a profile name that won't silently overwrite an existing pi-switch
/// profile with a different base URL: `name` if free, else `name (cc)`.
fn unique_profile_name(name: &str) -> String {
    let config = load_config().unwrap_or_default();
    if !config.profiles.contains_key(name) {
        return name.to_string();
    }
    // Same name + same URL → treated as existing; caller already skipped via exists.
    // Same name + different URL → suffix to avoid overwrite.
    let mut candidate = format!("{} (cc)", name);
    let mut n = 2;
    while config.profiles.contains_key(&candidate) {
        candidate = format!("{} (cc{})", name, n);
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn urls(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| normalize_url(s)).collect()
    }

    #[test]
    fn parse_claude_provider() {
        let config = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-test","ANTHROPIC_BASE_URL":"https://api.deepseek.com/anthropic","ANTHROPIC_MODEL":"deepseek-v4-pro","ANTHROPIC_DEFAULT_SONNET_MODEL":"deepseek-chat"},"model":"deepseek-v4-pro"}"#;
        let p = parse_cc_provider("id-1", "claude", "DeepSeek", config, &urls(&[])).unwrap();
        assert_eq!(p.api, "anthropic-messages");
        assert_eq!(p.base_url, "https://api.deepseek.com/anthropic");
        assert_eq!(p.api_key, "sk-test");
        assert_eq!(p.models, vec!["deepseek-v4-pro", "deepseek-chat"]);
        assert!(!p.exists);
    }

    #[test]
    fn parse_codex_provider() {
        let config = r#"{"auth":{"OPENAI_API_KEY":"sk-codex"},"config":"model_provider = \"custom\"\nmodel = \"gpt-5.5\"\n\n[model_providers.custom]\nname = \"custom\"\nwire_api = \"responses\"\nrequires_openai_auth = true\nbase_url = \"https://muyuan.do/v1\"\n","modelCatalog":{"models":[{"model":"gpt-5.5"},{"model":"gpt-5.4"}]}}"#;
        let p = parse_cc_provider("id-2", "codex", "Wong", config, &urls(&[])).unwrap();
        assert_eq!(p.api, "openai-responses");
        assert_eq!(p.base_url, "https://muyuan.do/v1");
        assert_eq!(p.api_key, "sk-codex");
        assert_eq!(p.models, vec!["gpt-5.5", "gpt-5.4"]);
    }

    #[test]
    fn parse_gemini_provider() {
        let config = r#"{"env":{"GEMINI_API_KEY":"sk-gem","GOOGLE_GEMINI_BASE_URL":"https://wzw.pp.ua","GEMINI_MODEL":"gemini-3-pro"}}"#;
        let p = parse_cc_provider("id-3", "gemini", "Wong", config, &urls(&[])).unwrap();
        assert_eq!(p.api, "google-generative-ai");
        assert_eq!(p.base_url, "https://wzw.pp.ua");
        assert_eq!(p.models, vec!["gemini-3-pro"]);
    }

    #[test]
    fn marks_exists_by_base_url() {
        let config = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-test","ANTHROPIC_BASE_URL":"https://x.example/v1"}}"#;
        let p = parse_cc_provider(
            "id-1",
            "claude",
            "X",
            config,
            &urls(&["https://x.example/v1/"]),
        )
        .unwrap();
        assert!(p.exists, "trailing slash should dedup to same URL");
    }

    #[test]
    fn skips_unknown_app_type() {
        let config = r#"{"env":{}}"#;
        assert!(parse_cc_provider("id", "hermes", "X", config, &urls(&[])).is_none());
    }

    #[test]
    fn skips_empty_key_or_url() {
        let config = r#"{"env":{"ANTHROPIC_BASE_URL":"https://x.example"}}"#;
        assert!(parse_cc_provider("id", "claude", "X", config, &urls(&[])).is_none());
    }
}
