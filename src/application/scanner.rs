use tokio::sync::mpsc::UnboundedSender;

use crate::domain::{AppEvent, CleanTarget};
use crate::infrastructure::scanner as infra_scanner;

pub fn start_background_scan(
    tx: UnboundedSender<AppEvent>,
    targets: Vec<CleanTarget>,
    excludes: Vec<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        for target in targets {
            let result = infra_scanner::scan_target(&target, &tx, &excludes);
            let _ = tx.send(AppEvent::TargetCompleted {
                target_name: result.target.name.to_string(),
                total_bytes: result.bytes,
                files_scanned: result.files_scanned,
            });
        }
        let _ = tx.send(AppEvent::ScanFinished);
    })
}
