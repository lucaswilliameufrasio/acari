//! `acari check`: a sub-second disk health check. It never scans; it reads
//! the volume numbers and the last saved guide report. The same check feeds
//! the terminal, agent hooks (`--hook`) and desktop notifications
//! (`--notify`).

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::application::guide::{self, GuideReport};
use crate::config::target_config::{self, TargetConfig};
use crate::domain::alerts::{self, AlertLevel, AlertReason, DiskStatus, Sample};
use crate::domain::format_bytes;
use crate::i18n::Language;
use crate::infrastructure::{df, notify, state};

const SAMPLES_FILE: &str = "disk-samples.json";
const NOTIFY_FILE: &str = "notify-state.json";
const RENOTIFY_SECS: u64 = 12 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    /// Always print a short status.
    Human,
    /// Print the status as JSON.
    Json,
    /// Agent hook: print context only when something is wrong.
    Hook,
    /// Desktop notification only when something is wrong (deduplicated).
    Notify,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub path: PathBuf,
    pub status: DiskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_age_days: Option<u64>,
    /// Biggest areas from the saved report, e.g. "Games 292 GB".
    pub highlights: Vec<String>,
    /// Space that regenerates on its own, from the saved report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regenerable: Option<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct NotifyState {
    last_at: u64,
    last_level: Option<AlertLevel>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn tr(lang: Language, pt: &'static str, en: &'static str) -> &'static str {
    match lang {
        Language::Portuguese => pt,
        Language::English => en,
    }
}

/// Measure, evaluate and record a sample. `None` when the volume cannot be
/// read.
pub fn measure(cfg: &TargetConfig) -> Option<CheckResult> {
    let path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let (total, used, free) = df::volume_usage(&path)?;
    let now = now_secs();
    let mut samples: Vec<Sample> = state::read_json(SAMPLES_FILE).unwrap_or_default();
    let status = alerts::evaluate(total, used, free, &samples, now, &cfg.alerts);
    alerts::push_sample(&mut samples, Sample { at: now, free });
    let _ = state::write_json(SAMPLES_FILE, &samples);

    let report = guide::load_saved_report();
    Some(CheckResult {
        path,
        status,
        report_age_days: report.as_ref().map(guide::report_age_days),
        highlights: report.as_ref().map(highlights).unwrap_or_default(),
        regenerable: report.as_ref().map(|r| r.totals.safe),
    })
}

fn highlights(report: &GuideReport) -> Vec<String> {
    report
        .categories
        .iter()
        .take(3)
        .map(|c| format!("{} {}", c.label, format_bytes(c.bytes)))
        .collect()
}

pub fn reason_text(reason: &AlertReason, lang: Language) -> String {
    match (reason, lang) {
        (AlertReason::LowFreePercent { percent, threshold }, Language::Portuguese) => {
            format!("só {percent:.0}% livre (limite {threshold:.0}%)")
        }
        (AlertReason::LowFreePercent { percent, threshold }, Language::English) => {
            format!("only {percent:.0}% free (threshold {threshold:.0}%)")
        }
        (AlertReason::LowFreeBytes { free, threshold }, Language::Portuguese) => format!(
            "só {} livres (limite {})",
            format_bytes(*free),
            format_bytes(*threshold)
        ),
        (AlertReason::LowFreeBytes { free, threshold }, Language::English) => format!(
            "only {} free (threshold {})",
            format_bytes(*free),
            format_bytes(*threshold)
        ),
        (AlertReason::FastFill { lost, hours }, Language::Portuguese) => {
            format!("perdeu {} nas últimas {hours}h", format_bytes(*lost))
        }
        (AlertReason::FastFill { lost, hours }, Language::English) => {
            format!("lost {} in the last {hours}h", format_bytes(*lost))
        }
    }
}

/// One-line headline, e.g. "Disk: 9% free (164 GB) — only 9% free ...".
pub fn headline(r: &CheckResult, lang: Language) -> String {
    let s = &r.status;
    let base = format!(
        "{} {:.0}% {} ({} {} {})",
        tr(lang, "Disco:", "Disk:"),
        s.free_percent,
        tr(lang, "livre", "free"),
        format_bytes(s.free),
        tr(lang, "de", "of"),
        format_bytes(s.total)
    );
    if s.reasons.is_empty() {
        format!("{base} — ok")
    } else {
        let reasons: Vec<String> = s.reasons.iter().map(|x| reason_text(x, lang)).collect();
        format!("{base} — {}", reasons.join("; "))
    }
}

fn report_line(r: &CheckResult, lang: Language) -> Option<String> {
    let age = r.report_age_days?;
    let when = match (age, lang) {
        (0, Language::Portuguese) => "de hoje".to_string(),
        (1, Language::Portuguese) => "de ontem".to_string(),
        (d, Language::Portuguese) => format!("de {d} dias atrás"),
        (0, Language::English) => "from today".to_string(),
        (1, Language::English) => "from yesterday".to_string(),
        (d, Language::English) => format!("from {d} days ago"),
    };
    let mut line = format!(
        "{} {when}: {}",
        tr(lang, "Relatório", "Report"),
        r.highlights.join(" · ")
    );
    if let Some(safe) = r.regenerable {
        line.push_str(&format!(
            " · {} {}",
            format_bytes(safe),
            tr(lang, "regeneram sozinhos", "regenerate on their own")
        ));
    }
    Some(line)
}

/// Text for humans and agent hooks.
pub fn render(r: &CheckResult, mode: CheckMode, lang: Language) -> Option<String> {
    let problem = r.status.level != AlertLevel::Ok;
    match mode {
        CheckMode::Json => serde_json::to_string_pretty(r).ok(),
        CheckMode::Hook if !problem => None,
        CheckMode::Hook => {
            let mut text = format!("acari: {}.", headline(r, lang));
            if let Some(line) = report_line(r, lang) {
                text.push(' ');
                text.push_str(&line);
                text.push('.');
            }
            text.push(' ');
            text.push_str(tr(
                lang,
                "Antes de builds, downloads ou imagens grandes, avise o usuário e sugira liberar espaço (ferramentas MCP do acari ou `acari guide`).",
                "Before large builds, downloads or images, tell the user and suggest freeing space (acari MCP tools or `acari guide`).",
            ));
            Some(text)
        }
        CheckMode::Human | CheckMode::Notify => {
            let mut lines = vec![headline(r, lang)];
            match report_line(r, lang) {
                Some(line) => lines.push(line),
                None => lines.push(
                    tr(
                        lang,
                        "Ainda não há relatório salvo.",
                        "No saved report yet.",
                    )
                    .to_string(),
                ),
            }
            lines.push(
                if problem {
                    tr(lang, "Rode: acari guide", "Run: acari guide")
                } else {
                    tr(lang, "Detalhes: acari guide", "Details: acari guide")
                }
                .to_string(),
            );
            Some(lines.join("\n"))
        }
    }
}

fn should_notify(prev: &NotifyState, level: AlertLevel, now: u64) -> bool {
    level != AlertLevel::Ok
        && (prev.last_level.is_none_or(|last| level > last)
            || now.saturating_sub(prev.last_at) >= RENOTIFY_SECS)
}

/// Entry point for the CLI. Always succeeds for hook/notify modes so a
/// broken check never breaks an agent session or a timer.
pub fn run_check(mode: CheckMode, lang: Language) -> anyhow::Result<()> {
    let cfg = target_config::load_config();
    let Some(result) = measure(&cfg) else {
        if matches!(mode, CheckMode::Human | CheckMode::Json) {
            anyhow::bail!("could not read disk usage");
        }
        return Ok(());
    };
    match mode {
        CheckMode::Notify => {
            let now = now_secs();
            let prev: NotifyState = state::read_json(NOTIFY_FILE).unwrap_or_default();
            let level = result.status.level;
            if should_notify(&prev, level, now) {
                let body = render(&result, CheckMode::Notify, lang).unwrap_or_default();
                let urgency = if level == AlertLevel::Critical {
                    notify::Urgency::Critical
                } else {
                    notify::Urgency::Normal
                };
                match notify::send("acari", &body, urgency) {
                    Ok(()) => {
                        let _ = state::write_json(
                            NOTIFY_FILE,
                            &NotifyState {
                                last_at: now,
                                last_level: Some(level),
                            },
                        );
                    }
                    Err(e) => eprintln!("acari: notification failed: {e}"),
                }
            } else if level == AlertLevel::Ok && prev.last_level.is_some() {
                let _ = state::write_json(NOTIFY_FILE, &NotifyState::default());
            }
        }
        _ => {
            if let Some(text) = render(&result, mode, lang) {
                println!("{text}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::alerts::AlertThresholds;

    const GB: u64 = 1_000_000_000;

    fn result(free: u64) -> CheckResult {
        let total = 1_000 * GB;
        CheckResult {
            path: PathBuf::from("/home/u"),
            status: alerts::evaluate(
                total,
                total - free,
                free,
                &[],
                0,
                &AlertThresholds::default(),
            ),
            report_age_days: Some(1),
            highlights: vec!["Games 292 GB".into()],
            regenerable: Some(134 * GB),
        }
    }

    #[test]
    fn hook_is_silent_when_healthy_and_explains_when_low() {
        assert_eq!(
            render(&result(500 * GB), CheckMode::Hook, Language::English),
            None
        );
        let text = render(&result(90 * GB), CheckMode::Hook, Language::English).unwrap();
        assert!(text.starts_with("acari: Disk: 9% free"), "{text}");
        assert!(text.contains("Games 292 GB"));
        assert!(text.contains("from yesterday"));
    }

    #[test]
    fn human_output_always_has_a_status() {
        let text = render(&result(500 * GB), CheckMode::Human, Language::Portuguese).unwrap();
        assert!(text.contains("— ok"));
        assert!(text.contains("acari guide"));
    }

    #[test]
    fn notifications_are_deduplicated() {
        let none = NotifyState::default();
        assert!(should_notify(&none, AlertLevel::Warning, 100));
        assert!(!should_notify(&none, AlertLevel::Ok, 100));
        let warned = NotifyState {
            last_at: 100,
            last_level: Some(AlertLevel::Warning),
        };
        assert!(!should_notify(&warned, AlertLevel::Warning, 200));
        assert!(should_notify(&warned, AlertLevel::Critical, 200));
        assert!(should_notify(
            &warned,
            AlertLevel::Warning,
            100 + RENOTIFY_SECS
        ));
    }
}
