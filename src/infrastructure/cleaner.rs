use std::fs;
use std::path::{Path, PathBuf};

use crate::application::cleaner::CleanMode;
use crate::domain::{CleanResult, CleanTarget};

fn safe_canonicalize(entry: &Path, root: &Path) -> Option<PathBuf> {
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

/// Remove a single entry (file, dir, or symlink). Returns the number of bytes freed,
/// or None on failure.
fn remove_entry(path: &Path) -> Option<u64> {
    let metadata = fs::symlink_metadata(path).ok()?;
    let is_sym = metadata.file_type().is_symlink();

    if is_sym || metadata.is_file() {
        let size = if is_sym { 0 } else { metadata.len() };
        fs::remove_file(path).ok().map(|_| size)
    } else if metadata.is_dir() {
        fs::remove_dir_all(path).ok().map(|_| 0)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn force_remove(path: &Path) -> Option<u64> {
    remove_entry(path).or_else(|| {
        let _ = std::process::Command::new("chflags")
            .arg("nouchg")
            .arg(path)
            .output();
        remove_entry(path)
    })
}

#[cfg(not(target_os = "macos"))]
fn force_remove(path: &Path) -> Option<u64> {
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

    if target.delete_entire {
        let ok = force_remove(&path).is_some();
        return CleanResult {
            target: target.clone(),
            reclaimed_bytes: if ok { estimated_bytes } else { 0 },
            removed_entries: if ok { estimated_entries } else { 0 },
            errors: if ok { 0 } else { 1 },
        };
    }

    let mut removed_entries = 0_u64;
    let mut errors = 0_u64;
    let mut reclaimed_bytes = 0_u64;

    if path.is_file() {
        match force_remove(&path) {
            Some(freed) => {
                removed_entries = 1;
                reclaimed_bytes = freed;
            }
            None => {
                errors = 1;
            }
        }
    } else if path.is_dir() {
        match fs::read_dir(&path) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let entry_path = entry.path();
                    let safe_path = safe_canonicalize(&entry_path, &path);
                    match safe_path {
                        Some(p) => match force_remove(&p) {
                            Some(freed) => {
                                removed_entries = removed_entries.saturating_add(1);
                                reclaimed_bytes = reclaimed_bytes.saturating_add(freed);
                            }
                            None => {
                                errors = errors.saturating_add(1);
                            }
                        },
                        None => {
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
        reclaimed_bytes: if errors == 0 {
            estimated_bytes
        } else {
            reclaimed_bytes
        },
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
            delete_entire: false,
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
            delete_entire: false,
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
            delete_entire: false,
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
                delete_entire: false,
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
            delete_entire: true,
        };

        let result = clean_target(&target, 4, 1, CleanMode::Execute);
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
            delete_entire: true,
        };

        let result = clean_target(&target, 100, 5, CleanMode::DryRun);
        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 100);
        assert_eq!(result.removed_entries, 5);
        assert!(root.exists(), "should not delete in dry run");
    }
}
