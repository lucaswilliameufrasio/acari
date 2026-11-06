//! Opt-in read-only GPUI migration slice; the existing desktop remains the default.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use acari::config::target_config;
use acari::domain::format_bytes;
use acari::infrastructure::disk_scan::{self, DiskNode, ScanProgress};
use gpui_kit::component::button::*;
use gpui_kit::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, WindowOptions, div,
};

enum ScanEvent {
    Progress(ScanProgress),
    Finished(Result<DiskNode, String>),
}

struct DiskPreview {
    path: PathBuf,
    status: String,
    result: Option<DiskNode>,
    navigation: Vec<usize>,
    page: usize,
    receiver: Option<mpsc::Receiver<ScanEvent>>,
    cancel: Option<Arc<AtomicBool>>,
}

impl DiskPreview {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            status: "Análise somente leitura. Nenhum arquivo será removido.".into(),
            result: None,
            navigation: Vec::new(),
            page: 0,
            receiver: None,
            cancel: None,
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.receiver.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let (progress_tx, progress_rx) = mpsc::channel();
        let path = self.path.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let priority = target_config::load_config().scan.io_priority;
        let progress_events = tx.clone();
        std::thread::spawn(move || {
            for progress in progress_rx {
                if progress_events.send(ScanEvent::Progress(progress)).is_err() {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            let result = disk_scan::scan_tree_cancellable_with_priority(
                &path,
                Some(progress_tx),
                Some(worker_cancel),
                priority,
            )
            .map_err(|error| error.to_string());
            let _ = tx.send(ScanEvent::Finished(result));
        });
        self.receiver = Some(rx);
        self.cancel = Some(cancel);
        self.result = None;
        self.navigation.clear();
        self.page = 0;
        self.status = "Analisando…".into();
        cx.notify();
        cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let Some(view) = weak.upgrade() else {
                    break;
                };
                let active = view.update(cx, |view, cx| {
                    view.poll();
                    cx.notify();
                    view.receiver.is_some()
                });
                if !active {
                    break;
                }
            }
        })
        .detach();
    }

    fn poll(&mut self) {
        let Some(rx) = self.receiver.as_ref() else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(ScanEvent::Progress(progress)) => {
                    self.status = format!(
                        "Analisando… {} itens · {}",
                        progress.entries,
                        format_bytes(progress.bytes)
                    );
                }
                Ok(ScanEvent::Finished(Ok(tree))) => {
                    self.status = format!("Análise concluída · {}", format_bytes(tree.bytes));
                    self.result = Some(tree);
                    self.receiver = None;
                    self.cancel = None;
                    break;
                }
                Ok(ScanEvent::Finished(Err(error))) => {
                    self.status = format!("Análise interrompida: {error}");
                    self.receiver = None;
                    self.cancel = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "A análise foi interrompida antes de concluir.".into();
                    self.receiver = None;
                    self.cancel = None;
                    break;
                }
            }
        }
    }

    fn current(&self) -> Option<&DiskNode> {
        let mut node = self.result.as_ref()?;
        for &index in &self.navigation {
            node = node.children.get(index)?;
        }
        Some(node)
    }

    fn enter(&mut self, index: usize) {
        if self
            .current()
            .is_some_and(|node| node.children.get(index).is_some())
        {
            self.navigation.push(index);
            self.page = 0;
        }
    }

    fn back(&mut self) {
        self.navigation.pop();
        self.page = 0;
    }
}

impl Drop for DiskPreview {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Render for DiskPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.receiver.is_some();
        let mut content = div()
            .id("preview-content")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .child(div().text_xl().child("Acarí · análise de disco (prévia)"))
            .child(self.path.display().to_string())
            .child(self.status.clone());
        if running {
            content = content.child(Button::new("cancel").label("Cancelar análise").on_click(
                cx.listener(|view, _, _, cx| {
                    if let Some(cancel) = &view.cancel {
                        cancel.store(true, Ordering::Relaxed);
                        view.status = "Cancelando análise…".into();
                        cx.notify();
                    }
                }),
            ));
        } else {
            content = content.child(
                Button::new("scan")
                    .primary()
                    .label("Analisar pasta")
                    .on_click(cx.listener(|view, _, _, cx| view.start(cx))),
            );
        }
        if let Some(node) = self.current() {
            content = content
                .child(node.path.display().to_string())
                .child(format!(
                    "Aparente: {} · Alocado: {}",
                    format_bytes(node.bytes),
                    format_bytes(node.allocated_bytes)
                ));
            if !self.navigation.is_empty() {
                content = content.child(Button::new("back").label("Voltar").on_click(cx.listener(
                    |view, _, _, cx| {
                        view.back();
                        cx.notify();
                    },
                )));
            }
            if node.is_dir {
                let total = node.children.len();
                content = content.child(format!("{total} itens diretos"));
                for (index, child) in node
                    .children
                    .iter()
                    .enumerate()
                    .skip(self.page * 20)
                    .take(20)
                {
                    let label = format!("{} · {}", child.name, format_bytes(child.bytes));
                    content = content.child(Button::new(("entry", index)).label(label).on_click(
                        cx.listener(move |view, _, _, cx| {
                            view.enter(index);
                            cx.notify();
                        }),
                    ));
                }
                if self.page > 0 {
                    content = content.child(
                        Button::new("previous-page")
                            .label("Página anterior")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.page -= 1;
                                cx.notify();
                            })),
                    );
                }
                if (self.page + 1) * 20 < total {
                    content =
                        content.child(Button::new("next-page").label("Próxima página").on_click(
                            cx.listener(|view, _, _, cx| {
                                view.page += 1;
                                cx.notify();
                            }),
                        ));
                }
            }
        }
        content
    }
}

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| DiskPreview::new(path))
        })
        .expect("failed to open GPUI preview");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_inspects_files_empty_directories_and_all_entries() {
        let mut view = DiskPreview::new("root".into());
        let children = (0..25)
            .map(|index| DiskNode {
                name: format!("entry-{index}"),
                path: format!("root/{index}").into(),
                bytes: index,
                allocated_bytes: index * 2,
                is_dir: index == 24,
                children: vec![],
            })
            .collect();
        view.result = Some(DiskNode {
            name: "root".into(),
            path: "root".into(),
            bytes: 300,
            allocated_bytes: 600,
            is_dir: true,
            children,
        });
        view.page = 1;
        view.enter(24);
        assert!(view.current().unwrap().is_dir);
        assert!(view.current().unwrap().children.is_empty());
        assert_eq!(view.page, 0);
        view.enter(0);
        assert_eq!(view.navigation.len(), 1);
        view.back();
        view.enter(21);
        assert!(!view.current().unwrap().is_dir);
        assert_eq!(view.current().unwrap().allocated_bytes, 42);
        view.back();
        view.back();
        assert!(view.navigation.is_empty());
        assert_eq!(view.current().unwrap().children.len(), 25);
    }

    #[test]
    fn scan_progress_and_completion_update_the_preview() {
        let (tx, rx) = mpsc::channel();
        let mut view = DiskPreview::new(PathBuf::from("fixture"));
        view.receiver = Some(rx);
        tx.send(ScanEvent::Progress(ScanProgress {
            entries: 3,
            bytes: 42,
        }))
        .unwrap();
        view.poll();
        assert!(view.status.contains("3 itens"));
        tx.send(ScanEvent::Finished(Ok(DiskNode {
            name: "fixture".into(),
            path: "fixture".into(),
            bytes: 42,
            allocated_bytes: 42,
            is_dir: true,
            children: Vec::new(),
        })))
        .unwrap();
        view.poll();
        assert!(view.status.contains("concluída"));
        assert_eq!(view.result.as_ref().unwrap().bytes, 42);
        assert!(view.receiver.is_none());
    }

    #[test]
    fn failed_and_disconnected_scans_never_show_success() {
        for send_error in [true, false] {
            let (tx, rx) = mpsc::channel();
            let mut view = DiskPreview::new(PathBuf::from("fixture"));
            view.receiver = Some(rx);
            if send_error {
                tx.send(ScanEvent::Finished(Err("falha".into()))).unwrap();
            }
            drop(tx);
            view.poll();
            assert!(view.status.contains("interrompida"));
            assert!(view.result.is_none());
            assert!(view.receiver.is_none());
        }
    }

    #[test]
    fn dropping_preview_requests_cancellation() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut view = DiskPreview::new(PathBuf::from("fixture"));
        view.cancel = Some(Arc::clone(&cancel));
        drop(view);
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn read_only_scanner_preserves_fixture_and_reports_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("example.txt");
        std::fs::write(&file, b"fixture").unwrap();
        let (tx, rx) = mpsc::channel();
        let result = disk_scan::scan_tree_cancellable_with_priority(
            directory.path(),
            Some(tx),
            Some(Arc::new(AtomicBool::new(false))),
            target_config::IoPriority::Normal,
        )
        .unwrap();
        assert_eq!(result.bytes, 7);
        assert!(rx.try_iter().any(|progress| progress.entries == 1));
        assert_eq!(std::fs::read(file).unwrap(), b"fixture");
    }
}
