//! `acari guide`: explain where the disk went and walk the user to the next
//! cleanup step, including space no built-in target covers.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::application::commands::prepare_targets;
use crate::config::target_config::{self, TargetConfig};
use crate::domain::format_bytes;
use crate::domain::insights::{
    self, Action, Category, ExploreData, Finding, InsightOptions, KnownDirs, KnownTarget, Risk,
    display_path, non_overlapping_total,
};
use crate::i18n::Language;
use crate::infrastructure::df;
use crate::infrastructure::explore::{self, ExploreOptions, ExploreProgress};

#[derive(Debug, Clone)]
pub struct GuideOptions {
    /// Paths to explore; empty means the home directory (plus the system
    /// temp directory when it lives on the same disk).
    pub paths: Vec<PathBuf>,
    pub min_bytes: u64,
    pub stale_days: u64,
    pub category: Option<Category>,
    /// Items shown per category in text output (0 = all).
    pub per_category: usize,
    pub show_progress: bool,
    /// Low I/O priority, no output: only refresh the saved report.
    pub background: bool,
}

impl Default for GuideOptions {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            min_bytes: 1_000_000_000,
            stale_days: 30,
            category: None,
            per_category: 5,
            show_progress: true,
            background: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub path: PathBuf,
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootUsage {
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: Category,
    pub label: String,
    pub bytes: u64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Totals {
    /// Everything the findings explain (nested findings counted once).
    pub explained: u64,
    /// Regenerates on its own: caches and build outputs.
    pub safe: u64,
    /// Managed by an application (games, VMs...).
    pub app: u64,
    /// User data or unknown content.
    pub review: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStep {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideReport {
    /// When the report was produced (unix seconds).
    #[serde(default)]
    pub generated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<Volume>,
    pub roots: Vec<RootUsage>,
    /// Used space on the volume that lives outside the explored roots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outside_roots: Option<u64>,
    pub categories: Vec<CategorySummary>,
    pub findings: Vec<Finding>,
    pub totals: Totals,
    pub next_steps: Vec<NextStep>,
    pub scan_errors: u64,
    pub elapsed_secs: f64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn known_dirs(home: &Path) -> KnownDirs {
    // XDG user dirs are localised ("Vídeos"); dirs resolves them properly.
    let keep = |dir: Option<PathBuf>| dir.filter(|d| d != home && d.is_dir());
    KnownDirs {
        home: home.to_path_buf(),
        videos: keep(dirs::video_dir()),
        music: keep(dirs::audio_dir()),
        pictures: keep(dirs::picture_dir()),
        downloads: keep(dirs::download_dir()),
        temp: std::env::temp_dir().canonicalize().ok(),
    }
}

fn default_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = vec![home.to_path_buf()];
    // The temp dir often hides gigabytes, but only matters when it is on the
    // same disk (a tmpfs /tmp lives in RAM).
    if let Ok(temp) = std::env::temp_dir().canonicalize()
        && !temp.starts_with(home)
        && explore::device_of(&temp).is_some()
        && explore::device_of(&temp) == explore::device_of(home)
    {
        roots.push(temp);
    }
    roots
}

fn known_targets(cfg: &TargetConfig) -> Vec<KnownTarget> {
    prepare_targets(&[], &[], &cfg.custom_targets)
        .into_iter()
        .filter(|t| !t.is_command() && !t.path.is_empty())
        .filter_map(|t| {
            let path = t.resolved_path().canonicalize().ok()?;
            Some(KnownTarget {
                name: t.name.to_string(),
                path,
                description: t.description.to_string(),
                dangerous: t.is_dangerous(),
            })
        })
        .collect()
}

/// Run the exploration and assemble the report (no printing besides the
/// optional progress line on stderr).
pub fn build_report(opts: &GuideOptions, cfg: &TargetConfig, lang: Language) -> GuideReport {
    let started = Instant::now();
    let home = dirs::home_dir()
        .and_then(|h| h.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("/"));
    let roots = if opts.paths.is_empty() {
        default_roots(&home)
    } else {
        opts.paths.clone()
    };

    let progress_enabled =
        opts.show_progress && !opts.background && std::io::stderr().is_terminal();
    let data = explore::explore(
        &roots,
        ExploreOptions {
            big_file_min: opts.min_bytes,
            keep_dir_min: (opts.min_bytes / 100).max(1_000_000),
            io_priority: if opts.background {
                crate::config::target_config::IoPriority::Low
            } else {
                cfg.scan.io_priority
            },
        },
        |p: ExploreProgress| {
            if progress_enabled {
                let label = match lang {
                    Language::Portuguese => "varrendo",
                    Language::English => "scanning",
                };
                eprint!(
                    "\r\x1b[2K[guide] {label}: {} itens, {}",
                    p.entries,
                    format_bytes(p.bytes)
                );
                let _ = std::io::stderr().flush();
            }
        },
    );
    if progress_enabled {
        eprint!("\r\x1b[2K");
    }

    let known = known_dirs(&home);
    let insight_opts = InsightOptions {
        min_bytes: opts.min_bytes,
        min_build_bytes: opts.min_bytes / 10,
        stale_days: opts.stale_days,
        now: now_secs(),
    };
    let findings =
        insights::build_findings(&data, &known, &known_targets(cfg), &insight_opts, lang);
    assemble(data, findings, cfg, opts, &home, lang, started)
}

fn assemble(
    data: ExploreData,
    findings: Vec<Finding>,
    cfg: &TargetConfig,
    opts: &GuideOptions,
    home: &Path,
    lang: Language,
    started: Instant,
) -> GuideReport {
    let roots: Vec<RootUsage> = data
        .roots
        .iter()
        .map(|r| RootUsage {
            path: r.clone(),
            bytes: data.dirs.get(r).map_or(0, |s| s.bytes),
        })
        .collect();

    let volume = data
        .roots
        .first()
        .and_then(|r| df::volume_usage(r).map(|(total, used, free)| (r, total, used, free)))
        .map(|(r, total, used, free)| Volume {
            path: r.clone(),
            total,
            used,
            free,
        });
    let outside_roots = volume.as_ref().map(|v| {
        let dev = explore::device_of(&v.path);
        let on_volume: u64 = roots
            .iter()
            .filter(|r| explore::device_of(&r.path) == dev)
            .map(|r| r.bytes)
            .sum();
        v.used.saturating_sub(on_volume)
    });

    let totals = Totals {
        explained: non_overlapping_total(&findings),
        safe: non_overlapping_total(findings.iter().filter(|f| f.risk == Risk::Safe)),
        app: non_overlapping_total(findings.iter().filter(|f| f.risk == Risk::App)),
        review: non_overlapping_total(findings.iter().filter(|f| f.risk == Risk::Review)),
    };
    let next_steps = next_steps(&findings, cfg, opts, outside_roots, home, lang);

    let categories = summarize_categories(&findings, lang);

    GuideReport {
        generated_at: now_secs(),
        volume,
        roots,
        outside_roots,
        categories,
        findings,
        totals,
        next_steps,
        scan_errors: data.errors,
        elapsed_secs: started.elapsed().as_secs_f64(),
    }
}

fn summarize_categories(findings: &[Finding], lang: Language) -> Vec<CategorySummary> {
    let mut by_cat: BTreeMap<Category, Vec<&Finding>> = BTreeMap::new();
    for f in findings {
        by_cat.entry(f.category).or_default().push(f);
    }
    let mut categories: Vec<CategorySummary> = by_cat
        .into_iter()
        .map(|(category, items)| CategorySummary {
            category,
            label: category.label(lang).to_string(),
            bytes: non_overlapping_total(items.iter().copied()),
            count: items.len(),
        })
        .collect();
    categories.sort_by_key(|c| std::cmp::Reverse(c.bytes));
    categories
}

/// Keep only one category's findings (totals and next steps stay global).
pub fn only_category(mut report: GuideReport, category: Category, lang: Language) -> GuideReport {
    report.findings.retain(|f| f.category == category);
    report.categories = summarize_categories(&report.findings, lang);
    report
}

/// File name of the last full report, inside the acari data dir.
pub const SAVED_REPORT: &str = "report.json";

pub fn save_report(report: &GuideReport) -> std::io::Result<()> {
    crate::infrastructure::state::write_json(SAVED_REPORT, report)
}

pub fn load_saved_report() -> Option<GuideReport> {
    crate::infrastructure::state::read_json(SAVED_REPORT)
}

/// Age of a report in whole days.
pub fn report_age_days(report: &GuideReport) -> u64 {
    now_secs().saturating_sub(report.generated_at) / 86_400
}

fn tr(lang: Language, pt: &'static str, en: &'static str) -> &'static str {
    match lang {
        Language::Portuguese => pt,
        Language::English => en,
    }
}

fn next_steps(
    findings: &[Finding],
    cfg: &TargetConfig,
    opts: &GuideOptions,
    outside_roots: Option<u64>,
    home: &Path,
    lang: Language,
) -> Vec<NextStep> {
    let mut steps = Vec::new();

    // 1. Stale project builds: the cheapest big win.
    let stale: Vec<&Finding> = findings
        .iter()
        .filter(|f| {
            f.category == Category::ProjectBuilds
                && f.idle_days.is_some_and(|d| d >= opts.stale_days)
        })
        .collect();
    if !stale.is_empty() {
        let bytes = non_overlapping_total(stale.iter().copied());
        let title = match lang {
            Language::Portuguese => format!(
                "Apagar {} builds de projetos parados há {}+ dias ({}).",
                stale.len(),
                opts.stale_days,
                format_bytes(bytes)
            ),
            Language::English => format!(
                "Delete {} project builds idle for {}+ days ({}).",
                stale.len(),
                opts.stale_days,
                format_bytes(bytes)
            ),
        };
        let command = if cfg.project_scan.roots.is_empty() {
            Some("acari setup".to_string())
        } else {
            Some("acari project".to_string())
        };
        steps.push(NextStep { title, command });
    } else if cfg.project_scan.roots.is_empty()
        && findings
            .iter()
            .any(|f| f.category == Category::ProjectBuilds)
    {
        steps.push(NextStep {
            title: tr(
                lang,
                "Configure suas pastas de projetos para limpar builds com `acari project`.",
                "Configure your project folders to clean builds with `acari project`.",
            )
            .to_string(),
            command: Some("acari setup".into()),
        });
    }

    // 2. Caches acari cleans by itself.
    let mut safe_targets: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.risk == Risk::Safe && matches!(f.action, Action::AcariTarget { .. }))
        .collect();
    safe_targets.sort_by_key(|f| std::cmp::Reverse(f.bytes));
    if !safe_targets.is_empty() {
        let bytes = non_overlapping_total(safe_targets.iter().copied());
        let args: Vec<String> = safe_targets
            .iter()
            .take(4)
            .filter_map(|f| match &f.action {
                Action::AcariTarget { target } => Some(format!("-t {}", quote_arg(target))),
                _ => None,
            })
            .collect();
        let title = match lang {
            Language::Portuguese => format!(
                "Limpar caches que se regeneram sozinhos ({}).",
                format_bytes(bytes)
            ),
            Language::English => format!(
                "Clean caches that regenerate on their own ({}).",
                format_bytes(bytes)
            ),
        };
        steps.push(NextStep {
            title,
            command: Some(format!("acari {}", args.join(" "))),
        });
    }

    // 3. The largest things only the user can decide about.
    let mut decide: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.risk != Risk::Safe && f.category != Category::Other)
        .collect();
    decide.sort_by_key(|f| std::cmp::Reverse(f.bytes));
    for f in decide.iter().take(3) {
        let title = match lang {
            Language::Portuguese => format!(
                "Revisar {} ({}): {}",
                display_or_label(f, home),
                format_bytes(f.bytes),
                f.hint
            ),
            Language::English => format!(
                "Review {} ({}): {}",
                display_or_label(f, home),
                format_bytes(f.bytes),
                f.hint
            ),
        };
        steps.push(NextStep {
            title,
            command: f.action.command_line(),
        });
    }

    // 4. Space outside the explored folders.
    if let Some(outside) = outside_roots
        && outside >= opts.min_bytes.saturating_mul(10)
        && opts.paths.is_empty()
    {
        let title = match lang {
            Language::Portuguese => format!(
                "{} estão fora do seu home (sistema, /var, /opt, outros usuários). Para investigar:",
                format_bytes(outside)
            ),
            Language::English => format!(
                "{} lives outside your home (system, /var, /opt, other users). To investigate:",
                format_bytes(outside)
            ),
        };
        steps.push(NextStep {
            title,
            command: Some("sudo acari guide /var /opt /usr".into()),
        });
    }

    steps
}

fn display_or_label(f: &Finding, home: &Path) -> String {
    let shown = display_path(&f.path, home);
    if f.label == shown || f.label.starts_with(&shown) {
        f.label.clone()
    } else {
        format!("{} ({shown})", f.label)
    }
}

fn quote_arg(raw: &str) -> String {
    insights::shell_quote(Path::new(raw))
}

/// Render the report as human text.
pub fn render_text(report: &GuideReport, opts: &GuideOptions, lang: Language) -> String {
    use std::fmt::Write as _;

    let home = dirs::home_dir()
        .and_then(|h| h.canonicalize().ok())
        .unwrap_or_default();
    let mut out = String::new();

    if let Some(v) = &report.volume {
        let pct = if v.total > 0 {
            v.used as f64 * 100.0 / v.total as f64
        } else {
            0.0
        };
        let _ = writeln!(
            out,
            "{} {}: {} {}, {} {} ({pct:.0}%), {} {}",
            tr(lang, "Disco", "Disk"),
            v.path.display(),
            format_bytes(v.total),
            tr(lang, "no total", "total"),
            format_bytes(v.used),
            tr(lang, "usados", "used"),
            format_bytes(v.free),
            tr(lang, "livres", "free"),
        );
    }
    let roots: Vec<String> = report
        .roots
        .iter()
        .map(|r| {
            format!(
                "{} ({})",
                display_path(&r.path, &home),
                format_bytes(r.bytes)
            )
        })
        .collect();
    let _ = writeln!(
        out,
        "{} {} {} {:.0}s.",
        tr(lang, "Varri", "Scanned"),
        roots.join(", "),
        tr(lang, "em", "in"),
        report.elapsed_secs
    );
    let scanned: u64 = report.roots.iter().map(|r| r.bytes).sum();
    let compressed = report.volume.as_ref().is_some_and(|v| scanned > v.used);
    if compressed {
        let _ = writeln!(
            out,
            "{}",
            tr(
                lang,
                "O sistema de arquivos comprime/deduplica dados (btrfs, APFS...): os tamanhos abaixo são antes da compressão e somam mais que o espaço usado.",
                "The filesystem compresses/deduplicates data (btrfs, APFS...): sizes below are pre-compression and add up to more than the used space."
            ),
        );
    } else if let Some(outside) = report.outside_roots {
        let _ = writeln!(
            out,
            "{} {}",
            format_bytes(outside),
            tr(
                lang,
                "do disco estão fora dessas pastas (sistema e outros usuários).",
                "of the disk lives outside these folders (system and other users)."
            ),
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{}",
        tr(lang, "Para onde foi o espaço:", "Where the space went:")
    );

    for cat in &report.categories {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{:>11}  {}",
            format_bytes(cat.bytes),
            cat.label.to_uppercase()
        );
        let items: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.category == cat.category)
            .collect();
        let limit = if opts.per_category == 0 || opts.category.is_some() {
            items.len()
        } else {
            opts.per_category
        };
        for f in items.iter().take(limit) {
            render_finding(&mut out, f, &home, lang);
        }
        if items.len() > limit {
            let rest = &items[limit..];
            let _ = writeln!(
                out,
                "             … {} {} ({}) → acari guide --category {}",
                tr(lang, "e mais", "and"),
                rest.len(),
                format_bytes(non_overlapping_total(rest.iter().copied())),
                category_slug(cat.category),
            );
        }
    }

    let t = &report.totals;
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", tr(lang, "Resumo:", "Summary:"));
    let _ = writeln!(
        out,
        "  {:>11}  {}",
        format_bytes(t.safe),
        tr(
            lang,
            "regenera sozinho (caches e builds)",
            "regenerates on its own (caches and builds)"
        )
    );
    let _ = writeln!(
        out,
        "  {:>11}  {}",
        format_bytes(t.app),
        tr(
            lang,
            "gerenciado por apps (jogos, VMs...)",
            "managed by apps (games, VMs...)"
        )
    );
    let _ = writeln!(
        out,
        "  {:>11}  {}",
        format_bytes(t.review),
        tr(
            lang,
            "seus dados ou desconhecido: revise",
            "your data or unknown: review"
        )
    );

    if !report.next_steps.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "{}", tr(lang, "Próximos passos:", "Next steps:"));
        for (i, step) in report.next_steps.iter().enumerate() {
            let _ = writeln!(out, "  {}. {}", i + 1, step.title);
            if let Some(cmd) = &step.command {
                let _ = writeln!(out, "     $ {cmd}");
            }
        }
    }
    if report.scan_errors > 0 {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{}",
            match lang {
                Language::Portuguese => format!(
                    "{} itens não puderam ser lidos (sem permissão?); os números podem estar um pouco abaixo.",
                    report.scan_errors
                ),
                Language::English => format!(
                    "{} entries could not be read (permissions?); numbers may be slightly low.",
                    report.scan_errors
                ),
            }
        );
    }
    out
}

fn render_finding(out: &mut String, f: &Finding, home: &Path, lang: Language) {
    use std::fmt::Write as _;

    let shown = display_path(&f.path, home);
    let mut head = if f.label == shown || f.label.starts_with(&shown) {
        f.label.clone()
    } else if shown.starts_with(&f.label) {
        shown.clone()
    } else {
        format!("{}  {}", f.label, shown)
    };
    if let Some(days) = f.idle_days
        && matches!(f.category, Category::ProjectBuilds | Category::Worktrees)
    {
        let _ = write!(
            head,
            "  ({} {days} {})",
            tr(lang, "parado há", "idle"),
            match (days == 1, lang) {
                (true, Language::Portuguese) => "dia",
                (true, Language::English) => "day",
                (false, Language::Portuguese) => "dias",
                (false, Language::English) => "days",
            }
        );
    }
    let _ = writeln!(
        out,
        "  {:>11}  {head}  [{}]",
        format_bytes(f.bytes),
        f.risk.label(lang)
    );
    if !f.details.is_empty() {
        let parts: Vec<String> = f
            .details
            .iter()
            .map(|d| format!("{} {}", d.name, format_bytes(d.bytes)))
            .collect();
        let _ = writeln!(out, "               {}", parts.join(" · "));
    }
    if !matches!(f.category, Category::ProjectBuilds) && !f.hint.is_empty() {
        let _ = writeln!(out, "               → {}", f.hint);
    }
    if let Some(cmd) = f.action.command_line() {
        let _ = writeln!(out, "               $ {cmd}");
    }
}

pub fn category_slug(category: Category) -> &'static str {
    match category {
        Category::Games => "games",
        Category::Media => "media",
        Category::Downloads => "downloads",
        Category::ProjectBuilds => "project-builds",
        Category::Worktrees => "worktrees",
        Category::DevCaches => "dev-caches",
        Category::Toolchains => "toolchains",
        Category::Containers => "containers",
        Category::VirtualMachines => "vms",
        Category::LargeFiles => "large-files",
        Category::Temp => "temp",
        Category::Trash => "trash",
        Category::Other => "other",
    }
}

pub fn parse_category(raw: &str) -> Option<Category> {
    const ALL: [Category; 13] = [
        Category::Games,
        Category::Media,
        Category::Downloads,
        Category::ProjectBuilds,
        Category::Worktrees,
        Category::DevCaches,
        Category::Toolchains,
        Category::Containers,
        Category::VirtualMachines,
        Category::LargeFiles,
        Category::Temp,
        Category::Trash,
        Category::Other,
    ];
    ALL.into_iter().find(|c| category_slug(*c) == raw)
}

pub fn category_slugs() -> Vec<&'static str> {
    [
        "games",
        "media",
        "downloads",
        "project-builds",
        "worktrees",
        "dev-caches",
        "toolchains",
        "containers",
        "vms",
        "large-files",
        "temp",
        "trash",
        "other",
    ]
    .to_vec()
}

/// Entry point for the CLI.
pub fn run_guide(opts: &GuideOptions, json: bool, lang: Language) -> anyhow::Result<()> {
    let cfg = target_config::load_config();
    let mut report = build_report(opts, &cfg, lang);
    if opts.paths.is_empty() {
        // Only full default-scope reports are reused by check, hooks and MCP.
        if let Err(e) = save_report(&report) {
            eprintln!("warning: could not save the report: {e}");
        }
    }
    if opts.background {
        return Ok(());
    }
    if let Some(category) = opts.category {
        report = only_category(report, category, lang);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render_text(&report, opts, lang));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn category_slugs_round_trip() {
        for slug in category_slugs() {
            let cat = parse_category(slug).expect("known slug");
            assert_eq!(category_slug(cat), slug);
        }
        assert_eq!(parse_category("nope"), None);
    }

    #[test]
    fn report_explains_a_project_tree_end_to_end() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let project = root.join("app");
        fs::create_dir_all(project.join("target/debug")).unwrap();
        fs::write(project.join("Cargo.toml"), "[package]").unwrap();
        fs::write(project.join("target/debug/big"), vec![1u8; 3_000_000]).unwrap();

        let opts = GuideOptions {
            paths: vec![root.clone()],
            min_bytes: 1_000_000,
            show_progress: false,
            ..GuideOptions::default()
        };
        let cfg = TargetConfig::default();
        let report = build_report(&opts, &cfg, Language::English);

        let build = report
            .findings
            .iter()
            .find(|f| f.category == Category::ProjectBuilds)
            .expect("cargo target found");
        assert_eq!(build.path, project.join("target"));
        assert!(report.totals.safe >= 3_000_000);
        // Fresh build: no "stale builds" step, but setup is suggested since
        // no project roots are configured.
        assert!(
            report
                .next_steps
                .iter()
                .any(|s| s.command.as_deref() == Some("acari setup"))
        );

        let text = render_text(&report, &opts, Language::English);
        assert!(text.contains("PROJECT BUILDS"), "{text}");
        assert!(text.contains("cargo clean --manifest-path"), "{text}");
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["findings"][0]["category"], "project_builds");
    }
}
