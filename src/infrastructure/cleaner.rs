use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use crate::application::cleaner::CleanMode;
use crate::domain::{CleanResult, CleanTarget};
use crate::infrastructure::exec;

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const DOCKER_VOLUME_PRUNE_TARGET: &str = "Docker Volumes Prune";

fn command_timeout() -> Duration {
    std::env::var("ACARI_COMMAND_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT)
}

fn remove_entry_with_progress(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> Option<(u64, u64)> {
    let metadata = fs::symlink_metadata(path).ok()?;
    let is_sym = metadata.file_type().is_symlink();

    if is_sym || metadata.is_file() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        let size = if is_sym { 0 } else { metadata.len() };
        fs::remove_file(path).ok().map(|_| (size, 1))
    } else if metadata.is_dir() {
        remove_directory_contents(path, true, progress, cancel)
    } else {
        None
    }
}

fn remove_entry_contents_with_progress(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> Option<(u64, u64)> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        return remove_entry_with_progress(path, progress, cancel);
    }
    if metadata.is_dir() {
        return remove_directory_contents(path, false, progress, cancel);
    }
    None
}

fn remove_directory_contents(
    path: &Path,
    remove_root: bool,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> Option<(u64, u64)> {
    let mut stack = vec![(path.to_path_buf(), false)];
    let mut reclaimed = 0_u64;
    let mut removed_entries = 0_u64;
    while let Some((current, is_post_order)) = stack.pop() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        if is_post_order {
            if remove_root || current != path {
                fs::remove_dir(&current).ok()?;
            }
            continue;
        }

        stack.push((current.clone(), true));
        for entry in fs::read_dir(&current).ok()? {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            let entry = entry.ok()?;
            let child = entry.path();
            let child_metadata = fs::symlink_metadata(&child).ok()?;
            if child_metadata.file_type().is_symlink() || child_metadata.is_file() {
                let size = if child_metadata.is_file() {
                    child_metadata.len()
                } else {
                    0
                };
                fs::remove_file(&child).ok()?;
                reclaimed = reclaimed.saturating_add(size);
                removed_entries = removed_entries.saturating_add(1);
                progress(0);
            } else if child_metadata.is_dir() {
                stack.push((child, false));
            }
        }
    }
    Some((reclaimed, removed_entries))
}

#[cfg(target_os = "macos")]
fn force_remove_with_progress(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> Option<(u64, u64)> {
    remove_entry_with_progress(path, progress, cancel).or_else(|| {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        let _ = std::process::Command::new("chflags")
            .arg("nouchg")
            .arg(path)
            .output();
        remove_entry_with_progress(path, progress, cancel)
    })
}

#[cfg(target_os = "macos")]
fn force_remove_contents_with_progress(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> Option<(u64, u64)> {
    remove_entry_contents_with_progress(path, progress, cancel).or_else(|| {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        let _ = std::process::Command::new("chflags")
            .arg("-R")
            .arg("nouchg")
            .arg(path)
            .output();
        remove_entry_contents_with_progress(path, progress, cancel)
    })
}

#[cfg(not(target_os = "macos"))]
fn force_remove_contents_with_progress(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> Option<(u64, u64)> {
    remove_entry_contents_with_progress(path, progress, cancel)
}

#[cfg(not(target_os = "macos"))]
fn force_remove_with_progress(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> Option<(u64, u64)> {
    remove_entry_with_progress(path, progress, cancel)
}

pub fn clean_target(
    target: &CleanTarget,
    estimated_bytes: u64,
    estimated_entries: u64,
    mode: CleanMode,
) -> CleanResult {
    clean_target_with_progress(
        target,
        estimated_bytes,
        estimated_entries,
        mode,
        &mut |_| {},
        &Arc::new(AtomicBool::new(false)),
    )
}

pub fn clean_target_with_progress(
    target: &CleanTarget,
    estimated_bytes: u64,
    estimated_entries: u64,
    mode: CleanMode,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> CleanResult {
    if target.is_command() {
        if is_builtin_docker_builder_prune(target) {
            return clean_docker_builder_prune(
                target,
                estimated_bytes,
                estimated_entries,
                mode,
                progress,
                cancel,
                std::ffi::OsStr::new("docker"),
            );
        }
        if is_builtin_ios_simulator_reset(target) {
            return clean_ios_simulator_reset(
                target,
                estimated_bytes,
                estimated_entries,
                mode,
                progress,
                cancel,
                std::ffi::OsStr::new("xcrun"),
            );
        }
        return clean_command_target(
            target,
            estimated_bytes,
            estimated_entries,
            mode,
            progress,
            cancel,
        );
    }

    let raw_path = target.resolved_path();

    // Missing targets are not an error: report zero work done.
    let raw_exists = fs::symlink_metadata(&raw_path).is_ok();
    if !raw_exists {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: 0,
            removed_entries: 0,
            errors: 0,
            error_detail: None,
        };
    }

    let (path, target_is_symlink, target_identity) = match canonicalize_cleanup_target(&raw_path) {
        Some(resolved) => resolved,
        None => {
            return CleanResult {
                target: target.clone(),
                reclaimed_bytes: 0,
                removed_entries: 0,
                errors: 1,
                error_detail: Some("target path could not be safely resolved".into()),
            };
        }
    };

    if mode == CleanMode::DryRun {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: estimated_bytes,
            removed_entries: estimated_entries,
            errors: 0,
            error_detail: None,
        };
    }

    // The confirmation and preview happen before execution. Refuse to clean if
    // the selected entry was replaced between resolution and this check.
    if !cleanup_target_identity_matches(&path, target_is_symlink, target_identity) {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: 0,
            removed_entries: 0,
            errors: 1,
            error_detail: Some("target changed before cleanup; scan and confirm it again".into()),
        };
    }

    if target.delete_entire {
        if target_is_symlink {
            return CleanResult {
                target: target.clone(),
                reclaimed_bytes: 0,
                removed_entries: 0,
                errors: 1,
                error_detail: Some("refusing to remove an entire symlink target".into()),
            };
        }
        let outcome = force_remove_with_progress(&path, progress, cancel);
        let ok = outcome.is_some();
        let (reclaimed_bytes, removed_entries) = outcome.unwrap_or_default();
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes,
            removed_entries: if ok { removed_entries } else { 0 },
            errors: if ok { 0 } else { 1 },
            error_detail: (!ok)
                .then(|| "one or more filesystem entries could not be removed".into()),
        };
    }

    let mut removed_entries = 0_u64;
    let mut errors = 0_u64;
    let mut reclaimed_bytes = 0_u64;

    if target_is_symlink || path.is_file() {
        match force_remove_with_progress(&path, progress, cancel) {
            Some((freed, removed)) => {
                removed_entries = removed;
                reclaimed_bytes = freed;
            }
            None => {
                errors = 1;
            }
        }
    } else if path.is_dir() {
        match force_remove_contents_with_progress(&path, progress, cancel) {
            Some((freed, removed)) => {
                reclaimed_bytes = freed;
                removed_entries = removed;
            }
            None => {
                errors = 1;
                reclaimed_bytes = 0;
                removed_entries = 0;
            }
        }
    }

    CleanResult {
        target: target.clone(),
        reclaimed_bytes,
        removed_entries,
        errors,
        error_detail: (errors > 0)
            .then(|| "one or more filesystem entries could not be removed".into()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CleanupTargetIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    is_dir: bool,
    is_file: bool,
    is_symlink: bool,
}

fn cleanup_target_identity(path: &Path) -> Option<CleanupTargetIdentity> {
    let metadata = fs::symlink_metadata(path).ok()?;
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Some(CleanupTargetIdentity {
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        is_dir: metadata.is_dir(),
        is_file: metadata.is_file(),
        is_symlink: metadata.file_type().is_symlink(),
    })
}

fn cleanup_target_identity_matches(
    path: &Path,
    target_is_symlink: bool,
    expected: CleanupTargetIdentity,
) -> bool {
    cleanup_target_identity(path)
        .is_some_and(|actual| actual == expected && actual.is_symlink == target_is_symlink)
}

fn canonicalize_cleanup_target(raw_path: &Path) -> Option<(PathBuf, bool, CleanupTargetIdentity)> {
    let metadata = fs::symlink_metadata(raw_path).ok()?;
    let identity = cleanup_target_identity(raw_path)?;
    if metadata.file_type().is_symlink() {
        // A symlink target itself is a valid cleanup entry: remove the link,
        // never follow it to its destination.
        return Some((raw_path.to_path_buf(), true, identity));
    }
    let canonical = fs::canonicalize(raw_path).ok()?;
    let canonical_identity = cleanup_target_identity(&canonical)?;
    if canonical_identity != identity {
        return None;
    }
    Some((canonical, false, canonical_identity))
}

fn is_builtin_ios_simulator_reset(target: &CleanTarget) -> bool {
    target.origin == crate::domain::TargetOrigin::Builtin
        && target.name == "iOS Simulators Reset"
        && target.command
            == [
                "sh",
                "-c",
                "xcrun simctl shutdown all 2>/dev/null; xcrun simctl erase all",
            ]
}

fn clean_ios_simulator_reset(
    target: &CleanTarget,
    estimated_bytes: u64,
    estimated_entries: u64,
    mode: CleanMode,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
    xcrun_executable: &std::ffi::OsStr,
) -> CleanResult {
    if mode == CleanMode::DryRun {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: estimated_bytes,
            removed_entries: estimated_entries,
            errors: 0,
            error_detail: None,
        };
    }
    let mut shutdown = std::process::Command::new(xcrun_executable);
    shutdown.args(["simctl", "shutdown", "all"]);
    let shutdown_output = match run_cancellable_command(shutdown, cancel, progress) {
        Ok(output) => output,
        Err(error) => return command_error(target, error),
    };
    if !shutdown_output.status.success() {
        return command_failure(
            target,
            "xcrun simctl shutdown all",
            shutdown_output.status,
            &shutdown_output.stderr,
        );
    }
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return command_error(target, "simulator reset was cancelled".into());
    }

    let mut erase = std::process::Command::new(xcrun_executable);
    erase.args(["simctl", "erase", "all"]);
    match run_cancellable_command(erase, cancel, progress) {
        Ok(output) if output.status.success() => CleanResult {
            target: target.clone(),
            reclaimed_bytes: estimated_bytes,
            removed_entries: estimated_entries,
            errors: 0,
            error_detail: None,
        },
        Ok(output) => command_failure(
            target,
            "xcrun simctl erase all",
            output.status,
            &output.stderr,
        ),
        Err(error) => command_error(target, error),
    }
}

fn run_cancellable_command(
    mut command: std::process::Command,
    cancel: &Arc<AtomicBool>,
    progress: &mut dyn FnMut(u64),
) -> Result<std::process::Output, String> {
    use std::io::Read;
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start command: {error}"))?;
    let stdout = child.stdout.take().map(|mut stream| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stream.read_to_end(&mut bytes);
            bytes
        })
    });
    let stderr = child.stderr.take().map(|mut stream| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stream.read_to_end(&mut bytes);
            bytes
        })
    });
    let started = Instant::now();
    let status = loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("command was cancelled".into());
        }
        if started.elapsed() >= command_timeout() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("command timed out".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                progress(started.elapsed().as_secs());
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("error waiting for command: {error}"));
            }
        }
    };
    let stdout = stdout
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let stderr = stderr
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn is_builtin_docker_builder_prune(target: &CleanTarget) -> bool {
    target.origin == crate::domain::TargetOrigin::Builtin
        && target.name == "Docker Builder Prune"
        && target.command
            == [
                "sh",
                "-c",
                "docker buildx ls --format '{{.Name}}' | while IFS= read -r builder; do [ -z \"$builder\" ] || [ \"$builder\" = default ] || docker buildx prune -a -f --builder \"$builder\" || exit; done",
            ]
}

fn clean_docker_builder_prune(
    target: &CleanTarget,
    estimated_bytes: u64,
    estimated_entries: u64,
    mode: CleanMode,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
    docker_executable: &std::ffi::OsStr,
) -> CleanResult {
    if mode == CleanMode::DryRun {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: estimated_bytes,
            removed_entries: estimated_entries,
            errors: 0,
            error_detail: None,
        };
    }
    let started = Instant::now();
    let builders = match std::process::Command::new(docker_executable)
        .args(["buildx", "ls", "--format", "{{.Name}}"])
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        Ok(output) => {
            return command_failure(target, "docker buildx ls", output.status, &output.stderr);
        }
        Err(error) => {
            return command_error(target, format!("failed to run docker buildx ls: {error}"));
        }
    };

    let mut reclaimed_bytes = 0_u64;
    let mut cleaned_builders = 0_u64;
    for builder in builders
        .lines()
        .map(str::trim)
        .filter(|builder| !builder.is_empty() && *builder != "default")
    {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return CleanResult {
                target: target.clone(),
                reclaimed_bytes,
                removed_entries: cleaned_builders,
                errors: 1,
                error_detail: Some("Docker builder cleanup was cancelled".into()),
            };
        }
        let mut child = match std::process::Command::new(docker_executable)
            .args(["buildx", "prune", "-a", "-f", "--builder"])
            .arg(builder)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return command_error_with_partial(
                    target,
                    format!("failed to run docker buildx prune: {error}"),
                    reclaimed_bytes,
                    cleaned_builders,
                );
            }
        };
        let stdout_reader = child.stdout.take().map(|mut stream| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut output = String::new();
                let _ = stream.read_to_string(&mut output);
                output
            })
        });
        let stderr_reader = child.stderr.take().map(|mut stream| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut output = String::new();
                let _ = stream.read_to_string(&mut output);
                output
            })
        });
        let status = loop {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return CleanResult {
                    target: target.clone(),
                    reclaimed_bytes,
                    removed_entries: cleaned_builders,
                    errors: 1,
                    error_detail: Some("Docker builder cleanup was cancelled".into()),
                };
            }
            if started.elapsed() >= command_timeout() {
                let _ = child.kill();
                let _ = child.wait();
                return CleanResult {
                    target: target.clone(),
                    reclaimed_bytes,
                    removed_entries: cleaned_builders,
                    errors: 1,
                    error_detail: Some("Docker builder cleanup timed out".into()),
                };
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    progress(started.elapsed().as_secs());
                    std::thread::sleep(Duration::from_millis(250));
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return command_error_with_partial(
                        target,
                        format!("error waiting for docker buildx prune: {error}"),
                        reclaimed_bytes,
                        cleaned_builders,
                    );
                }
            }
        };
        if !status.success() {
            let stderr = stderr_reader
                .and_then(|reader| reader.join().ok())
                .unwrap_or_default();
            return command_failure_with_partial(
                target,
                "docker buildx prune",
                status,
                stderr.as_bytes(),
                reclaimed_bytes,
                cleaned_builders,
            );
        }
        let stdout = stdout_reader
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        let _ = stderr_reader.and_then(|reader| reader.join().ok());
        let Some(builder_reclaimed_bytes) = exec::parse_total_reclaimed_space(&stdout) else {
            return command_error_with_partial(
                target,
                "docker buildx prune completed for a builder, but its reclaimed-space summary could not be read".into(),
                reclaimed_bytes,
                cleaned_builders.saturating_add(1),
            );
        };
        reclaimed_bytes = reclaimed_bytes.saturating_add(builder_reclaimed_bytes);
        cleaned_builders = cleaned_builders.saturating_add(1);
    }

    CleanResult {
        target: target.clone(),
        reclaimed_bytes,
        removed_entries: cleaned_builders,
        errors: 0,
        error_detail: None,
    }
}

fn command_error(target: &CleanTarget, error_detail: String) -> CleanResult {
    CleanResult {
        target: target.clone(),
        reclaimed_bytes: 0,
        removed_entries: 0,
        errors: 1,
        error_detail: Some(error_detail),
    }
}

fn command_error_with_partial(
    target: &CleanTarget,
    error_detail: String,
    reclaimed_bytes: u64,
    removed_entries: u64,
) -> CleanResult {
    CleanResult {
        target: target.clone(),
        reclaimed_bytes,
        removed_entries,
        errors: 1,
        error_detail: Some(error_detail),
    }
}

fn command_failure(
    target: &CleanTarget,
    command: &str,
    status: std::process::ExitStatus,
    stderr: &[u8],
) -> CleanResult {
    let details = String::from_utf8_lossy(stderr).trim().to_string();
    command_error(
        target,
        if details.is_empty() {
            format!("{command} failed ({status})")
        } else {
            format!("{command} failed: {details}")
        },
    )
}

fn command_failure_with_partial(
    target: &CleanTarget,
    command: &str,
    status: std::process::ExitStatus,
    stderr: &[u8],
    reclaimed_bytes: u64,
    removed_entries: u64,
) -> CleanResult {
    let mut result = command_failure(target, command, status, stderr);
    result.reclaimed_bytes = reclaimed_bytes;
    result.removed_entries = removed_entries;
    result
}

fn clean_command_target(
    target: &CleanTarget,
    estimated_bytes: u64,
    estimated_entries: u64,
    mode: CleanMode,
    progress: &mut dyn FnMut(u64),
    cancel: &Arc<AtomicBool>,
) -> CleanResult {
    if mode == CleanMode::DryRun {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: estimated_bytes,
            removed_entries: estimated_entries,
            errors: 0,
            error_detail: None,
        };
    }

    let cmd = target.command;
    if cmd.is_empty() {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: 0,
            removed_entries: 0,
            errors: 1,
            error_detail: Some("command target has no command configured".into()),
        };
    }

    let captures_docker_volume_prune_output = target.name == DOCKER_VOLUME_PRUNE_TARGET;
    let mut child = match std::process::Command::new(cmd[0])
        .args(&cmd[1..])
        .stdin(if target.requires_sudo {
            std::process::Stdio::inherit()
        } else {
            std::process::Stdio::null()
        })
        .stdout(if target.requires_sudo {
            std::process::Stdio::inherit()
        } else if captures_docker_volume_prune_output {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stderr(if target.requires_sudo {
            std::process::Stdio::inherit()
        } else {
            std::process::Stdio::piped()
        })
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return CleanResult {
                target: target.clone(),
                reclaimed_bytes: 0,
                removed_entries: 0,
                errors: 1,
                error_detail: Some(format!("failed to run {}: {e}", cmd[0])),
            };
        }
    };
    let stdout = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            crate::infrastructure::exec::parse_docker_volume_prune_output(std::io::BufReader::new(
                stdout,
            ))
        })
    });
    let stderr = child.stderr.take().map(|mut stderr| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut output = String::new();
            let _ = stderr.read_to_string(&mut output);
            output
        })
    });

    let started = Instant::now();
    let status = loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return CleanResult {
                target: target.clone(),
                reclaimed_bytes: 0,
                removed_entries: 0,
                errors: 1,
                error_detail: Some("command was cancelled".into()),
            };
        }
        if started.elapsed() >= command_timeout() {
            let timeout_seconds = command_timeout().as_secs();
            let _ = child.kill();
            let _ = child.wait();
            return CleanResult {
                target: target.clone(),
                reclaimed_bytes: 0,
                removed_entries: 0,
                errors: 1,
                error_detail: Some(format!(
                    "command timed out after {timeout_seconds}s: {}",
                    cmd[0]
                )),
            };
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                progress(started.elapsed().as_secs());
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(e) => {
                return CleanResult {
                    target: target.clone(),
                    reclaimed_bytes: 0,
                    removed_entries: 0,
                    errors: 1,
                    error_detail: Some(format!("error waiting for {}: {e}", cmd[0])),
                };
            }
        }
    };

    if status.success() {
        let docker_prune_result = captures_docker_volume_prune_output
            .then(|| stdout.and_then(|handle| handle.join().ok()).flatten());
        let (reclaimed_bytes, removed_entries, errors, error_detail) = match docker_prune_result {
            Some(Some((reclaimed_bytes, removed_entries))) => {
                (reclaimed_bytes, removed_entries, 0, None)
            }
            Some(None) => (
                0,
                0,
                1,
                Some(String::from(
                    "docker volume prune completed, but its reclaimed-space summary could not be read",
                )),
            ),
            None => (estimated_bytes, estimated_entries, 0, None),
        };
        CleanResult {
            target: target.clone(),
            reclaimed_bytes,
            removed_entries,
            errors,
            error_detail,
        }
    } else {
        let details = stderr
            .and_then(|handle| handle.join().ok())
            .map(|output| output.trim_end().to_string())
            .filter(|output| !output.trim().is_empty())
            .unwrap_or_default();
        let error_detail = if details.is_empty() {
            format!("command failed: {} ({status})", cmd[0])
        } else {
            format!("command failed: {details}")
        };
        CleanResult {
            target: target.clone(),
            reclaimed_bytes: 0,
            removed_entries: 0,
            errors: 1,
            error_detail: Some(error_detail),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::fs;

    use crate::domain::{CleanTarget, TargetOrigin};

    use super::{
        canonicalize_cleanup_target, clean_docker_builder_prune, clean_ios_simulator_reset,
        clean_target, clean_target_with_progress, cleanup_target_identity_matches,
    };
    use crate::application::cleaner::CleanMode;

    #[cfg(unix)]
    fn command_with_stderr_failure() -> &'static [&'static str] {
        &["sh", "-c", "printf 'docker daemon failed' >&2; exit 7"]
    }

    #[cfg(windows)]
    fn command_with_stderr_failure() -> &'static [&'static str] {
        &["cmd", "/C", "echo docker daemon failed 1>&2 & exit /B 7"]
    }

    #[cfg(unix)]
    fn command_without_stderr_failure() -> &'static [&'static str] {
        &["sh", "-c", "exit 7"]
    }

    #[cfg(windows)]
    fn command_without_stderr_failure() -> &'static [&'static str] {
        &["cmd", "/C", "exit 7"]
    }

    #[cfg(unix)]
    fn successful_interactive_command() -> &'static [&'static str] {
        &["sh", "-c", "exit 0"]
    }

    #[cfg(windows)]
    fn successful_interactive_command() -> &'static [&'static str] {
        &["cmd", "/C", "exit 0"]
    }

    #[cfg(unix)]
    fn docker_volume_prune_command() -> &'static [&'static str] {
        &[
            "sh",
            "-c",
            "printf 'Deleted Volumes:\\nvolume-one\\nvolume-two\\nTotal reclaimed space: 12.5GB\\n'",
        ]
    }

    #[cfg(windows)]
    fn docker_volume_prune_command() -> &'static [&'static str] {
        &[
            "cmd",
            "/C",
            "echo Deleted Volumes: & echo volume-one & echo volume-two & echo Total reclaimed space: 12.5GB",
        ]
    }

    #[cfg(unix)]
    fn docker_volume_prune_noop_command() -> &'static [&'static str] {
        &["sh", "-c", "printf 'Total reclaimed space: 0B\\n'"]
    }

    #[cfg(windows)]
    fn docker_volume_prune_noop_command() -> &'static [&'static str] {
        &["cmd", "/C", "echo Total reclaimed space: 0B"]
    }

    #[cfg(unix)]
    fn docker_volume_prune_unparseable_command() -> &'static [&'static str] {
        &["sh", "-c", "printf 'prune finished without a summary\\n'"]
    }

    #[cfg(windows)]
    fn docker_volume_prune_unparseable_command() -> &'static [&'static str] {
        &["cmd", "/C", "echo prune finished without a summary"]
    }

    #[cfg(unix)]
    fn expected_silent_failure_detail() -> &'static str {
        "command failed: sh (exit status: 7)"
    }

    #[cfg(windows)]
    fn expected_silent_failure_detail() -> &'static str {
        "command failed: cmd (exit code: 7)"
    }

    #[test]
    fn nonexistent_target_returns_zero_errors() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let missing = temp.path().join("does-not-exist");

        let target = CleanTarget {
            name: Cow::Borrowed("Missing Cache"),
            path: Cow::Owned(missing.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 0, 0, CleanMode::Execute);
        assert_eq!(result.errors, 0, "missing path must not count as error");
        assert_eq!(result.reclaimed_bytes, 0);
        assert_eq!(result.removed_entries, 0);
    }

    #[test]
    fn cleans_directory_contents() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create nested");
        fs::write(root.join("a.txt"), b"abc").expect("write file 1");
        fs::write(nested.join("b.txt"), b"defgh").expect("write file 2");

        let target = CleanTarget {
            name: Cow::Borrowed("Temp Cache"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 8, 2, CleanMode::Execute);
        assert_eq!(result.errors, 0);
        assert!(result.removed_entries >= 1);
        assert_eq!(result.reclaimed_bytes, 8);

        let remaining = fs::read_dir(&root).expect("read root").count();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn dry_run_does_not_remove_contents() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("a.txt"), b"abc").expect("write file");

        let target = CleanTarget {
            name: Cow::Borrowed("Temp Cache"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 3, 1, CleanMode::DryRun);
        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 3);
        assert_eq!(result.removed_entries, 1);

        let remaining = fs::read_dir(&root).expect("read root").count();
        assert_eq!(remaining, 1);
    }

    #[cfg(unix)]
    #[test]
    fn reports_permission_errors_on_read_only_directory() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("a.txt"), b"abc").expect("write file");

        let mut perms = fs::metadata(&root).expect("metadata").permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&root, perms).expect("set readonly perms");

        let target = CleanTarget {
            name: Cow::Borrowed("Readonly Cache"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 3, 1, CleanMode::Execute);
        assert!(result.errors > 0);

        let mut perms = fs::metadata(&root).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&root, perms).expect("restore perms");
    }

    #[test]
    fn removes_broken_symlink() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        fs::create_dir_all(&root).expect("create root");

        #[cfg(unix)]
        {
            let dangling = root.join("gone.lnk");
            std::os::unix::fs::symlink("/nonexistent-target", &dangling)
                .expect("create dangling symlink");

            let target = CleanTarget {
                name: Cow::Borrowed("Broken Link"),
                path: Cow::Owned(root.to_string_lossy().into_owned()),
                description: Cow::Borrowed("test"),
                command: &[],
                requires_sudo: false,
                dangerous: false,
                delete_entire: false,
                origin: TargetOrigin::Builtin,
            };

            let result = clean_target(&target, 1, 1, CleanMode::Execute);
            assert_eq!(
                result.errors, 0,
                "broken symlink should be removed without errors"
            );
            assert_eq!(result.removed_entries, 1);
            assert!(!dangling.exists(), "symlink should be removed");
        }
    }

    #[test]
    fn delete_entire_removes_directory() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("junk");
        fs::create_dir_all(root.join("deep").join("nested")).expect("create nested");
        fs::write(root.join("file.txt"), b"data").expect("write file");

        let target = CleanTarget {
            name: Cow::Borrowed("Junk"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: true,
            origin: TargetOrigin::Builtin,
        };

        // Deliberately stale scan counts must not be echoed as actual removals.
        let result = clean_target(&target, 4, 99, CleanMode::Execute);
        assert_eq!(result.errors, 0, "delete_entire should succeed");
        assert_eq!(result.reclaimed_bytes, 4, "should report estimated bytes");
        assert_eq!(result.removed_entries, 1);
        assert!(!root.exists(), "entire dir should be removed");
    }

    #[test]
    fn delete_entire_dry_run_reports_estimate() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("junk");
        fs::create_dir_all(&root).expect("create root");

        let target = CleanTarget {
            name: Cow::Borrowed("Junk"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: true,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 100, 5, CleanMode::DryRun);
        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 100);
        assert_eq!(result.removed_entries, 5);
        assert!(root.exists(), "should not delete in dry run");
    }

    // --- command target tests ---

    #[cfg(unix)]
    #[test]
    fn builder_prune_uses_argument_vector_and_skips_default_builder() {
        use std::sync::{Arc, atomic::AtomicBool};

        let directory = tempfile::tempdir().unwrap();
        let docker = directory.path().join("docker");
        let arguments_log = directory.path().join("arguments.log");
        let injection_marker = directory.path().join("injection-marker");
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = buildx ] && [ \"$2\" = ls ]; then printf '%s\\n' default 'builder; touch {marker}'; exit 0; fi\nif [ \"$1\" = buildx ] && [ \"$2\" = prune ]; then printf '<%s>\\n' \"$@\" >> '{log}'; echo 'Total reclaimed space: 1GB'; exit 0; fi\nexit 9\n",
            marker = injection_marker.display(),
            log = arguments_log.display(),
        );
        fs::write(&docker, script).unwrap();
        let mut permissions = fs::metadata(&docker).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&docker, permissions).unwrap();

        let target =
            crate::domain::targets::build_targets(&["Docker Builder Prune".to_string()], &[])
                .pop()
                .expect("Docker builder target");
        let result = clean_docker_builder_prune(
            &target,
            0,
            0,
            CleanMode::Execute,
            &mut |_| {},
            &Arc::new(AtomicBool::new(false)),
            docker.as_os_str(),
        );

        assert_eq!(result.errors, 0, "{:?}", result.error_detail);
        assert_eq!(result.reclaimed_bytes, 1_000_000_000);
        assert_eq!(result.removed_entries, 1);
        assert!(
            !injection_marker.exists(),
            "builder name must never run as shell code"
        );
        let arguments = fs::read_to_string(arguments_log).unwrap();
        assert!(arguments.contains("<--builder>\n"));
        assert!(arguments.contains(&format!(
            "<builder; touch {}>\n",
            injection_marker.display()
        )));
        assert!(!arguments.contains("<default>"));
    }

    #[cfg(unix)]
    #[test]
    fn builder_prune_failure_preserves_completed_builder_totals() {
        use std::sync::{Arc, atomic::AtomicBool};

        let directory = tempfile::tempdir().unwrap();
        let docker = directory.path().join("docker");
        let script = "#!/bin/sh\nif [ \"$1\" = buildx ] && [ \"$2\" = ls ]; then printf '%s\\n' default first second; exit 0; fi\nif [ \"$1\" = buildx ] && [ \"$2\" = prune ]; then if [ \"$6\" = first ]; then echo 'Total reclaimed space: 1GB'; exit 0; fi; echo 'second builder failed' >&2; exit 7; fi\nexit 9\n";
        fs::write(&docker, script).unwrap();
        let mut permissions = fs::metadata(&docker).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&docker, permissions).unwrap();
        let target =
            crate::domain::targets::build_targets(&["Docker Builder Prune".to_string()], &[])
                .pop()
                .expect("Docker builder target");

        let result = clean_docker_builder_prune(
            &target,
            0,
            0,
            CleanMode::Execute,
            &mut |_| {},
            &Arc::new(AtomicBool::new(false)),
            docker.as_os_str(),
        );

        assert_eq!(result.errors, 1);
        assert_eq!(result.reclaimed_bytes, 1_000_000_000);
        assert_eq!(result.removed_entries, 1);
        assert!(
            result
                .error_detail
                .unwrap()
                .contains("second builder failed")
        );
    }

    #[cfg(unix)]
    #[test]
    fn builder_prune_unreadable_reclaim_summary_is_not_reported_as_zero_success() {
        use std::sync::{Arc, atomic::AtomicBool};

        let directory = tempfile::tempdir().unwrap();
        let docker = directory.path().join("docker");
        let script = "#!/bin/sh\nif [ \"$1\" = buildx ] && [ \"$2\" = ls ]; then printf '%s\\n' first second; exit 0; fi\nif [ \"$1\" = buildx ] && [ \"$2\" = prune ]; then if [ \"$6\" = first ]; then echo 'Total reclaimed space: 1GB'; else echo 'unexpected successful output'; fi; exit 0; fi\nexit 9\n";
        fs::write(&docker, script).unwrap();
        let mut permissions = fs::metadata(&docker).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&docker, permissions).unwrap();
        let target =
            crate::domain::targets::build_targets(&["Docker Builder Prune".to_string()], &[])
                .pop()
                .expect("Docker builder target");

        let result = clean_docker_builder_prune(
            &target,
            0,
            0,
            CleanMode::Execute,
            &mut |_| {},
            &Arc::new(AtomicBool::new(false)),
            docker.as_os_str(),
        );

        assert_eq!(result.errors, 1);
        assert_eq!(result.reclaimed_bytes, 1_000_000_000);
        assert_eq!(result.removed_entries, 2);
        assert!(
            result
                .error_detail
                .unwrap()
                .contains("summary could not be read")
        );
    }

    #[cfg(unix)]
    #[test]
    fn simulator_reset_dry_run_is_read_only_and_execution_uses_fixed_arguments() {
        use std::sync::{Arc, atomic::AtomicBool};

        let directory = tempfile::tempdir().unwrap();
        let xcrun = directory.path().join("xcrun");
        let arguments_log = directory.path().join("xcrun-arguments.log");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}';\nif [ \"$2\" = erase ]; then echo 'simulator reset complete'; fi\n",
            arguments_log.display()
        );
        fs::write(&xcrun, script).unwrap();
        let mut permissions = fs::metadata(&xcrun).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&xcrun, permissions).unwrap();
        let target = CleanTarget {
            name: Cow::Borrowed("iOS Simulators Reset"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: &[
                "sh",
                "-c",
                "xcrun simctl shutdown all 2>/dev/null; xcrun simctl erase all",
            ],
            requires_sudo: false,
            dangerous: true,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };
        let cancellation = Arc::new(AtomicBool::new(false));

        let preview = clean_ios_simulator_reset(
            &target,
            1234,
            9,
            CleanMode::DryRun,
            &mut |_| {},
            &cancellation,
            xcrun.as_os_str(),
        );
        assert_eq!(preview.errors, 0);
        assert_eq!(preview.reclaimed_bytes, 1234);
        assert!(
            !arguments_log.exists(),
            "dry-run must not invoke simulator tools"
        );

        let result = clean_ios_simulator_reset(
            &target,
            1234,
            9,
            CleanMode::Execute,
            &mut |_| {},
            &cancellation,
            xcrun.as_os_str(),
        );
        assert_eq!(result.errors, 0, "{:?}", result.error_detail);
        assert_eq!(result.reclaimed_bytes, 1234);
        assert_eq!(
            fs::read_to_string(arguments_log).unwrap(),
            "simctl shutdown all\nsimctl erase all\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn simulator_reset_does_not_erase_when_shutdown_fails() {
        use std::sync::{Arc, atomic::AtomicBool};

        let directory = tempfile::tempdir().unwrap();
        let xcrun = directory.path().join("xcrun");
        let arguments_log = directory.path().join("xcrun-arguments.log");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}';\nif [ \"$2\" = shutdown ]; then echo 'shutdown failed' >&2; exit 7; fi\nif [ \"$2\" = erase ]; then echo 'erase must not run' >&2; exit 9; fi\n",
            arguments_log.display()
        );
        fs::write(&xcrun, script).unwrap();
        let mut permissions = fs::metadata(&xcrun).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&xcrun, permissions).unwrap();
        let target = CleanTarget {
            name: Cow::Borrowed("iOS Simulators Reset"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: &[
                "sh",
                "-c",
                "xcrun simctl shutdown all 2>/dev/null; xcrun simctl erase all",
            ],
            requires_sudo: false,
            dangerous: true,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_ios_simulator_reset(
            &target,
            100,
            2,
            CleanMode::Execute,
            &mut |_| {},
            &Arc::new(AtomicBool::new(false)),
            xcrun.as_os_str(),
        );

        assert_eq!(result.errors, 1);
        assert_eq!(result.reclaimed_bytes, 0);
        assert!(result.error_detail.unwrap().contains("shutdown failed"));
        assert_eq!(
            fs::read_to_string(arguments_log).unwrap(),
            "simctl shutdown all\n"
        );
    }

    #[test]
    fn clean_command_target_dry_run_returns_estimates() {
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

        let result = clean_target(&target, 42_000_000_000, 5, CleanMode::DryRun);
        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 42_000_000_000);
        assert_eq!(result.removed_entries, 5);
    }

    #[test]
    fn clean_command_target_successful_command_returns_estimates() {
        // echo always succeeds on any platform
        let target = CleanTarget {
            name: Cow::Borrowed("Echo Test"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: &["echo", "ok"],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 1000, 1, CleanMode::Execute);
        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 1000);
        assert_eq!(result.removed_entries, 1);
    }

    #[test]
    fn failed_command_returns_stderr_for_the_ui() {
        let target = CleanTarget {
            name: Cow::Borrowed("Failing Command"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: command_with_stderr_failure(),
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 0, 0, CleanMode::Execute);

        assert_eq!(result.errors, 1);
        assert_eq!(
            result.error_detail.as_deref(),
            Some("command failed: docker daemon failed")
        );
    }

    #[test]
    fn failed_command_without_stderr_includes_exit_status() {
        let target = CleanTarget {
            name: Cow::Borrowed("Silent Failure"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: command_without_stderr_failure(),
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 0, 0, CleanMode::Execute);

        assert_eq!(
            result.error_detail.as_deref(),
            Some(expected_silent_failure_detail())
        );
    }

    #[test]
    fn privileged_command_inherits_terminal_streams() {
        let target = CleanTarget {
            name: Cow::Borrowed("Interactive Command"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: successful_interactive_command(),
            requires_sudo: true,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 0, 0, CleanMode::Execute);

        assert_eq!(result.errors, 0);
        assert_eq!(result.error_detail, None);
    }

    #[test]
    fn docker_volume_prune_reports_actual_reclaimed_bytes_and_volume_count() {
        let target = CleanTarget {
            name: Cow::Borrowed("Docker Volumes Prune"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: docker_volume_prune_command(),
            requires_sudo: false,
            dangerous: true,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 16_680_000_000, 1, CleanMode::Execute);

        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 12_500_000_000);
        assert_eq!(result.removed_entries, 2);
    }

    #[test]
    fn docker_volume_prune_success_with_zero_reclaimed_does_not_report_estimate() {
        let target = CleanTarget {
            name: Cow::Borrowed("Docker Volumes Prune"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: docker_volume_prune_noop_command(),
            requires_sudo: false,
            dangerous: true,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 16_680_000_000, 1, CleanMode::Execute);

        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 0);
        assert_eq!(result.removed_entries, 0);
    }

    #[test]
    fn docker_volume_prune_unparseable_output_is_reported_as_measurement_error() {
        let target = CleanTarget {
            name: Cow::Borrowed("Docker Volumes Prune"),
            path: Cow::Borrowed(""),
            description: Cow::Borrowed("test"),
            command: docker_volume_prune_unparseable_command(),
            requires_sudo: false,
            dangerous: true,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 16_680_000_000, 1, CleanMode::Execute);

        assert_eq!(result.reclaimed_bytes, 0);
        assert_eq!(result.removed_entries, 0);
        assert_eq!(result.errors, 1);
        assert_eq!(
            result.error_detail.as_deref(),
            Some(
                "docker volume prune completed, but its reclaimed-space summary could not be read"
            )
        );
    }

    #[test]
    fn clean_command_target_file_target_is_untouched() {
        // A file target (empty command) should not be treated as command target
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("f.txt"), b"data").expect("write file");

        let target = CleanTarget {
            name: Cow::Borrowed("Normal File Target"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Builtin,
        };

        let result = clean_target(&target, 4, 1, CleanMode::Execute);
        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 4);
    }

    #[cfg(unix)]
    #[test]
    fn ordinary_directory_target_removes_contents_but_keeps_target_directory() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/data.bin"), b"data").unwrap();
        let target = CleanTarget {
            name: Cow::Borrowed("Cache contents"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Custom,
        };

        let result = clean_target(&target, 4, 1, CleanMode::Execute);

        assert_eq!(result.errors, 0, "{:?}", result.error_detail);
        assert!(root.is_dir(), "the target root must be preserved");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert_eq!(result.reclaimed_bytes, 4);
        assert_eq!(result.removed_entries, 1);
    }

    #[cfg(unix)]
    #[test]
    fn directory_cleanup_does_not_follow_nested_symlink() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep.txt"), b"safe").unwrap();
        symlink(&outside, root.join("external-link")).unwrap();
        let target = CleanTarget {
            name: Cow::Borrowed("Cache contents"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Custom,
        };

        let result = clean_target(&target, 0, 1, CleanMode::Execute);

        assert_eq!(result.errors, 0, "{:?}", result.error_detail);
        assert!(root.is_dir());
        assert!(!root.join("external-link").exists());
        assert_eq!(fs::read(outside.join("keep.txt")).unwrap(), b"safe");
    }

    #[cfg(unix)]
    #[test]
    fn cancelled_directory_cleanup_reports_partial_failure_and_preserves_root() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path().join("cache");
        fs::create_dir(&root).unwrap();
        for index in 0..8 {
            fs::write(root.join(format!("{index}.bin")), b"data").unwrap();
        }
        let target = CleanTarget {
            name: Cow::Borrowed("Cache contents"),
            path: Cow::Owned(root.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Custom,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_from_progress = Arc::clone(&cancel);

        let result = clean_target_with_progress(
            &target,
            32,
            8,
            CleanMode::Execute,
            &mut |_| cancel_from_progress.store(true, std::sync::atomic::Ordering::Relaxed),
            &cancel,
        );

        assert_eq!(result.errors, 1);
        assert_eq!(result.reclaimed_bytes, 0);
        assert_eq!(result.removed_entries, 0);
        assert!(root.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn pre_cancelled_single_file_cleanup_does_not_remove_file() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let temp = tempfile::tempdir().expect("create tempdir");
        let file = temp.path().join("keep.bin");
        fs::write(&file, b"keep").unwrap();
        let target = CleanTarget {
            name: Cow::Borrowed("Single file"),
            path: Cow::Owned(file.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Custom,
        };
        let cancel = Arc::new(AtomicBool::new(true));

        let result =
            clean_target_with_progress(&target, 4, 1, CleanMode::Execute, &mut |_| {}, &cancel);

        assert_eq!(result.errors, 1);
        assert!(file.exists());
        assert_eq!(fs::read(file).unwrap(), b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_cleanup_removes_link_without_touching_destination() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("create tempdir");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("precious.txt"), b"keep me").unwrap();
        let link = temp.path().join("custom-target");
        symlink(&outside, &link).unwrap();
        let target = CleanTarget {
            name: Cow::Borrowed("Symlink target"),
            path: Cow::Owned(link.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
            origin: TargetOrigin::Custom,
        };

        let result = clean_target(&target, 0, 1, CleanMode::Execute);

        assert_eq!(result.errors, 0, "{:?}", result.error_detail);
        assert!(!link.exists());
        assert_eq!(fs::read(outside.join("precious.txt")).unwrap(), b"keep me");
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_refuses_target_replaced_after_resolution() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let selected = temp.path().join("selected");
        let replacement = temp.path().join("replacement");
        fs::create_dir(&selected).unwrap();
        fs::write(selected.join("keep.txt"), b"preserve selected directory").unwrap();
        fs::create_dir(&replacement).unwrap();
        fs::write(
            replacement.join("keep.txt"),
            b"preserve replacement directory",
        )
        .unwrap();
        let resolved = canonicalize_cleanup_target(&selected).unwrap();
        fs::remove_dir_all(&selected).unwrap();
        symlink(&replacement, &selected).unwrap();

        assert!(!cleanup_target_identity_matches(
            &resolved.0,
            resolved.1,
            resolved.2
        ));
        assert_eq!(
            fs::read(replacement.join("keep.txt")).unwrap(),
            b"preserve replacement directory"
        );
    }

    #[cfg(unix)]
    #[test]
    fn delete_entire_symlink_target_is_refused() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("create tempdir");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("precious.txt"), b"keep me").unwrap();
        let link = temp.path().join("custom-target");
        symlink(&outside, &link).unwrap();
        let target = CleanTarget {
            name: Cow::Borrowed("Symlink target"),
            path: Cow::Owned(link.to_string_lossy().into_owned()),
            description: Cow::Borrowed("test"),
            command: &[],
            requires_sudo: false,
            dangerous: true,
            delete_entire: true,
            origin: TargetOrigin::Custom,
        };

        let result = clean_target(&target, 0, 1, CleanMode::Execute);

        assert_eq!(result.errors, 1);
        assert!(fs::symlink_metadata(&link).is_ok());
        assert_eq!(fs::read(outside.join("precious.txt")).unwrap(), b"keep me");
    }
}
