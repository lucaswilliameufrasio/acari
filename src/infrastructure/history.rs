use std::fs;
use std::io::Write;
use std::path::PathBuf;

const MAX_LOG_SIZE: u64 = 102_400; // 100 KB

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
fn set_restrictive_permissions(_path: &PathBuf) {}

fn history_path() -> PathBuf {
    if let Ok(dir) = std::env::var("ACARI_DATA_HOME") {
        return PathBuf::from(dir).join("history.log");
    }
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("acari").join("history.log")
}

fn maybe_rotate(path: &PathBuf) {
    if let Ok(meta) = fs::metadata(path)
        && meta.len() > MAX_LOG_SIZE
    {
        let rotated = path.with_extension("log.old");
        let _ = fs::rename(path, &rotated);
        set_restrictive_permissions(&rotated);
    }
}

pub fn append_entry(line: &str) {
    let path = history_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    maybe_rotate(&path);
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
    set_restrictive_permissions(&path);
}

/// Read all history log entries (most recent last), or an empty vec if none.
pub fn read_entries() -> Vec<String> {
    let path = history_path();
    match fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Remove the history log entirely.
pub fn clear() -> std::io::Result<()> {
    let path = history_path();
    if path.exists() {
        fs::remove_file(&path)
    } else {
        Ok(())
    }
}

/// Path to the history log (exposed for tests and the `history` command).
pub fn log_path() -> PathBuf {
    history_path()
}

pub fn format_local_time() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::config::target_config::unix_secs_to_local(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_path_ends_with_history_log() {
        let p = history_path();
        assert!(p.to_string_lossy().ends_with("history.log"));
    }
}
