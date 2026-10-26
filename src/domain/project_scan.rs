use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use jwalk::WalkDir;

use crate::domain::{CleanTarget, TargetOrigin, expand_tilde};

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
    ".dart_tool",
    ".packages",
    ".pub",
    "Temp",
    "Obj",
    "coverage",
    ".svelte-kit",
    ".parcel-cache",
    ".docusaurus",
    ".angular",
    ".serverless",
];

const STOP_DIRS: &[&str] = &[".git", ".hg", ".svn"];

/// A junk directory that lives under a `src/` tree is almost always source
/// code (e.g. `src/bin`, `src/lib/build`, `src/packages`) and must never be
/// flagged as deletable.
fn is_under_src(entry_path: &Path) -> bool {
    entry_path
        .parent()
        .map(|parent| {
            parent.components().any(
                |c| matches!(c, std::path::Component::Normal(n) if n.to_string_lossy() == "src"),
            )
        })
        .unwrap_or(false)
}

pub fn builtin_patterns() -> Vec<&'static str> {
    BUILTIN_PATTERNS.to_vec()
}

pub fn discover_junk_dirs(
    roots: &[impl AsRef<Path>],
    extra_patterns: &[String],
    no_default: bool,
    excludes: &[String],
) -> Vec<CleanTarget> {
    discover_junk_dirs_report(roots, extra_patterns, no_default, excludes).targets
}

pub struct DiscoveryReport {
    pub targets: Vec<CleanTarget>,
    pub errors: u64,
}

pub fn discover_junk_dirs_report(
    roots: &[impl AsRef<Path>],
    extra_patterns: &[String],
    no_default: bool,
    excludes: &[String],
) -> DiscoveryReport {
    let mut errors = 0_u64;
    let patterns: Vec<String> = if no_default {
        extra_patterns.to_vec()
    } else {
        let mut all: Vec<String> = BUILTIN_PATTERNS.iter().map(|s| s.to_string()).collect();
        all.extend(extra_patterns.iter().cloned());
        all
    };

    let excludes = excludes.to_vec();

    let discovered = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(Mutex::new(HashSet::<String>::new()));
    let patterns_arc = Arc::new(patterns);
    let excludes_arc = Arc::new(excludes);

    for root in roots {
        let raw = root.as_ref();
        let path = expand_tilde(&raw.to_string_lossy());
        if !path.is_dir() {
            errors = errors.saturating_add(1);
            eprintln!(
                "warning: project root is missing or not a directory: {} (expanded: {})",
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
        let ex = Arc::clone(&excludes_arc);

        let root_str = path.to_string_lossy().to_string();
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
                        if ex.iter().any(|pat| pat == &name) {
                            return false;
                        }
                        if !is_under_src(&e.path()) && p.iter().any(|pat| pat == &name) {
                            if s.lock()
                                .unwrap()
                                .insert(e.path().to_string_lossy().to_string())
                            {
                                let full = e.path().to_string_lossy().to_string();
                                let rel = full
                                    .strip_prefix(&root_str)
                                    .and_then(|s| s.strip_prefix(std::path::MAIN_SEPARATOR))
                                    .unwrap_or(&full);
                                let desc = format!("Project junk: {}", name);
                                d.lock().unwrap().push(CleanTarget {
                                    name: Cow::Owned(rel.to_string()),
                                    path: Cow::Owned(full),
                                    description: Cow::Owned(desc),
                                    command: &[],
                                    requires_sudo: false,
                                    // A matching name alone cannot prove this is disposable.
                                    dangerous: matches!(
                                        name.as_str(),
                                        "bin" | "vendor" | "build" | "dist" | "coverage"
                                    ) || (name == "target"
                                        && !e.path().parent().is_some_and(|parent| {
                                            parent.join("Cargo.toml").is_file()
                                        })),
                                    delete_entire: true,
                                    origin: TargetOrigin::Builtin,
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

        for entry in walker {
            if entry.is_err() {
                errors = errors.saturating_add(1);
            }
        }

        let found = discovered.lock().unwrap().len() - count_before;
        eprintln!(
            "[project-scan] {}: found {} junk dir(s)",
            path.display(),
            found
        );
    }

    if errors > 0 {
        eprintln!("[project-scan] incomplete discovery: {errors} error(s)");
    }
    let mut targets = discovered.lock().unwrap().clone();
    let mut name_counts = std::collections::HashMap::new();
    for target in &targets {
        *name_counts
            .entry(target.name.to_string())
            .or_insert(0_usize) += 1;
    }
    for target in &mut targets {
        if name_counts
            .get(target.name.as_ref())
            .copied()
            .unwrap_or_default()
            > 1
        {
            target.name = Cow::Owned(format!("{} ({})", target.name, target.path));
        }
    }
    DiscoveryReport { targets, errors }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn ambiguous_names_require_review_but_cargo_target_is_recognized() {
        let root = tempfile::tempdir().unwrap();
        for name in ["bin", "vendor", "build", "dist", "target"] {
            fs::create_dir(root.path().join(name)).unwrap();
        }
        let targets = discover_junk_dirs(&[root.path()], &[], false, &[]);
        assert_eq!(targets.len(), 5);
        assert!(targets.iter().all(|target| target.dangerous));
        fs::write(root.path().join("Cargo.toml"), "[package]").unwrap();
        let targets = discover_junk_dirs(&[root.path()], &[], false, &[]);
        assert!(
            !targets
                .iter()
                .find(|target| target.name == "target")
                .unwrap()
                .dangerous
        );
    }

    #[test]
    fn discovery_reports_missing_roots_as_incomplete() {
        let root = tempfile::tempdir().unwrap();
        let report = discover_junk_dirs_report(&[root.path().join("missing")], &[], false, &[]);
        assert_eq!(report.errors, 1);
        assert!(report.targets.is_empty());
    }

    #[test]
    fn finds_junk_dirs_under_root() {
        let root = tempfile::tempdir().unwrap();
        let junk = root.path().join("node_modules");
        fs::create_dir_all(junk.join("dep1")).unwrap();
        fs::write(junk.join("dep1").join("lib.js"), b"x").unwrap();
        let also = root.path().join("target");
        fs::create_dir_all(also.join("debug")).unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false, &[]);

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

        let results = discover_junk_dirs(&[root.path()], &[], false, &[]);

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

        let results = discover_junk_dirs(&[root.path(), root.path()], &[], false, &[]);

        let count = results.iter().filter(|t| t.name == "node_modules").count();
        assert_eq!(count, 1, "same path should not appear twice");
    }

    #[test]
    fn same_relative_candidate_in_distinct_roots_gets_unique_event_names() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        fs::create_dir(first.path().join("node_modules")).unwrap();
        fs::create_dir(second.path().join("node_modules")).unwrap();

        let targets = discover_junk_dirs(&[first.path(), second.path()], &[], false, &[]);
        assert_eq!(targets.len(), 2);
        assert_ne!(targets[0].name, targets[1].name);
        assert!(targets[0].name.contains("node_modules"));
        assert!(targets[1].name.contains("node_modules"));
        assert_ne!(targets[0].path, targets[1].path);
    }

    #[test]
    fn respects_custom_patterns() {
        let root = tempfile::tempdir().unwrap();
        let junk = root.path().join(".terraform");
        fs::create_dir_all(&junk).unwrap();
        fs::write(junk.join("state"), b"data").unwrap();

        let results = discover_junk_dirs(&[root.path()], &[".terraform".into()], false, &[]);

        let count = results.iter().filter(|t| t.name == ".terraform").count();
        assert_eq!(count, 1, "custom pattern should be matched");
    }

    #[test]
    fn no_default_patterns_uses_only_custom() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("node_modules")).unwrap();
        fs::create_dir_all(root.path().join("custom_junk")).unwrap();

        let results = discover_junk_dirs(&[root.path()], &["custom_junk".into()], true, &[]);

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
        let results = discover_junk_dirs(&["/tmp/acari-nonexistent-12345"], &[], false, &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn expands_tilde_in_roots() {
        let results = discover_junk_dirs(&["~/.acari-junk-test-unused"], &[], false, &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn skips_git_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".git")).unwrap();
        fs::write(root.path().join(".git").join("HEAD"), b"ref").unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false, &[]);
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

        let results = discover_junk_dirs(&[root.path()], &[], false, &[]);
        let names: Vec<&str> = results.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"node_modules"));
        assert!(!names.contains(&"src"), "plain 'src' should not match");
        assert!(names.contains(&".venv"));
    }

    #[test]
    fn does_not_flag_source_tree_subdirs_as_junk() {
        let root = tempfile::tempdir().unwrap();
        // Real-world layout that must NOT be deleted.
        fs::create_dir_all(root.path().join("src").join("bin")).unwrap();
        fs::create_dir_all(root.path().join("src").join("lib").join("build")).unwrap();
        fs::create_dir_all(root.path().join("src").join("packages")).unwrap();
        fs::write(
            root.path().join("src").join("bin").join("main.rs"),
            b"fn main() {}",
        )
        .unwrap();
        fs::write(
            root.path()
                .join("src")
                .join("lib")
                .join("build")
                .join("lib.rs"),
            b"pub fn x() {}",
        )
        .unwrap();

        // A real junk dir at top level must still be found.
        fs::create_dir_all(root.path().join("node_modules")).unwrap();
        fs::write(root.path().join("node_modules").join("pkg.js"), b"x").unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false, &[]);

        assert!(
            results.iter().all(|t| !t.path.as_ref().contains("/src/")),
            "subdirectories under src/ must never be flagged as junk"
        );
        let names: Vec<&str> = results.iter().map(|t| t.name.as_ref()).collect();
        assert!(
            names.contains(&"node_modules"),
            "top-level junk still found"
        );
    }

    #[test]
    fn does_not_flag_monorepo_packages_folder() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("packages").join("pkg-a")).unwrap();
        fs::write(
            root.path().join("packages").join("pkg-a").join("index.ts"),
            b"export const a = 1",
        )
        .unwrap();

        let results = discover_junk_dirs(&[root.path()], &[], false, &[]);
        assert!(
            results.iter().all(|t| t.name != "packages"),
            "monorepo packages/ folder must not be flagged as junk"
        );
    }

    #[test]
    fn excludes_prune_matching_dirs() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("node_modules")).unwrap();
        fs::write(root.path().join("node_modules").join("pkg.js"), b"x").unwrap();
        fs::create_dir_all(root.path().join("target").join("debug")).unwrap();
        fs::write(root.path().join("target").join("debug").join("out"), b"x").unwrap();

        // Excluding "target" must prevent it (and its subtree) from being flagged,
        // while "node_modules" is still found.
        let results = discover_junk_dirs(&[root.path()], &[], false, &["target".into()]);
        let names: Vec<&str> = results.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"node_modules"), "node_modules still found");
        assert!(
            !names.contains(&"target"),
            "excluded dir must not be flagged as junk"
        );
    }
}
