use std::fs;
use std::path::Path;

use jwalk::WalkDir;

use crate::application::cleaner::CleanMode;
use crate::domain::{CleanResult, CleanTarget};

pub fn clean_target(target: &CleanTarget, estimated_bytes: u64, mode: CleanMode) -> CleanResult {
    let path = target.resolved_path();

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
            removed_entries: count_entries_recursive(&path),
            errors: 0,
        };
    }

    let mut removed_entries = 0_u64;
    let mut errors = 0_u64;
    if path.is_file() {
        if fs::remove_file(&path).is_ok() {
            removed_entries = 1;
        } else {
            errors = 1;
        }
    } else if path.is_dir() {
        match fs::read_dir(&path) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let entry_path = entry.path();
                    if remove_entry(&entry_path) {
                        removed_entries = removed_entries.saturating_add(1);
                    } else {
                        errors = errors.saturating_add(1);
                    }
                }
            }
            Err(_) => {
                errors = errors.saturating_add(1);
            }
        }
    }

    let reclaimed_bytes = if errors == 0 { estimated_bytes } else { 0 };

    CleanResult {
        target: target.clone(),
        reclaimed_bytes,
        removed_entries,
        errors,
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

fn count_entries_recursive(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .skip(1)
        .count() as u64
}

#[cfg(test)]
mod tests {
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

        let leaked_path: &'static str =
            Box::leak(root.to_string_lossy().into_owned().into_boxed_str());
        let target = CleanTarget {
            name: "Temp Cache",
            path: leaked_path,
            description: "test",
        };

        let result = clean_target(&target, 8, CleanMode::Execute);
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

        let leaked_path: &'static str =
            Box::leak(root.to_string_lossy().into_owned().into_boxed_str());
        let target = CleanTarget {
            name: "Temp Cache",
            path: leaked_path,
            description: "test",
        };

        let result = clean_target(&target, 3, CleanMode::DryRun);
        assert_eq!(result.errors, 0);
        assert_eq!(result.reclaimed_bytes, 3);
        assert!(result.removed_entries >= 1);

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

        let leaked_path: &'static str =
            Box::leak(root.to_string_lossy().into_owned().into_boxed_str());
        let target = CleanTarget {
            name: "Readonly Cache",
            path: leaked_path,
            description: "test",
        };

        let result = clean_target(&target, 3, CleanMode::Execute);
        assert!(result.errors > 0);

        let mut perms = fs::metadata(&root).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&root, perms).expect("restore perms");
    }
}
