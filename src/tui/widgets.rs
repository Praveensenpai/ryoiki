use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

mod theme;
mod util;
pub use theme::{theme, Theme};
pub use util::{bar, centered_box, centered_rect, step, tail, truncate};

/// What kind of control a row represents.
#[derive(Clone, Debug)]
pub enum RowKind {
    Info,
    Divider,
    Stepper { value: u8, suffix: &'static str },
    Toggle { on: bool },
    Action,
}

/// A single line in the content pane.
#[derive(Clone, Debug)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub kind: RowKind,
}

const LABEL_WIDTH: usize = 30;

impl Row {
    #[must_use]
    pub fn info(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: RowKind::Info,
        }
    }

    #[must_use]
    pub fn divider() -> Self {
        Self {
            label: String::new(),
            value: String::new(),
            kind: RowKind::Divider,
        }
    }

    #[must_use]
    pub fn action(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            kind: RowKind::Action,
        }
    }

    #[must_use]
    pub fn action_state(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: RowKind::Action,
        }
    }

    #[must_use]
    pub fn stepper(label: impl Into<String>, value: u8, suffix: &'static str) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            kind: RowKind::Stepper { value, suffix },
        }
    }

    #[must_use]
    pub fn toggle(label: impl Into<String>, on: bool) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            kind: RowKind::Toggle { on },
        }
    }

    #[must_use]
    pub fn is_selectable(&self) -> bool {
        !matches!(self.kind, RowKind::Info | RowKind::Divider)
    }
}

/// Statics for one content panel render.
pub struct Pane<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub note: Option<String>,
    pub active: bool,
}

/// Draws the content panel and its rows, scrolling to keep the cursor visible.
pub fn render_rows(f: &mut Frame, area: Rect, rows: &[Row], cursor: usize, pane: Pane<'_>) {
    let t = theme();
    let width = usize::from(area.width.saturating_sub(2));
    let note = pane.note.map(|n| truncate(&n, width.saturating_sub(14)));
    let block = content_block(pane.title, pane.subtitle, rows.len(), note.as_deref(), t);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let width = usize::from(inner.width);
    let height = usize::from(inner.height);
    let last_selectable = rows.iter().rposition(Row::is_selectable).unwrap_or(0);
    let offset = scroll_offset(cursor, height, rows.len(), last_selectable);
    let (label_width, value_max) = row_metrics(rows, width);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(idx, row)| {
            render_row(
                row,
                idx == cursor && pane.active,
                width,
                label_width,
                value_max,
                t,
            )
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn content_block(
    title: &str,
    subtitle: &str,
    count: usize,
    note: Option<&str>,
    t: Theme,
) -> Block<'static> {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border())
        .style(Style::default().bg(t.panel))
        .title(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                title.to_string(),
                Style::default().fg(t.mauve).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {subtitle} "), Style::default().fg(t.overlay)),
        ]))
        .title_bottom(
            Line::from(vec![Span::styled(
                format!(" {count} rows "),
                Style::default().fg(t.overlay),
            )])
            .right_aligned(),
        );
    if let Some(note) = note {
        block = block.title_bottom(Line::from(vec![Span::styled(
            format!(" {note} "),
            Style::default().fg(t.subtext),
        )]));
    }
    block
}

fn scroll_offset(cursor: usize, height: usize, total: usize, last_selectable: usize) -> usize {
    if total <= height {
        return 0;
    }
    let max_offset = total - height;
    let min_offset = cursor.saturating_add(1).saturating_sub(height);
    // On the last selectable row, anchor the view to the end so trailing
    // info rows are revealed too; the cursor stays visible.
    if cursor == last_selectable && max_offset > min_offset && max_offset <= cursor {
        max_offset
    } else {
        min_offset.min(max_offset)
    }
}

fn row_metrics(rows: &[Row], width: usize) -> (usize, usize) {
    let max_label = rows
        .iter()
        .map(|row| row.label.chars().count())
        .max()
        .unwrap_or(0);
    let floor = if width >= 90 { 24 } else { 16 };
    let label_width = max_label
        .clamp(floor, LABEL_WIDTH)
        .min(width.saturating_sub(10));
    let value_max = width.saturating_sub(3 + label_width);
    (label_width, value_max)
}

fn render_row(
    row: &Row,
    selected: bool,
    width: usize,
    label_width: usize,
    value_max: usize,
    t: Theme,
) -> Line<'static> {
    if matches!(row.kind, RowKind::Divider) {
        return Line::from(Span::styled(
            "─".repeat(width),
            Style::default().fg(t.surface),
        ));
    }

    let pointer = if selected { "▸ " } else { "  " };
    let label_style = if selected {
        Style::default().fg(t.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.subtext)
    };
    let label = truncate(&row.label, label_width);

    let mut spans = vec![
        Span::styled(pointer, Style::default().fg(t.mauve)),
        Span::styled(format!("{label:<label_width$}"), label_style),
        Span::styled(" ", Style::default()),
    ];
    let value = value_spans(row, selected, t, value_max);
    let value_len = value
        .iter()
        .map(|span| span.content.chars().count())
        .sum::<usize>();
    spans.extend(value);

    let used = 2 + label_width + 1 + value_len;
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));

    let line = Line::from(spans);
    if selected {
        line.style(Style::default().bg(t.surface))
    } else {
        line
    }
}

fn value_spans(row: &Row, selected: bool, t: Theme, max: usize) -> Vec<Span<'static>> {
    match &row.kind {
        RowKind::Info => vec![Span::styled(
            truncate(&row.value, max),
            Style::default().fg(t.subtext),
        )],
        RowKind::Divider => Vec::new(),
        RowKind::Stepper { value, suffix } => {
            let arrow = if selected { t.mauve } else { t.overlay };
            vec![
                Span::styled("◀  ", Style::default().fg(arrow)),
                Span::styled(
                    format!("{value}{suffix}"),
                    Style::default().fg(t.blue).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ▶", Style::default().fg(arrow)),
            ]
        }
        RowKind::Toggle { on } => {
            if *on {
                vec![Span::styled(
                    "● on",
                    Style::default().fg(t.green).add_modifier(Modifier::BOLD),
                )]
            } else {
                vec![Span::styled("○ off", Style::default().fg(t.overlay))]
            }
        }
        RowKind::Action => {
            if row.value.is_empty() {
                vec![Span::styled("enter ↵", Style::default().fg(t.peach))]
            } else {
                vec![Span::styled(
                    truncate(&row.value, max),
                    Style::default().fg(t.peach),
                )]
            }
        }
    }
}

/// Draws a rounded modal frame and returns the inner area.
pub fn modal_frame(f: &mut Frame, area: Rect, title: &str, accent: Color) -> Rect {
    let t = theme();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(t.panel))
        .title(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                title.to_string(),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default()),
        ]))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);
    inner
}
