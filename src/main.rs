use acari::application::cleaner::CleanMode;
use acari::application::commands::{
    enforce_headless_clean_safety, prepare_targets, print_targets, start_scan,
};
use acari::application::headless::run_headless;
use acari::config::Cli;
use acari::ui::app::run_tui;
use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    enforce_headless_clean_safety(cli.headless, cli.clean, cli.dry_run, cli.yes)?;

    let targets = prepare_targets(&cli.targets, &cli.scan_paths);

    if cli.list {
        print_targets(&targets);
        return Ok(());
    }

    if targets.is_empty() {
        println!("No scan targets matched your filters.");
        return Ok(());
    }

    let (tx, rx, _scan_handle) = start_scan(targets.clone());

    if cli.headless {
        let clean_mode = if cli.dry_run {
            CleanMode::DryRun
        } else {
            CleanMode::Execute
        };
        run_headless(tx, rx, targets, cli.clean, clean_mode).await
    } else {
        run_tui(tx, rx, &targets)
    }
}
