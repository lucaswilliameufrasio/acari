use anyhow::{Result, bail};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::application::scanner::start_background_scan;
use crate::config::target_config;
use crate::config::target_config::IoPriority;
use crate::domain::{AppEvent, CleanTarget, append_custom_scan_paths, build_targets};
use crate::i18n::{Language, msg};

pub fn is_safe_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains("..") {
        return false;
    }
    if trimmed == "/" {
        return false;
    }
    let sensitive = [
        "/etc", "/var", "/sys", "/proc", "/dev", "/boot", "/bin", "/sbin", "/lib", "/lib64",
    ];
    if sensitive.contains(&trimmed) {
        return false;
    }
    true
}

pub fn prepare_targets(
    filter_names: &[String],
    scan_paths: &[String],
    custom_targets: &[target_config::CustomTargetEntry],
) -> Vec<CleanTarget> {
    let safe_custom: Vec<target_config::CustomTargetEntry> = custom_targets
        .iter()
        .filter(|ct| {
            if !is_safe_path(&ct.path) {
                eprintln!(
                    "warning: skipping unsafe custom target '{}' (path: {})",
                    ct.name, ct.path
                );
                false
            } else {
                true
            }
        })
        .cloned()
        .collect();
    let mut targets = build_targets(filter_names, &safe_custom);
    let safe_scan_paths: Vec<String> = scan_paths
        .iter()
        .filter(|p| {
            if !is_safe_path(p) {
                eprintln!("warning: skipping unsafe scan path: {p}");
                false
            } else {
                true
            }
        })
        .cloned()
        .collect();
    append_custom_scan_paths(&mut targets, &safe_scan_paths);
    targets
}

pub fn print_targets(targets: &[CleanTarget], lang: Language) {
    for target in targets {
        let origin = if target.description == "User provided path" {
            msg::target_list_custom(lang)
        } else {
            msg::target_list_builtin(lang)
        };
        println!("{} {}", target.name, origin);
        println!("  {}{}", msg::target_path(lang), target.path);
        println!("  {}{}", msg::target_desc(lang), target.description);
        println!();
    }
}

pub fn start_scan(
    targets: Vec<CleanTarget>,
    excludes: Vec<String>,
    io_priority: IoPriority,
) -> (
    UnboundedSender<AppEvent>,
    UnboundedReceiver<AppEvent>,
    tokio::task::JoinHandle<()>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let handle = start_background_scan(tx.clone(), targets, excludes, io_priority);
    (tx, rx, handle)
}

const MAX_EXCLUDE_LEN: usize = 256;

pub fn merge_excludes(cli_excludes: &[String], config_excludes: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    for e in cli_excludes {
        if e.is_empty() {
            eprintln!("warning: empty exclude pattern from CLI ignored");
            continue;
        }
        if e.len() > MAX_EXCLUDE_LEN {
            eprintln!(
                "warning: exclude pattern too long (max {MAX_EXCLUDE_LEN} chars), ignored: {e}"
            );
            continue;
        }
        if !merged.contains(e) {
            merged.push(e.clone());
        }
    }
    for e in config_excludes {
        if e.is_empty() {
            eprintln!("warning: empty exclude pattern in config ignored");
            continue;
        }
        if e.len() > MAX_EXCLUDE_LEN {
            eprintln!(
                "warning: exclude pattern too long (max {MAX_EXCLUDE_LEN} chars), ignored: {e}"
            );
            continue;
        }
        if !merged.contains(e) {
            merged.push(e.clone());
        }
    }
    merged
}

pub fn enforce_headless_clean_safety_l10n(
    is_headless: bool,
    clean: bool,
    dry_run: bool,
    yes: bool,
    lang: Language,
) -> Result<()> {
    if is_headless && clean && !dry_run && !yes {
        bail!("{}", msg::safety_refuse(lang));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{enforce_headless_clean_safety_l10n, prepare_targets};
    use crate::i18n::Language;

    #[test]
    fn prepare_targets_appends_custom_scan_paths() {
        let filters = vec![String::from("target-that-does-not-exist")];
        let scan_paths = vec![String::from("/tmp/acari-custom")];

        let targets = prepare_targets(&filters, &scan_paths, &[]);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "Custom Path 1");
        assert_eq!(targets[0].path, "/tmp/acari-custom");
    }

    #[test]
    fn safety_requires_yes_for_destructive_headless_clean() {
        let result =
            enforce_headless_clean_safety_l10n(true, true, false, false, Language::English);
        assert!(result.is_err());
    }

    #[test]
    fn safety_allows_dry_run_without_yes() {
        let result = enforce_headless_clean_safety_l10n(true, true, true, false, Language::English);
        assert!(result.is_ok());
    }
}
