//! Planned changes to files and programs outside acari's own config. They
//! are described to the user first and applied only after confirmation.

use std::path::{Path, PathBuf};
use std::process::Command;

/// One planned modification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Rewrite a file with new content (a backup is kept).
    WriteFile {
        path: PathBuf,
        content: String,
        /// Human summary of what changes, e.g. `+ hooks.SessionStart[acari]`.
        summary: String,
    },
    /// Run a program (an agent's CLI, systemctl, launchctl...).
    Run {
        program: String,
        args: Vec<String>,
        /// Failures are expected and ignored (e.g. unloading a job that is
        /// not loaded).
        allow_failure: bool,
    },
    /// Delete a file acari created earlier.
    RemoveFile { path: PathBuf },
    /// Cannot be automated: tell the user what to do.
    Manual { instructions: String },
}

impl Change {
    pub fn describe(&self, home: &Path) -> String {
        match self {
            Change::WriteFile { path, summary, .. } => {
                let shown = crate::domain::insights::display_path(path, home);
                format!("{shown}: {summary}")
            }
            Change::Run { program, args, .. } => {
                let quoted: Vec<String> = args
                    .iter()
                    .map(|a| crate::domain::insights::shell_quote(Path::new(a)))
                    .collect();
                format!("$ {program} {}", quoted.join(" "))
            }
            Change::RemoveFile { path } => {
                let shown = crate::domain::insights::display_path(path, home);
                format!("- {shown}")
            }
            Change::Manual { instructions } => instructions.clone(),
        }
    }

    pub fn apply(&self) -> Result<(), String> {
        match self {
            Change::WriteFile { path, content, .. } => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                if path.exists() {
                    let backup = PathBuf::from(format!("{}.acari-bak", path.display()));
                    std::fs::copy(path, &backup).map_err(|e| format!("backup failed: {e}"))?;
                }
                std::fs::write(path, content).map_err(|e| e.to_string())
            }
            Change::Run {
                program,
                args,
                allow_failure,
            } => {
                let out = Command::new(program)
                    .args(args)
                    .output()
                    .map_err(|e| format!("{program}: {e}"))?;
                if out.status.success() || *allow_failure {
                    Ok(())
                } else {
                    Err(format!(
                        "{program} failed: {}{}",
                        String::from_utf8_lossy(&out.stderr).trim(),
                        String::from_utf8_lossy(&out.stdout).trim()
                    ))
                }
            }
            Change::RemoveFile { path } => match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("{}: {e}", path.display())),
            },
            Change::Manual { .. } => Ok(()),
        }
    }
}
