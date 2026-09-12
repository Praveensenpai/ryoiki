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
use crate::modules::media::transfer::{MediaCategory, MediaItem, TransferDirection};

pub fn render_main_ui(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_main_header(f, chunks[0], state);
    render_main_body(f, chunks[1], state);
    render_main_footer(f, chunks[2]);
}

fn render_main_header(f: &mut Frame, area: Rect, state: &AppState) {
    let mut header_lines = vec![build_tab_line(state), build_storage_line(state)];
    if let Some(warn) = &state.warning_msg {
        header_lines.push(Line::from(vec![Span::styled(
            format!(" {warn} "),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    let block = Block::default()
        .title(" 🌸 領域 (Ryoiki) — Interactive Media Manager ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let para = Paragraph::new(header_lines)
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(para, area);
}

fn build_tab_line(state: &AppState) -> Line<'static> {
    let is_push = state.direction == TransferDirection::Push;
    let push_style = if is_push {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let pull_style = if is_push {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    };

    Line::from(vec![
        Span::styled(
            if is_push {
                " ● [Tab] 📤 Push to Cloud "
            } else {
                " ○ [Tab] 📤 Push to Cloud "
            },
            push_style,
        ),
        Span::raw(" │ "),
        Span::styled(
            if is_push {
                " ○ [Tab] 📥 Pull from Cloud "
            } else {
                " ● [Tab] 📥 Pull from Cloud "
            },
            pull_style,
        ),
        Span::raw("   "),
        Span::styled(
            format!(" Filter: [f] {} ", state.filter.label()),
            Style::default().fg(Color::Yellow),
        ),
    ])
}

fn build_storage_line(state: &AppState) -> Line<'static> {
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
                format!("Selected to Offload: {sel_str} (+{sel_str} freed) "),
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

fn render_main_body(f: &mut Frame, area: Rect, state: &AppState) {
    let indices = state.filtered_indices();
    let total = indices.len();
    let items = state.current_items();
    let selected_mask = match state.direction {
        TransferDirection::Push => &state.local_selected,
        TransferDirection::Pull => &state.remote_selected,
    };

    let total_selected_bytes = state.total_selected_bytes();
    let title = format!(
        " {} ({} titles) — Selected: {} ",
        if state.direction == TransferDirection::Push {
            "Local SSD Media"
        } else {
            "Google Drive Archive"
        },
        total,
        disk::format_bytes(total_selected_bytes)
    );

    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll_offset = if state.cursor >= inner_height {
        state.cursor - inner_height + 1
    } else {
        0
    };

    let visible_indices = indices.iter().skip(scroll_offset).take(inner_height);
    let mut lines = Vec::new();
    for (vis_pos, &idx) in visible_indices.enumerate() {
        let abs_pos = scroll_offset + vis_pos;
        let is_cursor = abs_pos == state.cursor;
        let is_checked = selected_mask.get(idx).copied().unwrap_or(false);
        if let Some(item) = items.get(idx) {
            lines.push(build_main_item_line(item, is_checked, is_cursor, state));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "   (No media items found matching current filter)",
            Style::default().fg(Color::DarkGray),
        )]));
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White));

    let body = Paragraph::new(lines).block(block);
    f.render_widget(body, area);
}

fn build_main_item_line<'a>(
    item: &'a MediaItem,
    checked: bool,
    cursor: bool,
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

    let check = build_check_span(item, checked);
    let (badge_text, badge_color) = match item.category {
        MediaCategory::Movie => ("[Movie] ", Color::Cyan),
        MediaCategory::Show => ("[Show]  ", Color::Yellow),
        MediaCategory::Anime => ("[Anime] ", Color::Magenta),
    };
    let badge = Span::styled(badge_text, Style::default().fg(badge_color));

    let title_style = if cursor {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let title = Span::styled(
        format!("{:<32} ", truncate_str(&item.title, 32)),
        title_style,
    );

    let season_badge = if item.has_seasons() {
        let sel = item.selected_seasons_count();
        let total = item.seasons.len();
        Span::styled(
            format!("[📁 {sel}/{total} ssn] "),
            Style::default().fg(Color::LightYellow),
        )
    } else {
        Span::raw("             ")
    };

    let display_bytes = item.selected_bytes(checked);
    let size_bytes = if display_bytes > 0 {
        display_bytes
    } else {
        item.size_bytes
    };
    let size_str = disk::format_bytes(size_bytes);
    let size = Span::styled(format!("{size_str:<10} "), Style::default().fg(Color::Cyan));

    let status = build_status_span(item, state);

    Line::from(vec![
        pointer,
        check,
        badge,
        title,
        season_badge,
        size,
        status,
    ])
}

fn build_check_span(item: &MediaItem, checked: bool) -> Span<'static> {
    if item.has_seasons() {
        if item.are_all_seasons_selected() {
            Span::styled(
                "[✓] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else if item.are_some_seasons_selected() {
            Span::styled(
                "[~] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
        }
    } else if checked {
        Span::styled(
            "[✓] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
    }
}

fn build_status_span(item: &MediaItem, state: &AppState) -> Span<'static> {
    if state.direction == TransferDirection::Pull {
        if let Some(disk) = state.local_disk {
            if item.size_bytes > disk.free_bytes {
                return Span::styled(
                    "🚫 No Space",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                );
            }
        }
    }

    if item.is_watched {
        Span::styled("👀 watched", Style::default().fg(Color::Green))
    } else {
        Span::styled("📦 unwatched", Style::default().fg(Color::DarkGray))
    }
}

fn render_main_footer(f: &mut Frame, area: Rect) {
    let shortcuts = [
        ("[Space]", "Toggle"),
        ("[Enter]", "Open Seasons"),
        ("[p]", "Proceed Transfer"),
        ("[Tab]", "Mode"),
        ("[f]", "Filter"),
        ("[q]", "Quit"),
    ];
    super::footer::render_footer(f, area, &shortcuts);
}
