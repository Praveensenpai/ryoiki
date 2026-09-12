use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub fn render_footer(f: &mut Frame, area: Rect, shortcuts: &[(&str, &str)]) {
    let mut spans = Vec::new();
    for (k, v) in shortcuts {
        spans.push(Span::styled(
            *k,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {v} ")));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let footer = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(footer, area);
}
