use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;

use crate::modules::{get_available_modules, Module};

/// Launches an interactive terminal checklist TUI for selecting setup modules.
pub fn select_modules() -> Result<Option<Vec<String>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let modules = get_available_modules();
    let mut selected: Vec<bool> = modules.iter().map(|m| m.default_enabled).collect();
    let mut cursor = 0;

    let res = run_app(&mut terminal, &modules, &mut selected, &mut cursor);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if res? {
        let chosen = modules
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| selected[*idx])
            .map(|(_, m)| m.id.to_string())
            .collect();
        Ok(Some(chosen))
    } else {
        Ok(None)
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    modules: &[Module],
    selected: &mut [bool],
    cursor: &mut usize,
) -> Result<bool> {
    loop {
        terminal.draw(|f| render_ui(f, modules, selected, *cursor))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if let Some(action) = handle_key(key.code, modules.len(), selected, cursor) {
                    return Ok(action);
                }
            }
        }
    }
}

fn render_ui(f: &mut Frame, modules: &[Module], selected: &[bool], cursor: usize) {
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(f.area());

    // 1. Header
    let title = Line::from(vec![
        Span::styled(
            " 領域 ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Ryoiki ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "— Minimal Ubuntu Server Setup",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let header = Paragraph::new(title).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(header, outer_chunks[0]);

    // 2. Module Checklist
    let lines = build_module_lines(modules, selected, cursor);
    let body = Paragraph::new(lines).block(
        Block::default()
            .title(" Select Modules ")
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(body, outer_chunks[1]);

    // 3. Footer / Help
    let footer_text = Line::from(vec![
        Span::styled(
            "[Space] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Toggle  "),
        Span::styled(
            "[a] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("All  "),
        Span::styled(
            "[Enter] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Start  "),
        Span::styled(
            "[q] ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Quit"),
    ]);
    let footer = Paragraph::new(footer_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(footer, outer_chunks[2]);
}

fn build_module_lines<'a>(
    modules: &'a [Module],
    selected: &[bool],
    cursor: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    for (idx, module) in modules.iter().enumerate() {
        let is_cursor = idx == cursor;
        let is_checked = selected[idx];

        let pointer = if is_cursor {
            Span::styled(
                " ▶ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("   ")
        };

        let check = if is_checked {
            Span::styled(
                "[✓] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
        };

        let name = if is_cursor {
            Span::styled(
                format!("{:<24}", module.title),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!("{:<24}", module.title),
                Style::default().fg(Color::White),
            )
        };

        let desc = Span::styled(
            format!(" {}", module.description),
            Style::default().fg(Color::DarkGray),
        );

        lines.push(Line::from(vec![pointer, check, name, desc]));
    }
    lines
}

fn handle_key(
    code: KeyCode,
    modules_len: usize,
    selected: &mut [bool],
    cursor: &mut usize,
) -> Option<bool> {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            *cursor = if *cursor > 0 {
                *cursor - 1
            } else {
                modules_len - 1
            };
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            *cursor = if *cursor + 1 < modules_len {
                *cursor + 1
            } else {
                0
            };
            None
        }
        KeyCode::Char(' ') => {
            selected[*cursor] = !selected[*cursor];
            None
        }
        KeyCode::Char('a') => {
            let all_selected = selected.iter().all(|&s| s);
            for item in selected.iter_mut() {
                *item = !all_selected;
            }
            None
        }
        KeyCode::Enter => Some(true),
        KeyCode::Char('q') | KeyCode::Esc => Some(false),
        _ => None,
    }
}
