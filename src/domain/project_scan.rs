use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use jwalk::WalkDir;

use crate::domain::CleanTarget;

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
        let path = root.as_ref();
        if !path.exists() {
            eprintln!("warning: project root does not exist: {}", path.display());
            continue;
        }

        let d = Arc::clone(&discovered);
        let s = Arc::clone(&seen);
        let p = Arc::clone(&patterns_arc);

        let walker = WalkDir::new(path).follow_links(false).process_read_dir(
            move |_, _, _, children: &mut Vec<_>| {
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
            },
        );

        for _ in walker {}
    }

    Arc::into_inner(discovered)
        .expect("discovered still referenced")
        .into_inner()
        .expect("mutex poisoned")
}
