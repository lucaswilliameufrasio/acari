use std::collections::HashMap;
use std::io::{self, Stdout};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::application::cleaner::{CleanMode, start_background_clean};
use crate::application::commands::start_scan;
use crate::config::target_config::IoPriority;
use crate::domain::{AppEvent, CleanTarget, format_bytes};
use crate::i18n::{Language, msg};
use crate::infrastructure::distro;
use crate::infrastructure::history;
use crate::ui::resolve_scroll;

#[derive(Clone, Copy, Debug, Default)]
struct TargetState {
    bytes: u64,
    files: u64,
    scan_done: bool,
    selected: bool,
    cleaned: bool,
    removed_entries: u64,
    clean_errors: u64,
    reclaimed_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Phase {
    Scanning,
    ReadyToClean,
    Confirming(Vec<(CleanTarget, u64, u64)>),
    Cleaning,
    Finished,
}

enum UiCommand {
    None,
    Quit,
    CancelScan,
    Rescan,
    ToggleDryRun,
    SortBySize,
    Confirm(Vec<(CleanTarget, u64, u64)>),
    Clean(Vec<(CleanTarget, u64, u64)>),
}

struct ScanResources {
    tx: UnboundedSender<AppEvent>,
    rx: UnboundedReceiver<AppEvent>,
    handle: tokio::task::JoinHandle<()>,
}

pub fn run_tui(
    targets: &[CleanTarget],
    excludes: Vec<String>,
    lang: Language,
    io_priority: IoPriority,
) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, targets, excludes, lang, io_priority);
    restore_terminal(&mut terminal)?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    targets: &[CleanTarget],
    excludes: Vec<String>,
    lang: Language,
    io_priority: IoPriority,
) -> Result<()> {
    let targets_owned = targets.to_vec();
    let mut rows: Vec<(CleanTarget, TargetState)> = targets_owned
        .iter()
        .cloned()
        .map(|t| (t, TargetState::default()))
        .collect();

    let mut by_name: HashMap<String, usize> = rows
        .iter()
        .enumerate()
        .map(|(idx, (t, _))| (t.name.to_string(), idx))
        .collect();

    let total_targets = rows.len() as u64;
    let mut scan_res: Option<ScanResources> =
        Some(start_new_scan(&targets_owned, &excludes, io_priority));
    let mut finished_targets = 0_u64;
    let mut total_scanned_bytes = 0_u64;
    let mut phase = Phase::Scanning;
    let mut selected_idx = 0_usize;
    let mut status_line = String::from(msg::tui_scanning_status(lang));
    let mut dry_run = false;
    let mut target_scroll = 0_usize;
    let mut clean_handle: Option<tokio::task::JoinHandle<()>> = None;

    let frame_time = Duration::from_millis(16);
    let mut last_tick = Instant::now();

    loop {
        if let Some(ref mut res) = scan_res {
            while let Ok(event) = res.rx.try_recv() {
                handle_event(
                    &mut rows,
                    &by_name,
                    &mut finished_targets,
                    &mut total_scanned_bytes,
                    &mut phase,
                    &mut status_line,
                    event,
                    dry_run,
                    lang,
                );
            }
        }

        terminal.draw(|frame| {
            draw_ui(
                frame.area(),
                frame,
                &rows,
                selected_idx,
                finished_targets,
                total_targets,
                total_scanned_bytes,
                &phase,
                &status_line,
                dry_run,
                lang,
                &mut target_scroll,
            )
        })?;

        if event::poll(Duration::from_millis(1))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match handle_key(
                &mut rows,
                &mut selected_idx,
                &mut phase,
                &mut status_line,
                key.code,
                lang,
                scan_res.is_some(),
            ) {
                UiCommand::None => {}
                UiCommand::Quit => break,
                UiCommand::CancelScan => {
                    if let Some(res) = scan_res.take() {
                        res.handle.abort();
                    }
                    phase = Phase::ReadyToClean;
                    status_line = String::from(msg::tui_cancelled_status(lang));
                }
                UiCommand::Rescan => {
                    if let Some(ref res) = scan_res {
                        res.handle.abort();
                    }
                    if let Some(h) = clean_handle.take() {
                        h.abort();
                    }
                    scan_res = Some(start_new_scan(&targets_owned, &excludes, io_priority));
                    for (_, state) in &mut rows {
                        *state = TargetState::default();
                    }
                    finished_targets = 0;
                    total_scanned_bytes = 0;
                    phase = Phase::Scanning;
                    target_scroll = 0;
                    status_line = String::from(msg::tui_scanning_status(lang));
                }
                UiCommand::ToggleDryRun => {
                    dry_run = !dry_run;
                    status_line = if dry_run {
                        msg::mode_dry_run(lang).to_string()
                    } else {
                        msg::mode_execute(lang).to_string()
                    };
                }
                UiCommand::SortBySize => {
                    rows.sort_by_key(|b| std::cmp::Reverse(b.1.bytes));
                    for (i, (t, _)) in rows.iter().enumerate() {
                        by_name.insert(t.name.to_string(), i);
                    }
                    selected_idx = 0;
                    status_line = msg::sorted_by_size(lang).to_string();
                }
                UiCommand::Confirm(selected) => {
                    let total_bytes: u64 = selected.iter().map(|(_, b, _)| *b).sum();
                    let count = selected.len();
                    status_line = msg::confirm_clean(lang)
                        .replace("{n}", &count.to_string())
                        .replace("{size}", &format_bytes(total_bytes));
                    phase = Phase::Confirming(selected);
                }
                UiCommand::Clean(selected) => {
                    if let Some(ref res) = scan_res {
                        let mode = if dry_run {
                            CleanMode::DryRun
                        } else {
                            CleanMode::Execute
                        };
                        let h = start_background_clean(res.tx.clone(), selected, mode);
                        clean_handle = Some(h);
                    }
                }
            }
        }

        let elapsed = last_tick.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
        last_tick = Instant::now();
    }

    if let Some(res) = scan_res {
        res.handle.abort();
    }

    Ok(())
}

fn start_new_scan(
    targets: &[CleanTarget],
    excludes: &[String],
    io_priority: IoPriority,
) -> ScanResources {
    let (tx, rx, handle) = start_scan(targets.to_vec(), excludes.to_vec(), io_priority);
    ScanResources { tx, rx, handle }
}

#[allow(clippy::too_many_arguments)]
fn handle_event(
    rows: &mut [(CleanTarget, TargetState)],
    by_name: &HashMap<String, usize>,
    finished_targets: &mut u64,
    total_scanned_bytes: &mut u64,
    phase: &mut Phase,
    status_line: &mut String,
    event: AppEvent,
    is_dry_run: bool,
    lang: Language,
) {
    match event {
        AppEvent::ScanProgress {
            target_name,
            bytes_found,
            files_scanned,
        } => {
            if let Some(idx) = by_name.get(&target_name).copied() {
                let (_, state) = &mut rows[idx];
                state.bytes = bytes_found;
                state.files = files_scanned;
            }
        }
        AppEvent::TargetCompleted {
            target_name,
            total_bytes,
            files_scanned,
        } => {
            if let Some(idx) = by_name.get(&target_name).copied() {
                let (_, state) = &mut rows[idx];
                if !state.scan_done {
                    state.bytes = total_bytes;
                    state.files = files_scanned;
                    state.scan_done = true;
                    *finished_targets = finished_targets.saturating_add(1);
                    *total_scanned_bytes = total_scanned_bytes.saturating_add(total_bytes);
                }
            }
        }
        AppEvent::ScanFinished => {
            *phase = Phase::ReadyToClean;
            *status_line = String::from(msg::tui_ready_status(lang));
        }
        AppEvent::TargetCleaned {
            target_name,
            reclaimed_bytes,
            removed_entries,
            errors,
        } => {
            if let Some(idx) = by_name.get(&target_name).copied() {
                let (_, state) = &mut rows[idx];
                state.cleaned = true;
                state.reclaimed_bytes = reclaimed_bytes;
                state.removed_entries = removed_entries;
                state.clean_errors = errors;
            }
        }
        AppEvent::CleaningFinished {
            cleaned_targets: done,
            reclaimed_bytes,
            errors,
        } => {
            *phase = Phase::Finished;
            let tmpl = msg::tui_finished_status(lang);
            *status_line = tmpl
                .replace("{done}", &done.to_string())
                .replace("{reclaimed}", &format_bytes(reclaimed_bytes))
                .replace("{errors}", &errors.to_string());
            if !is_dry_run {
                let time = history::format_local_time();
                history::append_entry(&format!(
                    "{time} | Clean completed | targets={done} reclaimed={reclaimed_bytes} errors={errors}"
                ));
            }
        }
        AppEvent::Tick => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_key(
    rows: &mut [(CleanTarget, TargetState)],
    selected_idx: &mut usize,
    phase: &mut Phase,
    status_line: &mut String,
    key_code: KeyCode,
    lang: Language,
    has_active_scan: bool,
) -> UiCommand {
    match key_code {
        KeyCode::Char('n') | KeyCode::Esc if matches!(phase, Phase::Confirming(_)) => {
            *phase = Phase::ReadyToClean;
            *status_line = String::from(msg::tui_ready_status(lang));
            UiCommand::None
        }
        KeyCode::Char('y') if matches!(phase, Phase::Confirming(_)) => {
            if let Phase::Confirming(selected) = phase {
                let sel = std::mem::take(selected);
                *phase = Phase::Cleaning;
                *status_line = String::from(msg::tui_cleaning_status(lang));
                return UiCommand::Clean(sel);
            }
            UiCommand::None
        }
        KeyCode::Char('q') => {
            if *phase == Phase::Scanning && has_active_scan {
                UiCommand::CancelScan
            } else {
                UiCommand::Quit
            }
        }
        KeyCode::Esc => UiCommand::Quit,
        KeyCode::Char('d') if *phase == Phase::ReadyToClean => UiCommand::ToggleDryRun,
        KeyCode::Char('s') if *phase == Phase::ReadyToClean => UiCommand::SortBySize,
        KeyCode::Char('r') if *phase != Phase::Scanning && *phase != Phase::Cleaning => {
            UiCommand::Rescan
        }
        KeyCode::Up if *phase == Phase::ReadyToClean => {
            *selected_idx = selected_idx.saturating_sub(1);
            UiCommand::None
        }
        KeyCode::Down if *phase == Phase::ReadyToClean => {
            if *selected_idx + 1 < rows.len() {
                *selected_idx += 1;
            }
            UiCommand::None
        }
        KeyCode::Char(' ') if *phase == Phase::ReadyToClean => {
            if let Some((_, state)) = rows.get_mut(*selected_idx) {
                state.selected = !state.selected;
            }
            UiCommand::None
        }
        KeyCode::Char('a') if *phase == Phase::ReadyToClean => {
            let should_select_all = rows.iter().any(|(_, s)| !s.selected);
            for (_, state) in rows {
                state.selected = should_select_all;
            }
            UiCommand::None
        }
        KeyCode::Enter if *phase == Phase::ReadyToClean => {
            let selected: Vec<(CleanTarget, u64, u64)> = rows
                .iter()
                .filter(|(_, s)| s.selected)
                .map(|(target, s)| (target.clone(), s.bytes, s.files))
                .collect();

            if selected.is_empty() {
                *status_line = String::from(msg::tui_no_selection(lang));
                UiCommand::None
            } else {
                *status_line = String::from(msg::confirm_prompt(lang));
                UiCommand::Confirm(selected)
            }
        }
        _ => UiCommand::None,
    }
}

fn format_scanning_label(
    rows: &[(CleanTarget, TargetState)],
    finished_targets: u64,
    total_targets: u64,
    total_scanned_bytes: u64,
    lang: Language,
) -> String {
    let live: u64 = rows
        .iter()
        .filter(|(_, s)| !s.scan_done)
        .map(|(_, s)| s.bytes)
        .sum();
    format!(
        "{} | {}",
        msg::scanning_progress(lang)
            .replace("{n}", &finished_targets.to_string())
            .replace("{total}", &total_targets.to_string()),
        format_bytes(total_scanned_bytes.saturating_add(live))
    )
}

fn visible_target_list<'a>(
    all_items: &'a [ListItem<'a>],
    selected: usize,
    scroll: usize,
    visible_rows: usize,
) -> (usize, Vec<ListItem<'a>>) {
    let new_scroll = resolve_scroll(selected, scroll, visible_rows);
    let start = new_scroll.min(all_items.len());
    let end = (start + visible_rows).min(all_items.len());
    (new_scroll, all_items[start..end].to_vec())
}

#[allow(clippy::too_many_arguments)]
fn draw_ui(
    area: Rect,
    frame: &mut ratatui::Frame,
    rows: &[(CleanTarget, TargetState)],
    selected_idx: usize,
    finished_targets: u64,
    total_targets: u64,
    total_scanned_bytes: u64,
    phase: &Phase,
    status_line: &str,
    dry_run: bool,
    lang: Language,
    target_scroll: &mut usize,
) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let dinfo = distro::detect();
    let title_text = format!("{} | {}", msg::tui_title(lang), dinfo.pretty_name);
    let title = Paragraph::new(title_text)
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(msg::panel_status(lang)),
        );
    frame.render_widget(title, vertical[0]);

    let ratio = if total_targets == 0 {
        0.0
    } else {
        finished_targets as f64 / total_targets as f64
    };

    let progress_label = match phase {
        Phase::Scanning => format_scanning_label(
            rows,
            finished_targets,
            total_targets,
            total_scanned_bytes,
            lang,
        ),
        Phase::ReadyToClean => {
            msg::scan_done_progress(lang).replace("{size}", &format_bytes(total_scanned_bytes))
        }
        Phase::Confirming(_) => {
            msg::scan_done_progress(lang).replace("{size}", &format_bytes(total_scanned_bytes))
        }
        Phase::Cleaning => msg::cleaning_progress(lang).to_string(),
        Phase::Finished => msg::cleaning_finished_progress(lang).to_string(),
    };

    let progress = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(msg::panel_progress(lang)),
        )
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black))
        .ratio(ratio)
        .label(progress_label);
    frame.render_widget(progress, vertical[1]);

    let all_items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(idx, (target, state))| {
            let cursor = if *phase == Phase::ReadyToClean && idx == selected_idx {
                ">"
            } else {
                " "
            };
            let sel = if state.selected { "[x]" } else { "[ ]" };
            let scan_mark = if state.scan_done {
                "scan:ok"
            } else {
                "scan:.."
            };
            let clean_mark = if state.cleaned {
                if state.clean_errors == 0 {
                    "clean:ok"
                } else {
                    "clean:err"
                }
            } else {
                "clean:--"
            };

            let text = format!(
                "{cursor} {sel} {} | {scan_mark} | {} | {} files | {clean_mark}",
                target.name,
                format_bytes(state.bytes),
                state.files,
            );

            let style = if *phase == Phase::ReadyToClean && idx == selected_idx {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default()
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let list_area = vertical[2];
    let visible_rows = (list_area.height.saturating_sub(2)).max(1) as usize;
    let (new_scroll, visible_items) =
        visible_target_list(&all_items, selected_idx, *target_scroll, visible_rows);
    *target_scroll = new_scroll;

    let list = List::new(visible_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(msg::panel_targets(lang)),
    );
    frame.render_widget(list, list_area);

    let mut footer_text = status_line.to_string();
    if *phase == Phase::Scanning {
        footer_text = format!("{} | {}", msg::tui_cancel_hint(lang), footer_text);
    } else if *phase == Phase::ReadyToClean || *phase == Phase::Finished {
        let mode = if dry_run {
            msg::mode_dry_run(lang)
        } else {
            msg::mode_execute(lang)
        };
        footer_text = format!(
            "[{}] {} | {}",
            mode,
            msg::tui_rescan_hint(lang),
            footer_text
        );
    }

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(msg::panel_footer(lang)),
        );
    frame.render_widget(footer, vertical[3]);
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::HashMap;

    use crossterm::event::KeyCode;

    use crate::domain::{AppEvent, CleanTarget};
    use crate::i18n::Language;

    use ratatui::widgets::ListItem;

    use super::{
        Phase, TargetState, UiCommand, format_scanning_label, handle_event, handle_key,
        visible_target_list,
    };
    use crate::ui::resolve_scroll;

    fn build_rows() -> Vec<(CleanTarget, TargetState)> {
        vec![
            (
                CleanTarget {
                    name: Cow::Borrowed("A"),
                    path: Cow::Borrowed("/tmp/a"),
                    description: Cow::Borrowed("a"),
                },
                TargetState::default(),
            ),
            (
                CleanTarget {
                    name: Cow::Borrowed("B"),
                    path: Cow::Borrowed("/tmp/b"),
                    description: Cow::Borrowed("b"),
                },
                TargetState::default(),
            ),
        ]
    }

    #[test]
    fn scan_finished_moves_to_ready_phase() {
        let mut rows = build_rows();
        let by_name = HashMap::from([(String::from("A"), 0_usize), (String::from("B"), 1_usize)]);
        let mut finished_targets = 0_u64;
        let mut total_bytes = 0_u64;
        let mut phase = Phase::Scanning;
        let mut status = String::new();

        handle_event(
            &mut rows,
            &by_name,
            &mut finished_targets,
            &mut total_bytes,
            &mut phase,
            &mut status,
            AppEvent::ScanFinished,
            false,
            Language::English,
        );

        assert_eq!(phase, Phase::ReadyToClean);
        assert!(status.contains("arrows") || status.contains("↑/↓"));
    }

    #[test]
    fn space_toggles_selected_row() {
        let mut rows = build_rows();
        let mut idx = 1_usize;
        let mut phase = Phase::ReadyToClean;
        let mut status = String::new();

        let cmd = handle_key(
            &mut rows,
            &mut idx,
            &mut phase,
            &mut status,
            KeyCode::Char(' '),
            Language::English,
            false,
        );

        assert!(matches!(cmd, UiCommand::None));
        assert!(rows[1].1.selected);
    }

    #[test]
    fn enter_with_selected_rows_starts_cleaning() {
        let mut rows = build_rows();
        rows[0].1.selected = true;
        rows[0].1.bytes = 42;
        rows[0].1.files = 7;

        let mut idx = 0_usize;
        let mut phase = Phase::ReadyToClean;
        let mut status = String::new();

        let cmd = handle_key(
            &mut rows,
            &mut idx,
            &mut phase,
            &mut status,
            KeyCode::Enter,
            Language::English,
            false,
        );

        assert_eq!(phase, Phase::ReadyToClean);
        assert!(status.contains("confirmar") || status.contains("y"));
        match cmd {
            UiCommand::Confirm(selected) => {
                assert_eq!(selected.len(), 1);
                assert_eq!(selected[0].0.name, "A");
                assert_eq!(selected[0].1, 42);
                assert_eq!(selected[0].2, 7);
            }
            _ => panic!("expected Confirm"),
        }
    }

    #[test]
    fn q_during_scanning_returns_cancel() {
        let mut rows = build_rows();
        let mut idx = 0_usize;
        let mut phase = Phase::Scanning;
        let mut status = String::new();

        let cmd = handle_key(
            &mut rows,
            &mut idx,
            &mut phase,
            &mut status,
            KeyCode::Char('q'),
            Language::English,
            true,
        );

        assert!(matches!(cmd, UiCommand::CancelScan));
    }

    #[test]
    fn q_when_scan_done_returns_quit() {
        let mut rows = build_rows();
        let mut idx = 0_usize;
        let mut phase = Phase::ReadyToClean;
        let mut status = String::new();

        let cmd = handle_key(
            &mut rows,
            &mut idx,
            &mut phase,
            &mut status,
            KeyCode::Char('q'),
            Language::English,
            false,
        );

        assert!(matches!(cmd, UiCommand::Quit));
    }

    #[test]
    fn r_in_ready_returns_rescan() {
        let mut rows = build_rows();
        let mut idx = 0_usize;
        let mut phase = Phase::ReadyToClean;
        let mut status = String::new();

        let cmd = handle_key(
            &mut rows,
            &mut idx,
            &mut phase,
            &mut status,
            KeyCode::Char('r'),
            Language::English,
            false,
        );

        assert!(matches!(cmd, UiCommand::Rescan));
    }

    #[test]
    fn r_during_scanning_is_ignored() {
        let mut rows = build_rows();
        let mut idx = 0_usize;
        let mut phase = Phase::Scanning;
        let mut status = String::new();

        let cmd = handle_key(
            &mut rows,
            &mut idx,
            &mut phase,
            &mut status,
            KeyCode::Char('r'),
            Language::English,
            true,
        );

        assert!(matches!(cmd, UiCommand::None));
    }

    // --- format_scanning_label ---

    fn label_rows(done: &[(bool, u64)]) -> Vec<(CleanTarget, TargetState)> {
        done.iter()
            .map(|(scan_done, bytes)| {
                (
                    CleanTarget {
                        name: Cow::Owned("x".into()),
                        path: Cow::Owned("/x".into()),
                        description: Cow::Owned("test".into()),
                    },
                    TargetState {
                        bytes: *bytes,
                        scan_done: *scan_done,
                        ..TargetState::default()
                    },
                )
            })
            .collect()
    }

    #[test]
    fn gauge_label_includes_in_progress_bytes() {
        let rows = label_rows(&[(true, 100), (false, 50)]);
        let label = format_scanning_label(&rows, 1, 2, 100, Language::English);
        assert!(
            label.contains("150"),
            "100 completed + 50 in-progress = 150"
        );
    }

    #[test]
    fn gauge_label_all_done_shows_only_total() {
        let rows = label_rows(&[(true, 100), (true, 50)]);
        let label = format_scanning_label(&rows, 2, 2, 150, Language::English);
        assert!(label.contains("150"), "all done, no in-progress bytes");
    }

    #[test]
    fn gauge_label_shows_progress_fraction() {
        let rows = label_rows(&[(false, 0), (false, 0)]);
        let label = format_scanning_label(&rows, 0, 2, 0, Language::English);
        assert!(label.contains("0/2") || label.contains("Scanning 0/2"));
    }

    // --- resolve_scroll ---

    #[test]
    fn resolve_scroll_above() {
        assert_eq!(resolve_scroll(0, 5, 3), 0);
    }

    #[test]
    fn resolve_scroll_below() {
        assert_eq!(resolve_scroll(9, 5, 3), 7);
    }

    #[test]
    fn resolve_scroll_stays_visible() {
        assert_eq!(resolve_scroll(6, 5, 3), 5);
    }

    #[test]
    fn resolve_scroll_zero_visible() {
        assert_eq!(resolve_scroll(3, 5, 0), 0);
    }

    // --- visible_target_list ---

    fn make_items<'a>(labels: &'a [&'a str]) -> Vec<ListItem<'a>> {
        labels
            .iter()
            .map(|l| ListItem::new(ratatui::text::Line::from(*l)))
            .collect()
    }

    #[test]
    fn visible_list_scrolls_down_when_selected_below() {
        let labels = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let items = make_items(&labels);
        let (new_scroll, vis) = visible_target_list(&items, 8, 0, 3);
        assert_eq!(new_scroll, 6, "scroll should advance to show selected at 8");
        assert_eq!(vis.len(), 3, "3 visible items");
        assert_eq!(vis.len(), 3, "3 items visible");
    }

    #[test]
    fn visible_list_scrolls_up_when_selected_above() {
        let labels = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let items = make_items(&labels);
        let (new_scroll, _vis) = visible_target_list(&items, 1, 5, 3);
        assert_eq!(new_scroll, 1, "scroll should go back to 1");
    }

    #[test]
    fn visible_list_stays_when_selected_visible() {
        let labels = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let items = make_items(&labels);
        let (new_scroll, vis) = visible_target_list(&items, 6, 5, 3);
        assert_eq!(new_scroll, 5, "scroll unchanged");
        assert_eq!(vis.len(), 3);
    }

    #[test]
    fn visible_list_handles_empty() {
        let items = vec![];
        let (new_scroll, vis) = visible_target_list(&items, 0, 0, 3);
        assert_eq!(new_scroll, 0);
        assert!(vis.is_empty());
    }
}
