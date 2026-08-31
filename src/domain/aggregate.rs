//! Aggregation of per-target scan results that avoids double counting.
//!
//! Two scanned targets overlap when they resolve to the same canonical path
//! (exact duplicates) or when one path is an ancestor of another (nested
//! targets, e.g. `User Caches` at `~/Library/Caches` containing `Homebrew
//! Cache` at `~/Library/Caches/Homebrew`). Overlapping bytes must reach the
//! grand total exactly once: broad ancestors then display only the remainder
//! their nested targets do not cover.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One aggregated entry, preserving the input order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateEntry {
    pub name: String,
    /// Canonical path when the target scans a directory; `None` for command
    /// targets, which always count towards the total.
    pub path: Option<PathBuf>,
    /// Bytes reported by the scan for this target.
    pub raw_bytes: u64,
    /// Bytes that go into the grand total. Zero for exact duplicates and for
    /// targets nested inside another target (their bytes are already counted
    /// by the top-most ancestor).
    pub counted_bytes: u64,
    /// Bytes to display: raw bytes minus the bytes covered by nested targets
    /// (the "remainder" for broad parents like `User Caches`).
    pub shown_bytes: u64,
    /// Name of the entry this one duplicates, when `counted_bytes` is zero
    /// because another target resolves to the same canonical path.
    pub duplicate_of: Option<String>,
    /// Name of the closest ancestor that also has a target, when
    /// `counted_bytes` is zero because this path nests inside it.
    pub covered_by: Option<String>,
    /// True when `shown_bytes` is smaller than `raw_bytes` because nested
    /// targets cover part of this target's bytes.
    pub remainder: bool,
}

/// Aggregated output: one entry per input (same order) plus the corrected
/// grand total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateOutput {
    pub entries: Vec<AggregateEntry>,
    pub total_bytes: u64,
}

fn canonical(path: &PathBuf) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.clone())
}

fn is_ancestor(parent: &Path, child: &Path) -> bool {
    parent != child && child.starts_with(parent)
}

fn component_count(path: &Path) -> usize {
    path.components().count()
}

/// Aggregate scanned entries, counting overlapping bytes exactly once.
///
/// Each input is `(name, raw path or None for command targets, raw bytes)`.
/// The output preserves input order and reports, for every entry, the bytes
/// that count towards the total and the bytes that should be displayed.
pub fn aggregate_scan(entries: Vec<(String, Option<PathBuf>, u64)>) -> AggregateOutput {
    let n = entries.len();

    // Canonicalize paths (resolves symlinks and e.g. /var -> /private/var).
    let paths: Vec<Option<PathBuf>> = entries
        .iter()
        .map(|(_, path, _)| path.as_ref().map(canonical))
        .collect();

    // Pass 1: exact duplicates by canonical path. The first occurrence wins.
    let mut duplicate_of: Vec<Option<String>> = vec![None; n];
    let mut first_by_path: HashMap<&PathBuf, usize> = HashMap::new();
    for i in 0..n {
        if let Some(path) = paths[i].as_ref() {
            match first_by_path.get(path) {
                Some(&first) => duplicate_of[i] = Some(entries[first].0.clone()),
                None => {
                    first_by_path.insert(path, i);
                }
            }
        }
    }

    // Pass 2: closest ancestor among the surviving targets.
    let mut parent_idx: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        if duplicate_of[i].is_some() {
            continue;
        }
        let Some(child) = paths[i].as_ref() else {
            continue;
        };
        let mut best: Option<(usize, usize)> = None;
        for j in 0..n {
            if j == i || duplicate_of[j].is_some() {
                continue;
            }
            let Some(candidate) = paths[j].as_ref() else {
                continue;
            };
            if is_ancestor(candidate, child) {
                let depth = component_count(candidate);
                if best.is_none() || depth > best.expect("checked above").1 {
                    best = Some((j, depth));
                }
            }
        }
        if let Some((j, _)) = best {
            parent_idx[i] = Some(j);
        }
    }

    // Pass 3: remainder, computed from the deepest targets up.
    let mut deepest_first: Vec<usize> = (0..n)
        .filter(|&i| duplicate_of[i].is_none() && paths[i].is_some())
        .collect();
    deepest_first.sort_by_key(|&i| {
        let path = paths[i].as_ref().expect("filtered above");
        std::cmp::Reverse(component_count(path))
    });

    let mut shown: Vec<u64> = entries.iter().map(|(_, _, bytes)| *bytes).collect();
    let mut remainder = vec![false; n];
    for &i in &deepest_first {
        let raw = entries[i].2;
        // Subtract the raw bytes of direct children: each tree level then
        // displays only its own "ring", so the displayed sizes add up to the
        // grand total (raw of a direct child already includes its own
        // nested targets).
        let children_raw: u64 = (0..n)
            .filter(|&c| parent_idx[c] == Some(i) && duplicate_of[c].is_none())
            .map(|c| entries[c].2)
            .sum();
        shown[i] = raw.saturating_sub(children_raw);
        remainder[i] = children_raw > 0 && shown[i] < raw;
    }

    let mut out_entries = Vec::with_capacity(n);
    let mut total = 0_u64;
    for i in 0..n {
        let (name, _, raw) = &entries[i];
        let is_duplicate = duplicate_of[i].is_some();
        let is_nested = parent_idx[i].is_some();
        // Only the top-most ancestor counts a nested path towards the total;
        // duplicates never count. Every surviving entry keeps displaying its
        // own remainder (nested children keep their full raw size).
        let counted = if is_duplicate || is_nested { 0 } else { *raw };
        let shown_bytes = if is_duplicate { 0 } else { shown[i] };
        total = total.saturating_add(counted);
        out_entries.push(AggregateEntry {
            name: name.clone(),
            path: paths[i].clone(),
            raw_bytes: *raw,
            counted_bytes: counted,
            shown_bytes,
            duplicate_of: duplicate_of[i].clone(),
            covered_by: parent_idx[i].map(|j| entries[j].0.clone()),
            remainder: remainder[i],
        });
    }

    AggregateOutput {
        entries: out_entries,
        total_bytes: total,
    }
}

#[cfg(test)]
mod tests {
    use super::aggregate_scan;
    use std::path::PathBuf;

    fn path(p: &str) -> Option<PathBuf> {
        Some(PathBuf::from(p))
    }

    #[test]
    fn nested_parent_shows_remainder_and_counts_once() {
        let out = aggregate_scan(vec![
            ("Parent".into(), path("/tmp/acari-x"), 100),
            ("Child".into(), path("/tmp/acari-x/a"), 60),
        ]);

        assert_eq!(out.total_bytes, 100);
        let parent = &out.entries[0];
        assert_eq!(parent.shown_bytes, 40);
        assert!(parent.remainder);
        assert_eq!(parent.counted_bytes, 100);
        let child = &out.entries[1];
        assert_eq!(child.shown_bytes, 60);
        assert!(!child.remainder);
        assert_eq!(child.covered_by.as_deref(), Some("Parent"));
        assert_eq!(child.counted_bytes, 0);
    }

    #[test]
    fn exact_duplicate_is_counted_once() {
        let out = aggregate_scan(vec![
            ("First".into(), path("/tmp/acari-dup"), 50),
            ("Second".into(), path("/tmp/acari-dup"), 50),
        ]);

        assert_eq!(out.total_bytes, 50);
        let second = &out.entries[1];
        assert_eq!(second.duplicate_of.as_deref(), Some("First"));
        assert_eq!(second.counted_bytes, 0);
        assert_eq!(second.shown_bytes, 0);
    }

    #[test]
    fn three_level_chain_counts_once() {
        let out = aggregate_scan(vec![
            ("Root".into(), path("/tmp/acari-chain"), 100),
            ("Mid".into(), path("/tmp/acari-chain/sub"), 70),
            ("Leaf".into(), path("/tmp/acari-chain/sub/leaf"), 30),
        ]);

        assert_eq!(out.total_bytes, 100);
        assert_eq!(out.entries[0].shown_bytes, 30);
        assert_eq!(out.entries[1].shown_bytes, 40);
        assert_eq!(out.entries[2].shown_bytes, 30);
        assert_eq!(out.entries[1].covered_by.as_deref(), Some("Root"));
        assert_eq!(out.entries[2].covered_by.as_deref(), Some("Mid"));
        // Each level displays only its own ring: displayed sizes add up to
        // the grand total exactly once.
        let displayed: u64 = out.entries.iter().map(|e| e.shown_bytes).sum();
        assert_eq!(displayed, out.total_bytes);
    }

    #[test]
    fn command_targets_always_count_towards_total() {
        let out = aggregate_scan(vec![
            ("Cmd".into(), None, 10),
            ("Parent".into(), path("/tmp/acari-cmd"), 100),
            ("Child".into(), path("/tmp/acari-cmd/a"), 60),
        ]);

        assert_eq!(out.total_bytes, 110);
        assert_eq!(out.entries[0].counted_bytes, 10);
        assert_eq!(out.entries[0].shown_bytes, 10);
    }

    #[test]
    fn sibling_paths_do_not_cover_each_other() {
        let out = aggregate_scan(vec![
            ("A".into(), path("/tmp/acari-sib/a"), 10),
            ("B".into(), path("/tmp/acari-sib/b"), 20),
        ]);

        assert_eq!(out.total_bytes, 30);
        assert_eq!(out.entries[0].shown_bytes, 10);
        assert_eq!(out.entries[1].shown_bytes, 20);
    }

    #[test]
    fn shared_text_prefix_is_not_an_ancestor() {
        let out = aggregate_scan(vec![
            ("Bar".into(), path("/tmp/acari-pfx/bar"), 10),
            ("Barquad".into(), path("/tmp/acari-pfx/barquad"), 20),
        ]);

        // /x/bar is an ancestor of /x/bar/qux, but NOT of /x/barquad: path
        // ancestry must compare path components, not string prefixes.
        assert_eq!(out.total_bytes, 30);
        assert!(out.entries[1].covered_by.is_none());
    }

    #[test]
    fn child_larger_than_parent_clamps_remainder() {
        let out = aggregate_scan(vec![
            ("Parent".into(), path("/tmp/acari-clamp"), 50),
            ("Child".into(), path("/tmp/acari-clamp/a"), 60),
        ]);

        assert_eq!(out.total_bytes, 50);
        assert_eq!(out.entries[0].shown_bytes, 0);
        assert!(out.entries[0].remainder);
    }

    #[test]
    fn input_order_is_preserved() {
        let out = aggregate_scan(vec![
            ("Zed".into(), path("/tmp/acari-order/zed"), 1),
            ("Alpha".into(), path("/tmp/acari-order"), 2),
            ("Mid".into(), None, 3),
        ]);

        assert_eq!(out.entries[0].name, "Zed");
        assert_eq!(out.entries[1].name, "Alpha");
        assert_eq!(out.entries[2].name, "Mid");
    }
}
