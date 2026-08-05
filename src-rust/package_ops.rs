use crate::database::Database;
use crate::error::{AppError, Result};
use crate::package::{Package, PackageSource, PackageType};
use std::path::PathBuf;

/// Run `pi uninstall <spec>` to actually remove the installed package files
/// (e.g. `~/.pi/agent/npm/node_modules/...`) in addition to settings.json.
/// Cross-platform: on Windows `pi` is a `.cmd`, so it must go through `cmd /c`.
fn run_pi_uninstall(spec: &str) -> Result<()> {
    let output = {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/c", "pi", "uninstall", spec])
                .output()
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("pi")
                .args(["uninstall", spec])
                .output()
        }
    }
    .map_err(|e| {
        AppError::Message(format!(
            "Failed to run `pi uninstall {}`: {}. The package was removed from pi's settings but its files may remain.",
            spec, e
        ))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Message(format!(
            "`pi uninstall {}` failed: {}",
            spec,
            stderr.trim()
        )));
    }
    Ok(())
}

/// Run `pi install <spec>` to actually download the package files into
/// `~/.pi/agent/npm/node_modules/` (npm) or `~/.pi/agent/git/` (git).
fn run_pi_install(spec: &str) -> Result<()> {
    let output = {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/c", "pi", "install", spec])
                .output()
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("pi")
                .args(["install", spec])
                .output()
        }
    }
    .map_err(|e| AppError::Message(format!("Failed to run `pi install {}`: {}", spec, e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Message(format!(
            "`pi install {}` failed: {}",
            spec,
            stderr.trim()
        )));
    }
    Ok(())
}

/// Directory where an installed package lives on disk.
/// npm → `~/.pi/agent/npm/node_modules/<name>`, git → `~/.pi/agent/git/<name>`,
/// local → the spec path itself.
fn installed_pkg_dir(pkg: &Package) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match pkg.pkg_type {
        PackageType::Npm => Some(
            home.join(".pi")
                .join("agent")
                .join("npm")
                .join("node_modules")
                .join(&pkg.name),
        ),
        PackageType::Git => Some(home.join(".pi").join("agent").join("git").join(&pkg.name)),
        PackageType::Local => Some(PathBuf::from(&pkg.spec)),
    }
}

/// Detect package capabilities from the installed `package.json` `pi` manifest
/// or conventional directories (`extensions/`, `skills/`, `prompts/`, `themes/`).
fn detect_capabilities(pkg: &Package) -> (bool, bool, bool, bool) {
    let Some(dir) = installed_pkg_dir(pkg) else {
        return (false, false, false, false);
    };

    let mut caps = (false, false, false, false);
    let manifest = dir.join("package.json");
    if let Ok(text) = std::fs::read_to_string(&manifest) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(pi) = value.get("pi") {
                caps.0 = pi
                    .get("extensions")
                    .and_then(|v| v.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
                caps.1 = pi
                    .get("skills")
                    .and_then(|v| v.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
                caps.2 = pi
                    .get("prompts")
                    .and_then(|v| v.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
                caps.3 = pi
                    .get("themes")
                    .and_then(|v| v.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
            }
        }
    }
    // Fallback: conventional directory auto-discovery.
    caps.0 = caps.0 || dir.join("extensions").is_dir();
    caps.1 = caps.1 || dir.join("skills").is_dir();
    caps.2 = caps.2 || dir.join("prompts").is_dir();
    caps.3 = caps.3 || dir.join("themes").is_dir();
    caps
}

/// Initialize package management (create database if not exists)
pub fn init_packages() -> Result<()> {
    let _db = Database::open()?;
    Ok(())
}

/// List all packages
pub fn list_packages() -> Result<Vec<Package>> {
    let db = Database::open()?;
    db.list_packages()
}

/// Get a specific package by ID
pub fn get_package(id: &str) -> Result<Option<Package>> {
    let db = Database::open()?;
    db.get_package(id)
}

/// Add a package (parse spec and insert)
pub fn add_package(spec: &str) -> Result<Package> {
    let db = Database::open()?;

    // Parse spec into Package
    let mut pkg = Package::from_spec(spec)?;

    // Check if already exists
    if db.get_package(&pkg.id)?.is_some() {
        return Err(AppError::InvalidInput(format!(
            "Package '{}' already exists",
            pkg.id
        )));
    }

    // Set timestamps
    let now = chrono::Utc::now().timestamp();
    pkg.installed_at = Some(now);
    pkg.updated_at = Some(now);

    // Insert into database
    db.insert_package(&pkg)?;

    Ok(pkg)
}

/// Install a package (actually run `pi install`, mark as installed, detect
/// capabilities, and sync to Pi Agent settings.json)
pub fn install_package(id: &str) -> Result<Package> {
    let db = Database::open()?;

    let mut pkg = db
        .get_package(id)?
        .ok_or_else(|| AppError::InvalidInput(format!("Package '{}' not found", id)))?;

    if pkg.installed {
        return Err(AppError::InvalidInput(format!(
            "Package '{}' is already installed",
            id
        )));
    }

    // Actually download the package files via `pi install`
    run_pi_install(&pkg.spec)?;

    // Detect capabilities from the installed package.json / directories
    let (has_extensions, has_skills, has_prompts, has_themes) = detect_capabilities(&pkg);
    pkg.has_extensions = has_extensions;
    pkg.has_skills = has_skills;
    pkg.has_prompts = has_prompts;
    pkg.has_themes = has_themes;

    // Mark as installed
    pkg.installed = true;
    pkg.installed_at = Some(chrono::Utc::now().timestamp());
    pkg.updated_at = Some(chrono::Utc::now().timestamp());

    db.update_package(&pkg)?;

    // Sync to Pi Agent settings.json
    sync_packages_to_pi()?;

    Ok(pkg)
}

/// Uninstall a package (mark as not installed, sync to Pi Agent, and run
/// `pi uninstall <spec>` to remove the actual package files).
pub fn uninstall_package(id: &str) -> Result<Package> {
    let db = Database::open()?;

    let mut pkg = db
        .get_package(id)?
        .ok_or_else(|| AppError::InvalidInput(format!("Package '{}' not found", id)))?;

    if !pkg.installed {
        return Err(AppError::InvalidInput(format!(
            "Package '{}' is not installed",
            id
        )));
    }

    // Mark as not installed
    pkg.installed = false;
    pkg.updated_at = Some(chrono::Utc::now().timestamp());

    db.update_package(&pkg)?;

    // Sync to Pi Agent settings.json (stop pi from loading it)
    sync_packages_to_pi()?;

    // Best-effort file removal — see uninstall_and_remove comment.
    let _ = run_pi_uninstall(&pkg.spec);

    Ok(pkg)
}

/// Enable a package
pub fn enable_package(id: &str) -> Result<Package> {
    let db = Database::open()?;

    let mut pkg = db
        .get_package(id)?
        .ok_or_else(|| AppError::InvalidInput(format!("Package '{}' not found", id)))?;

    pkg.enabled = true;
    pkg.updated_at = Some(chrono::Utc::now().timestamp());

    db.update_package(&pkg)?;

    // Sync to Pi Agent if installed
    if pkg.installed {
        sync_packages_to_pi()?;
    }

    Ok(pkg)
}

/// Disable a package
pub fn disable_package(id: &str) -> Result<Package> {
    let db = Database::open()?;

    let mut pkg = db
        .get_package(id)?
        .ok_or_else(|| AppError::InvalidInput(format!("Package '{}' not found", id)))?;

    pkg.enabled = false;
    pkg.updated_at = Some(chrono::Utc::now().timestamp());

    db.update_package(&pkg)?;

    // Sync to Pi Agent if installed
    if pkg.installed {
        sync_packages_to_pi()?;
    }

    Ok(pkg)
}

/// Delete a package from database
pub fn delete_package(id: &str) -> Result<()> {
    let db = Database::open()?;

    let pkg = db
        .get_package(id)?
        .ok_or_else(|| AppError::InvalidInput(format!("Package '{}' not found", id)))?;

    if pkg.installed {
        return Err(AppError::InvalidInput(format!(
            "Cannot delete installed package '{}'. Uninstall it first.",
            id
        )));
    }

    db.delete_package(id)?;

    Ok(())
}

/// Toggle package enabled/disabled state
pub fn toggle_package(id: &str) -> Result<Package> {
    let db = Database::open()?;

    let mut pkg = db
        .get_package(id)?
        .ok_or_else(|| AppError::InvalidInput(format!("Package '{}' not found", id)))?;

    pkg.enabled = !pkg.enabled;
    pkg.updated_at = Some(chrono::Utc::now().timestamp());

    db.update_package(&pkg)?;

    // Sync to Pi Agent if installed
    if pkg.installed {
        sync_packages_to_pi()?;
    }

    Ok(pkg)
}

/// Uninstall from Pi Agent (if installed) and remove the database record
/// entirely. This is what UI "delete" should do: one action fully removes
/// the package instead of erroring on installed packages.
pub fn uninstall_and_remove(id: &str) -> Result<()> {
    let db = Database::open()?;

    let pkg = db
        .get_package(id)?
        .ok_or_else(|| AppError::InvalidInput(format!("Package '{}' not found", id)))?;

    if pkg.installed {
        let mut p = pkg.clone();
        p.installed = false;
        p.updated_at = Some(chrono::Utc::now().timestamp());
        db.update_package(&p)?;
        sync_packages_to_pi()?;
        // Best-effort file removal: settings.json is already synced so the
        // package is unloaded from pi either way. A failure here (e.g. pi
        // reports "no matching package") just means leftover files — the
        // uninstall itself is still effective, so don't block on it.
        let _ = run_pi_uninstall(&pkg.spec);
    }

    db.delete_package(id)?;
    Ok(())
}

/// Sync enabled packages to Pi Agent's settings.json
pub fn sync_packages_to_pi() -> Result<()> {
    let db = Database::open()?;
    let packages = db.list_packages()?;

    // Filter installed and enabled packages
    let enabled_specs: Vec<String> = packages
        .into_iter()
        .filter(|p| p.installed && p.enabled)
        .map(|p| p.spec)
        .collect();

    // Get Pi Agent settings path
    let settings_path = get_pi_agent_settings_path()?;

    // Read existing settings
    let mut settings = if settings_path.exists() {
        let content =
            std::fs::read_to_string(&settings_path).map_err(|e| AppError::io(&settings_path, e))?;
        serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| AppError::json(&settings_path, e))?
    } else {
        serde_json::json!({})
    };

    // Update packages field
    settings["packages"] = serde_json::json!(enabled_specs);

    // Write back
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| AppError::Message(format!("Failed to serialize settings: {}", e)))?;

    std::fs::write(&settings_path, content).map_err(|e| AppError::io(&settings_path, e))?;

    Ok(())
}

/// Import packages from Pi Agent's settings.json
pub fn import_from_pi() -> Result<Vec<Package>> {
    let db = Database::open()?;
    let settings_path = get_pi_agent_settings_path()?;

    if !settings_path.exists() {
        return Err(AppError::InvalidInput(
            "Pi Agent settings.json not found".to_string(),
        ));
    }

    // Read settings
    let content =
        std::fs::read_to_string(&settings_path).map_err(|e| AppError::io(&settings_path, e))?;
    let settings: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| AppError::json(&settings_path, e))?;

    // Extract packages array
    let specs = settings["packages"]
        .as_array()
        .ok_or_else(|| AppError::InvalidInput("No packages field in settings.json".to_string()))?;

    // Collect the set of ids currently enabled in pi's settings.
    let mut active_ids = std::collections::HashSet::new();
    for spec_value in specs {
        if let Some(spec) = spec_value.as_str() {
            if let Ok(pkg) = Package::from_spec(spec) {
                active_ids.insert(pkg.id);
            }
        }
    }

    // Mark db records that pi no longer has (e.g. uninstalled via `pi uninstall`)
    // as not installed so UI counts match reality.
    for existing in db.list_packages()? {
        if existing.installed && !active_ids.contains(&existing.id) {
            let mut stale = existing;
            stale.installed = false;
            stale.updated_at = Some(chrono::Utc::now().timestamp());
            db.update_package(&stale)?;
        }
    }

    let mut imported = Vec::new();
    let now = chrono::Utc::now().timestamp();

    for spec_value in specs {
        let spec = spec_value
            .as_str()
            .ok_or_else(|| AppError::InvalidInput("Invalid package spec".to_string()))?;

        // Check if already exists
        let mut pkg = Package::from_spec(spec)?;

        // Detect capabilities from the already-installed package.json
        let (has_extensions, has_skills, has_prompts, has_themes) = detect_capabilities(&pkg);
        pkg.has_extensions = has_extensions;
        pkg.has_skills = has_skills;
        pkg.has_prompts = has_prompts;
        pkg.has_themes = has_themes;

        if db.get_package(&pkg.id)?.is_none() {
            // New package
            pkg.installed = true;
            pkg.enabled = true;
            pkg.installed_at = Some(now);
            pkg.updated_at = Some(now);

            db.insert_package(&pkg)?;
            imported.push(pkg);
        } else {
            // Update existing
            let mut existing = db.get_package(&pkg.id)?.unwrap();
            existing.installed = true;
            existing.enabled = true;
            existing.updated_at = Some(now);
            existing.has_extensions = pkg.has_extensions;
            existing.has_skills = pkg.has_skills;
            existing.has_prompts = pkg.has_prompts;
            existing.has_themes = pkg.has_themes;

            db.update_package(&existing)?;
            imported.push(existing);
        }
    }

    Ok(imported)
}

// ─── Package Sources ──────────────────────────────────────

/// List package sources
pub fn list_sources() -> Result<Vec<PackageSource>> {
    let db = Database::open()?;
    db.list_sources()
}

/// Add a package source
pub fn add_source(url: &str, source_type: &str, name: Option<&str>) -> Result<PackageSource> {
    let db = Database::open()?;

    let source = PackageSource {
        id: None,
        url: url.to_string(),
        source_type: source_type.to_string(),
        name: name.map(|s| s.to_string()),
        enabled: true,
        added_at: Some(chrono::Utc::now().timestamp()),
    };

    let id = db.add_source(&source)?;

    Ok(PackageSource {
        id: Some(id),
        ..source
    })
}

/// Delete a package source
pub fn delete_source(id: i64) -> Result<()> {
    let db = Database::open()?;
    db.delete_source(id)
}

// ─── Helpers ──────────────────────────────────────────────

/// Get Pi Agent settings.json path
fn get_pi_agent_settings_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::Message("Could not determine home directory".to_string()))?;

    Ok(home.join(".pi").join("agent").join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_from_spec() {
        let pkg = Package::from_spec("npm:lodash@4.17.21").unwrap();
        assert_eq!(pkg.name, "lodash");
        assert_eq!(pkg.version, Some("4.17.21".to_string()));
        assert_eq!(pkg.spec, "npm:lodash@4.17.21");
    }

    #[test]
    fn test_package_from_git_spec() {
        let pkg = Package::from_spec("git:https://github.com/user/repo").unwrap();
        assert_eq!(pkg.name, "repo");
        assert!(pkg.spec.starts_with("git:"));
    }
}
