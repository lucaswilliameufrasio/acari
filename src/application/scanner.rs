use std::sync::Arc;

use jwalk::Parallelism;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::target_config::IoPriority;
use crate::domain::{AppEvent, CleanTarget};
use crate::infrastructure::scanner as infra_scanner;

fn io_parallelism(io: IoPriority) -> Parallelism {
    match io {
        IoPriority::Low => Parallelism::Serial,
        IoPriority::Normal | IoPriority::High => Parallelism::RayonDefaultPool {
            busy_timeout: std::time::Duration::from_secs(1),
        },
    }
}

pub fn start_background_scan(
    tx: UnboundedSender<AppEvent>,
    targets: Vec<CleanTarget>,
    excludes: Vec<String>,
    io_priority: IoPriority,
) -> tokio::task::JoinHandle<()> {
    let chunk_size = match io_priority {
        IoPriority::High => 4,
        IoPriority::Normal | IoPriority::Low => 1,
    };
    let parallelism = io_parallelism(io_priority);

    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        if io_priority == IoPriority::Low {
            let _ = std::process::Command::new("ionice")
                .args(["-c", "3", "-p", &std::process::id().to_string()])
                .output();
        }

        let tx = Arc::new(tx);

        for chunk in targets.chunks(chunk_size) {
            let tx = Arc::clone(&tx);
            let excludes = Arc::new(excludes.clone());
            let parallelism = parallelism.clone();

            std::thread::scope(|s| {
                for target in chunk {
                    let tx = Arc::clone(&tx);
                    let excludes = Arc::clone(&excludes);
                    let p = parallelism.clone();
                    let target = target.clone();
                    s.spawn(move || {
                        let result =
                            infra_scanner::scan_target(&target, &tx, &excludes, p);
                        let _ = tx.send(AppEvent::TargetCompleted {
                            target_name: result.target.name.to_string(),
                            total_bytes: result.bytes,
                            files_scanned: result.files_scanned,
                        });
                    });
                }
            });
        }

        let _ = tx.send(AppEvent::ScanFinished);
    })
}
