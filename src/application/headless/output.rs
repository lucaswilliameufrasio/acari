use crate::application::cleaner::CleanMode;

pub fn print_scan_progress(target_name: &str, bytes_found: u64, files_scanned: u64) {
    println!("[progress] {target_name}: {bytes_found} bytes, {files_scanned} files");
}

pub fn print_target_done(target_name: &str, bytes: u64, files_scanned: u64) {
    println!("[done] {target_name}: {bytes} bytes across {files_scanned} files");
}

pub fn print_scan_finished(total_bytes: u64) {
    println!("\nScan finished. Total reclaimable bytes: {total_bytes}");
}

pub fn print_start_cleaning(num_targets: usize, mode: CleanMode) {
    match mode {
        CleanMode::Execute => println!("Starting clean for {num_targets} target(s)..."),
        CleanMode::DryRun => println!("Starting dry-run clean for {num_targets} target(s)..."),
    }
}

pub fn print_target_cleaned(
    target_name: &str,
    reclaimed_bytes: u64,
    removed_entries: u64,
    errors: u64,
    mode: CleanMode,
) {
    let mode_label = match mode {
        CleanMode::Execute => "cleaned",
        CleanMode::DryRun => "dry-run",
    };

    if errors > 0 {
        println!(
            "[{mode_label}] {target_name}: reclaimed={reclaimed_bytes} removed={removed_entries} errors={errors} (permission or deletion failures detected)"
        );
    } else {
        println!(
            "[{mode_label}] {target_name}: reclaimed={reclaimed_bytes} removed={removed_entries} errors={errors}"
        );
    }
}

pub fn print_cleaning_finished(
    cleaned_targets: u64,
    reclaimed_bytes: u64,
    errors: u64,
    mode: CleanMode,
) {
    let label = match mode {
        CleanMode::Execute => "Cleaning",
        CleanMode::DryRun => "Dry-run cleaning",
    };
    println!(
        "{label} finished. Targets: {cleaned_targets}, reclaimed bytes: {reclaimed_bytes}, errors: {errors}"
    );
}
