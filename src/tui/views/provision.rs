use crate::tui::app::{Action, App, Confirm};
use crate::tui::widgets::Row;

pub fn rows(app: &App) -> Vec<Row> {
    let mut rows: Vec<Row> = app
        .provision
        .iter()
        .map(|module| {
            let label = if module.installed {
                format!("✓ {}", module.title)
            } else {
                format!("  {}", module.title)
            };
            Row::toggle(label, module.selected)
        })
        .collect();

    let selected = app
        .provision
        .iter()
        .filter(|module| module.selected)
        .count();
    let installed = app
        .provision
        .iter()
        .filter(|module| module.installed)
        .count();
    rows.push(Row::action_state(
        format!("Run {selected} selected"),
        format!("{installed}/{} installed", app.provision.len()),
    ));
    rows
}

/// Bottom-of-panel note describing the module under the cursor.
#[must_use]
pub fn note(app: &App) -> String {
    let cursor = app.cursor();
    if let Some(module) = app.provision.get(cursor) {
        return module.description.clone();
    }
    if cursor == app.provision.len() {
        return "Enter runs the selected modules (dependencies included)".to_string();
    }
    "select a module to see details".to_string()
}

pub fn activate(app: &mut App, index: usize) -> Action {
    let count = app.provision.len();
    if index < count {
        app.provision[index].selected = !app.provision[index].selected;
        return Action::None;
    }
    if index != count {
        return Action::None;
    }

    let selected = app
        .provision
        .iter()
        .filter(|module| module.selected)
        .count();
    if selected == 0 {
        app.toast(
            "select at least one module",
            crate::tui::app::ToastLevel::Warn,
        );
        return Action::None;
    }
    app.confirm = Some(Confirm {
        title: "Run provisioning".to_string(),
        body: format!(
            "Install {selected} selected module(s) plus dependencies?\n\
             This may prompt for sudo and can take a while."
        ),
        action: Action::RunProvision,
    });
    Action::None
}
