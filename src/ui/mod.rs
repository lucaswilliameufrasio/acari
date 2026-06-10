pub mod app;
pub mod project;

pub(crate) fn resolve_scroll(selected: usize, current: usize, visible: usize) -> usize {
    if visible == 0 {
        return 0;
    }
    if selected < current {
        selected
    } else if selected >= current + visible {
        selected.saturating_sub(visible.saturating_sub(1))
    } else {
        current
    }
}
