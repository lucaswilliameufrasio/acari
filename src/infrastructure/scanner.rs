use std::io::ErrorKind;

use jwalk::WalkDir;
use tokio::sync::mpsc::UnboundedSender;

use crate::domain::{AppEvent, CleanTarget, ScanResult};

fn is_excluded(name: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|pat| name == pat)
}

pub fn scan_target(
    target: &CleanTarget,
    tx: &UnboundedSender<AppEvent>,
    excludes: &[String],
) -> ScanResult {
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

    let walker = if excludes.is_empty() {
        WalkDir::new(&path).follow_links(false)
    } else {
        let ex = excludes.to_vec();
        WalkDir::new(&path).follow_links(false).process_read_dir(
            move |_depth, _parent_path, _state, children: &mut Vec<_>| {
                children.retain(|entry| {
                    if let Ok(entry) = entry {
                        let name = entry.file_name.to_string_lossy();
                        !is_excluded(&name, &ex)
                    } else {
                        true
                    }
                });
            },
        )
    };

    for entry in walker {
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
            let file_size = entry.metadata().map_or(0, |meta| meta.len());

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
    use std::borrow::Cow;
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

        let target = CleanTarget {
            name: Cow::Borrowed("Temp Target"),
            path: Cow::Owned(temp.path().to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let result = scan_target(&target, &tx, &[]);

        assert_eq!(result.files_scanned, 2);
        assert_eq!(result.bytes, 10);
    }

    #[test]
    fn excludes_filter_out_entries() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let nested = temp.path().join("node_modules");
        fs::create_dir_all(&nested).expect("create node_modules");
        fs::write(nested.join("dep.js"), b"xxx").expect("write dep");
        fs::write(temp.path().join("main.js"), b"main").expect("write main");

        let target = CleanTarget {
            name: Cow::Borrowed("With Node"),
            path: Cow::Owned(temp.path().to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let result = scan_target(&target, &tx, &["node_modules".to_string()]);

        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.bytes, 4); // "main" = 4 bytes
    }
}
