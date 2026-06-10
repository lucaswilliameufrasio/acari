use std::fs;
use std::path::{Path, PathBuf};

use crate::application::cleaner::CleanMode;
use crate::domain::{CleanResult, CleanTarget};

fn safe_canonicalize(entry: &Path, root: &Path) -> Option<PathBuf> {
    // Broken symlinks fail fs::canonicalize() but can still be removed.
    // Handle them by checking the canonical root and using the entry as-is.
    if fs::symlink_metadata(entry).is_ok_and(|m| m.file_type().is_symlink()) {
        return match fs::canonicalize(root) {
            Ok(canonical_root) if entry.starts_with(&canonical_root) => Some(entry.to_path_buf()),
            _ => None,
        };
    }
    let canonical = fs::canonicalize(entry).ok()?;
    let canonical_root = fs::canonicalize(root).ok()?;
    if canonical.starts_with(&canonical_root) {
        Some(canonical)
    } else {
        None
    }
}

fn remove_entry(path: &Path) -> bool {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(_) => return false,
    };

    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path).is_ok()
    } else if metadata.is_dir() {
        fs::remove_dir_all(path).is_ok()
    } else {
        false
    }
}

#[cfg(target_os = "macos")]
fn force_remove(path: &Path) -> bool {
    if remove_entry(path) {
        return true;
    }
    let _ = std::process::Command::new("chflags")
        .arg("nouchg")
        .arg(path)
        .output();
    remove_entry(path)
}

#[cfg(not(target_os = "macos"))]
fn force_remove(path: &Path) -> bool {
    remove_entry(path)
}

pub fn clean_target(
    target: &CleanTarget,
    estimated_bytes: u64,
    estimated_entries: u64,
    mode: CleanMode,
) -> CleanResult {
    let raw_path = target.resolved_path();
    let path = match safe_canonicalize(&raw_path, &raw_path) {
        Some(p) => p,
        None => {
            return CleanResult {
                target: target.clone(),
                reclaimed_bytes: 0,
                removed_entries: 0,
                errors: 1,
            };
        }
    };

    if !path.exists() {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: 0,
            removed_entries: 0,
            errors: 0,
        };
    }

    if mode == CleanMode::DryRun {
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: estimated_bytes,
            removed_entries: estimated_entries,
            errors: 0,
        };
    }

    let mut removed_entries = 0_u64;
    let mut errors = 0_u64;
    if path.is_file() {
        if force_remove(&path) {
            removed_entries = 1;
        } else {
            errors = 1;
        }
    } else if path.is_dir() {
        match fs::read_dir(&path) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let entry_path = entry.path();
                    let safe_path = safe_canonicalize(&entry_path, &path);
                    match safe_path {
                        Some(p) if force_remove(&p) => {
                            removed_entries = removed_entries.saturating_add(1);
                        }
                        None | Some(_) => {
                            errors = errors.saturating_add(1);
                        }
                    }
                }
            }
            Err(_) => {
                errors = errors.saturating_add(1);
            }
        }
    }

    CleanResult {
        target: target.clone(),
        reclaimed_bytes: estimated_bytes,
        removed_entries,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::fs;

    use crate::domain::CleanTarget;

    use super::clean_target;
    use crate::application::cleaner::CleanMode;

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
        };

        let result = clean_target(&target, 3, 1, CleanMode::Execute);
        assert!(result.errors > 0);

        let mut perms = fs::metadata(&root).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&root, perms).expect("restore perms");
    }
}
