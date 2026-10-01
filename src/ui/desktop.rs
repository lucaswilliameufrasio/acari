use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::application::cleaner::{self, CleanMode};
use crate::application::commands::{merge_excludes, prepare_targets, start_scan};
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
    target_rx: Option<tokio::sync::mpsc::UnboundedReceiver<AppEvent>>,
    clean_rx: Option<tokio::sync::mpsc::UnboundedReceiver<AppEvent>>,
    clean_tx: Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>,
    target_scan_done: bool,
    target_scan_busy: bool,
    dry_run: bool,
    confirm_clean: bool,
    cleanup_status: String,
    search: String,
    sort_largest_first: bool,
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
            target_rx: None,
            clean_rx: None,
            clean_tx: None,
            target_scan_done: false,
            target_scan_busy: false,
            dry_run: false,
            confirm_clean: false,
            cleanup_status: String::new(),
            search: String::new(),
            sort_largest_first: true,
        }
    }
}

impl DesktopApp {
    fn start_scan(&mut self) {
        let root = PathBuf::from(self.path.trim());
        let (tx, rx) = mpsc::channel();
        let (progress_tx, progress_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::spawn(move || {
            let result =
                disk_scan::scan_tree_cancellable(&root, Some(progress_tx), Some(worker_cancel))
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
        let excludes = merge_excludes(&[], &config.scan.exclude_patterns);
        let (_, rx, _) = start_scan(targets.clone(), excludes, config.scan.io_priority, false);
        self.targets = targets
            .into_iter()
            .map(|target| (target, 0, 0, false))
            .collect();
        self.target_rx = Some(rx);
        self.target_scan_done = false;
        self.target_scan_busy = true;
        self.cleanup_status = "Verificando alvos…".into();
    }

    fn poll_target_events(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &mut self.target_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::TargetCompleted {
                        target_name,
                        total_bytes,
                        files_scanned,
                    } => {
                        if let Some(row) = self
                            .targets
                            .iter_mut()
                            .find(|row| row.0.name == target_name)
                        {
                            row.1 = total_bytes;
                            row.2 = files_scanned;
                        }
                    }
                    AppEvent::ScanFinished => {
                        self.target_scan_done = true;
                        self.target_scan_busy = false;
                        self.cleanup_status = "Verificação concluída.".into();
                        self.target_rx = None;
                        break;
                    }
                    _ => {}
                }
            }
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
                        self.cleanup_status = format!(
                            "{}{} liberados; {} erros{}.",
                            if self.dry_run {
                                "Simulação: "
                            } else {
                                "Limpeza: "
                            },
                            reclaimed_bytes,
                            errors,
                            if cancelled { " (cancelada)" } else { "" }
                        );
                        self.clean_rx = None;
                        self.clean_tx = None;
                        break;
                    }
                    AppEvent::TargetCleaned {
                        target_name,
                        error_detail: Some(detail),
                        ..
                    } => {
                        self.cleanup_status = format!("{target_name}: {detail}");
                    }
                    _ => {}
                }
            }
        }
        if self.target_scan_busy || self.clean_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn begin_clean(&mut self) {
        let selected: Vec<_> = self
            .targets
            .iter()
            .filter(|(_, _, _, selected)| *selected)
            .filter(|(target, _, _, _)| !target.is_command() && !target.is_dangerous())
            .map(|(target, bytes, files, _)| (target.clone(), *bytes, *files))
            .collect();
        if selected.is_empty() {
            self.cleanup_status = "Selecione pelo menos um alvo de arquivos.".into();
            return;
        }
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        cleaner::start_background_clean(
            tx.clone(),
            selected,
            if self.dry_run {
                CleanMode::DryRun
            } else {
                CleanMode::Execute
            },
        );
        self.clean_tx = Some(tx);
        self.clean_rx = Some(rx);
        self.confirm_clean = false;
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
                    ui.checkbox(&mut self.dry_run, "Simular (dry-run)");
                    if ui.add_enabled(self.target_scan_done && self.clean_rx.is_none(), egui::Button::new("Limpar selecionados…")).clicked() { self.confirm_clean = true; }
                });
                if !self.cleanup_status.is_empty() { ui.label(&self.cleanup_status); }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (target, bytes, files, selected) in &mut self.targets {
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(!target.is_command() && !target.is_dangerous(), |ui| {
                                ui.checkbox(selected, "");
                            });
                            ui.strong(target.name.as_ref());
                            ui.label(format!("{} bytes · {} arquivos", bytes, files));
                            if target.is_command() { ui.colored_label(Color32::YELLOW, "comando — indisponível nesta tela"); }
                            else if target.is_dangerous() { ui.colored_label(Color32::LIGHT_RED, "perigoso/requer privilégio — não disponível ainda"); }
                        });
                        ui.label(format!("{} — {}", target.path, target.description));
                        ui.separator();
                    }
                });
                if self.confirm_clean {
                    let mut confirm = false;
                    egui::Window::new("Confirmar limpeza").collapsible(false).resizable(false)
                        .show(ctx, |ui| {
                            let count = self.targets.iter().filter(|(_, _, _, selected)| *selected).count();
                            ui.label(format!("{} {} alvo(s) selecionado(s).", if self.dry_run { "Simular" } else { "Limpar" }, count));
                            ui.label("Esta ação não pode ser desfeita. A análise visual de disco não será afetada.");
                            ui.horizontal(|ui| {
                                if ui.button(if self.dry_run { "Executar simulação" } else { "Confirmar limpeza" }).clicked() { confirm = true; }
                                if ui.button("Cancelar").clicked() { self.confirm_clean = false; }
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
                    "{} arquivos/diretórios · {} bytes",
                    self.progress.entries, root.bytes
                ));
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(260.0).hint_text("Filtrar itens deste diretório…"));
                if ui.button(if self.sort_largest_first { "Maior primeiro" } else { "Nome A–Z" }).clicked() { self.sort_largest_first = !self.sort_largest_first; }
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
            );
            if let Some(path) = navigate {
                let mut indices = self.current.clone();
                if find_child_indices(root, &path, &mut indices) {
                    self.current = indices;
                }
            }
            if let Some(path) = &self.selected {
                let detail = self.tree.as_ref().and_then(|root| find_node(root, path));
                ui.group(|ui| {
                    ui.strong("Detalhes da seleção");
                    ui.label(format!("{}", path.display()));
                    if let Some(node) = detail {
                        ui.label(format!("{} bytes · {}", node.bytes, if node.is_dir { "diretório" } else { "arquivo" }));
                    }
                });
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

fn find_node<'a>(node: &'a DiskNode, path: &std::path::Path) -> Option<&'a DiskNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_node(child, path))
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
        children.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    } else {
        children.sort_by_key(|a| a.name.to_lowercase());
    }
    let boxes = squarified_layout(
        rect,
        &children.iter().map(|child| child.bytes).collect::<Vec<_>>(),
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
            response.on_hover_text(format!("{}\n{} bytes", child.path.display(), child.bytes));
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
    use super::squarified_layout;
    use eframe::egui::{Pos2, Rect};

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
