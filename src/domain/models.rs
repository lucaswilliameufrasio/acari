use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CleanTarget {
    pub name: Cow<'static, str>,
    pub path: Cow<'static, str>,
    pub description: Cow<'static, str>,
    pub delete_entire: bool,
}

impl CleanTarget {
    pub fn resolved_path(&self) -> PathBuf {
        expand_tilde(self.path.as_ref())
    }
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub target: CleanTarget,
    pub bytes: u64,
    pub files_scanned: u64,
}

#[derive(Debug, Clone)]
pub struct CleanResult {
    pub target: CleanTarget,
    pub reclaimed_bytes: u64,
    pub removed_entries: u64,
    pub errors: u64,
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(stripped);
    }

    if path == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }

    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::expand_tilde;

    #[test]
    fn expands_home_prefix() {
        let expanded = expand_tilde("~/acari-test");
        assert!(expanded.to_string_lossy().contains("acari-test"));
        assert!(!expanded.to_string_lossy().starts_with("~/"));
    }

    #[test]
    fn keeps_plain_paths() {
        let expanded = expand_tilde("/tmp/acari");
        assert_eq!(expanded.to_string_lossy(), "/tmp/acari");
    }
}
