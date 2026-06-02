use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TargetConfig {
    #[serde(default)]
    pub custom_targets: Vec<CustomTargetEntry>,
    #[serde(default)]
    pub scan: ScanConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
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
            if sensitive.iter().any(|s| trimmed.starts_with(s) || trimmed == "/") {
                anyhow::bail!("path '{}' is a system directory and cannot be added as a target", trimmed);
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

    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.custom_targets.len();
        self.custom_targets
            .retain(|t| !t.name.eq_ignore_ascii_case(name));
        self.custom_targets.len() < len
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
            eprintln!("warning: could not read config at {}: {}", path.display(), e);
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
    let total_secs = secs as i64;
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
}
