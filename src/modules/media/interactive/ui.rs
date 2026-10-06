pub mod files_view;
pub mod footer;
pub mod main_view;
pub mod sub_view;

use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style},
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use super::{AppState, ViewMode};
pub use files_view::render_files_ui;
pub use main_view::render_main_ui;
pub use sub_view::render_subview_ui;

pub fn render_ui(f: &mut Frame, state: &AppState) {
    match state.view_mode {
        ViewMode::Main => render_main_ui(f, state),
        ViewMode::SubView { item_idx, cursor } => render_subview_ui(f, state, item_idx, cursor),
        ViewMode::FilesView {
            item_idx,
            season_idx,
            cursor,
        } => render_files_ui(f, state, item_idx, season_idx, cursor),
    }
}

/// Draws a themed vertical scrollbar on the right edge of a list body.
///
/// The bar is only shown when the content overflows the viewport, and it is
/// inset by one cell so it sits inside the rounded border instead of
/// overwriting it.
pub fn render_scrollbar(
    f: &mut Frame,
    area: Rect,
    content_len: usize,
    viewport_len: usize,
    offset: usize,
) {
    if content_len <= viewport_len {
        return;
    }

    let mut state = ScrollbarState::new(content_len)
        .viewport_content_length(viewport_len)
        .position(offset);

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .style(Style::default().fg(Color::DarkGray))
        .thumb_style(Style::default().fg(Color::Cyan));

    let bar_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    f.render_stateful_widget(scrollbar, bar_area, &mut state);
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

#[cfg(test)]
mod tests {
    use super::render_scrollbar;
    use ratatui::{backend::TestBackend, Terminal};

    fn draw(content_len: usize, viewport_len: usize, offset: usize) -> String {
        let backend = TestBackend::new(20, 8);
        let Ok(mut terminal) = Terminal::new(backend) else {
            panic!("failed to create test terminal");
        };
        assert!(terminal
            .draw(|f| {
                render_scrollbar(f, f.area(), content_len, viewport_len, offset);
            })
            .is_ok());
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn scrollbar_draws_when_content_overflows() {
        let rendered = draw(100, 5, 0);
        assert!(
            rendered.contains('\u{2502}')
                || rendered.contains('\u{2503}')
                || rendered.contains('\u{2588}'),
            "expected a scrollbar glyph, got: {rendered:?}"
        );
    }

    #[test]
    fn scrollbar_hidden_when_content_fits() {
        let rendered = draw(5, 10, 0);
        assert!(
            !rendered.contains('\u{2502}') && !rendered.contains('\u{2588}'),
            "scrollbar should not render when content fits, got: {rendered:?}"
        );
    }
}
