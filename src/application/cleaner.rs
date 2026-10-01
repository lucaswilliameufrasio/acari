use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::domain::{AppEvent, CleanTarget};
use crate::infrastructure::cleaner as infra_cleaner;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanMode {
    Execute,
    DryRun,
}

pub type CancellationToken = Arc<AtomicBool>;

pub fn new_cancellation_token() -> CancellationToken {
    Arc::new(AtomicBool::new(false))
}

pub fn start_background_clean(
    tx: UnboundedSender<AppEvent>,
    targets: Vec<(CleanTarget, u64, u64)>,
    mode: CleanMode,
) -> tokio::task::JoinHandle<()> {
    start_background_clean_with_cancel(tx, targets, mode, new_cancellation_token())
}

pub fn start_background_clean_with_cancel(
    tx: UnboundedSender<AppEvent>,
    targets: Vec<(CleanTarget, u64, u64)>,
    mode: CleanMode,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut cleaned_targets = 0_u64;
        let mut reclaimed_bytes = 0_u64;
        let mut errors = 0_u64;

        let total_targets = targets.len() as u64;
        for (target, estimated_bytes, estimated_entries) in targets {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let target_name = target.name.to_string();
            let _ = tx.send(AppEvent::CleaningProgress {
                target_name: target_name.clone(),
                completed_targets: cleaned_targets,
                total_targets,
                elapsed_seconds: 0,
            });
            let result = {
                let mut report_progress = |elapsed_seconds: u64| {
                    let _ = tx.send(AppEvent::CleaningProgress {
                        target_name: target_name.clone(),
                        completed_targets: cleaned_targets,
                        total_targets,
                        elapsed_seconds,
                    });
                };
                infra_cleaner::clean_target_with_progress(
                    &target,
                    estimated_bytes,
                    estimated_entries,
                    mode,
                    &mut report_progress,
                    &cancel,
                )
            };
            cleaned_targets = cleaned_targets.saturating_add(1);
            reclaimed_bytes = reclaimed_bytes.saturating_add(result.reclaimed_bytes);
            errors = errors.saturating_add(result.errors);

            let _ = tx.send(AppEvent::TargetCleaned {
                target_name: result.target.name.to_string(),
                reclaimed_bytes: result.reclaimed_bytes,
                removed_entries: result.removed_entries,
                errors: result.errors,
                error_detail: result.error_detail.clone(),
            });
        }

        let _ = tx.send(AppEvent::CleaningFinished {
            cleaned_targets,
            reclaimed_bytes,
            errors,
            cancelled: cancel.load(Ordering::Relaxed),
        });
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn start_background_privileged_clean(
    tx: UnboundedSender<AppEvent>,
    target: CleanTarget,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let name = target.name.to_string();
        let _ = tx.send(AppEvent::CleaningProgress {
            target_name: name.clone(),
            completed_targets: 0,
            total_targets: 1,
            elapsed_seconds: 0,
        });
        let result = crate::infrastructure::privileged::operation_for_target(&name)
            .ok_or_else(|| "target has no approved privileged operation".to_string())
            .and_then(crate::infrastructure::privileged::run_operation);
        let (reclaimed_bytes, errors, error_detail) = match result {
            Ok(()) => (0, 0, None),
            Err(error) => (0, 1, Some(error)),
        };
        let _ = tx.send(AppEvent::TargetCleaned {
            target_name: name,
            reclaimed_bytes,
            removed_entries: 0,
            errors,
            error_detail,
        });
        let _ = tx.send(AppEvent::CleaningFinished {
            cleaned_targets: 1,
            reclaimed_bytes,
            errors,
            cancelled: false,
        });
    })
}
