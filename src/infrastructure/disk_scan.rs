//! Read-only filesystem scan used by the desktop disk-usage view.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use jwalk::{Parallelism, WalkDir};
use rayon::{ThreadPool, ThreadPoolBuilder};

use crate::config::target_config::IoPriority;

#[derive(Debug, Clone)]
pub struct DiskNode {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
    pub children: Vec<DiskNode>,
}

#[derive(Debug, Clone, Copy)]
pub struct ScanProgress {
    pub entries: u64,
    pub bytes: u64,
}

/// Walk a selected directory without following symlinks and aggregate a complete tree.
pub fn scan_tree(root: &Path, progress: Option<Sender<ScanProgress>>) -> std::io::Result<DiskNode> {
    scan_tree_cancellable_with_priority(root, progress, None, IoPriority::Normal)
}

pub fn scan_tree_cancellable(
    root: &Path,
    progress: Option<Sender<ScanProgress>>,
    cancel: Option<Arc<AtomicBool>>,
) -> std::io::Result<DiskNode> {
    scan_tree_cancellable_with_priority(root, progress, cancel, IoPriority::Normal)
}

pub fn scan_tree_cancellable_with_priority(
    root: &Path,
    progress: Option<Sender<ScanProgress>>,
    cancel: Option<Arc<AtomicBool>>,
    io_priority: IoPriority,
) -> std::io::Result<DiskNode> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "selected path is not a directory",
        ));
    }

    // Collect flat nodes first; aggregating ancestors as files arrive avoids a second walk.
    let mut nodes: HashMap<PathBuf, DiskNode> = HashMap::new();
    nodes.insert(
        root.clone(),
        DiskNode {
            name: root.file_name().map_or_else(
                || root.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            path: root.clone(),
            bytes: 0,
            is_dir: true,
            children: Vec::new(),
        },
    );
    let mut entries = 0_u64;
    let mut bytes = 0_u64;
    // jwalk must not compete for the process-wide Rayon pool: it can silently
    // stop producing entries under contention. Give this scan its own budget.
    let pool = build_walk_pool(io_priority);
    let walker =
        WalkDir::new(&root)
            .follow_links(false)
            .parallelism(Parallelism::RayonExistingPool {
                pool,
                busy_timeout: None,
            });

    for result in walker {
        if cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "scan cancelled",
            ));
        }
        let Ok(entry) = result else { continue };
        let path = entry.path();
        if path == root {
            continue;
        }
        let is_dir = entry.file_type().is_dir();
        let size = if entry.file_type().is_file() {
            entry.metadata().map_or(0, |metadata| metadata.len())
        } else {
            0
        };
        let name = entry.file_name.to_string_lossy().into_owned();
        nodes.entry(path.clone()).or_insert(DiskNode {
            name,
            path: path.clone(),
            bytes: size,
            is_dir,
            children: Vec::new(),
        });
        entries = entries.saturating_add(1);
        bytes = bytes.saturating_add(size);
        if entries.is_multiple_of(512)
            && let Some(tx) = &progress
        {
            let _ = tx.send(ScanProgress { entries, bytes });
        }
    }

    // Aggregate only after the walk: jwalk may yield a file before its parent
    // directory, so updating ancestors inline can miss bytes on large trees.
    let files: Vec<_> = nodes
        .values()
        .filter(|node| !node.is_dir)
        .map(|node| (node.path.clone(), node.bytes))
        .collect();
    for node in nodes.values_mut().filter(|node| node.is_dir) {
        node.bytes = 0;
    }
    for (path, size) in files {
        let mut parent = path.parent();
        while let Some(dir) = parent {
            if let Some(node) = nodes.get_mut(dir) {
                node.bytes = node.bytes.saturating_add(size);
            }
            if dir == root {
                break;
            }
            parent = dir.parent();
        }
    }

    // Attach deep-to-shallow using the flat path index. The parent is still in
    // the map when each child is moved, avoiding a recursive tree search for
    // every entry (which becomes quadratic on wide/deep trees).
    let mut paths: Vec<_> = nodes
        .keys()
        .filter(|path| **path != root)
        .cloned()
        .collect();
    paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in paths {
        if let Some(node) = nodes.remove(&path)
            && let Some(parent) = path.parent()
            && let Some(parent_node) = nodes.get_mut(parent)
        {
            parent_node.children.push(node);
        }
    }
    let mut root_node = nodes.remove(&root).expect("root node inserted");
    sort_tree(&mut root_node);
    if let Some(tx) = progress {
        let _ = tx.send(ScanProgress { entries, bytes });
    }
    Ok(root_node)
}

fn build_walk_pool(priority: IoPriority) -> Arc<ThreadPool> {
    let cores = std::thread::available_parallelism().map_or(4, usize::from);
    let threads = match priority {
        IoPriority::High => cores,
        IoPriority::Normal => (cores / 2).max(1),
        IoPriority::Low => 1,
    };
    Arc::new(
        ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|index| format!("acari-disk-walk-{index}"))
            .build()
            .expect("build dedicated disk scan pool"),
    )
}

/// Return usable mounted filesystem roots known to the host OS.
pub fn mounted_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let Ok(contents) = std::fs::read_to_string("/proc/mounts") else {
            return Vec::new();
        };
        let mut roots: Vec<PathBuf> = contents
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1))
            .map(|mount| PathBuf::from(mount.replace("\\040", " ")))
            .filter(|mount| mount.is_dir())
            .filter(|mount| {
                !mount.starts_with("/proc")
                    && !mount.starts_with("/sys")
                    && !mount.starts_with("/dev")
            })
            .collect();
        roots.sort();
        roots.dedup();
        roots
    }
    #[cfg(target_os = "macos")]
    {
        std::fs::read_dir("/Volumes")
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Vec::new()
    }
}

fn sort_tree(node: &mut DiskNode) {
    node.children
        .sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    for child in &mut node.children {
        sort_tree(child);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::scan_tree;

    #[test]
    fn builds_hierarchy_and_aggregates_sizes() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("nested")).unwrap();
        fs::write(temp.path().join("top.txt"), b"12345").unwrap();
        fs::write(temp.path().join("nested/file.txt"), b"1234567").unwrap();
        let tree = scan_tree(temp.path(), None).unwrap();
        assert_eq!(tree.bytes, 12);
        assert_eq!(tree.children.len(), 2);
        let nested = tree.children.iter().find(|child| child.is_dir).unwrap();
        assert_eq!(nested.bytes, 7);
        assert_eq!(nested.children[0].bytes, 7);
    }

    #[test]
    fn rejects_file_paths() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("file");
        fs::write(&file, "x").unwrap();
        assert!(scan_tree(&file, None).is_err());
    }
}
