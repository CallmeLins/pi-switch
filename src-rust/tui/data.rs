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
    pub exposed_count: usize,
    pub in_failover_chain: bool,
    pub failover_priority: Option<usize>, // 0=target, 1=p1, 2=p2, ...
    pub circuit_breaker_open: bool,
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
        targets: None,
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

fn profile_rows(config: &PiSwitchConfig, stats: &UsageStats) -> Vec<ProfileRow> {
    // Build failover priority map
    let target = config.settings.proxy.target.as_ref();
    let failover_chain = &config.settings.proxy.failover;

    let mut priority_map = std::collections::HashMap::new();
    if let Some(t) = target {
        priority_map.insert(t.clone(), 0);
    }
    for (idx, name) in failover_chain.iter().enumerate() {
        priority_map.insert(name.clone(), idx + 1);
    }

    config
        .profiles
        .iter()
        .map(|(name, profile)| {
            let provider_id = crate::config::provider_id_for(config, name);
            let proxy = profile
                .get("proxy")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let priority = priority_map.get(name).copied();
            let in_failover_chain = priority.is_some();

            // Check circuit breaker status
            let cb_status = stats.circuit_breaker.get(name);
            let circuit_breaker_open = cb_status
                .map(|s| s.state == "open" || s.state == "half_open")
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
                exposed_count: profile
                    .get("exposedModels")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0),
                in_failover_chain,
                failover_priority: priority,
                circuit_breaker_open,
            }
        })
        .collect()
}

impl UiData {
    pub fn load() -> Self {
        let config = load_config().unwrap_or_default();
        let stats = get_stats();
        let profiles = profile_rows(&config, &stats);
        Self {
            config,
            profiles,
            presets: all_presets(),
            daemon: daemon_status().unwrap_or_else(offline_daemon),
            stats,
            backups: list_backup_files(),
        }
    }

    pub fn refresh(&mut self) {
        *self = Self::load();
    }
}
