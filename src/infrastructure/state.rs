//! Small JSON state files kept next to the history log (saved guide report,
//! free-space samples, notification bookkeeping).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// `$ACARI_DATA_HOME`, or the platform data dir + `acari`.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ACARI_DATA_HOME") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("acari")
}

pub fn path_of(name: &str) -> PathBuf {
    data_dir().join(name)
}

pub fn read_json<T: DeserializeOwned>(name: &str) -> Option<T> {
    read_json_at(&path_of(name))
}

pub fn write_json<T: Serialize>(name: &str, value: &T) -> std::io::Result<()> {
    write_json_at(&path_of(name), value)
}

pub fn read_json_at<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let content = std::fs::read(path).ok()?;
    serde_json::from_slice(&content).ok()
}

/// Write atomically (temp file + rename) so readers never see half a file.
pub fn write_json_at<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Unique temp name: a hook and a timer may write the same file at once.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let tmp = path.with_extension(format!("{}-{nanos}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_json_in_given_dir() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nested/x.json");
        write_json_at(&path, &vec![1, 2, 3]).unwrap();
        let back: Vec<i32> = read_json_at(&path).unwrap();
        assert_eq!(back, vec![1, 2, 3]);
        assert!(read_json_at::<Vec<i32>>(&temp.path().join("missing.json")).is_none());
    }
}
