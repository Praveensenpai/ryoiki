use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use super::truncate_str;
use crate::modules::media::disk;
use crate::modules::media::interactive::AppState;
use crate::modules::media::transfer::{MediaItem, MediaSeason, TransferDirection};

pub fn render_subview_ui(f: &mut Frame, state: &AppState, item_idx: usize, cursor: usize) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    let Some(item) = state.current_items().get(item_idx) else {
        return;
    };

    render_subview_header(f, chunks[0], item, state);
    render_subview_body(f, chunks[1], item, cursor, state);
    render_subview_footer(f, chunks[2]);
}

fn render_subview_header(f: &mut Frame, area: Rect, item: &MediaItem, state: &AppState) {
    let total_ssn = item.seasons.len();
    let line1 = Line::from(vec![
        Span::raw(" Select individual seasons to transfer │ Total: "),
        Span::styled(
            format!("{total_ssn} seasons"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ Press "),
        Span::styled(
            "[Esc]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" or "),
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" when done"),
    ]);

    let mut header_lines = vec![line1, build_subview_storage_line(state)];
    if let Some(warn) = &state.warning_msg {
        header_lines.push(Line::from(vec![Span::styled(
            format!(" {warn} "),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    let block = Block::default()
        .title(format!(" 📁 {} ", item.title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    let para = Paragraph::new(header_lines)
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(para, area);
}

fn build_subview_storage_line(state: &AppState) -> Line<'static> {
    let Some(disk) = state.local_disk else {
        return Line::from(vec![Span::styled(
            " 💾 Local SSD: storage info unavailable ",
            Style::default().fg(Color::DarkGray),
        )]);
    };

    let total_str = disk::format_bytes(disk.total_bytes);
    let free_str = disk::format_bytes(disk.free_bytes);
    let pct = disk.used_pct;
    let sel_bytes = state.total_selected_bytes();
    let sel_str = disk::format_bytes(sel_bytes);

    match state.direction {
        TransferDirection::Push => Line::from(vec![
            Span::styled(
                format!(" 💾 Local SSD: {free_str} free / {total_str} ({pct}% used) │ "),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("Selected: {sel_str} (+{sel_str} freed) "),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        TransferDirection::Pull => {
            let rem = disk.free_bytes.saturating_sub(sel_bytes);
            let rem_str = disk::format_bytes(rem);
            let rem_color = if rem < 10 * 1024 * 1024 * 1024 {
                Color::Red
            } else {
                Color::Green
            };
            Line::from(vec![
                Span::styled(
                    format!(" 💾 Local SSD: {free_str} free / {total_str} ({pct}% used) │ Selected: {sel_str} │ "),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("Remaining Free: {rem_str} "),
                    Style::default().fg(rem_color).add_modifier(Modifier::BOLD),
                ),
            ])
        }
    }
}

fn render_subview_body(
    f: &mut Frame,
    area: Rect,
    item: &MediaItem,
    cursor: usize,
    state: &AppState,
) {
    let sel_count = item.selected_seasons_count();
    let sel_bytes: u64 = item
        .seasons
        .iter()
        .filter(|s| s.is_selected)
        .map(|s| s.size_bytes)
        .sum();

    let title = format!(
        " Seasons in {} — Selected: {}/{} ({}) ",
        item.title,
        sel_count,
        item.seasons.len(),
        disk::format_bytes(sel_bytes)
    );

    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll_offset = if cursor >= inner_height {
        cursor - inner_height + 1
    } else {
        0
    };

    let visible = item
        .seasons
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(inner_height);

    let mut lines = Vec::new();
    for (vis_idx, (abs_idx, season)) in visible.enumerate() {
        let is_cursor = (scroll_offset + vis_idx) == cursor;
        lines.push(build_season_line(season, is_cursor, abs_idx + 1, state));
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White));

    let body = Paragraph::new(lines).block(block);
    f.render_widget(body, area);
}

fn build_season_line<'a>(
    season: &'a MediaSeason,
    cursor: bool,
    num: usize,
    state: &AppState,
) -> Line<'a> {
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

    let check = if season.is_selected {
        Span::styled(
            "[✓] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
    };

    let title_style = if cursor {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let title = Span::styled(
        format!("{num:02}. {:<48} ", truncate_str(&season.title, 48)),
        title_style,
    );

    let size_str = disk::format_bytes(season.size_bytes);
    let size = Span::styled(format!("{size_str:<10} "), Style::default().fg(Color::Cyan));

    let mut spans = vec![pointer, check, title, size];

    if state.direction == TransferDirection::Pull {
        if let Some(disk) = state.local_disk {
            if season.size_bytes > disk.free_bytes {
                spans.push(Span::styled(
                    "🚫 No Space",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            }
        }
    }

    Line::from(spans)
}

fn render_subview_footer(f: &mut Frame, area: Rect) {
    let shortcuts = [
        ("[Space]", "Toggle Season"),
        ("[a]", "Select All"),
        ("[n]", "Deselect All"),
        ("[Esc]/[Enter]", "Done (Back to Main)"),
    ];
    super::footer::render_footer(f, area, &shortcuts);
}
