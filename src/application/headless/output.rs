use std::collections::HashMap;

use crate::application::cleaner::CleanMode;
use crate::domain::{CleanTarget, format_bytes};
use crate::i18n::Language;
use crate::i18n::msg;

/// Emit a compact JSON object describing the completed scan.
/// Shape: {"targets":[{"name":...,"bytes":...,"files":...}],"total_bytes":...}
pub fn print_scan_finished_json(
    completed: &HashMap<String, (CleanTarget, u64, u64)>,
    total_bytes: u64,
) {
    let mut out = String::from("{\"targets\":[");
    let mut first = true;
    for (name, (_t, bytes, files)) in completed {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!(
            "{{\"name\":{},\"bytes\":{},\"files\":{}}}",
            json_escape(name),
            bytes,
            files
        ));
    }
    out.push_str(&format!("],\"total_bytes\":{total_bytes}}}"));
    println!("{out}");
}

/// Emit a compact JSON object describing the completed cleaning run.
pub fn print_cleaning_finished_json(
    cleaned_targets: u64,
    reclaimed_bytes: u64,
    errors: u64,
    mode: CleanMode,
) {
    let dry_run = matches!(mode, CleanMode::DryRun);
    println!(
        "{{\"cleaned_targets\":{},\"reclaimed_bytes\":{},\"errors\":{},\"dry_run\":{}}}",
        cleaned_targets, reclaimed_bytes, errors, dry_run
    );
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn print_scan_progress(
    target_name: &str,
    bytes_found: u64,
    files_scanned: u64,
    lang: Language,
) {
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
    let fmt = msg::scan_finished(lang).replace("{total}", &format_bytes(total_bytes));
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
