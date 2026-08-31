//! `du`-style aggregation: walk a tree once and report its largest
//! directories, each one aggregating everything beneath it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jwalk::{Parallelism, WalkDir};

/// One aggregated directory: total bytes and file count of every file inside
/// it, recursively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuEntry {
    pub path: PathBuf,
    pub bytes: u64,
    pub files: u64,
}

/// Walk `root` and return the `top` largest directories holding at least
/// `min_size` bytes, largest first. Like `du | sort -rh | head`, ancestors
/// aggregate their descendants, so the root usually appears first.
pub fn largest_dirs(root: &Path, top: usize, min_size: u64) -> Vec<DuEntry> {
    // Own bytes/files per directory (files directly inside, not recursive).
    let mut own: HashMap<PathBuf, (u64, u64)> = HashMap::new();
    own.insert(root.to_path_buf(), (0, 0));

    let walker =
        WalkDir::new(root)
            .follow_links(false)
            .parallelism(Parallelism::RayonDefaultPool {
                busy_timeout: Duration::from_secs(60),
            });

    for entry in walker {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let bytes = entry.metadata().map_or(0, |meta| meta.len());
        let parent = entry.parent_path().to_path_buf();
        let slot = own.entry(parent).or_insert((0, 0));
        slot.0 = slot.0.saturating_add(bytes);
        slot.1 = slot.1.saturating_add(1);
    }

    // Propagate totals bottom-up: deepest directories first, each one adding
    // itself to its direct parent.
    let mut items: Vec<(PathBuf, u64, u64)> = own
        .into_iter()
        .map(|(path, (bytes, files))| (path, bytes, files))
        .collect();
    items.sort_by_key(|(path, _, _)| std::cmp::Reverse(path.components().count()));

    let index: HashMap<PathBuf, usize> = items
        .iter()
        .enumerate()
        .map(|(i, (path, _, _))| (path.clone(), i))
        .collect();
    for i in 0..items.len() {
        let Some(parent) = items[i].0.parent().map(Path::to_path_buf) else {
            continue;
        };
        if let Some(&pi) = index.get(&parent)
            && pi != i
        {
            items[pi].1 = items[pi].1.saturating_add(items[i].1);
            items[pi].2 = items[pi].2.saturating_add(items[i].2);
        }
    }

    items.sort_by_key(|&(_, bytes, _)| std::cmp::Reverse(bytes));
    items
        .into_iter()
        .filter(|&(_, bytes, _)| bytes >= min_size)
        .take(top)
        .map(|(path, bytes, files)| DuEntry { path, bytes, files })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::largest_dirs;

    fn write(path: &PathBuf, size: usize) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create dirs");
        fs::write(path, vec![0u8; size]).expect("write file");
    }

    #[test]
    fn aggregates_nested_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        write(&root.join("big.bin"), 10_000_000);
        write(&root.join("a").join("mid.bin"), 5_000_000);
        write(&root.join("a").join("sub").join("small.bin"), 2_000_000);
        write(&root.join("b").join("other.bin"), 5_000_000);

        let entries = largest_dirs(&root, 20, 1);

        assert_eq!(entries.len(), 4);
        // Largest first, ancestors aggregating descendants.
        assert_eq!(entries[0].path, root);
        assert_eq!(entries[0].bytes, 22_000_000);
        assert_eq!(entries[0].files, 4);
        assert_eq!(entries[1].path, root.join("a"));
        assert_eq!(entries[1].bytes, 7_000_000);
        assert_eq!(entries[2].path, root.join("b"));
        assert_eq!(entries[2].bytes, 5_000_000);
        assert_eq!(entries[3].path, root.join("a").join("sub"));
        assert_eq!(entries[3].bytes, 2_000_000);
    }

    #[test]
    fn respects_top_and_min_size() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        write(&root.join("big.bin"), 10_000_000);
        write(&root.join("a").join("mid.bin"), 2_000_000);
        write(&root.join("b").join("small.bin"), 100_000);

        // Only directories with at least 1 MB are listed.
        let entries = largest_dirs(&root, 20, 1_000_000);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, root);
        assert_eq!(entries[1].path, root.join("a"));

        // Top limits the number of results.
        let entries = largest_dirs(&root, 1, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, root);
    }

    #[test]
    fn empty_directory_returns_only_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let entries = largest_dirs(temp.path(), 20, 1);
        assert_eq!(entries.len(), 0, "root has 0 bytes and min_size is 1");
    }
}
