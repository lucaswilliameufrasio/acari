use acari::application::cleaner::CleanMode;
use acari::application::commands::{
    enforce_headless_clean_safety_l10n, merge_excludes, prepare_targets, start_scan,
};
use acari::application::headless::run_headless;
use acari::config::target_config::{self, format_modified_time};
use acari::config::{Cli, Commands, ProjectAction, TargetAction};
use acari::domain::CleanTarget;
use acari::domain::project_scan::{self, builtin_patterns};
use acari::i18n::{Language, detect_language, msg};
use acari::infrastructure::distro;
use acari::ui::app::run_tui;
use anyhow::{Context, Result};
use clap::Parser;

fn print_all_targets(lang: Language) {
    let cfg = target_config::load_config();
    let targets = prepare_targets(&[], &[], &cfg.custom_targets);
    let dinfo = distro::detect();
    println!(
        "{}",
        msg::distro_info(lang).replace("{os}", &dinfo.pretty_name)
    );
    println!();
    for target in &targets {
        let is_custom = target.description == "User provided path";
        let origin = if is_custom {
            msg::target_list_custom(lang)
        } else {
            msg::target_list_builtin(lang)
        };
        println!("{} {}", target.name, origin);
        if !is_custom {
            println!("  {}{}", msg::target_path(lang), target.path);
            println!("  {}{}", msg::target_desc(lang), target.description);
        } else {
            println!("  (custom path, use 'acari target list' for details)");
        }
        println!();
    }
    if let Some(time) = format_modified_time() {
        println!(
            "{}",
            msg::config_last_modified(lang).replace("{time}", &time)
        );
    }
}

fn print_project_patterns(lang: Language, cfg: &target_config::TargetConfig) {
    println!("{}", msg::builtin_patterns_header(lang));
    let builtins = builtin_patterns();
    for chunk in builtins.chunks(5) {
        println!("  {}", chunk.join("  "));
    }
    println!(
        "  {}",
        msg::pattern_count(lang).replace("{n}", &builtins.len().to_string())
    );
    println!();
    if cfg.project_scan.patterns.is_empty() {
        println!("{}", msg::no_custom_patterns(lang));
    } else {
        println!("{}", msg::custom_patterns_header(lang));
        for p in &cfg.project_scan.patterns {
            println!("  {}", p);
        }
    }
}

fn print_project_roots(lang: Language, cfg: &target_config::TargetConfig) {
    if cfg.project_scan.roots.is_empty() {
        println!("{}", msg::no_roots_configured(lang));
    } else {
        println!("{}", msg::roots_header(lang));
        for r in &cfg.project_scan.roots {
            println!("  {}", r);
        }
    }
}

fn collect_project_targets(
    roots: &[String],
    patterns: &[String],
    no_default_patterns: bool,
    _excludes: &[String],
    lang: Language,
) -> Result<Vec<CleanTarget>> {
    let roots: Vec<&str> = roots.iter().map(|s| s.as_str()).collect();
    let discovered = project_scan::discover_junk_dirs(&roots, patterns, no_default_patterns);
    if discovered.is_empty() {
        println!("{}", msg::no_junk_found(lang));
    } else {
        println!(
            "[project-scan] {}",
            msg::junk_found(lang).replace("{n}", &discovered.len().to_string())
        );
    }
    Ok(discovered)
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
                        println!("{}", msg::target_added(lang).replace("{name}", name));
                        println!("{}", msg::config_updated_at(lang).replace("{time}", &time));
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
                        println!("{}", msg::target_removed(lang).replace("{name}", name));
                        println!("{}", msg::config_updated_at(lang).replace("{time}", &time));
                    } else {
                        println!("{}", msg::target_not_found(lang).replace("{name}", name));
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
                            println!(
                                "\n{}",
                                msg::config_last_modified(lang).replace("{time}", &time)
                            );
                        }
                    }
                }
            },
            Commands::Project { action } => match action {
                None => {
                    let cfg = target_config::load_config();
                    acari::ui::project::run_project_tui(&cfg, lang)?;
                }
                Some(ProjectAction::AddRoot { path }) => {
                    let mut cfg = target_config::load_config();
                    if cfg.project_scan.roots.contains(path) {
                        println!("{}", msg::root_already_exists(lang).replace("{path}", path));
                    } else {
                        cfg.project_scan.roots.push(path.clone());
                        target_config::save_config(&cfg)?;
                        let time = format_modified_time().unwrap_or_default();
                        println!("{}", msg::root_added(lang).replace("{path}", path));
                        println!("{}", msg::config_updated_at(lang).replace("{time}", &time));
                    }
                }
                Some(ProjectAction::RemoveRoot { path }) => {
                    let mut cfg = target_config::load_config();
                    let len = cfg.project_scan.roots.len();
                    cfg.project_scan.roots.retain(|r| r != path);
                    if cfg.project_scan.roots.len() < len {
                        target_config::save_config(&cfg)?;
                        let time = format_modified_time().unwrap_or_default();
                        println!("{}", msg::root_removed(lang).replace("{path}", path));
                        println!("{}", msg::config_updated_at(lang).replace("{time}", &time));
                    } else {
                        println!("{}", msg::root_not_found(lang).replace("{path}", path));
                    }
                }
                Some(ProjectAction::ListRoots) => {
                    let cfg = target_config::load_config();
                    print_project_roots(lang, &cfg);
                }
                Some(ProjectAction::AddPattern { pattern }) => {
                    let mut cfg = target_config::load_config();
                    let builtins = builtin_patterns();
                    if builtins.iter().any(|p| p == pattern) {
                        println!(
                            "{}",
                            msg::pattern_is_builtin(lang).replace("{pattern}", pattern)
                        );
                    } else {
                        match cfg.add_pattern(pattern) {
                            Ok(true) => {
                                target_config::save_config(&cfg)?;
                                let time = format_modified_time().unwrap_or_default();
                                println!(
                                    "{}",
                                    msg::pattern_added(lang).replace("{pattern}", pattern)
                                );
                                println!(
                                    "{}",
                                    msg::config_updated_at(lang).replace("{time}", &time)
                                );
                            }
                            Ok(false) => {
                                println!(
                                    "{}",
                                    msg::pattern_exists(lang).replace("{pattern}", pattern)
                                );
                            }
                            Err(e) => {
                                println!("{}", msg::pattern_invalid_name(lang));
                                eprintln!("  ({e})");
                            }
                        }
                    }
                }
                Some(ProjectAction::RemovePattern { pattern }) => {
                    let mut cfg = target_config::load_config();
                    let len = cfg.project_scan.patterns.len();
                    cfg.project_scan.patterns.retain(|p| p != pattern);
                    if cfg.project_scan.patterns.len() < len {
                        target_config::save_config(&cfg)?;
                        let time = format_modified_time().unwrap_or_default();
                        println!(
                            "{}",
                            msg::pattern_removed(lang).replace("{pattern}", pattern)
                        );
                        println!("{}", msg::config_updated_at(lang).replace("{time}", &time));
                    } else {
                        println!(
                            "{}",
                            msg::pattern_not_found(lang).replace("{pattern}", pattern)
                        );
                    }
                }
                Some(ProjectAction::ListPatterns) => {
                    let cfg = target_config::load_config();
                    print_project_patterns(lang, &cfg);
                }
                Some(ProjectAction::Scan {
                    roots,
                    patterns,
                    no_default_patterns,
                    headless,
                    clean,
                    dry_run,
                    yes,
                    excludes,
                }) => {
                    let cfg = target_config::load_config();
                    let io_priority = cfg.scan.io_priority;

                    let roots = if roots.is_empty() {
                        cfg.project_scan.roots.clone()
                    } else {
                        roots.to_vec()
                    };

                    if roots.is_empty() {
                        anyhow::bail!("{}", msg::no_roots_configured(lang));
                    }

                    let all_excludes = merge_excludes(excludes, &cfg.scan.exclude_patterns);

                    let discovered = collect_project_targets(
                        &roots,
                        patterns,
                        *no_default_patterns,
                        &all_excludes,
                        lang,
                    )?;

                    if discovered.is_empty() {
                        return Ok(());
                    }

                    if *headless {
                        enforce_headless_clean_safety_l10n(
                            *headless, *clean, *dry_run, *yes, lang,
                        )?;
                        let clean_mode = if *dry_run {
                            CleanMode::DryRun
                        } else {
                            CleanMode::Execute
                        };
                        let (tx, rx, _scan_handle) =
                            start_scan(discovered.clone(), all_excludes, io_priority);
                        run_headless(tx, rx, discovered, *clean, clean_mode, lang).await?;
                    } else {
                        run_tui(&discovered, all_excludes, lang, io_priority)?;
                    }
                }
            },
        }
        return Ok(());
    }

    enforce_headless_clean_safety_l10n(cli.headless, cli.clean, cli.dry_run, cli.yes, lang)?;

    let cfg = target_config::load_config();
    let io_priority = cfg.scan.io_priority;
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

    let (tx, rx, _scan_handle) = start_scan(targets.clone(), excludes.clone(), io_priority);

    if cli.headless {
        let clean_mode = if cli.dry_run {
            CleanMode::DryRun
        } else {
            CleanMode::Execute
        };
        run_headless(tx, rx, targets, cli.clean, clean_mode, lang).await
    } else {
        run_tui(&targets, excludes, lang, io_priority)
    }
}
