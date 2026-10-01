#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    ScanProgress {
        target_name: String,
        bytes_found: u64,
        files_scanned: u64,
    },
    TargetCompleted {
        target_name: String,
        target_path: String,
        total_bytes: u64,
        files_scanned: u64,
    },
    ScanFinished,
    TargetCleaned {
        target_name: String,
        reclaimed_bytes: u64,
        removed_entries: u64,
        errors: u64,
        error_detail: Option<String>,
    },
    CleaningProgress {
        target_name: String,
        completed_targets: u64,
        total_targets: u64,
        elapsed_seconds: u64,
    },
    CleaningFinished {
        cleaned_targets: u64,
        reclaimed_bytes: u64,
        errors: u64,
        cancelled: bool,
    },
}
