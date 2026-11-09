//! One read-only pass over the scan roots that collects everything
//! `domain::insights` needs: recursive directory totals, big files, project
//! build outputs and linked git worktrees.
//!
//! Sizes are allocated bytes (what the disk actually spends), hard links are
//! counted once, and the walk never leaves the root's filesystem.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use jwalk::{Parallelism, WalkDir};
use rayon::ThreadPoolBuilder;

use crate::config::target_config::IoPriority;
use crate::domain::insights::{BigFile, BuildDir, BuildKind, DirStat, ExploreData, Worktree};

#[derive(Debug, Clone, Copy)]
pub struct ExploreOptions {
    /// Files at least this large are reported individually.
    pub big_file_min: u64,
    /// Directories smaller than this are dropped after aggregation to keep
    /// memory bounded (they can never become findings).
    pub keep_dir_min: u64,
    pub io_priority: IoPriority,
}

impl Default for ExploreOptions {
    fn default() -> Self {
        Self {
            big_file_min: 1_000_000_000,
            keep_dir_min: 10_000_000,
            io_priority: IoPriority::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExploreProgress {
    pub entries: u64,
    pub bytes: u64,
}

#[derive(Default, Clone, Copy)]
struct Own {
    bytes: u64,
    files: u64,
    newest: u64,
}

/// Walk every root and return the merged exploration data.
pub fn explore(
    roots: &[PathBuf],
    opts: ExploreOptions,
    mut progress: impl FnMut(ExploreProgress),
) -> ExploreData {
    let mut data = ExploreData::default();
    let mut own: HashMap<PathBuf, Own> = HashMap::new();
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
    let mut counter = ExploreProgress::default();

    let threads = match opts.io_priority {
        IoPriority::High => std::thread::available_parallelism().map_or(4, usize::from),
        IoPriority::Normal => {
            (std::thread::available_parallelism().map_or(4, usize::from) / 2).max(1)
        }
        IoPriority::Low => 1,
    };
    // Dedicated pool: see ADR 003 (jwalk on the global pool can stop early).
    let pool = Arc::new(
        ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("acari-explore-{i}"))
            .build()
            .expect("build explore pool"),
    );

    for root in roots {
        let Ok(root) = root.canonicalize() else {
            data.errors = data.errors.saturating_add(1);
            continue;
        };
        if !root.is_dir() || data.roots.iter().any(|r| root.starts_with(r)) {
            continue;
        }
        data.roots.push(root.clone());
        own.entry(root.clone()).or_default();
        let root_dev = device_of(&root);

        let walker = WalkDir::new(&root)
            .follow_links(false)
            .skip_hidden(false)
            .parallelism(Parallelism::RayonExistingPool {
                pool: Arc::clone(&pool),
                busy_timeout: None,
            })
            .process_read_dir(move |_, _, _, children| {
                // Stay on the root's filesystem: mounted disks, network
                // shares and FUSE mounts are not part of this volume.
                for child in children.iter_mut().flatten() {
                    if child.file_type.is_dir()
                        && let (Some(root_dev), Ok(meta)) = (root_dev, child.metadata())
                        && metadata_device(&meta) != Some(root_dev)
                    {
                        child.read_children = None;
                    }
                }
            });

        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    data.errors = data.errors.saturating_add(1);
                    continue;
                }
            };
            counter.entries = counter.entries.saturating_add(1);
            let file_type = entry.file_type();
            let path = entry.path();

            if file_type.is_dir() {
                if root_dev.is_some() && entry.read_children.is_none() && entry.depth > 0 {
                    // Different filesystem: skipped on purpose.
                    continue;
                }
                own.entry(path.clone()).or_default();
                let name = entry.file_name.to_string_lossy();
                if let Some(parent) = path.parent()
                    && let Some(kind) =
                        BuildKind::detect(&name, |marker| parent.join(marker).is_file())
                {
                    data.build_dirs.push(BuildDir {
                        path: path.clone(),
                        kind,
                    });
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                data.errors = data.errors.saturating_add(1);
                continue;
            };
            if entry.file_name == ".git"
                && let Some(main_repo) = linked_worktree_repo(&path)
            {
                data.worktrees.push(Worktree {
                    path: entry.parent_path().to_path_buf(),
                    main_repo,
                });
            }
            if let Some(key) = hard_link_key(&meta)
                && !seen_inodes.insert(key)
            {
                continue;
            }
            let bytes = allocated_size(&meta);
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            let slot = own.entry(entry.parent_path().to_path_buf()).or_default();
            slot.bytes = slot.bytes.saturating_add(bytes);
            slot.files = slot.files.saturating_add(1);
            slot.newest = slot.newest.max(modified);
            if bytes >= opts.big_file_min {
                data.big_files.push(BigFile {
                    path,
                    bytes,
                    modified,
                });
            }
            counter.bytes = counter.bytes.saturating_add(bytes);
            if counter.entries.is_multiple_of(20_000) {
                progress(counter);
            }
        }
    }
    progress(counter);

    data.dirs = aggregate(own, opts.keep_dir_min, &data.roots);
    data.big_files.sort_by_key(|f| std::cmp::Reverse(f.bytes));
    data
}

/// Propagate own totals bottom-up so every directory holds its subtree.
fn aggregate(
    own: HashMap<PathBuf, Own>,
    keep_min: u64,
    roots: &[PathBuf],
) -> HashMap<PathBuf, DirStat> {
    let mut items: Vec<(PathBuf, Own)> = own.into_iter().collect();
    items.sort_by_key(|(path, _)| std::cmp::Reverse(path.components().count()));
    let index: HashMap<PathBuf, usize> = items
        .iter()
        .enumerate()
        .map(|(i, (path, _))| (path.clone(), i))
        .collect();
    for i in 0..items.len() {
        if roots.contains(&items[i].0) {
            continue;
        }
        let Some(parent) = items[i].0.parent() else {
            continue;
        };
        if let Some(&pi) = index.get(parent)
            && pi != i
        {
            let child = items[i].1;
            let slot = &mut items[pi].1;
            slot.bytes = slot.bytes.saturating_add(child.bytes);
            slot.files = slot.files.saturating_add(child.files);
            slot.newest = slot.newest.max(child.newest);
        }
    }
    items
        .into_iter()
        .filter(|(path, own)| own.bytes >= keep_min || roots.contains(path))
        .map(|(path, own)| {
            (
                path,
                DirStat {
                    bytes: own.bytes,
                    files: own.files,
                    newest: own.newest,
                },
            )
        })
        .collect()
}

/// A `.git` *file* whose `gitdir:` points into another repository's
/// `worktrees/` folder marks a linked worktree (submodules point into
/// `modules/` instead). Returns `Some(main_repo)` for worktrees, where the
/// inner option is the owning repository when it can be derived.
fn linked_worktree_repo(dot_git: &Path) -> Option<Option<PathBuf>> {
    let content = std::fs::read_to_string(dot_git).ok()?;
    let gitdir = content
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    let gitdir = Path::new(gitdir);
    let gitdir = if gitdir.is_relative() {
        dot_git.parent()?.join(gitdir)
    } else {
        gitdir.to_path_buf()
    };
    // <common-dir>/worktrees/<name>
    let worktrees = gitdir.parent()?;
    if worktrees.file_name()? != "worktrees" {
        return None;
    }
    let common = worktrees.parent();
    Some(common.map(|common| {
        if common.file_name().is_some_and(|n| n == ".git") {
            common.parent().unwrap_or(common).to_path_buf()
        } else {
            common.to_path_buf()
        }
    }))
}

fn allocated_size(meta: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.blocks().saturating_mul(512)
    }
    #[cfg(not(unix))]
    {
        meta.len()
    }
}

fn hard_link_key(meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (meta.nlink() > 1).then(|| (meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

fn metadata_device(meta: &std::fs::Metadata) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(meta.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

/// Filesystem device of a path (unix only).
pub fn device_of(path: &Path) -> Option<u64> {
    std::fs::symlink_metadata(path)
        .ok()
        .and_then(|meta| metadata_device(&meta))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write(path: &Path, size: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![1u8; size]).unwrap();
    }

    fn opts() -> ExploreOptions {
        ExploreOptions {
            big_file_min: 300_000,
            keep_dir_min: 0,
            io_priority: IoPriority::Low,
        }
    }

    #[test]
    fn aggregates_hidden_dirs_and_finds_builds_files_and_worktrees() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        write(&root.join(".local/share/app/data.bin"), 200_000);
        write(&root.join("proj/Cargo.toml"), 10);
        write(&root.join("proj/target/debug/big.rlib"), 400_000);
        write(&root.join("notproj/target/x"), 10);
        write(&root.join("wt/.git"), 0);
        fs::write(root.join("wt/.git"), "gitdir: /repo/.git/worktrees/wt\n").unwrap();
        write(&root.join("sub/.git"), 0);
        fs::write(root.join("sub/.git"), "gitdir: ../.git/modules/sub\n").unwrap();

        let data = explore(std::slice::from_ref(&root), opts(), |_| {});

        let local = data.dirs.get(&root.join(".local")).expect(".local counted");
        assert!(local.bytes >= 200_000);
        let root_stat = data.dirs[&root];
        assert!(root_stat.bytes >= 600_000);
        assert!(root_stat.newest > 0);

        assert_eq!(data.build_dirs.len(), 1);
        assert_eq!(data.build_dirs[0].path, root.join("proj/target"));
        assert_eq!(data.big_files.len(), 1);
        assert!(data.big_files[0].path.ends_with("big.rlib"));
        assert_eq!(
            data.worktrees,
            vec![Worktree {
                path: root.join("wt"),
                main_repo: Some(PathBuf::from("/repo")),
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn counts_hard_links_once() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        write(&root.join("a/file.bin"), 500_000);
        fs::create_dir_all(root.join("b")).unwrap();
        fs::hard_link(root.join("a/file.bin"), root.join("b/file.bin")).unwrap();

        let data = explore(std::slice::from_ref(&root), opts(), |_| {});
        let total = data.dirs[&root].bytes;
        assert!(total < 1_000_000, "hard link counted twice: {total}");
    }

    #[test]
    fn nested_roots_are_walked_once() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        write(&root.join("inner/f.bin"), 1000);
        let data = explore(&[root.clone(), root.join("inner")], opts(), |_| {});
        assert_eq!(data.roots, vec![root]);
    }
}
