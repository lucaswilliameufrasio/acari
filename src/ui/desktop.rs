use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::application::cleaner::{self, CleanMode};
use crate::application::commands::prepare_targets;
use crate::config::target_config;
use crate::domain::{AppEvent, CleanTarget};
use crate::infrastructure::disk_scan::{self, DiskNode, ScanProgress};

#[derive(Default, PartialEq)]
enum Page {
    #[default]
    Disk,
    Cleanup,
}

enum Message {
    Progress(ScanProgress),
    Finished(Result<DiskNode, String>),
}

pub fn run_desktop() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Acarí — Disk Analyzer",
        options,
        Box::new(|_| Ok(Box::<DesktopApp>::default())),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

struct DesktopApp {
    path: String,
    tree: Option<DiskNode>,
    current: Vec<usize>,
    selected: Option<PathBuf>,
    rx: Option<Receiver<Message>>,
    progress: ScanProgress,
    error: Option<String>,
    cancel: Option<Arc<AtomicBool>>,
    mounts: Vec<PathBuf>,
    page: Page,
    targets: Vec<(CleanTarget, u64, u64, bool)>,
    incomplete_scan_targets: HashSet<String>,
    target_scan_progress: HashMap<(String, String), (u64, u64)>,
    target_rx: Option<tokio::sync::mpsc::UnboundedReceiver<AppEvent>>,
    target_scan_cancel: Option<Arc<AtomicBool>>,
    clean_rx: Option<tokio::sync::mpsc::UnboundedReceiver<AppEvent>>,
    clean_cancel: Option<cleaner::CancellationToken>,
    target_scan_done: bool,
    target_scan_busy: bool,
    dry_run: bool,
    confirm_clean: bool,
    confirmation_snapshot: Option<(Vec<(CleanTarget, PathBuf)>, bool)>,
    cleanup_status: String,
    cleanup_errors: Vec<String>,
    privileged_clean: bool,
    search: String,
    sort_largest_first: bool,
    cleanup_search: String,
    cleanup_sort_by_size: bool,
    allocated_size: bool,
}

impl Default for DesktopApp {
    fn default() -> Self {
        Self {
            path: dirs::home_dir().unwrap_or_default().display().to_string(),
            tree: None,
            current: Vec::new(),
            selected: None,
            rx: None,
            progress: ScanProgress {
                entries: 0,
                bytes: 0,
            },
            error: None,
            cancel: None,
            mounts: disk_scan::mounted_roots(),
            page: Page::Disk,
            targets: Vec::new(),
            incomplete_scan_targets: HashSet::new(),
            target_scan_progress: HashMap::new(),
            target_rx: None,
            target_scan_cancel: None,
            clean_rx: None,
            clean_cancel: None,
            target_scan_done: false,
            target_scan_busy: false,
            dry_run: false,
            confirm_clean: false,
            confirmation_snapshot: None,
            cleanup_status: String::new(),
            cleanup_errors: Vec::new(),
            privileged_clean: false,
            search: String::new(),
            sort_largest_first: true,
            cleanup_search: String::new(),
            cleanup_sort_by_size: true,
            allocated_size: false,
        }
    }
}

impl DesktopApp {
    fn start_scan(&mut self) {
        let root = PathBuf::from(self.path.trim());
        let io_priority = target_config::load_config().scan.io_priority;
        let (tx, rx) = mpsc::channel();
        let (progress_tx, progress_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::spawn(move || {
            let result = disk_scan::scan_tree_cancellable_with_priority(
                &root,
                Some(progress_tx),
                Some(worker_cancel),
                io_priority,
            )
            .map_err(|e| e.to_string());
            let _ = tx.send(Message::Finished(result));
        });
        // Bridge progress and completion without blocking egui's frame loop.
        let (ui_tx, ui_rx) = mpsc::channel();
        let progress_ui_tx = ui_tx.clone();
        thread::spawn(move || {
            loop {
                match progress_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(progress) => {
                        let _ = progress_ui_tx.send(Message::Progress(progress));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        let finish_tx = ui_tx.clone();
        thread::spawn(move || {
            if let Ok(message) = rx.recv() {
                let _ = finish_tx.send(message);
            }
        });
        self.rx = Some(ui_rx);
        self.cancel = Some(cancel);
        self.tree = None;
        self.current.clear();
        self.error = None;
    }

    fn current_node(&self) -> Option<&DiskNode> {
        let mut node = self.tree.as_ref()?;
        for &index in &self.current {
            node = node.children.get(index)?;
        }
        Some(node)
    }

    fn start_target_scan(&mut self) {
        let config = target_config::load_config();
        let targets = prepare_targets(&[], &[], &config.custom_targets);
        if targets.is_empty() {
            self.cleanup_status = "Nenhum alvo configurado.".into();
            return;
        }
        // Cleanup execution removes everything under each selected target;
        // don't apply scan-only excludes to its preview or the confirmation
        // would understate the affected scope.
        let cancel = Arc::new(AtomicBool::new(false));
        let (_, rx, _) = crate::application::commands::start_scan_cancellable(
            targets.clone(),
            Vec::new(),
            config.scan.io_priority,
            false,
            Arc::clone(&cancel),
        );
        self.targets = targets
            .into_iter()
            .map(|target| (target, 0, 0, false))
            .collect();
        self.incomplete_scan_targets.clear();
        self.target_scan_progress = self
            .targets
            .iter()
            .map(|(target, _, _, _)| {
                (
                    (
                        target.name.to_string(),
                        target.resolved_path().to_string_lossy().into_owned(),
                    ),
                    (0, 0),
                )
            })
            .collect();
        self.target_rx = Some(rx);
        self.target_scan_cancel = Some(cancel);
        self.target_scan_done = false;
        self.target_scan_busy = true;
        self.confirm_clean = false;
        self.confirmation_snapshot = None;
        self.cleanup_status = "Verificando alvos…".into();
    }

    fn poll_target_events(&mut self, ctx: &egui::Context) {
        if self.target_scan_busy || self.clean_rx.is_some() {
            // Tokio workers send through an unbounded channel; keep the UI
            // repainting while a worker is active so progress and disconnect
            // recovery are observed even if no further event arrives.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(rx) = &mut self.target_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::ScanProgress {
                        target_name,
                        target_path,
                        bytes_found,
                        files_scanned,
                    } => {
                        self.target_scan_progress.insert(
                            (target_name.clone(), target_path.clone()),
                            (bytes_found, files_scanned),
                        );
                        for row in self.targets.iter_mut().filter(|row| {
                            target_matches_scan_result(&row.0, &target_name, &target_path)
                        }) {
                            row.1 = bytes_found;
                            row.2 = files_scanned;
                        }
                    }
                    AppEvent::TargetCompleted {
                        target_name,
                        target_path,
                        total_bytes,
                        files_scanned,
                        scan_errors,
                    } => {
                        self.target_scan_progress
                            .remove(&(target_name.clone(), target_path.clone()));
                        if scan_errors > 0 {
                            self.incomplete_scan_targets.insert(target_path.clone());
                        }
                        for row in self.targets.iter_mut().filter(|row| {
                            target_matches_scan_result(&row.0, &target_name, &target_path)
                        }) {
                            row.1 = total_bytes;
                            row.2 = files_scanned;
                        }
                    }
                    AppEvent::ScanFinished => {
                        self.target_scan_busy = false;
                        let cancelled = self.target_scan_cancel.as_ref().is_some_and(|cancel| {
                            cancel.load(std::sync::atomic::Ordering::Relaxed)
                        });
                        self.target_scan_done = !cancelled;
                        self.cleanup_status = if cancelled {
                            "Verificação cancelada; execute um novo scan para habilitar a limpeza."
                                .into()
                        } else if self.incomplete_scan_targets.is_empty() {
                            "Verificação concluída.".into()
                        } else {
                            format!(
                                "Verificação incompleta para {} alvo(s); eles não poderão ser limpos até uma nova verificação bem-sucedida.",
                                self.incomplete_scan_targets.len()
                            )
                        };
                        self.target_rx = None;
                        self.target_scan_cancel = None;
                        self.target_scan_progress.clear();
                        break;
                    }
                    _ => {}
                }
            }
        }
        if self.target_rx.as_ref().is_some_and(|rx| rx.is_closed()) && self.target_scan_busy {
            self.target_rx = None;
            self.target_scan_cancel = None;
            self.target_scan_progress.clear();
            self.target_scan_busy = false;
            self.target_scan_done = false;
            self.cleanup_status =
                "A verificação foi interrompida antes de concluir; execute-a novamente.".into();
        }
        if let Some(rx) = &mut self.clean_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::CleaningFinished {
                        reclaimed_bytes,
                        errors,
                        cancelled,
                        ..
                    } => {
                        let error_suffix = if self.cleanup_errors.is_empty() {
                            String::new()
                        } else {
                            format!(" Detalhes: {}", self.cleanup_errors.join("; "))
                        };
                        self.cleanup_status = if self.privileged_clean && errors == 0 {
                            "Operação privilegiada concluída; espaço recuperado não medido.".into()
                        } else if self.privileged_clean {
                            format!(
                                "Operação privilegiada falhou; espaço recuperado não medido.{error_suffix}"
                            )
                        } else {
                            format!(
                                "{}{} liberados; {} erros{}.{}",
                                if self.dry_run {
                                    "Simulação: "
                                } else {
                                    "Limpeza: "
                                },
                                crate::domain::format_bytes(reclaimed_bytes),
                                errors,
                                if cancelled { " (cancelada)" } else { "" },
                                error_suffix
                            )
                        };
                        if !self.dry_run {
                            self.invalidate_target_scan();
                            self.cleanup_status
                                .push_str(" Verifique os alvos novamente antes de outra limpeza.");
                        }
                        self.privileged_clean = false;
                        self.clean_rx = None;
                        self.clean_cancel = None;
                        break;
                    }
                    AppEvent::TargetCleaned {
                        target_name,
                        error_detail: Some(detail),
                        ..
                    } => {
                        self.cleanup_errors.push(format!("{target_name}: {detail}"));
                    }
                    _ => {}
                }
            }
        }
        if self
            .clean_rx
            .as_ref()
            .is_some_and(|rx| rx.is_closed() && rx.is_empty())
        {
            self.clean_rx = None;
            self.clean_cancel = None;
            self.privileged_clean = false;
            self.invalidate_target_scan();
            self.cleanup_status =
                "A limpeza foi interrompida antes de concluir; verifique o estado dos alvos e execute uma nova verificação."
                    .into();
        }
        if self.target_scan_busy || self.clean_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn invalidate_target_scan(&mut self) {
        self.target_scan_done = false;
        for (_, _, _, selected) in &mut self.targets {
            *selected = false;
        }
    }

    fn selected_target_snapshot(&self) -> (Vec<(CleanTarget, PathBuf)>, bool) {
        let mut targets = self
            .targets
            .iter()
            .filter(|(_, _, _, selected)| *selected)
            .map(|(target, _, _, _)| (target.clone(), target.resolved_path()))
            .collect::<Vec<_>>();
        targets.sort_by(|(left_target, left_path), (right_target, right_path)| {
            left_target
                .name
                .cmp(&right_target.name)
                .then_with(|| left_target.path.cmp(&right_target.path))
                .then_with(|| left_path.cmp(right_path))
        });
        (targets, self.dry_run)
    }

    fn begin_clean(&mut self) {
        if self.confirmation_snapshot.as_ref() != Some(&self.selected_target_snapshot()) {
            self.confirm_clean = false;
            self.confirmation_snapshot = None;
            self.cleanup_status =
                "A seleção ou o modo de execução mudou; revise e confirme novamente.".into();
            return;
        }
        if !self.target_scan_done || self.target_scan_busy {
            self.confirm_clean = false;
            self.confirmation_snapshot = None;
            self.cleanup_status =
                "Verifique os alvos novamente antes de confirmar a limpeza.".into();
            return;
        }
        let selected_rows: Vec<_> = self
            .targets
            .iter()
            .filter(|(_, _, _, selected)| *selected)
            .collect();
        if selected_rows.iter().any(|(target, _, _, _)| {
            !desktop_cleanup_supported(target)
                || scan_target_is_incomplete(target, &self.incomplete_scan_targets)
        }) {
            self.cleanup_status = "A seleção contém alvos incompatíveis ou com verificação incompleta; revise-a antes de continuar.".into();
            self.confirm_clean = false;
            self.confirmation_snapshot = None;
            return;
        }
        let selected: Vec<_> = selected_rows
            .into_iter()
            .map(|(target, bytes, files, _)| (target.clone(), *bytes, *files))
            .collect();
        if selected.is_empty() {
            self.cleanup_status = "Selecione um alvo compatível com a limpeza desktop.".into();
            self.confirm_clean = false;
            self.confirmation_snapshot = None;
            return;
        }
        if selected.iter().enumerate().any(|(index, (target, _, _))| {
            selected[index + 1..]
                .iter()
                .any(|(other, _, _)| cleanup_targets_overlap(target, other))
        }) {
            self.cleanup_status = "Há caminhos sobrepostos na seleção; execute esses alvos individualmente para evitar prévias e resultados duplicados.".into();
            self.confirm_clean = false;
            self.confirmation_snapshot = None;
            return;
        }
        let special_count = selected
            .iter()
            .filter(|(target, _, _)| requires_individual_confirmation(target))
            .count();
        if special_count > 0 && (special_count != 1 || selected.len() != 1) {
            self.cleanup_status =
                "Alvos perigosos/comando devem ser executados individualmente.".into();
            self.confirm_clean = false;
            self.confirmation_snapshot = None;
            return;
        }
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let single_target = selected.len() == 1;
        self.privileged_clean = !self.dry_run && single_target && selected[0].0.requires_sudo;
        self.clean_cancel = None;
        self.cleanup_errors.clear();
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if !self.dry_run && single_target && selected[0].0.requires_sudo {
            cleaner::start_background_privileged_clean(tx.clone(), selected[0].0.clone());
        } else {
            let cancel = cleaner::new_cancellation_token();
            cleaner::start_background_clean_with_cancel(
                tx.clone(),
                selected,
                if self.dry_run {
                    CleanMode::DryRun
                } else {
                    CleanMode::Execute
                },
                cancel.clone(),
            );
            self.clean_cancel = Some(cancel);
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let cancel = cleaner::new_cancellation_token();
            cleaner::start_background_clean_with_cancel(
                tx.clone(),
                selected,
                if self.dry_run {
                    CleanMode::DryRun
                } else {
                    CleanMode::Execute
                },
                cancel.clone(),
            );
            self.clean_cancel = Some(cancel);
        }
        self.clean_rx = Some(rx);
        self.confirm_clean = false;
        self.confirmation_snapshot = None;
        self.cleanup_status = if self.dry_run {
            "Executando simulação…"
        } else {
            "Limpando…"
        }
        .into();
    }

    fn poll_scan(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        loop {
            match rx.try_recv() {
                Ok(Message::Progress(progress)) => self.progress = progress,
                Ok(Message::Finished(Ok(tree))) => {
                    self.tree = Some(tree);
                    self.rx = None;
                    self.cancel = None;
                    break;
                }
                Ok(Message::Finished(Err(error))) => {
                    if error != "scan cancelled" {
                        self.error = Some(error);
                    }
                    self.rx = None;
                    self.cancel = None;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.cancel = None;
                    break;
                }
            }
        }
        if self.rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan(ctx);
        self.poll_target_events(ctx);
        if !ctx.wants_keyboard_input() && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.current.pop();
            self.selected = None;
        }
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Acarí");
                if ui
                    .selectable_label(self.page == Page::Disk, "Análise de disco")
                    .clicked()
                {
                    self.page = Page::Disk;
                }
                if ui
                    .selectable_label(self.page == Page::Cleanup, "Limpeza")
                    .clicked()
                {
                    self.page = Page::Cleanup;
                }
                if self.page == Page::Cleanup {
                    return;
                }
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .desired_width(420.0)
                        .hint_text("Volume mount ou caminho da pasta"),
                );
                if ui.button("Escolher pasta…").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.path = path.display().to_string();
                }
                egui::ComboBox::from_id_salt("mounts")
                    .selected_text("Volumes montados")
                    .show_ui(ui, |ui| {
                        for mount in &self.mounts {
                            if ui
                                .selectable_label(false, mount.display().to_string())
                                .clicked()
                            {
                                self.path = mount.display().to_string();
                            }
                        }
                    });
                if ui
                    .add_enabled(self.rx.is_none(), egui::Button::new("Analisar"))
                    .clicked()
                {
                    self.start_scan();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.page == Page::Cleanup {
                ui.heading("Limpeza de alvos conhecidos");
                ui.label("A análise de disco é somente leitura. Esta tela usa os alvos de limpeza configurados no Acarí.");
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.target_scan_busy && self.clean_rx.is_none(), egui::Button::new("Verificar alvos")).clicked() { self.start_target_scan(); }
                    if ui.add_enabled(self.target_scan_busy, egui::Button::new("Cancelar verificação")).clicked()
                        && let Some(cancel) = &self.target_scan_cancel
                    {
                        cancel.store(true, Ordering::Relaxed);
                        self.cleanup_status = "Cancelando verificação; uma consulta de sistema em andamento pode concluir antes da interrupção…".into();
                    }
                    ui.checkbox(&mut self.dry_run, "Simular (dry-run)");
                    if ui.add_enabled(self.target_scan_done && self.clean_rx.is_none(), egui::Button::new("Limpar selecionados…")).clicked() {
                        self.confirmation_snapshot = Some(self.selected_target_snapshot());
                        self.confirm_clean = true;
                    }
                    if self.clean_rx.is_some()
                        && !self.privileged_clean
                        && ui.button("Cancelar limpeza").clicked()
                        && let Some(cancel) = &self.clean_cancel
                    {
                        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                        self.cleanup_status = "Cancelando limpeza… itens já removidos não podem ser restaurados.".into();
                    }
                });
                if !self.cleanup_status.is_empty() { ui.label(&self.cleanup_status); }
                if !self.targets.is_empty() && !self.target_scan_done && !self.target_scan_busy {
                    ui.colored_label(
                        Color32::YELLOW,
                        "Preview expirado ou indisponível — verifique os alvos para habilitar a seleção.",
                    );
                }
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.cleanup_search).desired_width(260.0).hint_text("Filtrar alvos por nome ou caminho…"));
                    if ui.button(if self.cleanup_sort_by_size { "Tamanho ↓" } else { "Nome A–Z" }).clicked() { self.cleanup_sort_by_size = !self.cleanup_sort_by_size; }
                    if ui.add_enabled(self.target_scan_done && !self.target_scan_busy, egui::Button::new("Selecionar visíveis")).clicked() {
                        for target in self.targets.iter_mut().filter(|row| cleanup_matches(row, &self.cleanup_search)) {
                            if desktop_cleanup_supported(&target.0)
                                && !requires_individual_confirmation(&target.0)
                                && !scan_target_is_incomplete(&target.0, &self.incomplete_scan_targets)
                            { target.3 = true; }
                        }
                    }
                    if ui.button("Limpar seleção").clicked() {
                        for target in self.targets.iter_mut().filter(|row| cleanup_matches(row, &self.cleanup_search)) { target.3 = false; }
                    }
                });
                let mut visible: Vec<usize> = (0..self.targets.len())
                    .filter(|&index| cleanup_matches(&self.targets[index], &self.cleanup_search))
                    .collect();
                if self.cleanup_sort_by_size {
                    visible.sort_by_key(|&index| std::cmp::Reverse(self.targets[index].1));
                } else {
                    visible.sort_by_key(|&index| self.targets[index].0.name.to_lowercase());
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for index in visible {
                        let (target, bytes, files, selected) = &mut self.targets[index];
                        let scan_incomplete = scan_target_is_incomplete(target, &self.incomplete_scan_targets);
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(
                                cleanup_target_selectable(
                                    target,
                                    self.target_scan_done,
                                    self.target_scan_busy,
                                    scan_incomplete,
                                ),
                                |ui| {
                                ui.checkbox(selected, "");
                            });
                            ui.strong(target.name.as_ref());
                            ui.label(cleanup_metrics_label(
                                target,
                                *bytes,
                                *files,
                                scan_incomplete,
                            ));
                            if !desktop_cleanup_supported(target) { ui.colored_label(Color32::GRAY, "não compatível com execução segura na UI"); }
                            else if target.is_command() { ui.colored_label(Color32::YELLOW, "comando permitido · confirmação individual obrigatória"); }
                            else if target.is_dangerous() { ui.colored_label(Color32::LIGHT_RED, "operação perigosa · confirmação individual obrigatória"); }
                            else if target.is_custom() { ui.colored_label(Color32::YELLOW, "alvo personalizado · confirmação individual obrigatória"); }
                            if scan_incomplete { ui.colored_label(Color32::LIGHT_RED, "scan incompleto · limpeza bloqueada"); }
                        });
                        ui.label(format!("{} — {}", target.path, target.description));
                        ui.label(format!("Escopo: {}", cleanup_scope_label(target)));
                        if self.target_scan_busy {
                            let progress_key = (
                                target.name.to_string(),
                                target.resolved_path().to_string_lossy().into_owned(),
                            );
                            if let Some((progress_bytes, progress_files)) =
                                self.target_scan_progress.get(&progress_key)
                            {
                                if *progress_bytes == 0 && *progress_files == 0 {
                                    ui.small("Verificando alvo…");
                                } else {
                                    ui.small(format!(
                                        "Lidos: {progress_files} arquivos · {}",
                                        crate::domain::format_bytes(*progress_bytes)
                                    ));
                                }
                            }
                        }
                        ui.separator();
                    }
                });
                if self.confirm_clean {
                    let mut confirm = false;
                    egui::Window::new("Confirmar limpeza").collapsible(false).resizable(false)
                        .show(ctx, |ui| {
                            let count = self.targets.iter().filter(|(_, _, _, selected)| *selected).count();
                            ui.label(format!("{} {} alvo(s) selecionado(s).", if self.dry_run { "Simular" } else { "Limpar" }, count));
                            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                                for (target, bytes, files, _) in self.targets.iter().filter(|(_, _, _, selected)| *selected) {
                                    ui.strong(target.name.as_ref());
                                    ui.label(format!(
                                        "{} · {} · {}",
                                        target.resolved_path().display(),
                                        cleanup_scope_label(target),
                                        cleanup_metrics_label(
                                            target,
                                            *bytes,
                                            *files,
                                            scan_target_is_incomplete(
                                                target,
                                                &self.incomplete_scan_targets,
                                            ),
                                        )
                                    ));
                                }
                            });
                            let special = self.targets.iter().filter(|(target, _, _, selected)| *selected && requires_individual_confirmation(target)).collect::<Vec<_>>();
                            if let Some((target, _, _, _)) = special.first() {
                                ui.colored_label(Color32::LIGHT_RED, format!("Operação especial: {}", target.name));
                                ui.label(target.description.as_ref());
                                ui.label(format!("Escopo: {}", target.resolved_path().display()));
                                if let Some((_, bytes, files, _)) = self.targets.iter().find(|(candidate, _, _, selected)| *selected && candidate.name == target.name) {
                                    ui.label(format!(
                                        "Prévia: {}",
                                        cleanup_metrics_label(
                                            target,
                                            *bytes,
                                            *files,
                                            scan_target_is_incomplete(
                                                target,
                                                &self.incomplete_scan_targets,
                                            ),
                                        )
                                    ));
                                }
                                ui.label(if target.requires_sudo { "Este alvo requer privilégio; a autorização será solicitada pelo sistema." } else if target.is_custom() { "Este caminho personalizado vem da sua configuração e pode conter dados únicos." } else { "Esta operação pode remover dados não regeneráveis." });
                            }
                            ui.label("A limpeza é irreversível. Revise o alvo e a estimativa; a análise visual de disco não será afetada.");
                            ui.horizontal(|ui| {
                                if ui.button(if self.dry_run { "Executar simulação" } else { "Confirmar limpeza" }).clicked() { confirm = true; }
                                if ui.button("Cancelar").clicked() {
                                    self.confirm_clean = false;
                                    self.confirmation_snapshot = None;
                                }
                            });
                        });
                    if confirm { self.begin_clean(); }
                }
                return;
            }
            if let Some(error) = &self.error {
                ui.colored_label(Color32::LIGHT_RED, error);
            }
            if self.rx.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!(
                        "Analisando… {} itens, {} bytes",
                        self.progress.entries, self.progress.bytes
                    ));
                });
                if ui.button("Cancelar análise").clicked()
                    && let Some(cancel) = &self.cancel
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
            if self.tree.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(90.0);
                    ui.heading("Entenda o que ocupa espaço");
                    ui.label(
                        "Escolha um volume montado ou uma pasta para explorar o uso de disco.",
                    );
                    ui.label("A análise é somente leitura; nenhum arquivo será removido.");
                });
                return;
            }
            let root = self.tree.as_ref().unwrap();
            egui::SidePanel::right("selection-details")
                .default_width(275.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    ui.heading("Detalhes");
                    if let Some(path) = &self.selected {
                        if let Some(node) = self.tree.as_ref().and_then(|tree| find_node(tree, path)) {
                            ui.label(if node.is_dir { "Diretório" } else { "Arquivo" });
                            ui.strong(&node.name);
                            ui.label(path.display().to_string());
                            ui.separator();
                            let node_size = node_bytes(node, self.allocated_size);
                            let root_size = node_bytes(root, self.allocated_size);
                            ui.label(format!("{}: {}", if self.allocated_size { "Alocado" } else { "Aparente" }, crate::domain::format_bytes(node_size)));
                            let share = if root_size > 0 { node_size as f64 * 100.0 / root_size as f64 } else { 0.0 };
                            ui.label(format!("{share:.2}% da análise"));
                            if node.is_dir { ui.label(format!("{} itens diretos", node.children.len())); }
                        }
                    } else {
                        ui.label("Selecione um bloco para ver os detalhes.");
                    }
                    ui.separator();
                    ui.label("Esc volta um nível no treemap.");
                    ui.label("A análise não remove arquivos.");
                });
            ui.horizontal(|ui| {
                if ui.button("Início").clicked() {
                    self.current.clear();
                }
                let mut node = root;
                for depth in 0..self.current.len() {
                    let index = self.current[depth];
                    if let Some(child) = node.children.get(index) {
                        ui.label("›");
                        if ui.button(&child.name).clicked() {
                            self.current.truncate(depth + 1);
                        }
                        node = child;
                    }
                }
                ui.separator();
                ui.label(format!(
                    "{} itens · {}",
                    self.progress.entries, crate::domain::format_bytes(node_bytes(root, self.allocated_size))
                ));
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(260.0).hint_text("Filtrar itens deste diretório…"));
                if ui.button(if self.sort_largest_first { "Maior primeiro" } else { "Nome A–Z" }).clicked() { self.sort_largest_first = !self.sort_largest_first; }
                ui.checkbox(&mut self.allocated_size, "Tamanho alocado");
                if ui.button("Voltar um nível").clicked() { self.current.pop(); }
            });
            let selected = self.selected.clone();
            let current = self.current_node().unwrap_or(root).clone();
            let mut navigate = None;
            let rect = ui.available_rect_before_wrap();
            draw_treemap(
                ui,
                &current,
                rect,
                &selected,
                &mut navigate,
                &mut self.selected,
                &self.search,
                self.sort_largest_first,
                self.allocated_size,
            );
            if let Some(path) = navigate {
                let mut indices = self.current.clone();
                if find_child_indices(root, &path, &mut indices) {
                    self.current = indices;
                }
            }
        });
    }
}

fn find_child_indices(node: &DiskNode, path: &std::path::Path, indices: &mut Vec<usize>) -> bool {
    if node.path == path {
        return true;
    }
    for (index, child) in node.children.iter().enumerate() {
        indices.push(index);
        if find_child_indices(child, path, indices) {
            return true;
        }
        indices.pop();
    }
    false
}

fn cleanup_matches(row: &(CleanTarget, u64, u64, bool), query: &str) -> bool {
    let query = query.trim();
    query.is_empty()
        || row.0.name.to_lowercase().contains(&query.to_lowercase())
        || row.0.path.to_lowercase().contains(&query.to_lowercase())
}

fn cleanup_metrics_label(
    target: &CleanTarget,
    bytes: u64,
    entries: u64,
    scan_incomplete: bool,
) -> String {
    if target.is_command() {
        if scan_incomplete {
            return "estimativa indisponível · consulta incompleta".into();
        }
        if target.name == "Time Machine Local Snapshots" && (bytes > 0 || entries > 0) {
            return format!(
                "estimativa indicativa (máx. solicitado) {} · {entries} snapshots",
                crate::domain::format_bytes(bytes)
            );
        }
        if target.name == "Apt Autoremove" && (bytes > 0 || entries > 0) {
            return format!(
                "estimativa aproximada {} · {entries} pacotes",
                crate::domain::format_bytes(bytes)
            );
        }
        if target.name == "Journalctl Vacuum" && (bytes > 0 || entries > 0) {
            return format!(
                "estimativa máxima indicativa {} · arquivos arquivados apenas",
                crate::domain::format_bytes(bytes)
            );
        }
        if target.name == "iOS Simulators Reset" && (bytes > 0 || entries > 0) {
            return format!(
                "dados encontrados {} · liberação real pode variar · {entries} arquivos",
                crate::domain::format_bytes(bytes)
            );
        }
        if bytes == 0 && entries == 0 {
            "nenhum item ou espaço recuperável (estimativa válida: 0 bytes)".into()
        } else {
            format!(
                "estimativa {} · {entries} itens",
                crate::domain::format_bytes(bytes)
            )
        }
    } else if std::fs::symlink_metadata(target.resolved_path())
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        "1 link simbólico · destino preservado · bytes do destino não contabilizados".into()
    } else if scan_incomplete {
        format!(
            "estimativa parcial · {} · {entries} arquivos",
            crate::domain::format_bytes(bytes)
        )
    } else {
        format!(
            "{} · {entries} arquivos",
            crate::domain::format_bytes(bytes)
        )
    }
}

fn target_matches_scan_result(target: &CleanTarget, name: &str, path: &str) -> bool {
    target.name == name && target.resolved_path().to_string_lossy() == path
}

fn scan_target_is_incomplete(target: &CleanTarget, incomplete: &HashSet<String>) -> bool {
    incomplete.contains(&target.resolved_path().to_string_lossy().into_owned())
}

fn cleanup_target_selectable(
    target: &CleanTarget,
    scan_done: bool,
    scan_busy: bool,
    scan_incomplete: bool,
) -> bool {
    scan_done && !scan_busy && desktop_cleanup_supported(target) && !scan_incomplete
}

fn cleanup_scope_label(target: &CleanTarget) -> &'static str {
    if target.is_command() {
        return "operação do sistema/comando; sem remoção direta de caminhos exibidos";
    }
    match std::fs::symlink_metadata(target.resolved_path()) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            "o link simbólico será removido; o destino será preservado"
        }
        Ok(metadata) if metadata.is_dir() && target.delete_entire => {
            "todo o conteúdo e a pasta raiz serão removidos"
        }
        Ok(metadata) if metadata.is_dir() => {
            "o conteúdo será removido; a pasta raiz será preservada"
        }
        Ok(_) => "o arquivo-alvo será removido",
        Err(_) => "o caminho não existe; nada será removido",
    }
}

fn cleanup_targets_overlap(left: &CleanTarget, right: &CleanTarget) -> bool {
    if left.is_command() || right.is_command() {
        return false;
    }
    let left_path = canonicalize_for_overlap(&left.resolved_path());
    let right_path = canonicalize_for_overlap(&right.resolved_path());
    left_path == right_path
        || left_path.starts_with(&right_path)
        || right_path.starts_with(&left_path)
}

fn canonicalize_for_overlap(path: &std::path::Path) -> PathBuf {
    let mut unresolved = path;
    let mut suffix = Vec::new();
    loop {
        if let Ok(mut canonical) = std::fs::canonicalize(unresolved) {
            for component in suffix.iter().rev() {
                canonical.push(component);
            }
            return normalize_overlap_path(&canonical);
        }
        let Some(file_name) = unresolved.file_name() else {
            return normalize_overlap_path(path);
        };
        suffix.push(file_name.to_os_string());
        let Some(parent) = unresolved.parent() else {
            return normalize_overlap_path(path);
        };
        unresolved = parent;
    }
}

fn normalize_overlap_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !absolute.is_absolute() {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn requires_individual_confirmation(target: &CleanTarget) -> bool {
    target.is_dangerous() || target.is_command() || target.is_custom()
}

fn find_node<'a>(node: &'a DiskNode, path: &std::path::Path) -> Option<&'a DiskNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_node(child, path))
}

fn node_bytes(node: &DiskNode, allocated: bool) -> u64 {
    if allocated {
        node.allocated_bytes
    } else {
        node.bytes
    }
}

/// Keep desktop command execution narrower than the CLI's configured targets.
/// Only exact, non-privileged built-in Docker operations are supported. The
/// builder operation is dispatched natively by the cleaner, never via its
/// legacy shell-wrapper definition.
fn desktop_cleanup_supported(target: &CleanTarget) -> bool {
    if !target.is_command() {
        return !target.requires_sudo;
    }
    if target.origin != crate::domain::TargetOrigin::Builtin {
        return false;
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if target.requires_sudo {
        return crate::infrastructure::privileged::operation_for_target(target)
            .is_some_and(|_| crate::infrastructure::privileged::authorization_broker_available());
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    if target.requires_sudo {
        return false;
    }
    if target.name == "iOS Simulators Reset" {
        #[cfg(target_os = "macos")]
        return target.command
            == [
                "sh",
                "-c",
                "xcrun simctl shutdown all 2>/dev/null; xcrun simctl erase all",
            ];
        #[cfg(not(target_os = "macos"))]
        return false;
    }
    matches!(
        (target.name.as_ref(), target.command),
        (
            "Docker System Prune",
            ["docker", "system", "prune", "-a", "--force"]
        ) | (
            "Docker Volumes Prune",
            ["docker", "volume", "prune", "--all", "--force"]
        ) | (
            "Docker Builder Prune",
            [
                "sh",
                "-c",
                "docker buildx ls --format '{{.Name}}' | while IFS= read -r builder; do [ -z \"$builder\" ] || [ \"$builder\" = default ] || docker buildx prune -a -f --builder \"$builder\" || exit; done"
            ]
        )
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_treemap(
    ui: &mut egui::Ui,
    node: &DiskNode,
    rect: Rect,
    selected: &Option<PathBuf>,
    navigate: &mut Option<PathBuf>,
    selected_path: &mut Option<PathBuf>,
    search: &str,
    sort_largest_first: bool,
    allocated_size: bool,
) {
    if node.children.is_empty() {
        return;
    }
    let query = search.trim().to_lowercase();
    let mut children: Vec<_> = node
        .children
        .iter()
        .filter(|child| {
            query.is_empty()
                || child.name.to_lowercase().contains(&query)
                || child.path.to_string_lossy().to_lowercase().contains(&query)
        })
        .collect();
    if sort_largest_first {
        children.sort_by(|a, b| {
            node_bytes(b, allocated_size)
                .cmp(&node_bytes(a, allocated_size))
                .then_with(|| a.name.cmp(&b.name))
        });
    } else {
        children.sort_by_key(|a| a.name.to_lowercase());
    }
    let boxes = squarified_layout(
        rect,
        &children
            .iter()
            .map(|child| node_bytes(child, allocated_size))
            .collect::<Vec<_>>(),
    );
    for (index, child_rect) in boxes {
        let child = children[index];
        let response = ui.allocate_rect(child_rect, Sense::click());
        let color = if selected.as_ref() == Some(&child.path) {
            Color32::from_rgb(245, 155, 65)
        } else {
            color_for(&child.name)
        };
        ui.painter().rect_filled(child_rect.shrink(1.0), 3.0, color);
        ui.painter().rect_stroke(
            child_rect.shrink(1.0),
            3.0,
            Stroke::new(1.0_f32, Color32::from_gray(30)),
            egui::StrokeKind::Inside,
        );
        if child_rect.width() > 44.0 && child_rect.height() > 24.0 {
            ui.painter().text(
                child_rect.left_top() + Vec2::splat(7.0),
                egui::Align2::LEFT_TOP,
                &child.name,
                egui::FontId::proportional(13.0),
                Color32::WHITE,
            );
        }
        if response.clicked() {
            *selected_path = Some(child.path.clone());
            if child.is_dir {
                *navigate = Some(child.path.clone());
            }
        }
        if response.hovered() {
            response.on_hover_text(format!(
                "{}\n{}",
                child.path.display(),
                crate::domain::format_bytes(node_bytes(child, allocated_size))
            ));
        }
    }
}

/// Allocate rectangles proportional to byte size while keeping aspect ratios
/// reasonably square. Zero-byte entries intentionally receive no rectangle.
fn squarified_layout(rect: Rect, weights: &[u64]) -> Vec<(usize, Rect)> {
    let total: u64 = weights.iter().copied().sum();
    if total == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Vec::new();
    }
    let scale = rect.area() / total as f32;
    let mut remaining: Vec<(usize, f32)> = weights
        .iter()
        .enumerate()
        .filter(|(_, weight)| **weight > 0)
        .map(|(index, weight)| (index, *weight as f32 * scale))
        .collect();
    let mut output = Vec::with_capacity(remaining.len());
    let mut bounds = rect;
    let mut row = Vec::new();

    while !remaining.is_empty() {
        let short_side = bounds.width().min(bounds.height());
        if short_side <= f32::EPSILON {
            break;
        }
        let candidate = remaining[0];
        let mut proposed = row.clone();
        proposed.push(candidate);
        if row.is_empty() || worst_ratio(&proposed, short_side) <= worst_ratio(&row, short_side) {
            row.push(remaining.remove(0));
        } else {
            bounds = place_row(bounds, &row, &mut output);
            row.clear();
        }
    }
    if !row.is_empty() {
        place_row(bounds, &row, &mut output);
    }
    output
}

fn worst_ratio(row: &[(usize, f32)], short_side: f32) -> f32 {
    let sum: f32 = row.iter().map(|(_, area)| *area).sum();
    let min = row
        .iter()
        .map(|(_, area)| *area)
        .fold(f32::INFINITY, f32::min);
    let max = row.iter().map(|(_, area)| *area).fold(0.0_f32, f32::max);
    if min <= 0.0 || sum <= 0.0 {
        return f32::INFINITY;
    }
    let side_sq = short_side * short_side;
    (side_sq * max / (sum * sum)).max((sum * sum) / (side_sq * min))
}

fn place_row(mut bounds: Rect, row: &[(usize, f32)], output: &mut Vec<(usize, Rect)>) -> Rect {
    let total: f32 = row.iter().map(|(_, area)| *area).sum();
    if bounds.width() >= bounds.height() {
        let height = (total / bounds.width()).min(bounds.height());
        let mut x = bounds.min.x;
        for (index, area) in row {
            let width = if height > 0.0 {
                (*area / height).min(bounds.max.x - x)
            } else {
                0.0
            };
            output.push((
                *index,
                Rect::from_min_max(
                    Pos2::new(x, bounds.min.y),
                    Pos2::new(x + width, bounds.min.y + height),
                ),
            ));
            x += width;
        }
        bounds.min.y += height;
    } else {
        let width = (total / bounds.height()).min(bounds.width());
        let mut y = bounds.min.y;
        for (index, area) in row {
            let height = if width > 0.0 {
                (*area / width).min(bounds.max.y - y)
            } else {
                0.0
            };
            output.push((
                *index,
                Rect::from_min_max(
                    Pos2::new(bounds.min.x, y),
                    Pos2::new(bounds.min.x + width, y + height),
                ),
            ));
            y += height;
        }
        bounds.min.x += width;
    }
    bounds
}

fn color_for(name: &str) -> Color32 {
    let hash = name.bytes().fold(0_u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u32)
    });
    Color32::from_rgb(
        70 + (hash as u8 % 80),
        90 + ((hash >> 8) as u8 % 80),
        125 + ((hash >> 16) as u8 % 70),
    )
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::{
        cleanup_metrics_label, cleanup_scope_label, cleanup_target_selectable,
        cleanup_targets_overlap, desktop_cleanup_supported, node_bytes,
        requires_individual_confirmation, scan_target_is_incomplete, squarified_layout,
        target_matches_scan_result,
    };
    #[cfg(windows)]
    use super::{
        cleanup_metrics_label, cleanup_target_selectable, cleanup_targets_overlap,
        desktop_cleanup_supported, node_bytes, requires_individual_confirmation,
        scan_target_is_incomplete, squarified_layout, target_matches_scan_result,
    };
    use crate::domain::{CleanTarget, TargetOrigin};
    use crate::infrastructure::disk_scan::DiskNode;
    use eframe::egui::{Pos2, Rect};
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn node_size_switches_between_apparent_and_allocated() {
        let node = DiskNode {
            name: "x".into(),
            path: PathBuf::from("x"),
            bytes: 3,
            allocated_bytes: 512,
            is_dir: false,
            children: Vec::new(),
        };
        assert_eq!(node_bytes(&node, false), 3);
        assert_eq!(node_bytes(&node, true), 512);
    }

    #[test]
    fn cleanup_metric_labels_distinguish_command_estimates_from_file_counts() {
        let command = CleanTarget {
            name: "Command target".into(),
            command: &["example", "clean"],
            ..CleanTarget::default()
        };
        assert!(cleanup_metrics_label(&command, 1024, 2, false).contains("estimativa"));
        assert!(cleanup_metrics_label(&command, 1024, 2, false).contains("2 itens"));
        assert!(
            cleanup_metrics_label(&command, 0, 0, false).contains("estimativa válida: 0 bytes")
        );
        assert!(cleanup_metrics_label(&command, 0, 0, true).contains("indisponível"));

        let snapshots = CleanTarget {
            name: "Time Machine Local Snapshots".into(),
            command: &["tmutil", "thinlocalsnapshots"],
            ..CleanTarget::default()
        };
        let snapshot_estimate = cleanup_metrics_label(&snapshots, 10_000_000_000, 2, false);
        assert!(snapshot_estimate.contains("indicativa"));
        assert!(snapshot_estimate.contains("máx. solicitado"));
        assert!(snapshot_estimate.contains("2 snapshots"));

        let apt = CleanTarget {
            name: "Apt Autoremove".into(),
            command: &["sudo", "apt", "autoremove", "-y"],
            ..CleanTarget::default()
        };
        let apt_estimate = cleanup_metrics_label(&apt, 20_000_000, 2, false);
        assert!(apt_estimate.contains("estimativa aproximada"));
        assert!(apt_estimate.contains("2 pacotes"));

        let journal = CleanTarget {
            name: "Journalctl Vacuum".into(),
            command: &["sudo", "journalctl", "--vacuum-size=100M"],
            ..CleanTarget::default()
        };
        let journal_estimate = cleanup_metrics_label(&journal, 500_000_000, 1, false);
        assert!(journal_estimate.contains("máxima indicativa"));
        assert!(journal_estimate.contains("arquivados apenas"));

        let simulators = CleanTarget {
            name: "iOS Simulators Reset".into(),
            command: &["xcrun", "simctl", "erase", "all"],
            ..CleanTarget::default()
        };
        let simulator_estimate = cleanup_metrics_label(&simulators, 200_000_000, 20, false);
        assert!(simulator_estimate.contains("dados encontrados"));
        assert!(simulator_estimate.contains("liberação real pode variar"));
        assert!(simulator_estimate.contains("20 arquivos"));

        let files = CleanTarget {
            path: "/tmp/cache".into(),
            ..CleanTarget::default()
        };
        assert!(cleanup_metrics_label(&files, 1024, 2, false).contains("2 arquivos"));
        assert!(cleanup_metrics_label(&files, 1024, 2, true).contains("estimativa parcial"));
    }

    #[test]
    fn custom_and_command_targets_require_single_target_confirmation() {
        let ordinary = CleanTarget::file("Cache", "/tmp/cache", "cache", false);
        assert!(!requires_individual_confirmation(&ordinary));

        let custom = CleanTarget {
            origin: TargetOrigin::Custom,
            ..CleanTarget::file("My target", "/home/user/data", "custom", false)
        };
        assert!(requires_individual_confirmation(&custom));

        let command = CleanTarget {
            command: &["tool", "clean"],
            ..CleanTarget::default()
        };
        assert!(requires_individual_confirmation(&command));
    }

    #[test]
    fn scan_results_match_target_name_and_resolved_path() {
        let first = CleanTarget {
            name: "same name".into(),
            path: "/tmp/first".into(),
            ..CleanTarget::default()
        };
        let second = CleanTarget {
            name: "same name".into(),
            path: "/tmp/second".into(),
            ..CleanTarget::default()
        };

        assert!(target_matches_scan_result(
            &first,
            "same name",
            "/tmp/first"
        ));
        assert!(!target_matches_scan_result(
            &first,
            "same name",
            "/tmp/second"
        ));
        assert!(!target_matches_scan_result(
            &second,
            "other name",
            "/tmp/second"
        ));
    }

    #[test]
    fn incomplete_scan_targets_are_identified_by_resolved_path() {
        let target = CleanTarget {
            name: "cache".into(),
            path: "/tmp/acari-incomplete-cache".into(),
            ..CleanTarget::default()
        };
        let incomplete = HashSet::from(["/tmp/acari-incomplete-cache".to_string()]);
        assert!(scan_target_is_incomplete(&target, &incomplete));
        assert!(!scan_target_is_incomplete(&target, &HashSet::new()));
    }

    #[test]
    fn expired_or_incomplete_previews_cannot_be_selected() {
        let target = CleanTarget {
            path: "/tmp/acari-cache".into(),
            ..CleanTarget::default()
        };
        assert!(!cleanup_target_selectable(&target, false, false, false));
        assert!(!cleanup_target_selectable(&target, true, true, false));
        assert!(!cleanup_target_selectable(&target, true, false, true));
        assert!(cleanup_target_selectable(&target, true, false, false));
    }

    #[test]
    fn cleanup_targets_detect_equal_and_nested_paths_but_not_siblings() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("acari-cache");
        let nested = root.join("nested");
        let target = |path: std::path::PathBuf| CleanTarget {
            path: path.to_string_lossy().into_owned().into(),
            ..CleanTarget::default()
        };

        assert!(cleanup_targets_overlap(
            &target(root.clone()),
            &target(root.clone())
        ));
        assert!(cleanup_targets_overlap(
            &target(root.clone()),
            &target(nested.clone())
        ));
        assert!(!cleanup_targets_overlap(
            &target(root.clone()),
            &target(root.with_file_name("acari-cache-other"))
        ));
        let dotdot_path = root.join("stale").join("..").join("nested");
        assert!(cleanup_targets_overlap(
            &target(dotdot_path),
            &target(nested)
        ));
        assert!(!cleanup_targets_overlap(
            &CleanTarget {
                command: &["docker", "system", "prune"],
                ..CleanTarget::default()
            },
            &target(root)
        ));
    }

    #[test]
    fn overlapping_file_targets_are_rejected_before_cleanup_worker_starts() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("cache");
        let child = parent.join("nested");
        std::fs::create_dir_all(&child).unwrap();
        let parent_target = CleanTarget {
            name: "Parent cache".into(),
            path: std::borrow::Cow::Owned(parent.to_string_lossy().into_owned()),
            description: "test parent".into(),
            ..CleanTarget::default()
        };
        let child_target = CleanTarget {
            name: "Nested cache".into(),
            path: std::borrow::Cow::Owned(child.to_string_lossy().into_owned()),
            description: "test child".into(),
            ..CleanTarget::default()
        };
        let mut app = super::DesktopApp {
            targets: vec![(parent_target, 10, 1, true), (child_target, 5, 1, true)],
            target_scan_done: true,
            confirm_clean: true,
            ..super::DesktopApp::default()
        };
        app.confirmation_snapshot = Some(app.selected_target_snapshot());

        app.begin_clean();

        assert!(app.clean_rx.is_none());
        assert!(!app.confirm_clean);
        assert!(app.cleanup_status.contains("sobrepostos"));
        assert!(parent.is_dir());
        assert!(child.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_scope_label_explains_root_deletion_and_symlink_handling() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("cache");
        let destination = temp.path().join("destination");
        let link = temp.path().join("cache-link");
        std::fs::create_dir(&directory).unwrap();
        std::fs::create_dir(&destination).unwrap();
        symlink(&destination, &link).unwrap();

        let whole_directory = CleanTarget {
            path: directory.to_string_lossy().into_owned().into(),
            delete_entire: true,
            ..CleanTarget::default()
        };
        let link_target = CleanTarget {
            path: link.to_string_lossy().into_owned().into(),
            delete_entire: false,
            ..CleanTarget::default()
        };

        assert!(cleanup_scope_label(&whole_directory).contains("pasta raiz"));
        assert!(cleanup_scope_label(&link_target).contains("destino será preservado"));
        let metrics = cleanup_metrics_label(&link_target, 0, 0, false);
        assert!(metrics.contains("1 link simbólico"));
        assert!(metrics.contains("destino preservado"));
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_target_overlap_resolves_symlinked_parents_for_missing_paths() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let actual = temp.path().join("actual");
        let alias = temp.path().join("alias");
        std::fs::create_dir_all(actual.join("cache")).unwrap();
        symlink(&actual, &alias).unwrap();
        let target = |path: String| CleanTarget {
            path: path.into(),
            ..CleanTarget::default()
        };

        let through_alias = target(
            alias
                .join("cache/not-created-yet")
                .to_string_lossy()
                .into_owned(),
        );
        let through_real_path = target(actual.join("cache").to_string_lossy().into_owned());
        assert!(cleanup_targets_overlap(&through_alias, &through_real_path));
    }

    #[test]
    fn disconnected_target_scan_does_not_mark_scan_complete() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx);
        let mut app = super::DesktopApp {
            target_rx: Some(rx),
            target_scan_busy: true,
            target_scan_done: false,
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert!(!app.target_scan_busy);
        assert!(!app.target_scan_done);
        assert!(app.target_rx.is_none());
        assert!(app.cleanup_status.contains("interrompida"));
    }

    #[test]
    fn cancelled_target_scan_does_not_mark_partial_preview_complete() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(crate::domain::AppEvent::ScanFinished).unwrap();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut app = super::DesktopApp {
            target_rx: Some(rx),
            target_scan_cancel: Some(cancel),
            target_scan_busy: true,
            target_scan_done: false,
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert!(!app.target_scan_busy);
        assert!(!app.target_scan_done);
        assert!(app.target_rx.is_none());
        assert!(app.cleanup_status.contains("cancelada"));
    }

    #[test]
    fn disconnected_target_scan_recovers_without_marking_preview_complete() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx);
        let mut app = super::DesktopApp {
            target_rx: Some(rx),
            target_scan_busy: true,
            target_scan_done: false,
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert!(!app.target_scan_busy);
        assert!(!app.target_scan_done);
        assert!(app.cleanup_status.contains("interrompida"));
    }

    #[test]
    fn target_scan_progress_updates_only_matching_target_identity() {
        let target = CleanTarget {
            name: "Same Name".into(),
            path: "/tmp/acari-progress-a".into(),
            ..CleanTarget::default()
        };
        let other = CleanTarget {
            name: "Same Name".into(),
            path: "/tmp/acari-progress-b".into(),
            ..CleanTarget::default()
        };
        let duplicate = other.clone();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(crate::domain::AppEvent::ScanProgress {
            target_name: "Same Name".into(),
            target_path: "/tmp/acari-progress-b".into(),
            bytes_found: 4096,
            files_scanned: 500,
        })
        .unwrap();
        let mut app = super::DesktopApp {
            targets: vec![
                (target, 0, 0, false),
                (other, 0, 0, false),
                (duplicate, 0, 0, false),
            ],
            target_rx: Some(rx),
            target_scan_busy: true,
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert_eq!(app.targets[0].1, 0);
        assert_eq!(app.targets[0].2, 0);
        assert_eq!(app.targets[1].1, 4096);
        assert_eq!(app.targets[1].2, 500);
        assert_eq!(app.targets[2].1, 4096);
        assert_eq!(app.targets[2].2, 500);
    }

    #[test]
    fn cleanup_requires_fresh_complete_scan_even_if_confirmation_is_open() {
        let target = CleanTarget {
            name: "Cache".into(),
            path: "/tmp/acari-cache".into(),
            ..CleanTarget::default()
        };
        let mut app = super::DesktopApp {
            targets: vec![(target.clone(), 10, 1, true)],
            confirm_clean: true,
            ..super::DesktopApp::default()
        };
        app.confirmation_snapshot = Some(app.selected_target_snapshot());

        app.begin_clean();
        assert!(app.clean_rx.is_none());
        assert!(!app.confirm_clean);
        assert!(app.cleanup_status.contains("Verifique"));

        app.target_scan_done = true;
        app.incomplete_scan_targets
            .insert(target.resolved_path().to_string_lossy().into_owned());
        app.confirm_clean = true;
        app.confirmation_snapshot = Some(app.selected_target_snapshot());
        app.begin_clean();
        assert!(app.clean_rx.is_none());
        assert!(!app.confirm_clean);
        assert!(app.cleanup_status.contains("incompleta"));
    }

    #[test]
    fn changing_selection_or_dry_run_invalidates_confirmation() {
        let first = CleanTarget {
            name: "First cache".into(),
            path: "/tmp/acari-first-cache".into(),
            ..CleanTarget::default()
        };
        let second = CleanTarget {
            name: "Second cache".into(),
            path: "/tmp/acari-second-cache".into(),
            ..CleanTarget::default()
        };
        let mut app = super::DesktopApp {
            targets: vec![(first, 10, 1, true), (second, 20, 2, false)],
            target_scan_done: true,
            confirm_clean: true,
            ..super::DesktopApp::default()
        };
        app.confirmation_snapshot = Some(app.selected_target_snapshot());

        app.targets[0].3 = false;
        app.targets[1].3 = true;
        app.begin_clean();
        assert!(app.clean_rx.is_none());
        assert!(!app.confirm_clean);
        assert!(app.cleanup_status.contains("mudou"));

        app.targets[0].3 = true;
        app.targets[1].3 = false;
        app.confirm_clean = true;
        app.confirmation_snapshot = Some(app.selected_target_snapshot());
        app.dry_run = true;
        app.begin_clean();
        assert!(app.clean_rx.is_none());
        assert!(!app.confirm_clean);
        assert!(app.cleanup_status.contains("mudou"));
    }

    #[test]
    fn special_target_cannot_start_as_part_of_batch_cleanup() {
        let ordinary = CleanTarget {
            name: "Ordinary cache".into(),
            path: "/tmp/acari-ordinary-cache".into(),
            ..CleanTarget::default()
        };
        let custom = CleanTarget {
            name: "Custom target".into(),
            path: "/tmp/acari-custom-target".into(),
            origin: TargetOrigin::Custom,
            ..CleanTarget::default()
        };
        let mut app = super::DesktopApp {
            targets: vec![(ordinary, 0, 0, true), (custom, 0, 0, true)],
            target_scan_done: true,
            confirm_clean: true,
            ..super::DesktopApp::default()
        };
        app.confirmation_snapshot = Some(app.selected_target_snapshot());

        app.begin_clean();

        assert!(app.clean_rx.is_none());
        assert!(!app.confirm_clean);
        assert!(app.cleanup_status.contains("individualmente"));
    }

    #[test]
    fn modified_privileged_target_is_disabled_and_rejected_before_worker_start() {
        let target = CleanTarget {
            name: "Apt Autoremove".into(),
            command: &["sudo", "apt", "autoremove", "-y", "--purge"],
            requires_sudo: true,
            dangerous: true,
            ..CleanTarget::default()
        };
        assert!(!desktop_cleanup_supported(&target));
        assert!(!cleanup_target_selectable(&target, true, false, false));

        let mut app = super::DesktopApp {
            targets: vec![(target, 100, 1, true)],
            target_scan_done: true,
            confirm_clean: true,
            ..super::DesktopApp::default()
        };
        app.confirmation_snapshot = Some(app.selected_target_snapshot());

        app.begin_clean();

        assert!(app.clean_rx.is_none());
        assert!(!app.confirm_clean);
        assert!(app.cleanup_status.contains("incompatíveis"));
    }

    #[test]
    fn cleanup_completion_keeps_error_detail_and_partial_cancel_state() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(crate::domain::AppEvent::TargetCleaned {
            target_name: "Cache".into(),
            reclaimed_bytes: 12,
            removed_entries: 1,
            errors: 1,
            error_detail: Some("permission denied".into()),
        })
        .unwrap();
        tx.send(crate::domain::AppEvent::CleaningFinished {
            cleaned_targets: 1,
            reclaimed_bytes: 12,
            errors: 1,
            cancelled: true,
        })
        .unwrap();
        let mut app = super::DesktopApp {
            clean_rx: Some(rx),
            cleanup_status: "Limpando…".into(),
            target_scan_done: true,
            targets: vec![(
                CleanTarget {
                    name: "Cache".into(),
                    path: "/tmp/acari-cache".into(),
                    ..CleanTarget::default()
                },
                12,
                1,
                true,
            )],
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert!(app.cleanup_status.contains("cancelada"));
        assert!(app.cleanup_status.contains("1 erros"));
        assert!(app.cleanup_status.contains("Cache: permission denied"));
        assert!(app.cleanup_status.contains("Verifique os alvos novamente"));
        assert!(!app.target_scan_done);
        assert!(!app.targets[0].3);
    }

    #[test]
    fn dry_run_completion_preserves_preview_and_selection() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(crate::domain::AppEvent::CleaningFinished {
            cleaned_targets: 1,
            reclaimed_bytes: 12,
            errors: 0,
            cancelled: false,
        })
        .unwrap();
        let target = CleanTarget {
            name: "Cache".into(),
            path: "/tmp/acari-dry-run-cache".into(),
            ..CleanTarget::default()
        };
        let mut app = super::DesktopApp {
            clean_rx: Some(rx),
            dry_run: true,
            target_scan_done: true,
            targets: vec![(target, 12, 1, true)],
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert!(app.target_scan_done);
        assert!(app.targets[0].3);
        assert!(app.cleanup_status.contains("Simulação"));
        assert!(!app.cleanup_status.contains("Verifique os alvos novamente"));
    }

    #[test]
    fn privileged_completion_never_reports_estimated_bytes_as_reclaimed() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(crate::domain::AppEvent::CleaningFinished {
            cleaned_targets: 1,
            reclaimed_bytes: 0,
            errors: 0,
            cancelled: false,
        })
        .unwrap();
        let target = CleanTarget {
            name: "Apt Autoremove".into(),
            command: &["sudo", "apt", "autoremove", "-y"],
            requires_sudo: true,
            ..CleanTarget::default()
        };
        let mut app = super::DesktopApp {
            clean_rx: Some(rx),
            privileged_clean: true,
            target_scan_done: true,
            targets: vec![(target, 30_000_000, 3, true)],
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert!(app.cleanup_status.contains("espaço recuperado não medido"));
        assert!(!app.cleanup_status.contains("30"));
        assert!(!app.target_scan_done);
        assert!(!app.targets[0].3);
    }

    #[test]
    fn disconnected_cleanup_worker_releases_ui_and_reports_incomplete_result() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx);
        let mut app = super::DesktopApp {
            clean_rx: Some(rx),
            clean_cancel: Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                false,
            ))),
            ..super::DesktopApp::default()
        };

        app.poll_target_events(&eframe::egui::Context::default());

        assert!(app.clean_rx.is_none());
        assert!(app.clean_cancel.is_none());
        assert!(app.cleanup_status.contains("interrompida"));
        assert!(!app.target_scan_done);
    }

    #[test]
    fn desktop_allows_only_exact_non_privileged_builtin_commands() {
        let docker = CleanTarget {
            name: "Docker System Prune".into(),
            command: &["docker", "system", "prune", "-a", "--force"],
            dangerous: true,
            ..CleanTarget::default()
        };
        assert!(desktop_cleanup_supported(&docker));

        let shell = CleanTarget {
            name: "Docker Builder Prune".into(),
            command: &["sh", "-c", "echo unsafe"],
            dangerous: true,
            ..CleanTarget::default()
        };
        assert!(!desktop_cleanup_supported(&shell));

        let privileged = CleanTarget {
            requires_sudo: true,
            ..docker.clone()
        };
        assert!(!desktop_cleanup_supported(&privileged));

        let custom = CleanTarget {
            origin: TargetOrigin::Custom,
            ..docker
        };
        assert!(!desktop_cleanup_supported(&custom));
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn desktop_allows_builtin_builder_prune_but_not_custom_shell_commands() {
        let target =
            crate::domain::targets::build_targets(&["Docker Builder Prune".to_string()], &[])
                .pop()
                .expect("built-in builder target is available");
        assert!(desktop_cleanup_supported(&target));

        let untrusted = CleanTarget {
            name: "Docker Builder Prune".into(),
            command: &["sh", "-c", "docker buildx prune -a -f; echo arbitrary"],
            dangerous: true,
            origin: TargetOrigin::Custom,
            ..CleanTarget::default()
        };
        assert!(!desktop_cleanup_supported(&untrusted));
    }

    #[test]
    fn squarified_layout_preserves_area_proportions_and_skips_zeroes() {
        let bounds = Rect::from_min_max(Pos2::ZERO, Pos2::new(400.0, 200.0));
        let boxes = squarified_layout(bounds, &[60, 30, 10, 0]);
        assert_eq!(boxes.len(), 3);
        let areas: Vec<_> = boxes.iter().map(|(_, rect)| rect.area()).collect();
        let total: f32 = areas.iter().sum();
        assert!((total - bounds.area()).abs() < 1.0);
        assert!((areas[0] / total - 0.6).abs() < 0.01);
        assert!((areas[1] / total - 0.3).abs() < 0.01);
        assert!((areas[2] / total - 0.1).abs() < 0.01);
    }

    #[test]
    fn squarified_layout_handles_empty_weights() {
        let bounds = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 80.0));
        assert!(squarified_layout(bounds, &[0, 0]).is_empty());
    }
}
