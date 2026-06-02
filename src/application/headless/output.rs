use crate::application::cleaner::CleanMode;
use crate::domain::format_bytes;
use crate::i18n::Language;
use crate::i18n::msg;

pub fn print_scan_progress(target_name: &str, bytes_found: u64, files_scanned: u64, lang: Language) {
    let fmt = msg::scan_progress(lang)
        .replace("{name}", target_name)
        .replace("{size}", &format_bytes(bytes_found))
        .replace("{files}", &files_scanned.to_string());
    println!("{fmt}");
}

pub fn print_target_done(target_name: &str, bytes: u64, files_scanned: u64, lang: Language) {
    let fmt = msg::target_done(lang)
        .replace("{name}", target_name)
        .replace("{size}", &format_bytes(bytes))
        .replace("{files}", &files_scanned.to_string());
    println!("{fmt}");
}

pub fn print_scan_finished(total_bytes: u64, lang: Language) {
    let fmt = msg::scan_finished(lang)
        .replace("{total}", &format_bytes(total_bytes));
    println!("{fmt}");
}

pub fn print_start_cleaning(num_targets: usize, mode: CleanMode, lang: Language) {
    let tmpl = match mode {
        CleanMode::Execute => msg::start_cleaning(lang),
        CleanMode::DryRun => msg::start_dry_run(lang),
    };
    let fmt = tmpl.replace("{n}", &num_targets.to_string());
    println!("{fmt}");
}

pub fn print_target_cleaned(
    target_name: &str,
    reclaimed_bytes: u64,
    removed_entries: u64,
    errors: u64,
    mode: CleanMode,
    lang: Language,
) {
    let mode_label = match mode {
        CleanMode::Execute => msg::clean_execute_label(lang),
        CleanMode::DryRun => msg::clean_dry_run_label(lang),
    };

    let tmpl = if errors > 0 {
        msg::target_cleaned_with_errors(lang)
    } else {
        msg::target_cleaned(lang)
    };

    let fmt = tmpl
        .replace("{mode}", mode_label)
        .replace("{name}", target_name)
        .replace("{reclaimed}", &format_bytes(reclaimed_bytes))
        .replace("{removed}", &removed_entries.to_string())
        .replace("{errors}", &errors.to_string());
    println!("{fmt}");
}

pub fn print_cleaning_finished(
    cleaned_targets: u64,
    reclaimed_bytes: u64,
    errors: u64,
    mode: CleanMode,
    lang: Language,
) {
    let tmpl = match mode {
        CleanMode::Execute => msg::cleaning_finished(lang),
        CleanMode::DryRun => msg::dry_run_finished(lang),
    };
    let fmt = tmpl
        .replace("{n}", &cleaned_targets.to_string())
        .replace("{size}", &format_bytes(reclaimed_bytes))
        .replace("{errors}", &errors.to_string());
    println!("{fmt}");
}
