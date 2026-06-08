use acari::application::cleaner::CleanMode;
use acari::application::commands::{
    enforce_headless_clean_safety_l10n, merge_excludes, prepare_targets, start_scan,
};
use acari::application::headless::run_headless;
use acari::config::Cli;
use acari::config::target_config;
use acari::i18n::{detect_language, msg};
use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let lang = detect_language();
    let cli = Cli::parse();
    enforce_headless_clean_safety_l10n(true, cli.clean, cli.dry_run, cli.yes, lang)?;

    let cfg = target_config::load_config();
    let io_priority = cfg.scan.io_priority;
    let excludes = merge_excludes(&cli.excludes, &cfg.scan.exclude_patterns);
    let targets = prepare_targets(&cli.targets, &cli.scan_paths, &cfg.custom_targets);

    if targets.is_empty() {
        println!("{}", msg::no_targets_matched(lang));
        return Ok(());
    }

    let (tx, rx, _scan_handle) = start_scan(targets.clone(), excludes, io_priority);

    let clean_mode = if cli.dry_run {
        CleanMode::DryRun
    } else {
        CleanMode::Execute
    };
    run_headless(tx, rx, targets, cli.clean, clean_mode, lang).await
}
