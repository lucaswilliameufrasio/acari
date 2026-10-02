use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use jwalk::Parallelism;
use jwalk::WalkDir;
use rayon::ThreadPool;
use tokio::sync::mpsc::UnboundedSender;

#[cfg(target_os = "macos")]
use crate::domain::expand_tilde;
use crate::domain::{AppEvent, CleanTarget, ScanResult};
use crate::infrastructure::exec;

#[cfg(any(target_os = "macos", test))]
const TIME_MACHINE_THIN_TARGET_BYTES: u64 = 10_000_000_000;

fn is_excluded(name: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|pat| name == pat)
}

/// Parallelism for a directory walk, routed through the dedicated rayon pool
/// provided by the scan. `busy_timeout: None` disables jwalk's shared-pool
/// deadlock check, which is safe here because the pool is dedicated to this scan
/// and never contended by unrelated work.
fn dedicated_walk_parallelism(pool: &Arc<ThreadPool>) -> Parallelism {
    Parallelism::RayonExistingPool {
        pool: Arc::clone(pool),
        busy_timeout: None,
    }
}

pub fn scan_target(
    target: &CleanTarget,
    tx: &UnboundedSender<AppEvent>,
    excludes: &[String],
    pool: &Arc<ThreadPool>,
    allocated: bool,
) -> ScanResult {
    let cancel = AtomicBool::new(false);
    scan_target_cancellable(target, tx, excludes, pool, allocated, &cancel)
}

pub fn scan_target_cancellable(
    target: &CleanTarget,
    tx: &UnboundedSender<AppEvent>,
    excludes: &[String],
    pool: &Arc<ThreadPool>,
    allocated: bool,
    cancel: &AtomicBool,
) -> ScanResult {
    if cancel.load(Ordering::Relaxed) {
        return ScanResult {
            target: target.clone(),
            bytes: 0,
            files_scanned: 0,
            scan_errors: 1,
        };
    }
    if target.is_command() {
        return scan_command_target(target, tx, pool);
    }

    let path = target.resolved_path();

    if std::fs::symlink_metadata(&path).is_err() {
        return ScanResult {
            target: target.clone(),
            bytes: 0,
            files_scanned: 0,
            scan_errors: 1,
        };
    }

    let mut total_bytes = 0_u64;
    let mut files_scanned = 0_u64;
    let mut scan_errors = 0_u64;

    let walker = if excludes.is_empty() {
        WalkDir::new(&path)
            .follow_links(false)
            .parallelism(dedicated_walk_parallelism(pool))
    } else {
        let ex = excludes.to_vec();
        WalkDir::new(&path)
            .follow_links(false)
            .parallelism(dedicated_walk_parallelism(pool))
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
        if cancel.load(Ordering::Relaxed) {
            scan_errors = scan_errors.saturating_add(1);
            break;
        }
        let entry = match entry {
            Ok(value) => value,
            // Skip unreadable entries (e.g. permission denied) instead of
            // failing the whole target scan; the walk is best-effort.
            Err(_) => {
                scan_errors = scan_errors.saturating_add(1);
                continue;
            }
        };

        if entry.file_type().is_file() {
            let file_size = match entry.metadata() {
                Ok(meta) => file_size_bytes(&meta, allocated),
                Err(_) => {
                    scan_errors = scan_errors.saturating_add(1);
                    continue;
                }
            };

            total_bytes = total_bytes.saturating_add(file_size);
            files_scanned = files_scanned.saturating_add(1);

            if files_scanned.is_multiple_of(500) {
                let _ = tx.send(AppEvent::ScanProgress {
                    target_name: target.name.to_string(),
                    target_path: target.resolved_path().to_string_lossy().into_owned(),
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
        scan_errors,
    }
}

/// Size of a file according to the requested mode: apparent size (`st_len`,
/// the default) or allocated on-disk blocks (`st_blocks * 512`, what `df` and
/// `du` report). On platforms without `st_blocks` both modes are apparent.
fn file_size_bytes(meta: &std::fs::Metadata, allocated: bool) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if allocated {
            return meta.blocks().saturating_mul(512);
        }
    }
    let _ = allocated;
    meta.len()
}

fn scan_command_target(
    target: &CleanTarget,
    _tx: &UnboundedSender<AppEvent>,
    pool: &Arc<ThreadPool>,
) -> ScanResult {
    let (bytes, count, scan_errors) = match estimate_command_target_bytes(&target.name, pool) {
        Some((bytes, count)) => (bytes, count, 0),
        None => (0, 0, 1),
    };

    ScanResult {
        target: target.clone(),
        bytes,
        files_scanned: count,
        scan_errors,
    }
}

fn estimate_command_target_bytes(name: &str, pool: &Arc<ThreadPool>) -> Option<(u64, u64)> {
    match name {
        "Time Machine Local Snapshots" => estimate_apfs_snapshots(),
        "Docker System Prune" => estimate_docker_reclaimable(),
        "Docker Volumes Prune" => estimate_docker_category("Local Volumes"),
        "Docker Builder Prune" => estimate_docker_builders(),
        "Apt Autoremove" => estimate_apt_autoremove(),
        "Journalctl Vacuum" => estimate_journalctl_usage(),
        "iOS Simulators Reset" => estimate_simctl_erase(pool),
        _ => None,
    }
}

fn estimate_docker_category(category: &str) -> Option<(u64, u64)> {
    let stdout =
        match exec::run_command_get_stdout(&["docker", "system", "df", "--format", "{{json .}}"]) {
            Ok(stdout) => stdout,
            Err(_) => return None,
        };
    Some((exec::parse_docker_df_json_category(&stdout, category)?, 1))
}

fn estimate_docker_builders() -> Option<(u64, u64)> {
    let builders =
        match exec::run_command_get_stdout(&["docker", "buildx", "ls", "--format", "{{.Name}}"]) {
            Ok(output) => output,
            Err(_) => return None,
        };
    let mut total = 0_u64;
    let mut count = 0_u64;
    for builder in builders
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && *name != "default")
    {
        let output =
            exec::run_command_get_stdout(&["docker", "buildx", "du", "--builder", builder]).ok()?;
        total = total.saturating_add(exec::parse_buildx_du_total_checked(&output)?);
        count = count.saturating_add(1);
    }
    Some((total, count))
}

/// Estimate the reclaimable bytes for `xcrun simctl erase all` by summing the
/// size of the local simulator devices directory.
#[cfg(target_os = "macos")]
fn estimate_simctl_erase(pool: &Arc<ThreadPool>) -> Option<(u64, u64)> {
    let path = expand_tilde("~/Library/Developer/CoreSimulator/Devices");
    if !std::fs::symlink_metadata(&path).ok()?.is_dir() {
        return None;
    }
    let mut bytes = 0_u64;
    let mut files = 0_u64;
    let walker = WalkDir::new(&path)
        .follow_links(false)
        .parallelism(dedicated_walk_parallelism(pool))
        .into_iter();
    for entry in walker {
        let entry = entry.ok()?;
        if entry.file_type().is_file() {
            bytes = bytes.saturating_add(entry.metadata().ok()?.len());
            files = files.saturating_add(1);
        }
    }
    Some((bytes, files))
}

#[cfg(not(target_os = "macos"))]
fn estimate_simctl_erase(_pool: &Arc<ThreadPool>) -> Option<(u64, u64)> {
    None
}

#[cfg(target_os = "macos")]
fn estimate_apfs_snapshots() -> Option<(u64, u64)> {
    let snap_count = match exec::run_command_get_stdout(&["tmutil", "listlocalsnapshots", "/"]) {
        Ok(stdout) => exec::parse_tmutil_list_output_checked(&stdout)?,
        Err(_) => return None,
    };

    // Cache the diskutil query per process so concurrent command-target
    // estimates never re-spawn the (relatively expensive) subprocess.
    static PURGEABLE: OnceLock<Option<u64>> = OnceLock::new();
    let purgeable = *PURGEABLE.get_or_init(|| {
        exec::run_command_get_stdout(&["diskutil", "info", "/"])
            .ok()
            .and_then(|stdout| exec::parse_diskutil_info_output(&stdout))
    });

    Some(estimate_snapshot_thinning(snap_count, purgeable))
}

#[cfg(not(target_os = "macos"))]
fn estimate_apfs_snapshots() -> Option<(u64, u64)> {
    None
}

#[cfg(any(target_os = "macos", test))]
fn estimate_snapshot_thinning(snapshot_count: u64, purgeable_bytes: Option<u64>) -> (u64, u64) {
    if snapshot_count == 0 {
        return (0, 0);
    }
    let estimate = purgeable_bytes.unwrap_or(snapshot_count.saturating_mul(5_000_000_000));
    (estimate.min(TIME_MACHINE_THIN_TARGET_BYTES), snapshot_count)
}

/// Estimate what `docker system prune -a --force` actually reclaims:
/// unused images, stopped containers and build cache. Local volumes are
/// never removed by prune, so they must not be part of the estimate.
fn estimate_docker_reclaimable() -> Option<(u64, u64)> {
    if let Ok(stdout) =
        exec::run_command_get_stdout(&["docker", "system", "df", "--format", "{{json .}}"])
        && let Some(bytes) = exec::parse_docker_df_json(&stdout)
    {
        return Some((bytes, 1));
    }
    // Keep the type in the fallback so local volumes are not counted: the
    // selected `system prune` command intentionally leaves them untouched.
    match exec::run_command_get_stdout(&[
        "docker",
        "system",
        "df",
        "--format",
        "{{.Type}}|{{.Reclaimable}}",
    ]) {
        Ok(stdout) => Some((exec::parse_docker_df_legacy_by_type_checked(&stdout)?, 1)),
        Err(_) => None,
    }
}

fn estimate_apt_autoremove() -> Option<(u64, u64)> {
    match exec::run_command_get_stdout(&["apt", "--just-print", "autoremove"]) {
        Ok(stdout) => exec::parse_apt_autoremove_output(&stdout),
        Err(_) => None,
    }
}

fn estimate_journalctl_usage() -> Option<(u64, u64)> {
    match exec::run_command_get_stdout(&["journalctl", "--disk-usage"]) {
        Ok(stdout) => {
            let current = exec::parse_journalctl_output(&stdout)?;
            let reclaimable = current.saturating_sub(100_000_000);
            if reclaimable > 0 {
                Some((reclaimable, 1))
            } else {
                Some((0, 0))
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::fs;
    use std::sync::Arc;

    use rayon::ThreadPoolBuilder;
    use tokio::sync::mpsc;

    use crate::domain::{CleanTarget, TargetOrigin};

    use super::{estimate_command_target_bytes, scan_target};

    fn test_pool() -> Arc<rayon::ThreadPool> {
        Arc::new(
            ThreadPoolBuilder::new()
                .num_threads(2)
                .build()
                .expect("build test pool"),
        )
    }

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
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let pool = test_pool();
        let result = scan_target(&target, &tx, &[], &pool, false);

        assert_eq!(result.files_scanned, 2);
        assert_eq!(result.bytes, 10);
        assert_eq!(result.scan_errors, 0);
    }

    #[test]
    fn missing_target_is_reported_as_incomplete_scan() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let missing = temp.path().join("does-not-exist");
        let target = CleanTarget {
            name: Cow::Borrowed("Missing Target"),
            path: Cow::Owned(missing.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };
        let (tx, _rx) = mpsc::unbounded_channel();
        let result = scan_target(&target, &tx, &[], &test_pool(), false);

        assert_eq!(result.bytes, 0);
        assert_eq!(result.files_scanned, 0);
        assert_eq!(result.scan_errors, 1);
    }

    #[test]
    fn pre_cancelled_target_scan_returns_incomplete_without_walking() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let target = CleanTarget {
            name: Cow::Borrowed("Cancelled Target"),
            path: Cow::Owned(temp.path().to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };
        let (tx, _rx) = mpsc::unbounded_channel();
        let cancel = std::sync::atomic::AtomicBool::new(true);
        let result =
            super::scan_target_cancellable(&target, &tx, &[], &test_pool(), false, &cancel);

        assert_eq!(result.bytes, 0);
        assert_eq!(result.scan_errors, 1);
    }

    #[test]
    fn snapshot_estimate_is_bounded_by_operation_and_available_snapshots() {
        assert_eq!(
            super::estimate_snapshot_thinning(0, Some(40_000_000_000)),
            (0, 0)
        );
        assert_eq!(
            super::estimate_snapshot_thinning(3, Some(4_000_000_000)),
            (4_000_000_000, 3)
        );
        assert_eq!(
            super::estimate_snapshot_thinning(3, Some(40_000_000_000)),
            (10_000_000_000, 3)
        );
        assert_eq!(
            super::estimate_snapshot_thinning(3, None),
            (10_000_000_000, 3)
        );
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
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let pool = test_pool();
        let result = scan_target(&target, &tx, &["node_modules".to_string()], &pool, false);

        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.bytes, 4); // "main" = 4 bytes
    }

    #[test]
    fn allocated_size_counts_blocks() {
        // A 1-byte file reports apparent size 1, while its allocated size is
        // rounded up to full 512-byte sectors (what du/df report).
        let temp = tempfile::tempdir().expect("create tempdir");
        fs::write(temp.path().join("tiny.bin"), b"x").expect("write file");

        let target = CleanTarget {
            name: Cow::Borrowed("Tiny"),
            path: Cow::Owned(temp.path().to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let pool = test_pool();
        let apparent = scan_target(&target, &tx, &[], &pool, false);
        let allocated = scan_target(&target, &tx, &[], &pool, true);

        assert_eq!(apparent.bytes, 1);
        #[cfg(unix)]
        {
            assert!(
                allocated.bytes >= 512,
                "allocated {} < 512",
                allocated.bytes
            );
            assert_eq!(allocated.bytes % 512, 0);
        }
        #[cfg(not(unix))]
        // Non-Unix platforms report apparent size in both modes.
        assert_eq!(allocated.bytes, 1);
    }

    #[test]
    fn unknown_command_target_has_no_estimate() {
        let pool = test_pool();
        assert_eq!(
            estimate_command_target_bytes("Some Unknown Target", &pool),
            None
        );
    }

    #[test]
    fn scan_target_command_target_dispatches_and_returns_zero_on_linux() {
        let target = CleanTarget {
            name: Cow::Borrowed("Time Machine Local Snapshots"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: &["tmutil", "deletelocalsnapshots", "/"],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        let pool = test_pool();
        let result = scan_target(&target, &tx, &[], &pool, false);

        // This target cannot be estimated on Linux because its platform tools
        // are unavailable; it must not be confused with a valid zero estimate.
        #[cfg(target_os = "linux")]
        {
            assert_eq!(result.bytes, 0);
            assert_eq!(result.files_scanned, 0);
            assert_eq!(result.scan_errors, 1);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = result;

        // Command targets no longer emit TargetCompleted from within scan_target;
        // that event is emitted by the caller (start_background_scan).
        let event = rx.try_recv();
        assert!(event.is_err(), "should NOT emit TargetCompleted event");
    }
}
