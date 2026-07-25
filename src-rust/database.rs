use crate::config;
use crate::error::{AppError, Result};
use crate::package::{Package, PackageSource, PackageType};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::str::FromStr;

const SCHEMA: &str = r#"
-- Packages table
CREATE TABLE IF NOT EXISTS packages (
    id TEXT PRIMARY KEY,
    spec TEXT NOT NULL,
    type TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT,
    description TEXT,
    homepage TEXT,

    has_extensions INTEGER NOT NULL DEFAULT 0,
    has_skills INTEGER NOT NULL DEFAULT 0,
    has_prompts INTEGER NOT NULL DEFAULT 0,
    has_themes INTEGER NOT NULL DEFAULT 0,

    installed INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    installed_at INTEGER,
    updated_at INTEGER,

    package_json TEXT
);

-- Package sources table
CREATE TABLE IF NOT EXISTS package_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL,
    name TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    added_at INTEGER
);

-- Insert default sources if not exist
INSERT OR IGNORE INTO package_sources (url, type, name, added_at) VALUES
    ('https://registry.npmjs.org', 'npm-registry', 'npm', strftime('%s', 'now')),
    ('https://github.com/PSPDFKit-labs/pi-skills', 'github-org', 'pi-skills', strftime('%s', 'now'));

-- Index for faster queries
CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
CREATE INDEX IF NOT EXISTS idx_packages_type ON packages(type);
CREATE INDEX IF NOT EXISTS idx_packages_installed ON packages(installed);
CREATE INDEX IF NOT EXISTS idx_packages_enabled ON packages(enabled);
"#;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create the database
    pub fn open() -> Result<Self> {
        let path = db_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::io(parent, e))?;
        }

        let conn = Connection::open(&path)
            .map_err(|e| AppError::Message(format!("Failed to open database: {}", e)))?;

        // Enable WAL mode for better concurrency
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| AppError::Message(format!("Failed to set WAL mode: {}", e)))?;

        // Initialize schema
        conn.execute_batch(SCHEMA)
            .map_err(|e| AppError::Message(format!("Failed to initialize schema: {}", e)))?;

        Ok(Self { conn })
    }

    // ─── Package CRUD ─────────────────────────────────────────

    /// Insert a new package
    pub fn insert_package(&self, pkg: &Package) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO packages (
                id, spec, type, name, version, description, homepage,
                has_extensions, has_skills, has_prompts, has_themes,
                installed, enabled, installed_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                pkg.id,
                pkg.spec,
                pkg.pkg_type.to_string(),
                pkg.name,
                pkg.version,
                pkg.description,
                pkg.homepage,
                pkg.has_extensions as i32,
                pkg.has_skills as i32,
                pkg.has_prompts as i32,
                pkg.has_themes as i32,
                pkg.installed as i32,
                pkg.enabled as i32,
                pkg.installed_at,
                pkg.updated_at,
            ],
        ).map_err(|e| AppError::Message(format!("Failed to insert package: {}", e)))?;

        Ok(())
    }

    /// Get a package by ID
    pub fn get_package(&self, id: &str) -> Result<Option<Package>> {
        self.conn
            .query_row(
                r#"
                SELECT id, spec, type, name, version, description, homepage,
                       has_extensions, has_skills, has_prompts, has_themes,
                       installed, enabled, installed_at, updated_at
                FROM packages WHERE id = ?
                "#,
                params![id],
                |row| {
                    Ok(Package {
                        id: row.get(0)?,
                        spec: row.get(1)?,
                        pkg_type: PackageType::from_str(&row.get::<_, String>(2)?).unwrap(),
                        name: row.get(3)?,
                        version: row.get(4)?,
                        description: row.get(5)?,
                        homepage: row.get(6)?,
                        has_extensions: row.get::<_, i32>(7)? != 0,
                        has_skills: row.get::<_, i32>(8)? != 0,
                        has_prompts: row.get::<_, i32>(9)? != 0,
                        has_themes: row.get::<_, i32>(10)? != 0,
                        installed: row.get::<_, i32>(11)? != 0,
                        enabled: row.get::<_, i32>(12)? != 0,
                        installed_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    })
                },
            )
            .optional()
            .map_err(|e| AppError::Message(format!("Failed to get package: {}", e)))
    }

    /// List all packages
    pub fn list_packages(&self) -> Result<Vec<Package>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, spec, type, name, version, description, homepage,
                   has_extensions, has_skills, has_prompts, has_themes,
                   installed, enabled, installed_at, updated_at
            FROM packages
            ORDER BY name ASC
            "#,
        ).map_err(|e| AppError::Message(format!("Failed to prepare statement: {}", e)))?;

        let packages = stmt
            .query_map([], |row| {
                Ok(Package {
                    id: row.get(0)?,
                    spec: row.get(1)?,
                    pkg_type: PackageType::from_str(&row.get::<_, String>(2)?).unwrap(),
                    name: row.get(3)?,
                    version: row.get(4)?,
                    description: row.get(5)?,
                    homepage: row.get(6)?,
                    has_extensions: row.get::<_, i32>(7)? != 0,
                    has_skills: row.get::<_, i32>(8)? != 0,
                    has_prompts: row.get::<_, i32>(9)? != 0,
                    has_themes: row.get::<_, i32>(10)? != 0,
                    installed: row.get::<_, i32>(11)? != 0,
                    enabled: row.get::<_, i32>(12)? != 0,
                    installed_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            })
            .map_err(|e| AppError::Message(format!("Failed to query packages: {}", e)))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AppError::Message(format!("Failed to collect packages: {}", e)))?;

        Ok(packages)
    }

    /// Update a package
    pub fn update_package(&self, pkg: &Package) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE packages SET
                spec = ?, type = ?, name = ?, version = ?,
                description = ?, homepage = ?,
                has_extensions = ?, has_skills = ?, has_prompts = ?, has_themes = ?,
                installed = ?, enabled = ?, installed_at = ?, updated_at = ?
            WHERE id = ?
            "#,
            params![
                pkg.spec,
                pkg.pkg_type.to_string(),
                pkg.name,
                pkg.version,
                pkg.description,
                pkg.homepage,
                pkg.has_extensions as i32,
                pkg.has_skills as i32,
                pkg.has_prompts as i32,
                pkg.has_themes as i32,
                pkg.installed as i32,
                pkg.enabled as i32,
                pkg.installed_at,
                pkg.updated_at,
                pkg.id,
            ],
        ).map_err(|e| AppError::Message(format!("Failed to update package: {}", e)))?;

        Ok(())
    }

    /// Delete a package
    pub fn delete_package(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM packages WHERE id = ?", params![id])
            .map_err(|e| AppError::Message(format!("Failed to delete package: {}", e)))?;

        Ok(())
    }

    // ─── Package Sources ──────────────────────────────────────

    /// List all package sources
    pub fn list_sources(&self) -> Result<Vec<PackageSource>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, url, type, name, enabled, added_at
            FROM package_sources
            ORDER BY id ASC
            "#,
        ).map_err(|e| AppError::Message(format!("Failed to prepare statement: {}", e)))?;

        let sources = stmt
            .query_map([], |row| {
                Ok(PackageSource {
                    id: Some(row.get(0)?),
                    url: row.get(1)?,
                    source_type: row.get(2)?,
                    name: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                    added_at: row.get(5)?,
                })
            })
            .map_err(|e| AppError::Message(format!("Failed to query sources: {}", e)))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AppError::Message(format!("Failed to collect sources: {}", e)))?;

        Ok(sources)
    }

    /// Add a package source
    pub fn add_source(&self, source: &PackageSource) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO package_sources (url, type, name, enabled, added_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
            params![
                source.url,
                source.source_type,
                source.name,
                source.enabled as i32,
                source.added_at,
            ],
        ).map_err(|e| AppError::Message(format!("Failed to add source: {}", e)))?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Delete a package source
    pub fn delete_source(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM package_sources WHERE id = ?", params![id])
            .map_err(|e| AppError::Message(format!("Failed to delete source: {}", e)))?;

        Ok(())
    }
}

/// Get the database file path
fn db_path() -> PathBuf {
    config::config_dir().join("pi-switch.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Database { conn }
    }

    #[test]
    fn test_insert_and_get_package() {
        let db = test_db();
        let pkg = Package::from_spec("npm:lodash@4.17.21").unwrap();

        db.insert_package(&pkg).unwrap();

        let retrieved = db.get_package(&pkg.id).unwrap().unwrap();
        assert_eq!(retrieved.name, "lodash");
        assert_eq!(retrieved.version, Some("4.17.21".to_string()));
    }

    #[test]
    fn test_list_packages() {
        let db = test_db();

        let pkg1 = Package::from_spec("npm:lodash@4.17.21").unwrap();
        let pkg2 = Package::from_spec("npm:react@18.0.0").unwrap();

        db.insert_package(&pkg1).unwrap();
        db.insert_package(&pkg2).unwrap();

        let packages = db.list_packages().unwrap();
        assert_eq!(packages.len(), 2);
    }

    #[test]
    fn test_update_package() {
        let db = test_db();
        let mut pkg = Package::from_spec("npm:lodash@4.17.21").unwrap();

        db.insert_package(&pkg).unwrap();

        pkg.enabled = false;
        pkg.description = Some("Test description".to_string());
        db.update_package(&pkg).unwrap();

        let retrieved = db.get_package(&pkg.id).unwrap().unwrap();
        assert!(!retrieved.enabled);
        assert_eq!(retrieved.description, Some("Test description".to_string()));
    }

    #[test]
    fn test_delete_package() {
        let db = test_db();
        let pkg = Package::from_spec("npm:lodash@4.17.21").unwrap();

        db.insert_package(&pkg).unwrap();
        db.delete_package(&pkg.id).unwrap();

        let retrieved = db.get_package(&pkg.id).unwrap();
        assert!(retrieved.is_none());
    }
}
