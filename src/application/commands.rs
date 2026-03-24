use anyhow::{Result, bail};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::application::scanner::start_background_scan;
use crate::domain::{AppEvent, CleanTarget, append_custom_scan_paths, build_targets};

pub fn prepare_targets(filter_names: &[String], scan_paths: &[String]) -> Vec<CleanTarget> {
    let mut targets = build_targets(filter_names);
    append_custom_scan_paths(&mut targets, scan_paths);
    targets
}

pub fn print_targets(targets: &[CleanTarget]) {
    for target in targets {
        println!(
            "{}\n  path: {}\n  desc: {}\n",
            target.name, target.path, target.description
        );
    }
}

pub fn start_scan(
    targets: Vec<CleanTarget>,
) -> (
    UnboundedSender<AppEvent>,
    UnboundedReceiver<AppEvent>,
    tokio::task::JoinHandle<()>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let handle = start_background_scan(tx.clone(), targets);
    (tx, rx, handle)
}

pub fn enforce_headless_clean_safety(
    is_headless: bool,
    clean: bool,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    if is_headless && clean && !dry_run && !yes {
        bail!(
            "Refusing destructive clean without --yes. Use --clean --yes to proceed, or --clean --dry-run."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{enforce_headless_clean_safety, prepare_targets};

    #[test]
    fn prepare_targets_appends_custom_scan_paths() {
        let filters = vec![String::from("target-that-does-not-exist")];
        let scan_paths = vec![String::from("/tmp/acari-custom")];

        let targets = prepare_targets(&filters, &scan_paths);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "Custom Path 1");
        assert_eq!(targets[0].path, "/tmp/acari-custom");
    }

    #[test]
    fn safety_requires_yes_for_destructive_headless_clean() {
        let result = enforce_headless_clean_safety(true, true, false, false);
        assert!(result.is_err());
    }

    #[test]
    fn safety_allows_dry_run_without_yes() {
        let result = enforce_headless_clean_safety(true, true, true, false);
        assert!(result.is_ok());
    }
}
