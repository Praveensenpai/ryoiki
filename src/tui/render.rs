use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::{App, Confirm, Focus, Tab, TextInput, ToastLevel};
use crate::tui::views;
use crate::tui::widgets::{centered_box, centered_rect, modal_frame, tail, theme, truncate};

/// Paints the whole interface for one frame.
pub fn draw(f: &mut Frame, app: &App) {
    let t = theme();
    f.render_widget(Block::default().style(Style::default().bg(t.bg)), f.area());

    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(4),
    ])
    .split(f.area());

    draw_header(f, layout[0], app);
    draw_body(f, layout[1], app);
    draw_footer(f, layout[2], app);

    if app.help {
        draw_help(f, f.area());
    }
    if let Some(confirm) = &app.confirm {
        draw_confirm(f, f.area(), confirm);
    }
    if let Some(input) = &app.input {
        draw_input(f, f.area(), input);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let t = theme();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border())
        .style(Style::default().bg(t.panel))
        .title(Line::from(vec![
            Span::styled(
                " 領域 ",
                Style::default().fg(t.mauve).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Ryoiki",
                Style::default().fg(t.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · Control Center ", Style::default().fg(t.overlay)),
        ]))
        .title_bottom(
            Line::from(vec![Span::styled(
                format!(" v{} ", app.version),
                Style::default().fg(t.overlay),
            )])
            .right_aligned(),
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    let battery = app
        .snapshot
        .battery_percent
        .map_or_else(|| "no battery".to_string(), |pct| format!("{pct}%"));
    let power_color = if app.snapshot.ac_online {
        t.green
    } else {
        t.yellow
    };
    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled("● ", Style::default().fg(t.green)),
        Span::styled(app.snapshot.host.clone(), Style::default().fg(t.text)),
        Span::styled("   │   ", Style::default().fg(t.surface)),
        Span::styled(
            app.snapshot.power_label().to_string(),
            Style::default().fg(power_color),
        ),
        Span::styled("   │   ", Style::default().fg(t.surface)),
        Span::styled(battery, Style::default().fg(t.teal)),
        Span::styled("   │   up ", Style::default().fg(t.overlay)),
        Span::styled(app.snapshot.uptime.clone(), Style::default().fg(t.subtext)),
        Span::styled("   │   ", Style::default().fg(t.surface)),
        Span::styled(
            app.tab.title().to_string(),
            Style::default().fg(t.mauve).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let columns = Layout::horizontal([Constraint::Length(22), Constraint::Min(30)]).split(area);
    draw_nav(f, columns[0], app);
    views::render(f, columns[1], app);
}

fn draw_nav(f: &mut Frame, area: Rect, app: &App) {
    let t = theme();
    let focused = app.focus == Focus::Nav;
    let border = if focused { t.mauve } else { t.surface };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(t.panel))
        .title(Line::from(vec![Span::styled(
            " Menu ",
            Style::default().fg(t.mauve).add_modifier(Modifier::BOLD),
        )]))
        .title_bottom(
            Line::from(vec![Span::styled(
                if focused { " focus " } else { " tab " },
                Style::default().fg(if focused { t.mauve } else { t.overlay }),
            )])
            .right_aligned(),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let width = usize::from(inner.width);
    let mut lines = vec![Line::from("")];
    for tab in Tab::ALL {
        let selected = tab == app.tab;
        let pointer = if selected { "▸ " } else { "  " };
        let label = format!("{} {}", tab.index() + 1, tab.title());
        let padding = width.saturating_sub(2 + label.chars().count());
        let style = if selected {
            Style::default()
                .fg(t.text)
                .bg(t.surface)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.subtext)
        };
        lines.push(Line::from(vec![
            Span::styled(
                pointer,
                Style::default().fg(if selected { t.mauve } else { t.surface }),
            ),
            Span::styled(format!("{label}{}", " ".repeat(padding)), style),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            app.settings.ui.refresh_secs.to_string(),
            Style::default().fg(t.blue),
        ),
        Span::styled("s auto-refresh", Style::default().fg(t.overlay)),
    ]));
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let t = theme();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.border())
        .style(Style::default().bg(t.panel))
        .title(Line::from(vec![Span::styled(
            " Keys ",
            Style::default().fg(t.mauve).add_modifier(Modifier::BOLD),
        )]));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let hints = if app.input.is_some() {
        "type value   ·   Enter save   ·   Esc cancel   ·   Ctrl-U clear"
    } else if app.confirm.is_some() {
        "y / Enter confirm   ·   n / Esc cancel"
    } else if app.help {
        "any key closes help"
    } else {
        "j/k move · h/l tabs · ←/→ adjust · Enter · Tab pane · ? help · q quit"
    };
    let (status, color) = status_line(app, t.overlay, t.yellow, t.green, t.red);
    let width = usize::from(inner.width);
    let lines = vec![
        Line::from(Span::styled(
            truncate(hints, width),
            Style::default().fg(t.overlay),
        )),
        Line::from(Span::styled(
            truncate(&status, width),
            Style::default().fg(color),
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn status_line(
    app: &App,
    overlay: ratatui::style::Color,
    yellow: ratatui::style::Color,
    green: ratatui::style::Color,
    red: ratatui::style::Color,
) -> (String, ratatui::style::Color) {
    if let Some(toast) = app.toast.as_ref().filter(|toast| toast.alive()) {
        let color = match toast.level {
            ToastLevel::Info => overlay,
            ToastLevel::Success => green,
            ToastLevel::Warn => yellow,
            ToastLevel::Error => red,
        };
        return (toast.message.clone(), color);
    }
    if let Some(input) = &app.input {
        return (format!("editing: {}", input.label), yellow);
    }
    if let Some(warning) = &app.settings_warning {
        return (warning.clone(), yellow);
    }
    (
        format!("settings: {}", crate::settings::settings_path().display()),
        overlay,
    )
}

fn draw_help(f: &mut Frame, area: Rect) {
    let t = theme();
    let popup = centered_rect(74, 82, area);
    let inner = modal_frame(f, popup, " Help ", t.mauve);
    let key = Style::default().fg(t.blue);
    let section = Style::default().fg(t.yellow).add_modifier(Modifier::BOLD);
    let text = Style::default().fg(t.subtext);

    let lines = vec![
        Line::from(Span::styled(
            "領域 Ryoiki · Control Center",
            Style::default().fg(t.mauve).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("Navigate", section)),
        Line::from(vec![
            Span::styled("  j/k move · h/l · 1-8", key),
            Span::styled("  change section / move", text),
        ]),
        Line::from(vec![
            Span::styled("  Enter · Tab · r · s", key),
            Span::styled("  activate · pane · refresh · save", text),
        ]),
        Line::from(vec![
            Span::styled("  ? · q / Ctrl-C", key),
            Span::styled("  help · quit", text),
        ]),
        Line::from(""),
        Line::from(Span::styled("Battery safety", section)),
        Line::from(Span::styled(
            "  Ceiling writes to the controller. Auto poweroff fires",
            text,
        )),
        Line::from(Span::styled(
            "  once per discharge cycle below the threshold.",
            text,
        )),
        Line::from(""),
        Line::from(Span::styled("Files", section)),
        Line::from(Span::styled(
            "  settings  ~/.config/ryoiki/settings.json",
            text,
        )),
        Line::from(Span::styled(
            "  telegram  ~/.config/ryoiki/telegram.json",
            text,
        )),
        Line::from(Span::styled("  logs      ~/.local/state/ryoiki/", text)),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_confirm(f: &mut Frame, area: Rect, confirm: &Confirm) {
    let t = theme();
    let popup = centered_box(66, 7, area);
    let title = format!(" {} ", truncate(&confirm.title, 50));
    let inner = modal_frame(f, popup, &title, t.yellow);

    let mut lines: Vec<Line> = confirm
        .body
        .lines()
        .map(|line| Line::from(Span::styled(line.to_string(), Style::default().fg(t.text))))
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "y / Enter confirm   ·   n / Esc cancel",
        Style::default().fg(t.overlay),
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_input(f: &mut Frame, area: Rect, input: &TextInput) {
    let t = theme();
    let popup = centered_box(72, 7, area);
    let inner = modal_frame(f, popup, " Edit value ", t.mauve);
    let width = usize::from(inner.width).saturating_sub(2);
    let value = tail(&input.value, width);

    let lines = vec![
        Line::from(Span::styled(
            input.label.clone(),
            Style::default().fg(t.subtext),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(" "),
            Span::styled(value, Style::default().fg(t.text)),
            Span::styled("█", Style::default().fg(t.mauve)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter save   ·   Esc cancel   ·   Ctrl-U clear",
            Style::default().fg(t.overlay),
        )),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
