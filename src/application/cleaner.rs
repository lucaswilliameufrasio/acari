use tokio::sync::mpsc::UnboundedSender;

use crate::domain::{AppEvent, CleanTarget};
use crate::infrastructure::cleaner as infra_cleaner;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanMode {
    Execute,
    DryRun,
}

pub fn start_background_clean(
    tx: UnboundedSender<AppEvent>,
    targets: Vec<(CleanTarget, u64, u64)>,
    mode: CleanMode,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut cleaned_targets = 0_u64;
        let mut reclaimed_bytes = 0_u64;
        let mut errors = 0_u64;

        for (target, estimated_bytes, estimated_entries) in targets {
            let result =
                infra_cleaner::clean_target(&target, estimated_bytes, estimated_entries, mode);
            cleaned_targets = cleaned_targets.saturating_add(1);
            reclaimed_bytes = reclaimed_bytes.saturating_add(result.reclaimed_bytes);
            errors = errors.saturating_add(result.errors);

            let _ = tx.send(AppEvent::TargetCleaned {
                target_name: result.target.name.to_string(),
                reclaimed_bytes: result.reclaimed_bytes,
                removed_entries: result.removed_entries,
                errors: result.errors,
            });
        }

        let _ = tx.send(AppEvent::CleaningFinished {
            cleaned_targets,
            reclaimed_bytes,
            errors,
        });
    })
}
