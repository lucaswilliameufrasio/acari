//! Root-side entry point for a deliberately tiny, identifier-only operation set.

#[cfg(target_os = "linux")]
use std::os::unix::process::ExitStatusExt;
#[cfg(target_os = "linux")]
use std::process::{Command, ExitCode};

#[cfg(target_os = "linux")]
fn command_for_operation(operation: &str) -> Option<(&'static str, &'static [&'static str])> {
    match operation {
        "apt-autoremove" => Some(("/usr/bin/apt-get", &["autoremove", "-y"])),
        "journal-vacuum" => Some(("/usr/bin/journalctl", &["--vacuum-size=100M"])),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("This helper must be launched by the system authorization broker.");
        return ExitCode::FAILURE;
    }
    use std::os::unix::fs::MetadataExt;
    let Ok(executable) = std::env::current_exe() else {
        eprintln!("Could not resolve the privileged helper executable.");
        return ExitCode::FAILURE;
    };
    let Ok(metadata) = std::fs::metadata(executable) else {
        eprintln!("Could not verify the privileged helper installation.");
        return ExitCode::FAILURE;
    };
    if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        eprintln!("The privileged helper must be root-owned and not writable by others.");
        return ExitCode::FAILURE;
    }

    let mut args = std::env::args_os().skip(1);
    let Some(operation) = args.next() else {
        eprintln!("Missing operation identifier.");
        return ExitCode::FAILURE;
    };
    if args.next().is_some() {
        eprintln!("Unexpected arguments.");
        return ExitCode::FAILURE;
    }

    let Some(command) = operation.to_str().and_then(command_for_operation) else {
        eprintln!("Unknown operation identifier.");
        return ExitCode::FAILURE;
    };

    match Command::new(command.0).args(command.1).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(
            status
                .code()
                .unwrap_or_else(|| status.signal().map_or(1, |signal| (128 + signal).min(255)))
                as u8,
        ),
        Err(error) => {
            eprintln!("Could not run the allowlisted operation: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::command_for_operation;

    #[test]
    fn helper_exposes_only_fixed_operations() {
        assert_eq!(
            command_for_operation("apt-autoremove"),
            Some(("/usr/bin/apt-get", &["autoremove", "-y"][..]))
        );
        assert_eq!(
            command_for_operation("journal-vacuum"),
            Some(("/usr/bin/journalctl", &["--vacuum-size=100M"][..]))
        );
        assert_eq!(command_for_operation("/bin/sh"), None);
        assert_eq!(command_for_operation("unknown"), None);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The privileged helper is supported only on Linux.");
    std::process::exit(1);
}
