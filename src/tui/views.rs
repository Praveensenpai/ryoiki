pub mod battery;
pub mod dashboard;
pub mod media;
pub mod notifications;
pub mod provision;
pub mod services;
pub mod settings;
pub mod updates;

use ratatui::{layout::Rect, Frame};

use crate::tui::app::{Action, App, Focus, Tab};
use crate::tui::widgets::{self, Row};

/// Rows for the active tab.
#[must_use]
pub fn rows(app: &App) -> Vec<Row> {
    match app.tab {
        Tab::Dashboard => dashboard::rows(app),
        Tab::Provision => provision::rows(app),
        Tab::Battery => battery::rows(app),
        Tab::Notifications => notifications::rows(app),
        Tab::Services => services::rows(app),
        Tab::Media => media::rows(app),
        Tab::Updates => updates::rows(app),
        Tab::Settings => settings::rows(app),
    }
}

/// Draws the content pane for the active tab.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let rows = rows(app);
    widgets::render_rows(
        f,
        area,
        &rows,
        app.cursor(),
        app.focus == Focus::Content,
        app.tab.title(),
        app.tab.subtitle(),
    );
}

/// Maps Enter/Space on the current row to an action.
#[must_use]
pub fn activate(app: &mut App, index: usize) -> Action {
    match app.tab {
        Tab::Dashboard => dashboard::activate(app, index),
        Tab::Provision => provision::activate(app, index),
        Tab::Battery => battery::activate(app, index),
        Tab::Notifications => notifications::activate(app, index),
        Tab::Services => services::activate(app, index),
        Tab::Media => media::activate(app, index),
        Tab::Updates => updates::activate(app, index),
        Tab::Settings => settings::activate(app, index),
    }
}

/// Maps Left/Right on steppers to adjustments.
pub fn adjust(app: &mut App, index: usize, delta: i32) {
    match app.tab {
        Tab::Battery => battery::adjust(app, index, delta),
        Tab::Media => media::adjust(app, index, delta),
        Tab::Settings => settings::adjust(app, index, delta),
        _ => {}
    }
}

/// First selectable row on the active tab.
#[must_use]
pub fn first_selectable(app: &App) -> usize {
    rows(app).iter().position(Row::is_selectable).unwrap_or(0)
}

/// Moves the cursor among selectable rows, wrapping at the edges.
pub fn move_cursor(app: &mut App, forward: bool) {
    let all = rows(app);
    let selectable: Vec<usize> = all
        .iter()
        .enumerate()
        .filter(|(_, row)| row.is_selectable())
        .map(|(index, _)| index)
        .collect();

    if selectable.is_empty() {
        app.set_cursor(0);
        return;
    }

    let current = app.cursor();
    let position = selectable.iter().position(|index| *index == current);
    let next = match position {
        Some(pos) if forward => selectable[(pos + 1) % selectable.len()],
        Some(pos) => selectable[(pos + selectable.len() - 1) % selectable.len()],
        None => selectable[0],
    };
    app.set_cursor(next);
}

/// Snaps the cursor to the first selectable row when it lands on decoration.
pub fn ensure_cursor(app: &mut App) {
    let all = rows(app);
    if all.get(app.cursor()).is_some_and(Row::is_selectable) {
        return;
    }
    app.set_cursor(first_selectable(app));
}
