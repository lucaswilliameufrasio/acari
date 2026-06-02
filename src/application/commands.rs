use anyhow::{Result, bail};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::application::scanner::start_background_scan;
use crate::config::target_config;
use crate::domain::{AppEvent, CleanTarget, append_custom_scan_paths, build_targets};
use crate::i18n::{Language, msg};

pub fn prepare_targets(
    filter_names: &[String],
    scan_paths: &[String],
    custom_targets: &[target_config::CustomTargetEntry],
) -> Vec<CleanTarget> {
    let mut targets = build_targets(filter_names, custom_targets);
    append_custom_scan_paths(&mut targets, scan_paths);
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
) -> (
    UnboundedSender<AppEvent>,
    UnboundedReceiver<AppEvent>,
    tokio::task::JoinHandle<()>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let handle = start_background_scan(tx.clone(), targets, excludes);
    (tx, rx, handle)
}

pub fn merge_excludes(cli_excludes: &[String], config_excludes: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    for e in cli_excludes {
        if !merged.contains(e) {
            merged.push(e.clone());
        }
    }
    for e in config_excludes {
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
        let result = enforce_headless_clean_safety_l10n(true, true, false, false, Language::English);
        assert!(result.is_err());
    }

    #[test]
    fn safety_allows_dry_run_without_yes() {
        let result = enforce_headless_clean_safety_l10n(true, true, true, false, Language::English);
        assert!(result.is_ok());
    }
}
