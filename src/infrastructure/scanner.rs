use std::fs;
use std::io::ErrorKind;

use jwalk::WalkDir;
use tokio::sync::mpsc::UnboundedSender;

use crate::domain::{AppEvent, CleanTarget, ScanResult};

pub fn scan_target(target: &CleanTarget, tx: &UnboundedSender<AppEvent>) -> ScanResult {
    let path = target.resolved_path();

    if !path.exists() {
        return ScanResult {
            target: target.clone(),
            bytes: 0,
            files_scanned: 0,
        };
    }

    let mut total_bytes = 0_u64;
    let mut files_scanned = 0_u64;

    for entry in WalkDir::new(&path).follow_links(false) {
        let entry = match entry {
            Ok(value) => value,
            Err(err) => {
                if let Some(io_err) = err.io_error()
                    && io_err.kind() == ErrorKind::PermissionDenied
                {
                    continue;
                }
                continue;
            }
        };

        if entry.file_type().is_file() {
            let file_size = match fs::metadata(entry.path()) {
                Ok(meta) => meta.len(),
                Err(_) => 0,
            };

            total_bytes = total_bytes.saturating_add(file_size);
            files_scanned = files_scanned.saturating_add(1);

            if files_scanned.is_multiple_of(500) {
                let _ = tx.send(AppEvent::ScanProgress {
                    target_name: target.name.to_string(),
                    bytes_found: total_bytes,
                    files_scanned,
                });
            }
        }
    }

    ScanResult {
        target: target.clone(),
        bytes: total_bytes,
        files_scanned,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tokio::sync::mpsc;

    use crate::domain::CleanTarget;

    use super::scan_target;

    #[test]
    fn scans_directory_and_counts_bytes() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let p1 = temp.path().join("a.txt");
        let p2 = temp.path().join("nested").join("b.bin");
        fs::create_dir_all(p2.parent().expect("parent")).expect("create nested");
        fs::write(&p1, b"abcd").expect("write file 1");
        fs::write(&p2, b"123456").expect("write file 2");

        let leaked_path: &'static str =
            Box::leak(temp.path().to_string_lossy().into_owned().into_boxed_str());
        let target = CleanTarget {
            name: "Temp Target",
            path: leaked_path,
            description: "test",
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let result = scan_target(&target, &tx);

        assert_eq!(result.files_scanned, 2);
        assert_eq!(result.bytes, 10);
    }
}
