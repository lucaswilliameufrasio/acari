use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
fn set_restrictive_permissions(path: &PathBuf) {
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        let _ = fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn set_restrictive_permissions(_path: &std::path::Path) {
    // Windows: no-op, file permissions are handled by the OS ACL model
}

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum IoPriority {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "normal")]
    #[default]
    Normal,
    #[serde(rename = "high")]
    High,
}

impl std::fmt::Display for IoPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoPriority::Low => write!(f, "low"),
            IoPriority::Normal => write!(f, "normal"),
            IoPriority::High => write!(f, "high"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetConfig {
    #[serde(default)]
    pub custom_targets: Vec<CustomTargetEntry>,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub project_scan: ProjectScanConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default)]
    pub io_priority: IoPriority,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            exclude_patterns: Vec::new(),
            io_priority: IoPriority::Normal,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectScanConfig {
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTargetEntry {
    pub name: String,
    pub path: String,
    #[serde(default = "default_description")]
    pub description: String,
}

fn default_description() -> String {
    String::from("User defined target")
}

impl TargetConfig {
    /// Register a directory only after an explicit user action.
    pub fn add_project_root(&mut self, raw: &str) -> Result<bool> {
        let path = normalized_project_root(raw)?;
        if self
            .project_scan
            .roots
            .iter()
            .any(|root| normalized_project_root(root).is_ok_and(|existing| existing == path))
        {
            return Ok(false);
        }
        self.project_scan.roots.push(path.display().to_string());
        Ok(true)
    }

    pub fn remove_project_root(&mut self, raw: &str) -> bool {
        let normalized = normalized_project_root(raw).ok();
        let before = self.project_scan.roots.len();
        self.project_scan.roots.retain(|root| {
            root != raw
                && !(normalized.is_some() && normalized_project_root(root).ok() == normalized)
        });
        self.project_scan.roots.len() != before
    }

    pub fn add(&mut self, name: &str, path: &str, description: &str) -> Result<bool> {
        if self
            .custom_targets
            .iter()
            .any(|t| t.name.eq_ignore_ascii_case(name))
        {
            return Ok(false);
        }

        let trimmed = path.trim();
        if trimmed.is_empty() {
            anyhow::bail!("path must not be empty");
        }
        if trimmed.contains("..") {
            anyhow::bail!("path must not contain '..' (traversal not allowed)");
        }
        if trimmed.starts_with('/') {
            let sensitive = ["/etc", "/var", "/sys", "/proc", "/dev", "/boot", "/usr"];
            if sensitive
                .iter()
                .any(|s| trimmed.starts_with(s) || trimmed == "/")
            {
                anyhow::bail!(
                    "path '{}' is a system directory and cannot be added as a target",
                    trimmed
                );
            }
        }

        self.custom_targets.push(CustomTargetEntry {
            name: name.to_owned(),
            path: trimmed.to_owned(),
            description: if description.is_empty() {
                default_description()
            } else {
                description.to_owned()
            },
        });
        Ok(true)
    }

    pub fn add_pattern(&mut self, pattern: &str) -> Result<bool> {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            anyhow::bail!("pattern name must not be empty");
        }
        if trimmed.len() > 64 {
            anyhow::bail!("pattern name too long (max 64 chars)");
        }
        if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
            anyhow::bail!("pattern name must not contain '/', '\\', or '..'");
        }
        if trimmed != pattern {
            anyhow::bail!("pattern name must not have leading/trailing whitespace");
        }
        if self.project_scan.patterns.contains(&trimmed.to_string()) {
            return Ok(false);
        }
        self.project_scan.patterns.push(trimmed.to_string());
        Ok(true)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.custom_targets.len();
        self.custom_targets
            .retain(|t| !t.name.eq_ignore_ascii_case(name));
        self.custom_targets.len() < len
    }
}

pub fn normalized_project_root(raw: &str) -> Result<PathBuf> {
    let expanded = crate::domain::expand_tilde(raw.trim());
    if raw.trim().is_empty() {
        anyhow::bail!("project root must not be empty");
    }
    let path = expanded.canonicalize().context("Project root must exist")?;
    if !path.is_dir() {
        anyhow::bail!("project root must be a directory");
    }
    let system_alias = [
        "/etc", "/var", "/sys", "/proc", "/dev", "/boot", "/bin", "/sbin", "/lib", "/lib64",
        "/usr", "/run",
    ]
    .iter()
    .any(|root| {
        std::path::Path::new(root)
            .canonicalize()
            .is_ok_and(|system| path.starts_with(system))
    });
    if path.parent().is_none()
        || system_alias
        || !crate::application::commands::is_safe_path(&path.to_string_lossy())
        || dirs::home_dir()
            .and_then(|home| home.canonicalize().ok())
            .as_ref()
            == Some(&path)
    {
        anyhow::bail!("choose a project directory, not home or a system directory");
    }
    Ok(path)
}

/// Shallow suggestions only: no recursive discovery, persistence or scanning.
pub fn suggested_project_roots(home: &std::path::Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for name in [
        "projects",
        "Projects",
        "Developer",
        "code",
        "src",
        "git",
        "git-worktrees",
    ] {
        if let Ok(path) = normalized_project_root(&home.join(name).display().to_string())
            && !roots.contains(&path)
        {
            roots.push(path);
        }
    }
    roots
}

#[cfg(test)]
mod project_root_tests {
    use super::*;

    fn root_fixture() -> tempfile::TempDir {
        #[cfg(target_os = "macos")]
        {
            tempfile::tempdir_in("/tmp").unwrap()
        }
        #[cfg(not(target_os = "macos"))]
        {
            tempfile::tempdir().unwrap()
        }
    }

    #[test]
    fn validates_and_deduplicates_existing_directories() {
        let temp = root_fixture();
        let root = temp.path().join("projects");
        fs::create_dir(&root).unwrap();
        let mut cfg = TargetConfig::default();
        cfg.project_scan.patterns.push(".custom".into());
        assert!(cfg.add_project_root(root.to_str().unwrap()).unwrap());
        assert!(
            !cfg.add_project_root(root.join(".").to_str().unwrap())
                .unwrap()
        );
        assert_eq!(cfg.project_scan.roots.len(), 1);
        assert!(cfg.remove_project_root(root.join(".").to_str().unwrap()));
        assert!(root.is_dir());
        assert_eq!(cfg.project_scan.patterns, [".custom"]);
    }

    #[test]
    fn rejects_missing_files_home_and_system_roots_without_mutation() {
        let temp = root_fixture();
        let file = temp.path().join("file");
        fs::write(&file, "data").unwrap();
        let mut cfg = TargetConfig::default();
        for raw in [
            "",
            "/",
            "/etc",
            file.to_str().unwrap(),
            temp.path().join("missing").to_str().unwrap(),
        ] {
            assert!(cfg.add_project_root(raw).is_err(), "{raw}");
        }
        if let Some(home) = dirs::home_dir() {
            assert!(cfg.add_project_root(home.to_str().unwrap()).is_err());
        }
        assert!(cfg.project_scan.roots.is_empty());
    }

    #[test]
    fn suggestions_are_shallow_existing_and_never_persisted() {
        let temp = root_fixture();
        fs::create_dir(temp.path().join("projects")).unwrap();
        fs::create_dir(temp.path().join("unrelated")).unwrap();
        let roots = suggested_project_roots(temp.path());
        assert_eq!(
            roots,
            [temp.path().join("projects").canonicalize().unwrap()]
        );
        assert!(!temp.path().join("acari").exists());
    }

    #[cfg(unix)]
    #[test]
    fn aliases_are_deduplicated_and_system_symlinks_rejected() {
        let temp = root_fixture();
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(temp.path(), &alias).unwrap();
        let mut cfg = TargetConfig::default();
        assert!(cfg.add_project_root(temp.path().to_str().unwrap()).unwrap());
        assert!(!cfg.add_project_root(alias.to_str().unwrap()).unwrap());
        let system = temp.path().join("system");
        std::os::unix::fs::symlink("/etc", &system).unwrap();
        assert!(cfg.add_project_root(system.to_str().unwrap()).is_err());
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("ACARI_CONFIG_HOME") {
        return PathBuf::from(dir).join("acari").join("config.toml");
    }
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("acari").join("config.toml");
    }
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("acari").join("config.toml")
}

pub fn load_config() -> TargetConfig {
    let path = config_path();
    if !path.exists() {
        return TargetConfig::default();
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "warning: could not read config at {}: {}",
                path.display(),
                e
            );
            return TargetConfig::default();
        }
    };
    match toml::from_str(&content) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "warning: invalid TOML in {}: {}. Using defaults.",
                path.display(),
                e
            );
            TargetConfig::default()
        }
    }
}

pub fn save_config(config: &TargetConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(&path, content).context("Failed to write config file")?;
    set_restrictive_permissions(&path);
    Ok(())
}

pub fn config_modified_time() -> Option<std::time::SystemTime> {
    let path = config_path();
    if !path.exists() {
        return None;
    }
    fs::metadata(&path).ok().and_then(|m| m.modified().ok())
}

pub fn config_modified_secs() -> Option<u64> {
    config_modified_time().and_then(|t| {
        t.duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
    })
}

pub fn unix_secs_to_local(secs: u64) -> String {
    // Saturate to i64::MAX to avoid overflow for timestamps beyond year 292 billion
    let total_secs = if secs > i64::MAX as u64 {
        i64::MAX
    } else {
        secs as i64
    };
    let mut remaining = total_secs;

    let secs_per_min: i64 = 60;
    let secs_per_hour: i64 = 3600;
    let secs_per_day: i64 = 86400;

    let days_from_epoch = remaining / secs_per_day;
    remaining %= secs_per_day;
    let hour = remaining / secs_per_hour;
    remaining %= secs_per_hour;
    let min = remaining / secs_per_min;
    let sec = remaining % secs_per_min;

    let z = days_from_epoch + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day: i64 = doy - (153 * mp + 2) / 5 + 1;
    let month: i64 = if mp < 10 { mp + 3 } else { mp - 9 };
    y = if month <= 2 { y + 1 } else { y };

    if (0..=9999).contains(&y) {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            y, month, day, hour, min, sec
        )
    } else {
        format!(
            "{:05}-{:02}-{:02} {:02}:{:02}:{:02}",
            y, month, day, hour, min, sec
        )
    }
}

pub fn format_modified_time() -> Option<String> {
    config_modified_time().and_then(|t| {
        t.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| unix_secs_to_local(d.as_secs()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_duplicate_returns_false() {
        let mut cfg = TargetConfig::default();
        cfg.add("test", "/tmp/test", "desc").unwrap();
        let dup = cfg.add("TEST", "/tmp/test2", "desc2").unwrap();
        assert!(!dup);
        assert_eq!(cfg.custom_targets.len(), 1);
    }

    #[test]
    fn remove_existing_returns_true() {
        let mut cfg = TargetConfig::default();
        cfg.add("test", "/tmp/test", "desc").unwrap();
        assert!(cfg.remove("TEST"));
        assert!(cfg.custom_targets.is_empty());
    }

    #[test]
    fn remove_missing_returns_false() {
        let mut cfg = TargetConfig::default();
        assert!(!cfg.remove("nonexistent"));
    }

    #[test]
    fn add_pattern_empty_rejected() {
        let mut cfg = TargetConfig::default();
        assert!(cfg.add_pattern("").is_err());
    }

    #[test]
    fn add_pattern_too_long_rejected() {
        let mut cfg = TargetConfig::default();
        let long = "a".repeat(65);
        assert!(cfg.add_pattern(&long).is_err());
    }

    #[test]
    fn add_pattern_invalid_chars_rejected() {
        let mut cfg = TargetConfig::default();
        assert!(cfg.add_pattern("foo/bar").is_err(), "slash");
        assert!(cfg.add_pattern("foo\\bar").is_err(), "backslash");
        assert!(cfg.add_pattern("..etc").is_err(), "dots");
        assert!(cfg.add_pattern(".terraform").is_ok(), "valid");
    }

    #[test]
    fn add_pattern_duplicate_returns_false() {
        let mut cfg = TargetConfig::default();
        assert!(cfg.add_pattern(".terraform").unwrap());
        let dup = cfg.add_pattern(".terraform").unwrap();
        assert!(!dup, "duplicate should return false");
        assert_eq!(cfg.project_scan.patterns.len(), 1);
    }
}
