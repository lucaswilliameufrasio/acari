use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use jwalk::WalkDir;

use crate::domain::{CleanTarget, expand_tilde};

const BUILTIN_PATTERNS: &[&str] = &[
    "node_modules",
    ".next",
    ".nuxt",
    ".output",
    ".turbo",
    "dist",
    ".cache",
    "target",
    "vendor",
    "build",
    ".gradle",
    "bin",
    "__pycache__",
    ".venv",
    "venv",
    ".pytest_cache",
    ".tox",
    ".mypy_cache",
    "obj",
    "packages",
    ".dart_tool",
    ".packages",
    ".pub",
    "Library",
    "Temp",
    "Obj",
    "coverage",
];

const STOP_DIRS: &[&str] = &[".git", ".hg", ".svn"];

pub fn builtin_patterns() -> Vec<&'static str> {
    BUILTIN_PATTERNS.to_vec()
}

pub fn discover_junk_dirs(
    roots: &[impl AsRef<Path>],
    extra_patterns: &[String],
    no_default: bool,
) -> Vec<CleanTarget> {
    let patterns: Vec<String> = if no_default {
        extra_patterns.to_vec()
    } else {
        let mut all: Vec<String> = BUILTIN_PATTERNS.iter().map(|s| s.to_string()).collect();
        all.extend(extra_patterns.iter().cloned());
        all
    };

    let discovered = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(Mutex::new(HashSet::<String>::new()));
    let patterns_arc = Arc::new(patterns);

    for root in roots {
        let raw = root.as_ref();
        let path = expand_tilde(&raw.to_string_lossy());
        if !path.exists() {
            eprintln!(
                "warning: project root does not exist: {} (expanded: {})",
                raw.display(),
                path.display()
            );
            continue;
        }

        eprintln!("[project-scan] scanning {}...", path.display());

        let count_before = discovered.lock().unwrap().len();
        let d = Arc::clone(&discovered);
        let s = Arc::clone(&seen);
        let p = Arc::clone(&patterns_arc);

        let dir_counter = Arc::new(AtomicU64::new(0));
        let dc = Arc::clone(&dir_counter);

        let walker = WalkDir::new(&path)
            .follow_links(false)
            .skip_hidden(false)
            .process_read_dir(move |_, _, _, children: &mut Vec<_>| {
                let n = dc.fetch_add(1, Ordering::Relaxed);
                if n > 0 && n.is_multiple_of(1000) {
                    eprintln!(
                        "[project-scan] ...{} dirs, {} junk found",
                        n,
                        d.lock().unwrap().len()
                    );
                }
                children.retain(|entry| {
                    if let Ok(e) = entry {
                        let name = e.file_name.to_string_lossy().to_string();
                        if STOP_DIRS.contains(&name.as_str()) {
                            return false;
                        }
                        if p.iter().any(|pat| pat == &name) {
                            if s.lock()
                                .unwrap()
                                .insert(e.path().to_string_lossy().to_string())
                            {
                                let desc = format!("Project junk: {}", name);
                                d.lock().unwrap().push(CleanTarget {
                                    name: Cow::Owned(name),
                                    path: Cow::Owned(e.path().to_string_lossy().to_string()),
                                    description: Cow::Owned(desc),
                                });
                            }
                            return false;
                        }
                        true
                    } else {
                        true
                    }
                });
            });

        for _ in walker {}

        let found = discovered.lock().unwrap().len() - count_before;
        eprintln!(
            "[project-scan] {}: found {} junk dir(s)",
            path.display(),
            found
        );
    }

    discovered.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn finds_junk_dirs_under_root() {
        let root = tempfile::tempdir().unwrap();
        let junk = root.path().join("node_modules");
        fs::create_dir_all(junk.join("dep1")).unwrap();
        fs::write(junk.join("dep1").join("lib.js"), b"x").unwrap();
        let also = root.path().join("target");
        fs::create_dir_all(also.join("debug")).unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false);

        assert_eq!(results.len(), 2, "should find node_modules and target");
        let names: Vec<&str> = results.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"node_modules"));
        assert!(names.contains(&"target"));
    }

    #[test]
    fn does_not_descend_into_junk() {
        let root = tempfile::tempdir().unwrap();
        let outer = root.path().join("node_modules");
        fs::create_dir_all(outer.join("inner").join("node_modules")).unwrap();
        fs::write(
            outer.join("inner").join("node_modules").join("pkg.js"),
            b"x",
        )
        .unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false);

        // Only one top-level node_modules should be found,
        // not the nested one (since jwalk prunes the sub-tree)
        let count = results.iter().filter(|t| t.name == "node_modules").count();
        assert_eq!(
            count, 1,
            "should not find nested node_modules inside node_modules"
        );
    }

    #[test]
    fn dedup_same_path_from_multiple_roots() {
        let root = tempfile::tempdir().unwrap();
        let junk = root.path().join("node_modules");
        fs::create_dir_all(&junk).unwrap();
        fs::write(junk.join("pkg.js"), b"x").unwrap();

        let results = discover_junk_dirs(&[root.path(), root.path()], &[], false);

        let count = results.iter().filter(|t| t.name == "node_modules").count();
        assert_eq!(count, 1, "same path should not appear twice");
    }

    #[test]
    fn respects_custom_patterns() {
        let root = tempfile::tempdir().unwrap();
        let junk = root.path().join(".terraform");
        fs::create_dir_all(&junk).unwrap();
        fs::write(junk.join("state"), b"data").unwrap();

        let results = discover_junk_dirs(&[root.path()], &[".terraform".into()], false);

        let count = results.iter().filter(|t| t.name == ".terraform").count();
        assert_eq!(count, 1, "custom pattern should be matched");
    }

    #[test]
    fn no_default_patterns_uses_only_custom() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("node_modules")).unwrap();
        fs::create_dir_all(root.path().join("custom_junk")).unwrap();

        let results = discover_junk_dirs(&[root.path()], &["custom_junk".into()], true);

        let names: Vec<&str> = results.iter().map(|t| t.name.as_ref()).collect();
        assert!(
            names.contains(&"custom_junk"),
            "custom pattern should be found"
        );
        assert!(
            !names.contains(&"node_modules"),
            "built-in should be ignored with no_default"
        );
    }

    #[test]
    fn nonexistent_root_does_not_crash() {
        let results = discover_junk_dirs(&["/tmp/acari-nonexistent-12345"], &[], false);
        assert!(results.is_empty());
    }

    #[test]
    fn expands_tilde_in_roots() {
        let results = discover_junk_dirs(&["~/.acari-junk-test-unused"], &[], false);
        assert!(results.is_empty());
    }

    #[test]
    fn skips_git_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".git")).unwrap();
        fs::write(root.path().join(".git").join("HEAD"), b"ref").unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false);
        assert!(
            results.iter().all(|t| t.name != ".git"),
            ".git should not appear as junk"
        );
    }

    #[test]
    fn finds_only_matching_names() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("node_modules")).unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join(".venv")).unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false);
        let names: Vec<&str> = results.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"node_modules"));
        assert!(!names.contains(&"src"), "plain 'src' should not match");
        assert!(names.contains(&".venv"));
    }
}
