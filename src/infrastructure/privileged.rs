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
pub fn operation_for_target(target: &crate::domain::CleanTarget) -> Option<&'static str> {
    use crate::domain::TargetOrigin;

    if target.origin != TargetOrigin::Builtin || !target.requires_sudo || !target.is_command() {
        return None;
    }
    match target.name.as_ref() {
        #[cfg(target_os = "linux")]
        "Apt Autoremove" if target.command == ["sudo", "apt", "autoremove", "-y"] => {
            Some("apt-autoremove")
        }
        #[cfg(target_os = "linux")]
        "Journalctl Vacuum" if target.command == ["sudo", "journalctl", "--vacuum-size=100M"] => {
            Some("journal-vacuum")
        }
        #[cfg(target_os = "macos")]
        "Time Machine Local Snapshots"
            if target.command
                == [
                    "sudo",
                    "sh",
                    "-c",
                    "if tmutil listlocalsnapshots / 2>/dev/null | grep -qE \"com.apple.TimeMachine|localhost\"; then tmutil deletelocalsnapshots /; fi",
                ] =>
        {
            Some("tm-snapshot-thin")
        }
        _ => None,
    }
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
        use crate::domain::{CleanTarget, TargetOrigin};

        fn target(name: &'static str, command: &'static [&'static str]) -> CleanTarget {
            CleanTarget {
                name: name.into(),
                command,
                requires_sudo: true,
                origin: TargetOrigin::Builtin,
                ..CleanTarget::default()
            }
        }

        #[cfg(target_os = "linux")]
        {
            assert_eq!(
                super::operation_for_target(&target(
                    "Apt Autoremove",
                    &["sudo", "apt", "autoremove", "-y"]
                )),
                Some("apt-autoremove")
            );
            assert_eq!(
                super::operation_for_target(&target(
                    "Journalctl Vacuum",
                    &["sudo", "journalctl", "--vacuum-size=100M"]
                )),
                Some("journal-vacuum")
            );
            assert_eq!(
                super::operation_for_target(&target(
                    "Apt Autoremove",
                    &["sudo", "apt", "autoremove", "-y; arbitrary"]
                )),
                None
            );
            assert_eq!(
                super::operation_for_target(&target("Docker System Prune", &["docker"])),
                None
            );
        }
        #[cfg(target_os = "macos")]
        {
            assert_eq!(
                super::operation_for_target(&target(
                    "Time Machine Local Snapshots",
                    &[
                        "sudo",
                        "sh",
                        "-c",
                        "if tmutil listlocalsnapshots / 2>/dev/null | grep -qE \"com.apple.TimeMachine|localhost\"; then tmutil deletelocalsnapshots /; fi",
                    ]
                )),
                Some("tm-snapshot-thin")
            );
            assert_eq!(
                super::operation_for_target(&target("iOS Simulators Reset", &["xcrun"])),
                None
            );
        }
        let mut custom = target("Apt Autoremove", &["sudo", "apt", "autoremove", "-y"]);
        custom.origin = TargetOrigin::Custom;
        assert_eq!(super::operation_for_target(&custom), None);
        let mut unprivileged = target("Apt Autoremove", &["sudo", "apt", "autoremove", "-y"]);
        unprivileged.requires_sudo = false;
        assert_eq!(super::operation_for_target(&unprivileged), None);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn helper_validation_rejects_symlinks_and_writable_ancestors() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = tempfile::tempdir().expect("temporary directory");
        let helper_dir = temp.path().join("helper-dir");
        std::fs::create_dir(&helper_dir).expect("create helper directory");
        let helper = helper_dir.join("helper");
        std::fs::write(&helper, b"not executed").expect("write placeholder helper");
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755))
            .expect("set helper permissions");

        assert!(
            !super::is_root_owned_system_helper(&helper),
            "non-root-owned temporary files must not qualify as installed helpers"
        );

        let link = helper_dir.join("helper-link");
        symlink(&helper, &link).expect("create helper symlink");
        assert!(
            !super::is_root_owned_system_helper(&link),
            "a symlink must never qualify as the privileged helper"
        );

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            if unsafe { libc::geteuid() } == 0 {
                let uid = unsafe { libc::getuid() };
                let gid = unsafe { libc::getgid() };
                let chown_status = std::process::Command::new("chown")
                    .arg(format!("{uid}:{gid}"))
                    .arg(&helper_dir)
                    .status();
                if chown_status.is_ok_and(|status| status.success()) {
                    std::fs::set_permissions(&helper_dir, std::fs::Permissions::from_mode(0o777))
                        .expect("make temporary ancestor writable");
                    assert_eq!(std::fs::symlink_metadata(&helper_dir).unwrap().uid(), uid);
                    assert!(
                        !super::is_root_owned_system_helper(&helper),
                        "a writable/non-root-owned ancestor must invalidate the helper"
                    );
                }
            }
        }
    }
}
