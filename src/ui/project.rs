use std::io::{self, Stdout};

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
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::application::commands::merge_excludes;
use crate::config::target_config::{self, TargetConfig};
use crate::domain::project_scan::{self, builtin_patterns};
use crate::i18n::{Language, msg};
use crate::ui::app::run_tui;

#[derive(Clone, Copy, PartialEq)]
enum InputMode {
    Idle,
    AddPattern,
    AddRoot,
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Patterns,
    Roots,
}

pub struct ProjectTui {
    cfg: TargetConfig,
    builtins: Vec<&'static str>,
    selected_pattern: usize,
    selected_root: usize,
    focus: Focus,
    input_mode: InputMode,
    input_buf: String,
    status: String,
    lang: Language,
}

impl ProjectTui {
    fn new(cfg: TargetConfig, lang: Language) -> Self {
        let builtins = builtin_patterns();
        let status = if cfg.project_scan.roots.is_empty() {
            format!(
                "{} — {}.",
                msg::no_roots_configured(lang),
                msg::project_hint_add_root(lang)
            )
        } else if cfg.project_scan.patterns.is_empty() {
            msg::project_hint_add_pattern(lang).to_string()
        } else {
            String::new()
        };
        Self {
            cfg,
            builtins,
            selected_pattern: 0,
            selected_root: 0,
            focus: Focus::Patterns,
            input_mode: InputMode::Idle,
            input_buf: String::new(),
            status,
            lang,
        }
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> UiAction {
        loop {
            let _ = terminal.draw(|frame| self.draw(frame.area(), frame));

            if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false)
                && let Event::Key(key) = event::read().unwrap_or(Event::Key(KeyCode::Esc.into()))
                && key.kind == KeyEventKind::Press
            {
                match self.input_mode {
                    InputMode::AddPattern | InputMode::AddRoot => {
                        self.handle_input(key.code);
                    }
                    InputMode::Idle => {
                        let action = self.handle_key(key.code);
                        if !matches!(action, UiAction::None) {
                            return action;
                        }
                    }
                }
            }
        }
    }

    fn handle_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) if c != '\n' && c != '\r' => {
                self.input_buf.push(c);
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Idle;
                self.input_buf.clear();
                self.status = String::new();
            }
            KeyCode::Enter => {
                self.confirm_input();
            }
            _ => {}
        }
    }

    fn confirm_input(&mut self) {
        let val = self.input_buf.trim().to_string();
        match self.input_mode {
            InputMode::AddPattern => {
                if val.is_empty() {
                    self.status = msg::project_empty_pattern(self.lang).to_string();
                } else if self.builtins.iter().any(|p| p == &val) {
                    self.status = msg::pattern_is_builtin(self.lang).replace("{pattern}", &val);
                } else if self.cfg.project_scan.patterns.contains(&val) {
                    self.status = msg::pattern_exists(self.lang).replace("{pattern}", &val);
                } else {
                    self.cfg.project_scan.patterns.push(val.clone());
                    let _ = target_config::save_config(&self.cfg);
                    self.status = msg::pattern_added(self.lang).replace("{pattern}", &val);
                }
            }
            InputMode::AddRoot => {
                if val.is_empty() {
                    self.status = msg::root_empty(self.lang).to_string();
                } else if self.cfg.project_scan.roots.contains(&val) {
                    self.status = msg::root_already_exists(self.lang).replace("{path}", &val);
                } else {
                    self.cfg.project_scan.roots.push(val.clone());
                    let _ = target_config::save_config(&self.cfg);
                    self.status = msg::root_added(self.lang).replace("{path}", &val);
                }
            }
            InputMode::Idle => {}
        }
        self.input_mode = InputMode::Idle;
        self.input_buf.clear();
    }

    fn handle_key(&mut self, key: KeyCode) -> UiAction {
        match key {
            KeyCode::Char('q') => return UiAction::Quit,
            KeyCode::Esc => return UiAction::Quit,
            KeyCode::Char('a') => {
                self.input_mode = InputMode::AddPattern;
                self.input_buf.clear();
                self.status = msg::project_prompt_pattern(self.lang).to_string();
            }
            KeyCode::Char('r') => match self.focus {
                Focus::Patterns => {
                    let idx = self.selected_pattern;
                    let custom_start = self.builtins.len();
                    if idx < custom_start {
                        self.status = msg::pattern_is_builtin(self.lang)
                            .replace("{pattern}", self.builtins.get(idx).unwrap_or(&""));
                    } else {
                        let custom_idx = idx - custom_start;
                        if custom_idx < self.cfg.project_scan.patterns.len() {
                            let removed = self.cfg.project_scan.patterns.remove(custom_idx);
                            let _ = target_config::save_config(&self.cfg);
                            self.status =
                                msg::pattern_removed(self.lang).replace("{pattern}", &removed);
                        }
                    }
                    if self.selected_pattern >= custom_start + self.cfg.project_scan.patterns.len()
                    {
                        self.selected_pattern = self.selected_pattern.saturating_sub(1);
                    }
                }
                Focus::Roots => {
                    let idx = self.selected_root;
                    if idx < self.cfg.project_scan.roots.len() {
                        let removed = self.cfg.project_scan.roots.remove(idx);
                        let _ = target_config::save_config(&self.cfg);
                        self.status = msg::root_removed(self.lang).replace("{path}", &removed);
                    }
                    if self.selected_root >= self.cfg.project_scan.roots.len() {
                        self.selected_root = self.selected_root.saturating_sub(1);
                    }
                }
            },
            KeyCode::Char('p') => {
                self.input_mode = InputMode::AddRoot;
                self.input_buf.clear();
                self.status = msg::project_prompt_root(self.lang).to_string();
            }
            KeyCode::Char('s') => {
                if self.cfg.project_scan.roots.is_empty() {
                    self.status = msg::no_roots_configured(self.lang).to_string();
                } else {
                    self.status = msg::starting_scan(self.lang).to_string();
                    let _ = target_config::save_config(&self.cfg);
                    let _ = terminal_cleanup();
                    return UiAction::RunScan;
                }
            }
            KeyCode::Char('d') => {
                if self.cfg.project_scan.roots.is_empty() {
                    self.status = msg::no_roots_configured(self.lang).to_string();
                } else {
                    self.status = msg::starting_dry_run(self.lang).to_string();
                    let _ = target_config::save_config(&self.cfg);
                    let _ = terminal_cleanup();
                    return UiAction::RunDryRun;
                }
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Patterns => Focus::Roots,
                    Focus::Roots => Focus::Patterns,
                };
            }
            KeyCode::Up => match self.focus {
                Focus::Patterns => {
                    self.selected_pattern = self.selected_pattern.saturating_sub(1);
                }
                Focus::Roots => {
                    self.selected_root = self.selected_root.saturating_sub(1);
                }
            },
            KeyCode::Down => match self.focus {
                Focus::Patterns => {
                    let max = self.builtins.len() + self.cfg.project_scan.patterns.len();
                    if self.selected_pattern + 1 < max {
                        self.selected_pattern += 1;
                    }
                }
                Focus::Roots => {
                    if self.selected_root + 1 < self.cfg.project_scan.roots.len() {
                        self.selected_root += 1;
                    }
                }
            },
            _ => {}
        }
        UiAction::None
    }

    fn draw(&self, area: Rect, frame: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(8),
                Constraint::Length(1),
            ])
            .split(area);

        let title = Paragraph::new(msg::project_tui_title(self.lang))
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        self.draw_patterns_roots(chunks[1], frame);
        self.draw_actions(chunks[2], frame);
        self.draw_footer(chunks[3], frame);
    }

    fn draw_patterns_roots(&self, area: Rect, frame: &mut ratatui::Frame) {
        let sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        self.draw_patterns_list(sections[0], frame);
        self.draw_roots_list(sections[1], frame);
    }

    fn draw_patterns_list(&self, area: Rect, frame: &mut ratatui::Frame) {
        let total = self.builtins.len() + self.cfg.project_scan.patterns.len();
        let mut items: Vec<ListItem> = Vec::new();

        items.push(
            ListItem::new(format!(
                "  {} ({} {})",
                msg::project_builtin_label(self.lang),
                self.builtins.len(),
                msg::project_patterns(self.lang),
            ))
            .style(Style::default().add_modifier(Modifier::DIM)),
        );

        for chunk in self.builtins.chunks(6) {
            items.push(
                ListItem::new(format!("    {}", chunk.join("  ")))
                    .style(Style::default().fg(Color::DarkGray)),
            );
        }

        if !self.cfg.project_scan.patterns.is_empty() {
            items.push(
                ListItem::new(format!(
                    "  {} {}",
                    msg::project_custom_label(self.lang),
                    msg::project_patterns(self.lang),
                ))
                .style(Style::default().add_modifier(Modifier::DIM)),
            );

            for (i, p) in self.cfg.project_scan.patterns.iter().enumerate() {
                let idx = self.builtins.len() + i;
                let prefix = if self.focus == Focus::Patterns && idx == self.selected_pattern {
                    ">"
                } else {
                    " "
                };
                items.push(ListItem::new(format!("{} {}", prefix, p)));
            }
        }

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
            "{} ({})",
            msg::project_patterns_title(self.lang),
            total
        )));
        frame.render_widget(list, area);
    }

    fn draw_roots_list(&self, area: Rect, frame: &mut ratatui::Frame) {
        let mut items: Vec<ListItem> = Vec::new();

        if self.cfg.project_scan.roots.is_empty() {
            items.push(
                ListItem::new(format!("  {}", msg::no_roots_configured(self.lang)))
                    .style(Style::default().fg(Color::DarkGray)),
            );
        }

        for (i, root) in self.cfg.project_scan.roots.iter().enumerate() {
            let prefix = if self.focus == Focus::Roots && i == self.selected_root {
                ">"
            } else {
                " "
            };
            items.push(ListItem::new(format!("{} {}", prefix, root)));
        }

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(msg::project_roots_title(self.lang)),
        );
        frame.render_widget(list, area);
    }

    fn draw_actions(&self, area: Rect, frame: &mut ratatui::Frame) {
        let lines = [
            format!(
                "[s] {}    [d] {}",
                msg::project_action_scan(self.lang),
                msg::project_action_dry_run(self.lang)
            ),
            format!(
                "[a] {}    [p] {}",
                msg::project_action_add_pattern(self.lang),
                msg::project_action_add_root(self.lang)
            ),
            format!(
                "[r] {}    [Tab] {}",
                msg::project_action_remove(self.lang),
                msg::project_action_switch(self.lang)
            ),
            format!("[q] {}", msg::project_action_quit(self.lang)),
        ];

        let text = lines.join("\n");
        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(Color::Cyan))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(msg::project_actions_title(self.lang)),
            );
        frame.render_widget(paragraph, area);
    }

    fn draw_footer(&self, area: Rect, frame: &mut ratatui::Frame) {
        let text = match self.input_mode {
            InputMode::AddPattern | InputMode::AddRoot => {
                let prompt = if self.input_mode == InputMode::AddPattern {
                    msg::project_prompt_pattern(self.lang)
                } else {
                    msg::project_prompt_root(self.lang)
                };
                format!("{} {}", prompt, self.input_buf)
            }
            InputMode::Idle => {
                if self.status.is_empty() {
                    String::new()
                } else {
                    self.status.clone()
                }
            }
        };
        let paragraph = Paragraph::new(text).style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
    }
}

enum UiAction {
    None,
    Quit,
    RunScan,
    RunDryRun,
}

fn terminal_cleanup() -> Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen)?;
    Ok(())
}

fn terminal_setup() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn run_project_tui(cfg: &TargetConfig, lang: Language) -> Result<()> {
    let mut current_cfg = cfg.clone();

    loop {
        let mut terminal = terminal_setup()?;
        let mut tui = ProjectTui::new(current_cfg.clone(), lang);
        let action = tui.run(&mut terminal);
        // Terminal is already cleaned up inside run() when action is RunScan/RunDryRun
        // For Quit, we need to cleanup here
        if matches!(action, UiAction::Quit) {
            terminal_cleanup()?;
            return Ok(());
        }

        let io_priority = tui.cfg.scan.io_priority;
        let roots = tui.cfg.project_scan.roots.clone();
        let patterns = tui.cfg.project_scan.patterns.clone();
        let excludes = merge_excludes(&[], &tui.cfg.scan.exclude_patterns);

        let discovered = project_scan::discover_junk_dirs(&roots, &patterns, false);

        if discovered.is_empty() {
            println!("{}", msg::no_junk_found(lang));
            println!("Press Enter to continue...");
            current_cfg = tui.cfg;
            wait_for_key();
            continue;
        }

        run_tui(&discovered, excludes, lang, io_priority)?;

        current_cfg = target_config::load_config();
    }
}

fn wait_for_key() {
    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
}
