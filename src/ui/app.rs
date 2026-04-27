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
use crate::domain::{AppEvent, CleanTarget};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Scanning,
    ReadyToClean,
    Cleaning,
    Finished,
}

enum UiCommand {
    None,
    Quit,
    StartCleaning(Vec<(CleanTarget, u64, u64)>),
}

pub fn run_tui(
    tx: UnboundedSender<AppEvent>,
    rx: UnboundedReceiver<AppEvent>,
    targets: &[CleanTarget],
) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, tx, rx, targets);
    restore_terminal(&mut terminal)?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    tx: UnboundedSender<AppEvent>,
    mut rx: UnboundedReceiver<AppEvent>,
    targets: &[CleanTarget],
) -> Result<()> {
    let mut rows: Vec<(CleanTarget, TargetState)> = targets
        .iter()
        .cloned()
        .map(|t| (t, TargetState::default()))
        .collect();

    let by_name: HashMap<String, usize> = rows
        .iter()
        .enumerate()
        .map(|(idx, (t, _))| (t.name.to_string(), idx))
        .collect();

    let total_targets = rows.len() as u64;
    let mut finished_targets = 0_u64;
    let mut total_scanned_bytes = 0_u64;
    let mut phase = Phase::Scanning;
    let mut selected_idx = 0_usize;
    let mut status_line = String::from("Scanning in background...");

    let frame_time = Duration::from_millis(16);
    let mut last_tick = Instant::now();

    loop {
        while let Ok(msg) = rx.try_recv() {
            handle_event(
                &mut rows,
                &by_name,
                &mut finished_targets,
                &mut total_scanned_bytes,
                &mut phase,
                &mut status_line,
                msg,
            );
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
                phase,
                &status_line,
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
            ) {
                UiCommand::None => {}
                UiCommand::Quit => break,
                UiCommand::StartCleaning(selected) => {
                    let _clean_handle =
                        start_background_clean(tx.clone(), selected, CleanMode::Execute);
                }
            }
        }

        let elapsed = last_tick.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
        last_tick = Instant::now();
    }

    Ok(())
}

fn handle_event(
    rows: &mut [(CleanTarget, TargetState)],
    by_name: &HashMap<String, usize>,
    finished_targets: &mut u64,
    total_scanned_bytes: &mut u64,
    phase: &mut Phase,
    status_line: &mut String,
    msg: AppEvent,
) {
    match msg {
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
            *status_line =
                String::from("Scan finalizado. Use ↑/↓, espaço para marcar e Enter para limpar.");
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
            *status_line = format!(
                "Limpeza concluída: {} alvos, {} recuperados, {} erros.",
                done,
                format_bytes(reclaimed_bytes),
                errors
            );
        }
        AppEvent::Tick => {}
    }
}

fn handle_key(
    rows: &mut [(CleanTarget, TargetState)],
    selected_idx: &mut usize,
    phase: &mut Phase,
    status_line: &mut String,
    key_code: KeyCode,
) -> UiCommand {
    match key_code {
        KeyCode::Char('q') | KeyCode::Esc => UiCommand::Quit,
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
                *status_line = String::from("Nenhum alvo selecionado para limpeza.");
                UiCommand::None
            } else {
                *phase = Phase::Cleaning;
                *status_line = String::from("Limpando alvos selecionados...");
                UiCommand::StartCleaning(selected)
            }
        }
        _ => UiCommand::None,
    }
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
    phase: Phase,
    status_line: &str,
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

    let title = Paragraph::new("Acari Cleaner | macOS/Linux cache scanner")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(title, vertical[0]);

    let ratio = if total_targets == 0 {
        0.0
    } else {
        finished_targets as f64 / total_targets as f64
    };

    let progress_label = match phase {
        Phase::Scanning => format!("Scanning {finished_targets}/{total_targets}"),
        Phase::ReadyToClean => format!("Scan done: {}", format_bytes(total_scanned_bytes)),
        Phase::Cleaning => String::from("Cleaning selected targets"),
        Phase::Finished => String::from("Cleaning finished"),
    };

    let progress = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black))
        .ratio(ratio)
        .label(progress_label);
    frame.render_widget(progress, vertical[1]);

    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(idx, (target, state))| {
            let cursor = if phase == Phase::ReadyToClean && idx == selected_idx {
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

            let style = if phase == Phase::ReadyToClean && idx == selected_idx {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default()
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Targets (space: toggle, a: all, enter: clean)"),
    );
    frame.render_widget(list, vertical[2]);

    let footer = Paragraph::new(status_line)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Footer"));
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

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0_usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{Phase, TargetState, UiCommand, handle_event, handle_key};
    use crate::domain::{AppEvent, CleanTarget};
    use crossterm::event::KeyCode;
    use std::collections::HashMap;

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
        );

        assert_eq!(phase, Phase::ReadyToClean);
        assert!(status.contains("Enter"));
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

        let cmd = handle_key(&mut rows, &mut idx, &mut phase, &mut status, KeyCode::Enter);

        assert_eq!(phase, Phase::Cleaning);
        assert!(status.contains("Limpando"));
        match cmd {
            UiCommand::StartCleaning(selected) => {
                assert_eq!(selected.len(), 1);
                assert_eq!(selected[0].0.name, "A");
                assert_eq!(selected[0].1, 42);
                assert_eq!(selected[0].2, 7);
            }
            _ => panic!("expected StartCleaning"),
        }
    }
}
