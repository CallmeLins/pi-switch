use crate::database::Database;
use crate::error::{AppError, Result};
use crate::package::{Package, PackageSource};
use std::path::PathBuf;

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

/// Install a package (mark as installed and sync to Pi Agent)
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

    // Mark as installed
    pkg.installed = true;
    pkg.installed_at = Some(chrono::Utc::now().timestamp());
    pkg.updated_at = Some(chrono::Utc::now().timestamp());

    db.update_package(&pkg)?;

    // Sync to Pi Agent settings.json
    sync_packages_to_pi()?;

    Ok(pkg)
}

/// Uninstall a package (mark as not installed and sync to Pi Agent)
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

    // Sync to Pi Agent settings.json
    sync_packages_to_pi()?;

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

/// Alias for delete_package (for backward compatibility)
pub fn remove_package(id: &str) -> Result<()> {
    delete_package(id)
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
        let content = std::fs::read_to_string(&settings_path)
            .map_err(|e| AppError::io(&settings_path, e))?;
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

    std::fs::write(&settings_path, content)
        .map_err(|e| AppError::io(&settings_path, e))?;

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
    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| AppError::io(&settings_path, e))?;
    let settings: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::json(&settings_path, e))?;

    // Extract packages array
    let specs = settings["packages"]
        .as_array()
        .ok_or_else(|| AppError::InvalidInput("No packages field in settings.json".to_string()))?;

    let mut imported = Vec::new();
    let now = chrono::Utc::now().timestamp();

    for spec_value in specs {
        let spec = spec_value
            .as_str()
            .ok_or_else(|| AppError::InvalidInput("Invalid package spec".to_string()))?;

        // Check if already exists
        let mut pkg = Package::from_spec(spec)?;

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
