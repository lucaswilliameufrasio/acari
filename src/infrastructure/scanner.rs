use std::io::ErrorKind;

use jwalk::Parallelism;
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
    parallelism: Parallelism,
) -> ScanResult {
    if target.is_command() {
        return scan_command_target(target, tx);
    }

    let path = target.resolved_path();

    let mut total_bytes = 0_u64;
    let mut files_scanned = 0_u64;

    let walker = if excludes.is_empty() {
        WalkDir::new(&path)
            .follow_links(false)
            .parallelism(parallelism)
    } else {
        let ex = excludes.to_vec();
        WalkDir::new(&path)
            .follow_links(false)
            .parallelism(parallelism)
            .process_read_dir(move |_depth, _parent_path, _state, children: &mut Vec<_>| {
                children.retain(|entry| {
                    if let Ok(entry) = entry {
                        let name = entry.file_name.to_string_lossy();
                        !is_excluded(&name, &ex)
                    } else {
                        true
                    }
                });
            })
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

fn scan_command_target(
    target: &CleanTarget,
    tx: &UnboundedSender<AppEvent>,
) -> ScanResult {
    let (bytes, count) = estimate_command_target_bytes(&target.name);

    let _ = tx.send(AppEvent::TargetCompleted {
        target_name: target.name.to_string(),
        total_bytes: bytes,
        files_scanned: count,
    });

    ScanResult {
        target: target.clone(),
        bytes,
        files_scanned: count,
    }
}

fn estimate_command_target_bytes(name: &str) -> (u64, u64) {
    match name {
        "Time Machine Local Snapshots" => estimate_apfs_snapshots(),
        _ => (0, 0),
    }
}

#[cfg(target_os = "macos")]
fn estimate_apfs_snapshots() -> (u64, u64) {
    let snap_count = match std::process::Command::new("tmutil")
        .args(["listlocalsnapshots", "/"])
        .output()
    {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.lines().filter(|l| l.starts_with("localhost")).count() as u64
        }
        Err(_) => return (0, 0),
    };

    let purgeable = match std::process::Command::new("diskutil")
        .args(["info", "/"])
        .output()
    {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            parse_purgeable_bytes(&stdout)
        }
        Err(_) => None,
    };

    let bytes = purgeable.unwrap_or(snap_count.saturating_mul(5_000_000_000));
    (bytes, snap_count)
}

#[cfg(not(target_os = "macos"))]
fn estimate_apfs_snapshots() -> (u64, u64) {
    (0, 0)
}

#[cfg(target_os = "macos")]
fn parse_purgeable_bytes(output: &str) -> Option<u64> {
    for line in output.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Purgeable:") {
            let val = val.trim();
            if let Some(bytes_str) = val.strip_suffix("bytes") {
                let cleaned: String = bytes_str.chars().filter(|c| c.is_ascii_digit()).collect();
                return cleaned.parse::<u64>().ok();
            }
            if let Some(gb) = val.strip_suffix("GB") {
                if let Ok(num) = gb.trim().parse::<f64>() {
                    return Some((num * 1_000_000_000.0) as u64);
                }
            }
            if let Some(gib) = val.strip_suffix("GiB") {
                if let Ok(num) = gib.trim().parse::<f64>() {
                    return Some((num * 1_073_741_824.0) as u64);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::fs;

    use jwalk::Parallelism;
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
            command: &[],
            delete_entire: false,
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let result = scan_target(&target, &tx, &[], Parallelism::Serial);

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
            command: &[],
            delete_entire: false,
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let result = scan_target(
            &target,
            &tx,
            &["node_modules".to_string()],
            Parallelism::Serial,
        );

        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.bytes, 4); // "main" = 4 bytes
    }
}
