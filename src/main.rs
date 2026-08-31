use acari::application::cleaner::CleanMode;
use acari::application::commands::{
    enforce_headless_clean_safety_l10n, merge_excludes, prepare_targets, start_scan,
};
use acari::application::headless::run_headless;
use acari::config::target_config::{self, format_modified_time};
use acari::config::{Cli, Commands, ProjectAction, TargetAction};
use acari::domain::project_scan::{self, builtin_patterns};
use acari::domain::{CleanTarget, format_bytes};
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
        let is_custom = target.is_custom();
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
    excludes: &[String],
    lang: Language,
) -> Result<Vec<CleanTarget>> {
    let roots: Vec<&str> = roots.iter().map(|s| s.as_str()).collect();
    let discovered =
        project_scan::discover_junk_dirs(&roots, patterns, no_default_patterns, excludes);
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

fn run_history(clear: bool, lang: Language) {
    use acari::infrastructure::history;

    let entries = history::read_entries();
    if entries.is_empty() {
        println!("{}", msg::history_empty(lang));
    } else {
        println!("{}", msg::history_header(lang));
        for entry in &entries {
            println!("  {entry}");
        }
    }

    if clear {
        if let Err(e) = history::clear() {
            eprintln!(
                "{}",
                msg::history_clear_error(lang).replace("{err}", &e.to_string())
            );
        } else {
            println!("{}", msg::history_cleared(lang));
        }
    }
}

fn print_df(lang: Language) {
    use acari::infrastructure::df::disk_overview;
    let overview = disk_overview();
    println!("{}", msg::df_title(lang));
    println!("  {} {}", msg::df_device(lang), overview.device);
    if !overview.available {
        println!("  {}", msg::df_unavailable(lang));
        return;
    }
    println!("  {} {}", msg::df_total(lang), format_bytes(overview.total));
    println!("  {} {}", msg::df_used(lang), format_bytes(overview.used));
    println!("  {} {}", msg::df_free(lang), format_bytes(overview.free));
    println!("  {} {:.0}%", msg::df_usage(lang), overview.usage_percent);
    if let Some(purgeable) = overview.purgeable {
        println!("  {} {}", msg::df_purgeable(lang), format_bytes(purgeable));
    }
}

/// Scan the given targets silently and return the grand total that counts
/// every scanned path exactly once (nested/duplicate targets count once).
async fn scan_targets_total(targets: Vec<CleanTarget>) -> u64 {
    use std::collections::HashMap;

    use acari::application::headless::count_non_overlapping_total;
    use acari::domain::AppEvent;

    let lookup: HashMap<String, CleanTarget> = targets
        .iter()
        .cloned()
        .map(|target| (target.name.to_string(), target))
        .collect();
    let (_tx, mut rx, handle) =
        start_scan(targets, Vec::new(), target_config::IoPriority::Low, false);
    let mut completed: HashMap<String, (CleanTarget, u64, u64)> = HashMap::new();
    while let Some(event) = rx.recv().await {
        match event {
            AppEvent::TargetCompleted {
                target_name,
                total_bytes,
                files_scanned,
            } => {
                if let Some(target) = lookup.get(&target_name) {
                    completed.insert(target_name, (target.clone(), total_bytes, files_scanned));
                }
            }
            AppEvent::ScanFinished => break,
            _ => {}
        }
    }
    let _ = handle.await;
    count_non_overlapping_total(&completed)
}

/// `acari df --breakdown`: explicitly split the used space into acari junk,
/// project junk and everything else, so the total no longer looks wrong.
async fn print_df_breakdown(lang: Language) {
    use acari::infrastructure::df::disk_overview;

    let cfg = target_config::load_config();
    println!("{}", msg::df_breakdown_scanning(lang));

    let targets = prepare_targets(&[], &[], &cfg.custom_targets);
    let junk_total = scan_targets_total(targets).await;

    let project_total = if cfg.project_scan.roots.is_empty() {
        0
    } else {
        let roots: Vec<&str> = cfg.project_scan.roots.iter().map(String::as_str).collect();
        let discovered = project_scan::discover_junk_dirs(&roots, &[], false, &[]);
        if discovered.is_empty() {
            0
        } else {
            scan_targets_total(discovered).await
        }
    };

    let overview = disk_overview();
    let others = overview
        .used
        .saturating_sub(junk_total)
        .saturating_sub(project_total);

    println!("  {}", msg::df_breakdown_title(lang));
    for (label, bytes) in [
        (msg::df_breakdown_junk_targets(lang), junk_total),
        (msg::df_breakdown_junk_projects(lang), project_total),
        (msg::df_breakdown_others(lang), others),
    ] {
        let pct = if overview.used > 0 {
            (bytes as f64 / overview.used as f64) * 100.0
        } else {
            0.0
        };
        println!("    {} {} ({pct:.0}%)", label, format_bytes(bytes));
    }
}

/// `acari du [path]`: largest directories under a path, du-style, largest
/// first. This is what finds the multi-GB stragglers no cache target covers
/// (emulator images, toolchain stores, stray screenshot folders).
fn run_du(path: Option<&str>, top: usize, min_size: &str, lang: Language) -> Result<()> {
    use acari::infrastructure::du;
    use acari::infrastructure::exec::parse_human_size;

    let root = match path {
        Some(raw) => acari::domain::expand_tilde(raw),
        None => dirs::home_dir().context("could not determine home directory")?,
    };
    let min_bytes = parse_human_size(min_size)
        .with_context(|| format!("invalid --min-size value: {min_size}"))?;

    println!(
        "{}",
        msg::du_title(lang)
            .replace("{path}", &root.display().to_string())
            .replace("{min}", min_size)
    );
    let entries = du::largest_dirs(&root, top, min_bytes);
    if entries.is_empty() {
        println!("{}", msg::du_empty(lang));
        return Ok(());
    }
    for entry in &entries {
        println!(
            "  {:>12}  {}",
            format_bytes(entry.bytes),
            entry.path.display()
        );
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let lang = detect_language();
    let cli = Cli::parse();

    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::History { clear } => {
                run_history(*clear, lang);
                return Ok(());
            }
            Commands::Df { breakdown } => {
                print_df(lang);
                if *breakdown {
                    print_df_breakdown(lang).await;
                }
                return Ok(());
            }
            Commands::Du {
                path,
                top,
                min_size,
            } => {
                run_du(path.as_deref(), *top, min_size, lang)?;
                return Ok(());
            }
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
                Some(ProjectAction::ClearPatterns) => {
                    let mut cfg = target_config::load_config();
                    let count = cfg.project_scan.patterns.len();
                    cfg.project_scan.patterns.clear();
                    target_config::save_config(&cfg)?;
                    println!(
                        "{}",
                        msg::patterns_cleared(lang).replace("{n}", &count.to_string())
                    );
                    if let Some(time) = format_modified_time() {
                        println!("{}", msg::config_updated_at(lang).replace("{time}", &time));
                    }
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
                    json,
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
                        let (tx, rx, _scan_handle) = start_scan(
                            discovered.clone(),
                            all_excludes,
                            io_priority,
                            cli.allocated_size,
                        );
                        run_headless(tx, rx, discovered, *clean, clean_mode, lang, *json).await?;
                    } else {
                        run_tui(
                            &discovered,
                            all_excludes,
                            lang,
                            io_priority,
                            cli.allocated_size,
                        )?;
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

    let (tx, rx, _scan_handle) = start_scan(
        targets.clone(),
        excludes.clone(),
        io_priority,
        cli.allocated_size,
    );

    if cli.headless {
        let clean_mode = if cli.dry_run {
            CleanMode::DryRun
        } else {
            CleanMode::Execute
        };
        run_headless(tx, rx, targets, cli.clean, clean_mode, lang, cli.json).await
    } else {
        run_tui(&targets, excludes, lang, io_priority, cli.allocated_size)
    }
}
