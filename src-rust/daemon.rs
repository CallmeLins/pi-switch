use crate::config::{config_dir, load_config};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::net::TcpStream;
use std::time::Duration;

// ─── Service descriptor ───────────────────────────────────
//
// A daemon-managed service. Both the proxy and the web UI run as background
// `node bin/pi-switch.js <subcommand> start` processes with their own pid/log
// files, so the same start/stop/status machinery drives both.

#[derive(Debug, Clone, Copy)]
pub struct Service {
    /// pid file name under ~/.pi-switch/
    pub pid_file: &'static str,
    /// log file name under ~/.pi-switch/
    pub log_file: &'static str,
    /// the `pi-switch <subcommand> start` CLI subcommand to spawn
    pub subcommand: &'static str,
    /// human label used in status/result messages
    pub label: &'static str,
}

pub const PROXY: Service = Service {
    pid_file: "proxy.pid",
    log_file: "proxy.log",
    subcommand: "proxy",
    label: "Proxy",
};

pub const WEBUI: Service = Service {
    pid_file: "webui.pid",
    log_file: "webui.log",
    subcommand: "webui",
    label: "WebUI",
};

/// Resolve a service by its subcommand name (used by the napi boundary).
pub fn service_by_name(name: &str) -> Option<Service> {
    match name {
        "proxy" => Some(PROXY),
        "webui" => Some(WEBUI),
        _ => None,
    }
}

/// Fallback host/port for a service, read from the matching config section.
fn service_defaults(service: &Service) -> (String, u16) {
    let cfg = load_config().ok();
    match service.subcommand {
        "webui" => cfg
            .map(|c| (c.settings.web.host, c.settings.web.port))
            .unwrap_or_else(|| ("127.0.0.1".into(), 43110)),
        _ => cfg
            .map(|c| (c.settings.proxy.host, c.settings.proxy.port))
            .unwrap_or_else(|| ("127.0.0.1".into(), 43112)),
    }
}

fn pid_path(service: &Service) -> PathBuf { config_dir().join(service.pid_file) }

// Check if proxy server is actually listening on the port
fn check_health(host: &str, port: u16, max_attempts: u32) -> bool {
    for _ in 0..max_attempts {
        if let Ok(addr) = format!("{}:{}", host, port).parse() {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

// Get pi-switch.js path. The project_dir (passed from JS via import.meta.url)
// is the canonical source on macOS / Linux where current_exe() is `node`.
fn get_bin_path(project_dir: Option<&str>) -> PathBuf {
    // 1. Prefer the project dir passed from JS (always correct: index.js's parent)
    if let Some(dir) = project_dir {
        let p = PathBuf::from(dir).join("bin").join("pi-switch.js");
        if p.exists() { return p; }
    }
    // 2. Try executable-relative (works on Windows, or when the binary is a real file)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let bin_path = exe_dir.join("bin").join("pi-switch.js");
            if bin_path.exists() { return bin_path; }
            if let Some(parent) = exe_dir.parent() {
                let bin_path = parent.join("bin").join("pi-switch.js");
                if bin_path.exists() { return bin_path; }
            }
        }
    }
    // 3. Fallback: relative to CWD (dev convenience)
    PathBuf::from("bin").join("pi-switch.js")
}

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
    pub targets: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failover: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "startedAt")]
    pub started_at: Option<u64>,
    pub message: String,
}

// ─── Platform-specific process helpers ────────────────────

#[cfg(unix)]
fn is_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn is_alive(pid: u32) -> bool {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // CSV format: "Image Name","PID","Session Name","Session#","Mem Usage"
            // Parse second column (PID) for exact match
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 2 {
                    if let Some(pid_str) = parts[1].strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                        if pid_str.trim() == pid.to_string() {
                            return true;
                        }
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}

#[cfg(unix)]
fn kill_process(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, libc::SIGTERM) == 0 }
}

#[cfg(unix)]
fn force_kill(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, libc::SIGKILL) == 0 }
}

#[cfg(windows)]
fn kill_process(pid: u32) -> bool {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn force_kill(pid: u32) -> bool {
    Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─── PID file I/O ─────────────────────────────────────────

fn read_pid_file(service: &Service) -> Option<DaemonInfo> {
    let path = pid_path(service);
    if !path.exists() { return None; }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn write_pid_file(service: &Service, info: &DaemonInfo) {
    let path = pid_path(service);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string(info) {
        std::fs::write(&path, json).ok();
    }
}

fn remove_pid_file(service: &Service) {
    std::fs::remove_file(pid_path(service)).ok();
}

// ─── Daemon start ─────────────────────────────────────────

pub fn daemon_start(service: &Service, host: Option<String>, port: Option<u16>, project_dir: Option<String>) -> Result<DaemonResult, String> {
    if let Some(info) = read_pid_file(service) {
        if is_alive(info.pid) && check_health(&info.host, info.port, 2) {
            let msg = format!(
                "{} daemon already running (PID {}) on http://{}:{}",
                service.label, info.pid, info.host, info.port
            );
            return Ok(DaemonResult {
                running: true,
                pid: Some(info.pid),
                host: Some(info.host),
                port: Some(info.port),
                targets: None,
                failover: None,
                started_at: None,
                message: msg,
            });
        }
        remove_pid_file(service);
    }

    let (default_host, default_port) = service_defaults(service);
    let host = host.unwrap_or(default_host);
    let port = port.unwrap_or(default_port);

    let bin_path = get_bin_path(project_dir.as_deref());
    if !bin_path.exists() {
        return Err(format!("pi-switch.js not found at {:?}", bin_path));
    }

    let log_path = config_dir().join(service.log_file);
    // Ensure config dir exists before opening log (fresh install has no ~/.pi-switch/)
    std::fs::create_dir_all(config_dir())
        .map_err(|e| format!("Failed to create config dir: {}", e))?;
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    #[cfg(windows)]
    let child = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("node")
            .arg(&bin_path)
            .arg(service.subcommand)
            .arg("start")
            .arg("--host")
            .arg(&host)
            .arg("--port")
            .arg(port.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(log_file.try_clone().unwrap())
            .stderr(log_file)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to spawn daemon: {}", e))?
    };

    #[cfg(not(windows))]
    let child = {
        Command::new("node")
            .arg(&bin_path)
            .arg(service.subcommand)
            .arg("start")
            .arg("--host")
            .arg(&host)
            .arg("--port")
            .arg(port.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(log_file.try_clone().unwrap())
            .stderr(log_file)
            .spawn()
            .map_err(|e| format!("Failed to spawn daemon: {}", e))?
    };

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

    write_pid_file(service, &info);

    // Wait for server to be ready (max 5 seconds)
    if !check_health(&host, port, 25) {
        // Cleanup on failure
        remove_pid_file(service);
        #[cfg(windows)]
        { force_kill(pid); }
        #[cfg(not(windows))]
        unsafe { libc::kill(pid as i32, libc::SIGKILL); }

        return Err(format!(
            "{} daemon started but failed health check on http://{}:{}. Check ~/.pi-switch/{} for errors.",
            service.label, host, port, service.log_file
        ));
    }

    Ok(DaemonResult {
        running: true,
        pid: Some(pid),
        host: Some(host.clone()),
        port: Some(port),
        targets: None,
        failover: None,
        started_at: Some(now_ms),
        message: format!("{} daemon started (PID {}) on http://{}:{}", service.label, pid, host, port),
    })
}

// ─── Daemon stop ──────────────────────────────────────────

pub fn daemon_stop(service: &Service) -> Result<DaemonResult, String> {
    let info = match read_pid_file(service) {
        Some(i) => i,
        None => {
            return Ok(DaemonResult {
                running: false,
                pid: None,
                host: None,
                port: None,
                targets: None,
                failover: None,
                started_at: None,
                message: format!("No {} daemon PID file found", service.label),
            });
        }
    };

    if !is_alive(info.pid) {
        remove_pid_file(service);
        return Ok(DaemonResult {
            running: false,
            pid: Some(info.pid),
            host: None,
            port: None,
            targets: None,
            failover: None,
            started_at: None,
            message: format!("PID {} is not alive (cleaned up stale PID)", info.pid),
        });
    }

    // Graceful stop first
    kill_process(info.pid);
    for _ in 0..20 {  // Reduced from 50 to 20 (2 seconds max)
        std::thread::sleep(std::time::Duration::from_millis(100));
        if !is_alive(info.pid) {
            remove_pid_file(service);
            return Ok(DaemonResult {
                running: false,
                pid: Some(info.pid),
                host: None,
                port: None,
                targets: None,
                failover: None,
                started_at: None,
                message: format!("{} daemon (PID {}) stopped", service.label, info.pid),
            });
        }
    }

    // Force kill
    force_kill(info.pid);
    remove_pid_file(service);
    Ok(DaemonResult {
        running: false,
        pid: Some(info.pid),
        host: None,
        port: None,
        targets: None,
        failover: None,
        started_at: None,
        message: format!("{} daemon (PID {}) force killed", service.label, info.pid),
    })
}

// ─── Daemon status ────────────────────────────────────────

pub fn daemon_status(service: &Service) -> Result<DaemonResult, String> {
    let info = match read_pid_file(service) {
        Some(i) => i,
        None => {
            return Ok(DaemonResult {
                running: false,
                pid: None,
                host: None,
                port: None,
                targets: None,
                failover: None,
                started_at: None,
                message: format!("{} daemon is not running (no PID file)", service.label),
            });
        }
    };

    if is_alive(info.pid) {
        // Verify port is actually listening
        if !check_health(&info.host, info.port, 2) {
            remove_pid_file(service);
            return Ok(DaemonResult {
                running: false,
                pid: Some(info.pid),
                host: None,
                port: None,
                targets: None,
                failover: None,
                started_at: None,
                message: format!(
                    "{} daemon process exists (PID {}) but port {}:{} is not responding. Cleaned up stale PID.",
                    service.label, info.pid, info.host, info.port
                ),
            });
        }

        // targets/failover only apply to the proxy; the web UI has neither.
        let (targets, failover) = if service.subcommand == "proxy" {
            let proxy = load_config().map(|c| c.settings.proxy).unwrap_or_default();
            let targets: Vec<String> = load_config()
                .ok()
                .as_ref()
                .map(|cfg| {
                    cfg.profiles
                        .iter()
                        .filter_map(|(name, profile)| {
                            profile
                                .get("exposedModels")
                                .and_then(|v| v.as_array())
                                .filter(|arr| !arr.is_empty())
                                .map(|_| name.clone())
                        })
                        .collect()
                })
                .unwrap_or_default();
            let failover = if proxy.failover.is_empty() { None } else { Some(proxy.failover.clone()) };
            (if targets.is_empty() { None } else { Some(targets) }, failover)
        } else {
            (None, None)
        };

        Ok(DaemonResult {
            running: true,
            pid: Some(info.pid),
            host: Some(info.host.clone()),
            port: Some(info.port),
            targets,
            failover,
            started_at: Some(info.started_at),
            message: format!(
                "{} daemon is running (PID {}) on http://{}:{}",
                service.label, info.pid, info.host, info.port
            ),
        })
    } else {
        remove_pid_file(service);
        Ok(DaemonResult {
            running: false,
            pid: Some(info.pid),
            host: None,
            port: None,
            targets: None,
            failover: None,
            started_at: None,
            message: format!("PID {} is not alive (cleaned up stale PID)", info.pid),
        })
    }
}
