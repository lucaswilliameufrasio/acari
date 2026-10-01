//! Narrow Linux privilege broker used by the desktop UI.
//!
//! The UI passes stable operation identifiers, never command strings. The
//! companion helper independently validates the identifier before doing work.

#[cfg(target_os = "linux")]
pub const OPERATIONS: &[(&str, &str)] = &[
    ("apt-autoremove", "Apt Autoremove"),
    ("journal-vacuum", "Journalctl Vacuum"),
];

#[cfg(target_os = "linux")]
pub fn operation_for_target(name: &str) -> Option<&'static str> {
    OPERATIONS
        .iter()
        .find_map(|(operation, target)| (*target == name).then_some(*operation))
}

/// Locate the companion helper next to the installed Acarí executable.
#[cfg(target_os = "linux")]
pub fn helper_path() -> Option<std::path::PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let executable = std::env::current_exe().ok()?;
    let helper = executable.with_file_name("acari-privileged-helper");
    let metadata = std::fs::metadata(&helper).ok()?;
    (metadata.is_file() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0).then_some(helper)
}

#[cfg(target_os = "linux")]
pub fn run_operation(operation: &str) -> Result<(), String> {
    if !OPERATIONS.iter().any(|(known, _)| *known == operation) {
        return Err("unknown privileged operation".into());
    }
    let helper = helper_path().ok_or_else(|| {
        "the acari-privileged-helper companion is not installed next to Acarí".to_string()
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

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn only_known_privileged_targets_map_to_operations() {
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
}
