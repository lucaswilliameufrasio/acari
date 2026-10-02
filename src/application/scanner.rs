use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::target_config::IoPriority;
use crate::domain::{AppEvent, CleanTarget};
use crate::infrastructure::scanner as infra_scanner;

/// Build a dedicated rayon pool for a scan. We avoid the rayon *global* pool
/// because jwalk's `RayonDefaultPool` can saturate it when many targets walk the
/// filesystem concurrently and then abort the iteration silently (under-counting
/// results). A dedicated pool gives intra-directory parallelism without that
/// shared-pool hazard, and lets us budget threads per I/O priority.
fn build_walk_pool(io_priority: IoPriority) -> Arc<ThreadPool> {
    let cores = std::thread::available_parallelism().map_or(4, usize::from);
    let threads = match io_priority {
        IoPriority::High => cores,
        IoPriority::Normal => (cores / 2).max(1),
        IoPriority::Low => 1,
    };
    Arc::new(
        ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("acari-walk-{i}"))
            .build()
            .expect("build dedicated walk pool"),
    )
}

pub fn start_background_scan(
    tx: UnboundedSender<AppEvent>,
    targets: Vec<CleanTarget>,
    excludes: Vec<String>,
    io_priority: IoPriority,
    allocated: bool,
) -> tokio::task::JoinHandle<()> {
    start_background_scan_cancellable(
        tx,
        targets,
        excludes,
        io_priority,
        allocated,
        Arc::new(AtomicBool::new(false)),
    )
}

pub fn start_background_scan_cancellable(
    tx: UnboundedSender<AppEvent>,
    targets: Vec<CleanTarget>,
    excludes: Vec<String>,
    io_priority: IoPriority,
    allocated: bool,
    cancel: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    // Concurrency between targets is bounded by chunk_size, using dedicated OS
    // threads (std::thread::scope). Each walk then parallelises *within* its
    // directory on a per-scan rayon pool (not the global one).
    let chunk_size = match io_priority {
        IoPriority::High => 4,
        IoPriority::Normal | IoPriority::Low => 1,
    };

    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        if io_priority == IoPriority::Low {
            let _ = std::process::Command::new("ionice")
                .args(["-c", "3", "-p", &std::process::id().to_string()])
                .output();
        }

        let pool = build_walk_pool(io_priority);
        let tx = Arc::new(tx);

        for chunk in targets.chunks(chunk_size) {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let tx = Arc::clone(&tx);
            let excludes = Arc::new(excludes.clone());
            let pool = Arc::clone(&pool);

            std::thread::scope(|s| {
                for target in chunk {
                    let tx = Arc::clone(&tx);
                    let excludes = Arc::clone(&excludes);
                    let pool = Arc::clone(&pool);
                    let cancel = Arc::clone(&cancel);
                    let target = target.clone();
                    s.spawn(move || {
                        let result = infra_scanner::scan_target_cancellable(
                            &target, &tx, &excludes, &pool, allocated, &cancel,
                        );
                        let _ = tx.send(AppEvent::TargetCompleted {
                            target_name: result.target.name.to_string(),
                            target_path: result
                                .target
                                .resolved_path()
                                .to_string_lossy()
                                .into_owned(),
                            total_bytes: result.bytes,
                            files_scanned: result.files_scanned,
                            scan_errors: result.scan_errors,
                        });
                    });
                }
            });
        }

        let _ = tx.send(AppEvent::ScanFinished);
    })
}
