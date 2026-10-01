//! Narrow platform privilege brokers used by the desktop UI.
//!
//! The UI passes stable operation identifiers, never command strings. The
//! companion helper independently validates the identifier before doing work.

#[cfg(target_os = "linux")]
pub const OPERATIONS: &[(&str, &str)] = &[
    ("apt-autoremove", "Apt Autoremove"),
    ("journal-vacuum", "Journalctl Vacuum"),
];

#[cfg(target_os = "macos")]
pub const OPERATIONS: &[(&str, &str)] = &[("tm-snapshot-thin", "Time Machine Local Snapshots")];

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn operation_for_target(name: &str) -> Option<&'static str> {
    OPERATIONS
        .iter()
        .find_map(|(operation, target)| (*target == name).then_some(*operation))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn is_root_owned_system_helper(path: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Ok(helper_metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !helper_metadata.file_type().is_file()
        || helper_metadata.uid() != 0
        || helper_metadata.mode() & 0o022 != 0
    {
        return false;
    }

    let mut ancestor = path.parent();
    while let Some(directory) = ancestor {
        let Ok(metadata) = std::fs::symlink_metadata(directory) else {
            return false;
        };
        if !metadata.file_type().is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return false;
        }
        if directory == std::path::Path::new("/") {
            return true;
        }
        ancestor = directory.parent();
    }
    false
}

/// Locate the root-owned system-installed companion helper.
#[cfg(target_os = "linux")]
pub fn helper_path() -> Option<std::path::PathBuf> {
    let helper = std::path::PathBuf::from("/usr/local/libexec/acari/acari-privileged-helper");
    is_root_owned_system_helper(&helper).then_some(helper)
}

#[cfg(target_os = "linux")]
pub fn authorization_broker_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        helper_path().is_some()
            && std::process::Command::new("pkexec")
                .arg("--version")
                .output()
                .is_ok()
    })
}

#[cfg(target_os = "macos")]
pub fn helper_path() -> Option<std::path::PathBuf> {
    let helper =
        std::path::PathBuf::from("/Library/PrivilegedHelperTools/com.acari.privileged-helper");
    is_root_owned_system_helper(&helper).then_some(helper)
}

#[cfg(target_os = "macos")]
pub fn authorization_broker_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        helper_path().is_some() && std::path::Path::new("/usr/bin/osascript").is_file()
    })
}

#[cfg(target_os = "linux")]
pub fn run_operation(operation: &str) -> Result<(), String> {
    if !OPERATIONS.iter().any(|(known, _)| *known == operation) {
        return Err("unknown privileged operation".into());
    }
    let helper = helper_path().ok_or_else(|| {
        "the acari-privileged-helper is missing or not securely installed".to_string()
    })?;
    let output = std::process::Command::new("pkexec")
        .arg(helper)
        .arg(operation)
        .output()
        .map_err(|error| format!("could not start the system authorization prompt: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!("authorization or operation failed ({})", output.status)
        } else {
            detail
        })
    }
}

#[cfg(target_os = "macos")]
pub fn run_operation(operation: &str) -> Result<(), String> {
    if !OPERATIONS.iter().any(|(known, _)| *known == operation) {
        return Err("unknown privileged operation".into());
    }
    let helper = helper_path()
        .ok_or_else(|| "the com.acari.privileged-helper is not securely installed".to_string())?;
    let output = std::process::Command::new(helper)
        .arg(operation)
        .output()
        .map_err(|error| format!("could not start the macOS authorization prompt: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!("authorization or operation failed ({})", output.status)
        } else {
            detail
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn only_known_privileged_targets_map_to_operations() {
        #[cfg(target_os = "linux")]
        {
            assert_eq!(
                super::operation_for_target("Apt Autoremove"),
                Some("apt-autoremove")
            );
            assert_eq!(
                super::operation_for_target("Journalctl Vacuum"),
                Some("journal-vacuum")
            );
            assert_eq!(super::operation_for_target("Docker System Prune"), None);
            assert_eq!(super::operation_for_target("custom command"), None);
        }
        #[cfg(target_os = "macos")]
        {
            assert_eq!(
                super::operation_for_target("Time Machine Local Snapshots"),
                Some("tm-snapshot-thin")
            );
            assert_eq!(super::operation_for_target("iOS Simulators Reset"), None);
            assert_eq!(super::operation_for_target("custom command"), None);
        }
        #[cfg(target_os = "linux")]
        assert!(!super::is_root_owned_system_helper(std::path::Path::new(
            "/tmp/nonexistent-acari-helper"
        )));
    }
}
