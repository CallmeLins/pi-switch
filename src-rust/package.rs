use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

/// Package type enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Npm,
    Git,
    Local,
}

impl std::fmt::Display for PackageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageType::Npm => write!(f, "npm"),
            PackageType::Git => write!(f, "git"),
            PackageType::Local => write!(f, "local"),
        }
    }
}

impl std::str::FromStr for PackageType {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "npm" => Ok(PackageType::Npm),
            "git" => Ok(PackageType::Git),
            "local" => Ok(PackageType::Local),
            _ => Err(AppError::Message(format!("Unknown package type: {}", s))),
        }
    }
}

/// Package metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: String,
    pub spec: String,
    #[serde(rename = "type")]
    pub pkg_type: PackageType,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,

    // Capabilities (parsed from package.json)
    pub has_extensions: bool,
    pub has_skills: bool,
    pub has_prompts: bool,
    pub has_themes: bool,

    // Status
    pub installed: bool,
    pub enabled: bool,
    pub installed_at: Option<i64>,
    pub updated_at: Option<i64>,
}

impl Package {
    /// Parse a package spec into a Package struct
    ///
    /// Supported formats:
    /// - npm:@scope/name@version
    /// - npm:package-name@version
    /// - git:github.com/user/repo
    /// - /path/to/local/package
    pub fn from_spec(spec: &str) -> Result<Self> {
        if spec.starts_with("npm:") {
            Self::parse_npm(spec)
        } else if spec.starts_with("git:") {
            Self::parse_git(spec)
        } else {
            Self::parse_local(spec)
        }
    }

    /// Parse npm package spec: npm:@scope/name@version or npm:name@version
    fn parse_npm(spec: &str) -> Result<Self> {
        let without_prefix = spec.strip_prefix("npm:").unwrap();

        // Split by last @ to separate version
        let (name_part, version) = if let Some(pos) = without_prefix.rfind('@') {
            // Handle scoped packages: @scope/name@version or @scope/name (no version)
            if pos == 0 && without_prefix.contains('/') {
                // This is @scope/name without version (@ is at the beginning)
                (without_prefix, None)
            } else {
                let before_at = &without_prefix[..pos];
                if before_at.starts_with('@') && before_at.contains('/') {
                    // This is @scope/name@version
                    let version = &without_prefix[pos + 1..];
                    (before_at, Some(version.to_string()))
                } else {
                    // Simple name@version
                    (before_at, Some(without_prefix[pos + 1..].to_string()))
                }
            }
        } else {
            (without_prefix, None)
        };

        let name = name_part.to_string();
        let id = if let Some(ref v) = version {
            format!("npm:{}@{}", name, v)
        } else {
            format!("npm:{}", name)
        };

        Ok(Package {
            id: id.clone(),
            spec: spec.to_string(),
            pkg_type: PackageType::Npm,
            name,
            version,
            description: None,
            homepage: None,
            has_extensions: false,
            has_skills: false,
            has_prompts: false,
            has_themes: false,
            installed: false,
            enabled: true,
            installed_at: None,
            updated_at: None,
        })
    }

    /// Parse git package spec: git:github.com/user/repo or git:https://github.com/user/repo
    fn parse_git(spec: &str) -> Result<Self> {
        let without_prefix = spec.strip_prefix("git:").unwrap();

        // Remove https:// or http:// if present
        let url = without_prefix
            .strip_prefix("https://")
            .or_else(|| without_prefix.strip_prefix("http://"))
            .unwrap_or(without_prefix);

        // Extract repo name from URL (last part)
        let parts: Vec<&str> = url.split('/').collect();
        let repo_name = parts
            .last()
            .ok_or_else(|| AppError::Message("Invalid git URL".to_string()))?
            .trim_end_matches(".git");

        let id = format!("git:{}", url);

        Ok(Package {
            id: id.clone(),
            spec: spec.to_string(),
            pkg_type: PackageType::Git,
            name: repo_name.to_string(),
            version: None,
            description: None,
            homepage: None,
            has_extensions: false,
            has_skills: false,
            has_prompts: false,
            has_themes: false,
            installed: false,
            enabled: true,
            installed_at: None,
            updated_at: None,
        })
    }

    /// Parse local package spec: /path/to/package
    fn parse_local(spec: &str) -> Result<Self> {
        let path = std::path::Path::new(spec);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::Message("Invalid local path".to_string()))?;

        let id = format!("local:{}", spec);

        Ok(Package {
            id: id.clone(),
            spec: spec.to_string(),
            pkg_type: PackageType::Local,
            name: name.to_string(),
            version: None,
            description: None,
            homepage: None,
            has_extensions: false,
            has_skills: false,
            has_prompts: false,
            has_themes: false,
            installed: false,
            enabled: true,
            installed_at: None,
            updated_at: None,
        })
    }
}

/// Package source (registry or repository)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSource {
    pub id: Option<i64>,
    pub url: String,
    #[serde(rename = "type")]
    pub source_type: String,
    pub name: Option<String>,
    pub enabled: bool,
    pub added_at: Option<i64>,
}

impl PackageSource {
    pub fn new(url: String, source_type: String, name: Option<String>) -> Self {
        Self {
            id: None,
            url,
            source_type,
            name,
            enabled: true,
            added_at: Some(chrono::Utc::now().timestamp()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_npm_simple() {
        let pkg = Package::from_spec("npm:lodash@4.17.21").unwrap();
        assert_eq!(pkg.pkg_type, PackageType::Npm);
        assert_eq!(pkg.name, "lodash");
        assert_eq!(pkg.version, Some("4.17.21".to_string()));
        assert_eq!(pkg.spec, "npm:lodash@4.17.21");
    }

    #[test]
    fn test_parse_npm_scoped() {
        let pkg = Package::from_spec("npm:@cokefenta/pi-switch@1.0.0").unwrap();
        assert_eq!(pkg.pkg_type, PackageType::Npm);
        assert_eq!(pkg.name, "@cokefenta/pi-switch");
        assert_eq!(pkg.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_npm_no_version() {
        let pkg = Package::from_spec("npm:@cokefenta/test-package").unwrap();
        assert_eq!(pkg.pkg_type, PackageType::Npm);
        assert_eq!(pkg.name, "@cokefenta/test-package");
        assert_eq!(pkg.version, None);
    }

    #[test]
    fn test_parse_git() {
        let pkg = Package::from_spec("git:github.com/user/repo").unwrap();
        assert_eq!(pkg.pkg_type, PackageType::Git);
        assert_eq!(pkg.name, "repo");
        assert_eq!(pkg.spec, "git:github.com/user/repo");
    }

    #[test]
    fn test_parse_git_with_https() {
        let pkg = Package::from_spec("git:https://github.com/user/repo.git").unwrap();
        assert_eq!(pkg.pkg_type, PackageType::Git);
        assert_eq!(pkg.name, "repo");
    }

    #[test]
    fn test_parse_local() {
        let pkg = Package::from_spec("/path/to/my-package").unwrap();
        assert_eq!(pkg.pkg_type, PackageType::Local);
        assert_eq!(pkg.name, "my-package");
        assert_eq!(pkg.spec, "/path/to/my-package");
    }
}
