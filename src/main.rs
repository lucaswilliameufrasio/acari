use acari::application::cleaner::CleanMode;
use acari::application::commands::{
    enforce_headless_clean_safety_l10n, merge_excludes, prepare_targets, start_scan,
};
use acari::application::headless::run_headless;
use acari::config::target_config::{self, format_modified_time};
use acari::config::{Cli, Commands, TargetAction};
use acari::i18n::{Language, detect_language, msg};
use acari::infrastructure::distro;
use acari::ui::app::run_tui;
use anyhow::{Context, Result};
use clap::Parser;

fn print_all_targets(lang: Language) {
    let cfg = target_config::load_config();
    let targets = prepare_targets(&[], &[], &cfg.custom_targets);
    let dinfo = distro::detect();
    println!("{}", msg::distro_info(lang).replace("{os}", &dinfo.pretty_name));
    println!();
    for target in &targets {
        let origin = if target.description == "User provided path" {
            msg::target_list_custom(lang)
        } else {
            msg::target_list_builtin(lang)
        };
        println!("{} {}", target.name, origin);
        println!("  {}{}", msg::target_path(lang), target.path);
        println!("  {}{}", msg::target_desc(lang), target.description);
        println!();
    }
    if let Some(time) = format_modified_time() {
        println!("{}", msg::config_last_modified(lang).replace("{time}", &time));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let lang = detect_language();
    let cli = Cli::parse();

    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::Target { action } => match action {
                TargetAction::Add {
                    name,
                    path,
                    description,
                } => {
                    let desc = description.as_deref().unwrap_or("");
                    let mut cfg = target_config::load_config();
                    if cfg.add(name, path, desc).context("Failed to add target")? {
                        target_config::save_config(&cfg)?;
                        let time = format_modified_time().unwrap_or_default();
                        println!(
                            "{}",
                            msg::target_added(lang)
                                .replace("{name}", name)
                        );
                        println!(
                            "{}",
                            msg::config_updated_at(lang).replace("{time}", &time)
                        );
                    } else {
                        println!(
                            "{}",
                            msg::target_add_duplicate(lang).replace("{name}", name)
                        );
                    }
                }
                TargetAction::Remove { name } => {
                    let mut cfg = target_config::load_config();
                    if cfg.remove(name) {
                        target_config::save_config(&cfg)?;
                        let time = format_modified_time().unwrap_or_default();
                        println!(
                            "{}",
                            msg::target_removed(lang).replace("{name}", name)
                        );
                        println!(
                            "{}",
                            msg::config_updated_at(lang).replace("{time}", &time)
                        );
                    } else {
                        println!(
                            "{}",
                            msg::target_not_found(lang).replace("{name}", name)
                        );
                    }
                }
                TargetAction::List => {
                    let cfg = target_config::load_config();
                    if cfg.custom_targets.is_empty() {
                        println!("{}", msg::target_list_empty(lang));
                    } else {
                        println!("{}", msg::target_list_header(lang));
                        for t in &cfg.custom_targets {
                            println!("  {} (path: {})", t.name, t.path);
                        }
                        if let Some(time) = format_modified_time() {
                            println!("\n{}", msg::config_last_modified(lang).replace("{time}", &time));
                        }
                    }
                }
            },
        }
        return Ok(());
    }

    enforce_headless_clean_safety_l10n(cli.headless, cli.clean, cli.dry_run, cli.yes, lang)?;

    let cfg = target_config::load_config();
    let excludes = merge_excludes(&cli.excludes, &cfg.scan.exclude_patterns);
    let targets = prepare_targets(&cli.targets, &cli.scan_paths, &cfg.custom_targets);

    if cli.list {
        print_all_targets(lang);
        return Ok(());
    }

    if targets.is_empty() {
        println!("{}", msg::no_targets_matched(lang));
        return Ok(());
    }

    let (tx, rx, _scan_handle) = start_scan(targets.clone(), excludes.clone());

    if cli.headless {
        let clean_mode = if cli.dry_run {
            CleanMode::DryRun
        } else {
            CleanMode::Execute
        };
        run_headless(tx, rx, targets, cli.clean, clean_mode, lang).await
    } else {
        run_tui(&targets, excludes, lang)
    }
}
