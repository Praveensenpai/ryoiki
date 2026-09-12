pub mod footer;
pub mod main_view;
pub mod sub_view;

use ratatui::Frame;

use super::{AppState, ViewMode};
pub use main_view::render_main_ui;
pub use sub_view::render_subview_ui;

pub fn render_ui(f: &mut Frame, state: &AppState) {
    match state.view_mode {
        ViewMode::Main => render_main_ui(f, state),
        ViewMode::SubView { item_idx, cursor } => render_subview_ui(f, state, item_idx, cursor),
    }
}

pub fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    }
}
