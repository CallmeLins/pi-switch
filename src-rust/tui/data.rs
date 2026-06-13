use crate::config::{load_config, PiSwitchConfig};
use crate::daemon::{daemon_status, DaemonResult};
use crate::presets::{all_presets, Preset};
use crate::stats::{get_stats, UsageStats};

pub struct ProfileRow {
    pub name: String,
    pub api: String,
    pub base_url: String,
    pub models: Vec<String>,
    pub provider_id: String,
    pub proxy: bool,
    pub is_current: bool,
}

pub struct UiData {
    pub config: PiSwitchConfig,
    pub profiles: Vec<ProfileRow>,
    pub presets: Vec<Preset>,
    pub daemon: DaemonResult,
    pub stats: UsageStats,
    pub backups: Vec<String>,
}

fn offline_daemon(message: String) -> DaemonResult {
    DaemonResult {
        running: false,
        pid: None,
        host: None,
        port: None,
        target: None,
        failover: None,
        started_at: None,
        message,
    }
}

fn list_backup_files() -> Vec<String> {
    let dir = crate::config::backup_dir();
    if !dir.exists() {
        return vec![];
    }
    let mut entries: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    entries.reverse();
    entries
}

fn profile_rows(config: &PiSwitchConfig) -> Vec<ProfileRow> {
    config
        .profiles
        .iter()
        .map(|(name, profile)| {
            let provider_id = crate::config::provider_id_for(config, name);
            let proxy = profile
                .get("proxy")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            ProfileRow {
                name: name.clone(),
                api: profile
                    .get("api")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                base_url: profile
                    .get("baseUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                models: profile
                    .get("models")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|m| m.get("id").and_then(|v| v.as_str()))
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                provider_id,
                proxy,
                is_current: config.current.as_deref() == Some(name.as_str()),
            }
        })
        .collect()
}

impl UiData {
    pub fn load() -> Self {
        let config = load_config().unwrap_or_default();
        let profiles = profile_rows(&config);
        Self {
            config,
            profiles,
            presets: all_presets(),
            daemon: daemon_status().unwrap_or_else(offline_daemon),
            stats: get_stats(),
            backups: list_backup_files(),
        }
    }

    pub fn refresh(&mut self) {
        *self = Self::load();
    }
}
