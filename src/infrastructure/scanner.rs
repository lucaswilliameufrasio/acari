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
        "Docker System Prune" => estimate_docker_reclaimable(),
        "Apt Autoremove" => estimate_apt_autoremove(),
        "Journalctl Vacuum" => estimate_journalctl_usage(),
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

fn estimate_docker_reclaimable() -> (u64, u64) {
    let output = match std::process::Command::new("docker")
        .args(["system", "df", "--format", "{{.ReclaimableSize}}"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return (0, 0),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut total: u64 = 0;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line == "<unknown>" {
            continue;
        }
        total += parse_human_size(line).unwrap_or(0);
    }
    (total, 1)
}

fn estimate_apt_autoremove() -> (u64, u64) {
    let output = match std::process::Command::new("apt")
        .args(["--just-print", "autoremove"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return (0, 0),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let count = stdout.lines().filter(|l| l.starts_with("Purg")).count() as u64;
    // Rough ~10MB per package heuristic when no precise size available
    (count.saturating_mul(10_000_000), count)
}

fn estimate_journalctl_usage() -> (u64, u64) {
    let output = match std::process::Command::new("journalctl")
        .args(["--disk-usage"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return (0, 0),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let current = parse_human_usage(&stdout).unwrap_or(0);
    // Target after vacuum: 100MB
    let reclaimable = current.saturating_sub(100_000_000);
    if reclaimable > 0 { (reclaimable, 1) } else { (0, 0) }
}

fn parse_human_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let lower = s.to_lowercase();
    if lower.ends_with("gib") {
        let n = s[..s.len()-3].trim().parse::<f64>().ok()?;
        Some((n * 1_073_741_824.0) as u64)
    } else if lower.ends_with("mib") {
        let n = s[..s.len()-3].trim().parse::<f64>().ok()?;
        Some((n * 1_048_576.0) as u64)
    } else if lower.ends_with("kib") {
        let n = s[..s.len()-3].trim().parse::<f64>().ok()?;
        Some((n * 1_024.0) as u64)
    } else if lower.ends_with("gb") {
        let n = s[..s.len()-2].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000_000.0) as u64)
    } else if lower.ends_with("mb") {
        let n = s[..s.len()-2].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000.0) as u64)
    } else if lower.ends_with("kb") {
        let n = s[..s.len()-2].trim().parse::<f64>().ok()?;
        Some((n * 1_000.0) as u64)
    } else if lower.ends_with('g') {
        let n = s[..s.len()-1].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000_000.0) as u64)
    } else if lower.ends_with('m') {
        let n = s[..s.len()-1].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000.0) as u64)
    } else if lower.ends_with('k') {
        let n = s[..s.len()-1].trim().parse::<f64>().ok()?;
        Some((n * 1_000.0) as u64)
    } else if lower.ends_with('b') {
        let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u64>().ok()
    } else {
        s.parse::<u64>().ok()
    }
}

/// Parse output like "Archived and active journals use 1.2G." to bytes
fn parse_human_usage(output: &str) -> Option<u64> {
    let line = output.lines().find(|l| l.contains("use"))?;
    let word = line.split_whitespace().find(|w| {
        w.ends_with('G') || w.ends_with('M') || w.ends_with('K') || w.ends_with("GB") || w.ends_with("MB") || w.ends_with("KB")
    })?;
    parse_human_size(word)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
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

    use super::{estimate_command_target_bytes, parse_purgeable_bytes, scan_target};

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

    // --- parse_purgeable_bytes ---

    #[test]
    fn parse_purgeable_bytes_full_bytes() {
        let input = "   Purgeable:   51,234,567,890 bytes\n";
        assert_eq!(parse_purgeable_bytes(input), Some(51_234_567_890));
    }

    #[test]
    fn parse_purgeable_bytes_gb() {
        let input = "   Purgeable:   5.2 GB\n";
        assert_eq!(parse_purgeable_bytes(input), Some(5_200_000_000));
    }

    #[test]
    fn parse_purgeable_bytes_gib() {
        let input = "   Purgeable:   2.5 GiB\n";
        assert_eq!(parse_purgeable_bytes(input), Some(2_684_354_560));
    }

    #[test]
    fn parse_purgeable_bytes_bytes_without_commas() {
        let input = "   Purgeable:   12345678 bytes\n";
        assert_eq!(parse_purgeable_bytes(input), Some(12_345_678));
    }

    #[test]
    fn parse_purgeable_bytes_no_match() {
        let input = "   Something:  1234 bytes\n";
        assert_eq!(parse_purgeable_bytes(input), None);
    }

    #[test]
    fn parse_purgeable_bytes_empty() {
        assert_eq!(parse_purgeable_bytes(""), None);
    }

    // --- estimate_command_target_bytes ---

    #[test]
    fn unknown_command_target_returns_zero() {
        assert_eq!(estimate_command_target_bytes("Some Unknown Target"), (0, 0));
    }

    // --- scan_target dispatch ---

    #[test]
    fn scan_target_command_target_dispatches_and_returns_zero_on_linux() {
        let target = CleanTarget {
            name: Cow::Borrowed("Time Machine Local Snapshots"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: &["tmutil", "deletelocalsnapshots", "/"],
            delete_entire: false,
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        let result = scan_target(&target, &tx, &[], Parallelism::Serial);

        // On non-macOS, estimate returns (0, 0)
        assert_eq!(result.bytes, 0);
        assert_eq!(result.files_scanned, 0);

        // Should emit TargetCompleted event
        let event = rx.try_recv();
        assert!(event.is_ok(), "should emit TargetCompleted event");
    }
}
