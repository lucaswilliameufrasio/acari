use std::collections::HashMap;

use anyhow::Result;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::application::cleaner::{CleanMode, start_background_clean};
use crate::application::headless::output::{
    print_cleaning_finished, print_scan_finished, print_scan_progress, print_start_cleaning,
    print_target_cleaned, print_target_done,
};
use crate::domain::{AppEvent, CleanTarget};
use crate::i18n::Language;
use crate::infrastructure::history;

pub mod output;

pub async fn run_headless(
    tx: UnboundedSender<AppEvent>,
    mut rx: UnboundedReceiver<AppEvent>,
    targets: Vec<CleanTarget>,
    clean_after_scan: bool,
    clean_mode: CleanMode,
    lang: Language,
) -> Result<()> {
    let mut total_bytes = 0_u64;
    let mut completed: HashMap<String, (CleanTarget, u64, u64)> = HashMap::new();
    let mut waiting_clean_finish = false;
    let target_lookup: HashMap<String, CleanTarget> = targets
        .iter()
        .cloned()
        .map(|target| (target.name.to_string(), target))
        .collect();

    while let Some(event) = rx.recv().await {
        match event {
            AppEvent::ScanProgress {
                target_name,
                bytes_found,
                files_scanned,
            } => {
                print_scan_progress(&target_name, bytes_found, files_scanned, lang);
            }
            AppEvent::TargetCompleted {
                target_name,
                total_bytes: bytes,
                files_scanned,
            } => {
                total_bytes = total_bytes.saturating_add(bytes);
                print_target_done(&target_name, bytes, files_scanned, lang);

                if let Some(target) = target_lookup.get(&target_name) {
                    completed.insert(target_name, (target.clone(), bytes, files_scanned));
                }
            }
            AppEvent::ScanFinished => {
                print_scan_finished(total_bytes, lang);

                if clean_after_scan {
                    let selected: Vec<(CleanTarget, u64, u64)> =
                        completed.values().cloned().collect();
                    if selected.is_empty() {
                        break;
                    }

                    print_start_cleaning(selected.len(), clean_mode, lang);
                    let _clean_handle = start_background_clean(tx.clone(), selected, clean_mode);
                    waiting_clean_finish = true;
                } else {
                    break;
                }
            }
            AppEvent::TargetCleaned {
                target_name,
                reclaimed_bytes,
                removed_entries,
                errors,
            } => {
                if waiting_clean_finish {
                    print_target_cleaned(
                        &target_name,
                        reclaimed_bytes,
                        removed_entries,
                        errors,
                        clean_mode,
                        lang,
                    );
                }
            }
            AppEvent::CleaningFinished {
                cleaned_targets,
                reclaimed_bytes,
                errors,
            } => {
                if waiting_clean_finish {
                    print_cleaning_finished(cleaned_targets, reclaimed_bytes, errors, clean_mode, lang);
                    if clean_mode == CleanMode::Execute {
                        let time = history::format_local_time();
                        history::append_entry(&format!(
                            "{time} | Clean completed | targets={cleaned_targets} reclaimed={reclaimed_bytes} errors={errors}"
                        ));
                    }
                }
                break;
            }
            AppEvent::Tick => {}
        }
    }

    Ok(())
}
