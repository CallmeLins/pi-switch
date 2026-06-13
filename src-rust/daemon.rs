use crate::config::{config_dir, load_config};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command};

fn pid_path() -> PathBuf { config_dir().join("proxy.pid") }

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub pid: u32,
    pub host: String,
    pub port: u16,
    #[serde(rename = "startedAt")]
    pub started_at: u64,
}

#[derive(Debug, Serialize)]
pub struct DaemonResult {
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failover: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "startedAt")]
    pub started_at: Option<u64>,
    pub message: String,
}

fn is_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn read_pid_file() -> Option<DaemonInfo> {
    let path = pid_path();
    if !path.exists() { return None; }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn write_pid_file(info: &DaemonInfo) {
    if let Some(parent) = pid_path().parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string(info) {
        std::fs::write(pid_path(), json).ok();
    }
}

fn remove_pid_file() {
    std::fs::remove_file(pid_path()).ok();
}

pub fn daemon_start(host: Option<String>, port: Option<u16>) -> Result<DaemonResult, String> {
    // Check if already running
    if let Some(info) = read_pid_file() {
        if is_alive(info.pid) {
            let msg = format!("Proxy daemon already running (PID {}) on http://{}:{}", info.pid, info.host, info.port);
            return Ok(DaemonResult {
                running: true,
                pid: Some(info.pid),
                host: Some(info.host),
                port: Some(info.port),
                target: None, failover: None, started_at: None,
                message: msg,
            });
        }
        remove_pid_file();
    }

    let config = load_config().map_err(|e| e.to_string())?;
    let host = host.unwrap_or_else(|| config.settings.proxy.host.clone());
    let port = port.unwrap_or(config.settings.proxy.port);

    // Spawn detached child process
    let child: Child = Command::new(std::env::current_exe().unwrap_or_else(|_| "pi-switch".into()))
        .arg("proxy")
        .arg("start")
        .arg("--host").arg(&host)
        .arg("--port").arg(port.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn daemon: {}", e))?;

    let pid = child.id();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let info = DaemonInfo {
        pid,
        host: host.clone(),
        port,
        started_at: now_ms,
    };

    write_pid_file(&info);

    Ok(DaemonResult {
        running: true,
        pid: Some(pid),
        host: Some(host.clone()),
        port: Some(port),
        target: None,
        failover: None,
        started_at: Some(now_ms),
        message: format!("Proxy daemon started (PID {}) on http://{}:{}", pid, host, port),
    })
}

pub fn daemon_stop() -> Result<DaemonResult, String> {
    let info = match read_pid_file() {
        Some(i) => i,
        None => return Ok(DaemonResult {
            running: false, pid: None, host: None, port: None, target: None, failover: None, started_at: None,
            message: "No proxy daemon PID file found".into(),
        }),
    };

    if !is_alive(info.pid) {
        remove_pid_file();
        return Ok(DaemonResult {
            running: false, pid: Some(info.pid), host: None, port: None, target: None, failover: None, started_at: None,
            message: format!("PID {} is not alive (cleaned up stale PID)", info.pid),
        });
    }

    // Send SIGTERM
    unsafe { libc::kill(info.pid as i32, libc::SIGTERM); }

    // Wait up to 5 seconds
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if !is_alive(info.pid) {
            remove_pid_file();
            return Ok(DaemonResult {
                running: false, pid: Some(info.pid), host: None, port: None, target: None, failover: None, started_at: None,
                message: format!("Proxy daemon (PID {}) stopped", info.pid),
            });
        }
    }

    // Force kill
    unsafe { libc::kill(info.pid as i32, libc::SIGKILL); }
    remove_pid_file();
    Ok(DaemonResult {
        running: false, pid: Some(info.pid), host: None, port: None, target: None, failover: None, started_at: None,
        message: format!("Proxy daemon (PID {}) force killed", info.pid),
    })
}

pub fn daemon_status() -> Result<DaemonResult, String> {
    let info = match read_pid_file() {
        Some(i) => i,
        None => return Ok(DaemonResult {
            running: false, pid: None, host: None, port: None, target: None, failover: None, started_at: None,
            message: "Proxy daemon is not running (no PID file)".into(),
        }),
    };

    if !is_alive(info.pid) {
        remove_pid_file();
        return Ok(DaemonResult {
            running: false, pid: Some(info.pid), host: None, port: None, target: None, failover: None, started_at: None,
            message: format!("Proxy daemon is not running (PID {} is dead)", info.pid),
        });
    }

    let host = info.host.clone();
    let port = info.port;
    let config = load_config().map_err(|e| e.to_string())?;

    Ok(DaemonResult {
        running: true,
        pid: Some(info.pid),
        host: Some(host.clone()),
        port: Some(port),
        target: config.settings.proxy.target.clone(),
        failover: Some(config.settings.proxy.failover.clone()),
        started_at: Some(info.started_at),
        message: format!("Proxy daemon running (PID {}) on http://{}:{}", info.pid, host, port),
    })
}
