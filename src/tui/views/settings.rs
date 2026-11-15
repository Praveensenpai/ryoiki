use std::path::Path;

use crate::tui::app::{Action, App, Confirm, InputTarget};
use crate::tui::widgets::{step, Row};

pub fn rows(app: &App) -> Vec<Row> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let log = Path::new(&home).join(".local/state/ryoiki/install.log");
    let mut rows = vec![
        Row::info("Host", app.snapshot.host.as_str()),
        Row::info(
            "Settings file",
            crate::settings::settings_path().display().to_string(),
        ),
        Row::info(
            "Config dir",
            crate::settings::config_dir().display().to_string(),
        ),
        Row::info("Log file", log.display().to_string()),
        Row::info("Version", format!("v{}", app.version)),
        Row::divider(),
        Row::stepper("UI refresh", app.settings.ui.refresh_secs, "s"),
        Row::action("Save settings"),
        Row::action("Reload from disk"),
        Row::action("Edit notification timezone"),
        Row::action("Clear resume state"),
    ];
    if let Some(warning) = &app.settings_warning {
        rows.push(Row::info("Warning", warning.as_str()));
    }
    rows
}

pub fn activate(app: &mut App, index: usize) -> Action {
    match index {
        7 => Action::SaveSettings,
        8 => Action::ReloadSettings,
        9 => Action::Edit(InputTarget::Timezone),
        10 => {
            app.confirm = Some(Confirm {
                title: "Clear resume state".to_string(),
                body: "Delete the interrupted provisioning checkpoint?\n\
                       The next CLI install starts from scratch."
                    .to_string(),
                action: Action::ClearResume,
            });
            Action::None
        }
        _ => Action::None,
    }
}

pub fn adjust(app: &mut App, index: usize, delta: i32) {
    if index != 6 {
        return;
    }
    app.settings.ui.refresh_secs = step(app.settings.ui.refresh_secs, delta, 1, 1, 60);
    app.save_settings();
}
