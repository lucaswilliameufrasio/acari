use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn history_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("~/.local/share"));
    base.join("acari").join("history.log")
}

pub fn append_entry(line: &str) {
    let path = history_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
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
