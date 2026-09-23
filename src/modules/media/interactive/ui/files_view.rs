use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use super::truncate_str;
use crate::modules::media::disk;
use crate::modules::media::interactive::{AppState, ViewMode};
use crate::modules::media::transfer::{MediaFile, TransferDirection};

pub fn render_files_ui(
    f: &mut Frame,
    state: &AppState,
    item_idx: usize,
    season_idx: Option<usize>,
    cursor: usize,
) {
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

    let (title, files, sync_status) = if let Some(s_idx) = season_idx {
        if let Some(season) = item.seasons.get(s_idx) {
            (
                format!("{} > {}", item.title, season.title),
                &season.files,
                season.sync_status,
            )
        } else {
            return;
        }
    } else {
        (item.title.clone(), &item.files, item.sync_status)
    };

    render_files_header(
        f,
        chunks[0],
        &title,
        files.len(),
        sync_status,
        state.direction,
    );
    render_files_body(f, chunks[1], &title, files, cursor, state.direction);
    render_files_footer(f, chunks[2]);
}

fn render_files_header(
    f: &mut Frame,
    area: Rect,
    title: &str,
    total_files: usize,
    sync_status: crate::modules::media::transfer::SyncStatus,
    dir: TransferDirection,
) {
    let total_sz = disk::format_bytes(sync_status.total_bytes);
    let line1 = match dir {
        TransferDirection::Push => Line::from(vec![
            Span::raw(" Total: "),
            Span::styled(
                format!("{total_files} files ({total_sz})"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" │ In Cloud: "),
            Span::styled(
                format!("{} files", sync_status.other_files),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" │ Local Only: "),
            Span::styled(
                format!(
                    "{} files",
                    total_files.saturating_sub(sync_status.other_files)
                ),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        TransferDirection::Pull => {
            let miss_sz = disk::format_bytes(sync_status.missing_bytes);
            let missing_files = total_files.saturating_sub(sync_status.other_files);
            Line::from(vec![
                Span::raw(" Total: "),
                Span::styled(
                    format!("{total_files} files ({total_sz})"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" │ On SSD: "),
                Span::styled(
                    format!("{} files", sync_status.other_files),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" │ Need Download: "),
                Span::styled(
                    format!("{missing_files} files ({miss_sz})"),
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        }
    };

    let block = Block::default()
        .title(format!(" 📄 Files in {title} "))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let para = Paragraph::new(vec![line1])
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(para, area);
}

fn render_files_body(
    f: &mut Frame,
    area: Rect,
    title: &str,
    files: &[MediaFile],
    cursor: usize,
    dir: TransferDirection,
) {
    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll_offset = if cursor >= inner_height {
        cursor - inner_height + 1
    } else {
        0
    };

    let visible = files
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(inner_height);
    let mut lines = Vec::new();

    for (vis_idx, (abs_idx, file)) in visible.enumerate() {
        let is_cursor = (scroll_offset + vis_idx) == cursor;
        lines.push(build_file_line(file, is_cursor, abs_idx + 1, dir));
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "   (No media files found in this folder)",
            Style::default().fg(Color::DarkGray),
        )]));
    }

    let block = Block::default()
        .title(format!(" Breakdown for {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White));

    let body = Paragraph::new(lines).block(block);
    f.render_widget(body, area);
}

fn build_file_line(file: &MediaFile, cursor: bool, num: usize, dir: TransferDirection) -> Line<'_> {
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

    let badge = match dir {
        TransferDirection::Push => {
            if file.exists_in_other {
                Span::styled("[☁️ In Cloud] ", Style::default().fg(Color::Green))
            } else {
                Span::styled("[💾 SSD Only] ", Style::default().fg(Color::Cyan))
            }
        }
        TransferDirection::Pull => {
            if file.exists_in_other {
                Span::styled("[💾 On SSD]   ", Style::default().fg(Color::Green))
            } else {
                Span::styled("[☁️ Missing]  ", Style::default().fg(Color::Yellow))
            }
        }
    };

    let title_style = if cursor {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let title = Span::styled(
        format!("{num:02}. {:<50} ", truncate_str(&file.name, 50)),
        title_style,
    );

    let sz_str = disk::format_bytes(file.size_bytes);
    let size = Span::styled(format!("{sz_str:<10}"), Style::default().fg(Color::Cyan));

    Line::from(vec![pointer, badge, title, size])
}

fn render_files_footer(f: &mut Frame, area: Rect) {
    let shortcuts = [
        ("[Esc]/[q]/[Enter]", "Back to List"),
        ("[↑/k, ↓/j]", "Navigate"),
    ];
    super::footer::render_footer(f, area, &shortcuts);
}

pub fn handle_files_key(
    code: KeyCode,
    state: &mut AppState,
    item_idx: usize,
    season_idx: Option<usize>,
    mut cursor: usize,
) -> Option<bool> {
    let files_len = {
        let item = state.current_items().get(item_idx)?;
        match season_idx {
            Some(s_idx) => item.seasons.get(s_idx).map_or(0, |s| s.files.len()),
            None => item.files.len(),
        }
    };

    match code {
        KeyCode::Up | KeyCode::Char('k') if files_len > 0 => {
            cursor = if cursor > 0 {
                cursor - 1
            } else {
                files_len - 1
            };
            state.view_mode = ViewMode::FilesView {
                item_idx,
                season_idx,
                cursor,
            };
            None
        }
        KeyCode::Down | KeyCode::Char('j') if files_len > 0 => {
            cursor = if cursor + 1 < files_len {
                cursor + 1
            } else {
                0
            };
            state.view_mode = ViewMode::FilesView {
                item_idx,
                season_idx,
                cursor,
            };
            None
        }
        KeyCode::Enter | KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') => {
            state.view_mode = match season_idx {
                Some(s_idx) => ViewMode::SubView {
                    item_idx,
                    cursor: s_idx,
                },
                None => ViewMode::Main,
            };
            None
        }
        _ => None,
    }
}
