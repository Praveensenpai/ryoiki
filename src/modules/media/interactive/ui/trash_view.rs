use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use super::truncate_str;
use crate::modules::media::disk;
use crate::modules::media::interactive::{trash, AppState, ConfirmState};
use crate::modules::media::trash::TrashEntry;

pub fn render_trash_ui(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_trash_header(f, chunks[0], state);
    render_trash_body(f, chunks[1], state);
    render_trash_footer(f, chunks[2]);
}

fn render_trash_header(f: &mut Frame, area: Rect, state: &AppState) {
    let count = state.trash_entries.len();
    let total: u64 = state.trash_entries.iter().map(|e| e.size_bytes).sum();
    let headline = Line::from(vec![
        Span::styled(
            " 🗑 Trash ",
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "│ {count} {} ({}) │ auto-delete after 7 days ",
            if count == 1 { "entry" } else { "entries" },
            disk::format_bytes(total)
        )),
    ]);

    let mut lines = vec![headline];
    if let Some(warn) = &state.warning_msg {
        lines.push(Line::from(vec![Span::styled(
            format!(" {warn} "),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    let block = Block::default()
        .title(" 🗑 Trash — Recoverable Media ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightRed));

    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

fn render_trash_body(f: &mut Frame, area: Rect, state: &AppState) {
    let inner_height = area.height.saturating_sub(3) as usize;
    let scroll_offset = if state.trash_cursor >= inner_height {
        state.trash_cursor - inner_height + 1
    } else {
        0
    };

    let mut lines = vec![build_trash_header_line()];
    for (vis, entry) in state
        .trash_entries
        .iter()
        .skip(scroll_offset)
        .take(inner_height)
        .enumerate()
    {
        let is_cursor = scroll_offset + vis == state.trash_cursor;
        lines.push(build_trash_line(entry, is_cursor));
    }

    if lines.len() == 1 {
        lines.push(Line::from(vec![Span::styled(
            "   (Trash is empty)",
            Style::default().fg(Color::DarkGray),
        )]));
    }

    let block = Block::default()
        .title(" Trashed Items ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White));

    f.render_widget(Paragraph::new(lines).block(block), area);
    super::render_scrollbar(
        f,
        area,
        state.trash_entries.len(),
        inner_height,
        scroll_offset,
    );
}

fn build_trash_header_line() -> Line<'static> {
    Line::from(vec![Span::styled(
        "       Type     Title                              Size      Origin   Expires",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )])
}

fn build_trash_line(entry: &TrashEntry, cursor: bool) -> Line<'static> {
    let pointer = if cursor {
        Span::styled(
            " ▶ ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("   ")
    };

    let kind = Span::styled(
        format!("{:<8} ", entry.kind.label()),
        Style::default().fg(Color::Yellow),
    );
    let title_style = if cursor {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let title = Span::styled(
        format!("{:<34} ", truncate_str(&entry.title, 34)),
        title_style,
    );
    let size = Span::styled(
        format!("{:<9} ", disk::format_bytes(entry.size_bytes)),
        Style::default().fg(Color::Cyan),
    );
    let origin = Span::styled(
        format!("{:<8} ", entry.origin.label()),
        Style::default().fg(Color::Magenta),
    );

    let days = entry.days_left();
    let expires = if days == 0 {
        Span::styled(
            "expired",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            format!("{days}d left"),
            Style::default().fg(if days <= 2 {
                Color::LightRed
            } else {
                Color::Green
            }),
        )
    };

    Line::from(vec![pointer, kind, title, size, origin, expires])
}

fn render_trash_footer(f: &mut Frame, area: Rect) {
    let shortcuts = [
        ("[↑/k, ↓/j]", "Navigate"),
        ("[r]", "Restore"),
        ("[x]", "Delete forever"),
        ("[Tab]", "Mode"),
        ("[Esc]/[q]", "Back"),
    ];
    super::footer::render_footer(f, area, &shortcuts);
}

/// Draws the confirmation dialog centred over the current screen.
pub fn render_modal(f: &mut Frame, modal: &ConfirmState) {
    let area = f.area();
    let width = 64.min(area.width.saturating_sub(4));
    let height = 9;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);

    f.render_widget(Clear, rect);

    let yes_label = format!("  [ {} ]  ", modal.yes_label);
    let yes = if modal.yes_focused {
        Span::styled(
            yes_label.clone(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(yes_label, Style::default().fg(Color::DarkGray))
    };
    let no = if modal.yes_focused {
        Span::styled("  [ No, keep it ]  ", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(
            "  [ No, keep it ]  ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            modal.prompt.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            modal.detail.clone(),
            Style::default().fg(Color::Gray),
        )]),
        Line::from(""),
        Line::from(vec![no, Span::raw("   "), yes]),
    ];

    let block = Block::default()
        .title(" Confirm ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        rect,
    );
}

/// Handles navigation inside the Trash tab.
///
/// Returns `Some(false)` to quit the whole TUI.
pub fn handle_trash_key(code: KeyCode, state: &mut AppState) -> Option<bool> {
    let len = state.trash_entries.len();
    match code {
        KeyCode::Up | KeyCode::Char('k') if len > 0 => {
            state.warning_msg = None;
            state.trash_cursor = if state.trash_cursor > 0 {
                state.trash_cursor - 1
            } else {
                len - 1
            };
            None
        }
        KeyCode::Down | KeyCode::Char('j') if len > 0 => {
            state.warning_msg = None;
            state.trash_cursor = if state.trash_cursor + 1 < len {
                state.trash_cursor + 1
            } else {
                0
            };
            None
        }
        KeyCode::Char('r') => {
            trash::request_restore(state);
            None
        }
        KeyCode::Char('x' | 'd') => {
            trash::request_purge(state);
            None
        }
        KeyCode::Tab => {
            state.trash_open = false;
            state.direction = crate::modules::media::transfer::TransferDirection::Push;
            state.cursor = 0;
            state.warning_msg = None;
            None
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            state.trash_open = false;
            state.warning_msg = None;
            None
        }
        _ => None,
    }
}
