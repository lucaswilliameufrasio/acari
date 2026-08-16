use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TargetOrigin {
    Builtin,
    Custom,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CleanTarget {
    pub name: Cow<'static, str>,
    pub path: Cow<'static, str>,
    pub description: Cow<'static, str>,
    pub delete_entire: bool,
    pub command: &'static [&'static str],
    pub requires_sudo: bool,
    pub dangerous: bool,
    pub origin: TargetOrigin,
}

impl CleanTarget {
    pub fn is_command(&self) -> bool {
        !self.command.is_empty()
    }

    pub fn is_dangerous(&self) -> bool {
        self.dangerous || self.requires_sudo
    }

    pub fn is_custom(&self) -> bool {
        self.origin == TargetOrigin::Custom
    }
}

impl CleanTarget {
    pub const fn file(
        name: &'static str,
        path: &'static str,
        description: &'static str,
        delete_entire: bool,
    ) -> Self {
        Self {
            name: Cow::Borrowed(name),
            path: Cow::Borrowed(path),
            description: Cow::Borrowed(description),
            delete_entire,
            command: &[],
            requires_sudo: false,
            dangerous: false,
            origin: TargetOrigin::Builtin,
        }
    }

    pub const fn cmd(
        name: &'static str,
        description: &'static str,
        command: &'static [&'static str],
    ) -> Self {
        Self {
            name: Cow::Borrowed(name),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed(description),
            delete_entire: false,
            command,
            requires_sudo: false,
            dangerous: false,
            origin: TargetOrigin::Builtin,
        }
    }

    pub const fn with_sudo(mut self) -> Self {
        self.requires_sudo = true;
        self
    }

    pub const fn dangerous(mut self) -> Self {
        self.dangerous = true;
        self
    }

    pub const fn custom(mut self) -> Self {
        self.origin = TargetOrigin::Custom;
        self
    }
}

impl Default for CleanTarget {
    fn default() -> Self {
        Self {
            name: Cow::Borrowed(""),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed(""),
            delete_entire: false,
            command: &[],
            requires_sudo: false,
            dangerous: false,
            origin: TargetOrigin::Builtin,
        }
    }
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
