use crate::tui::app::{Action, App, Confirm};
use crate::tui::widgets::Row;

pub fn rows(app: &App) -> Vec<Row> {
    let latest = app
        .latest_version
        .clone()
        .unwrap_or_else(|| "not checked".to_string());
    let binary = std::env::current_exe()
        .map_or_else(|_| "unknown".to_string(), |path| path.display().to_string());

    vec![
        Row::info("Installed", format!("v{}", app.version)),
        Row::info("Latest release", latest),
        Row::info("Status", app.update_status.as_str()),
        Row::info("Binary", binary),
        Row::divider(),
        Row::action("Check GitHub releases"),
        Row::action("Install latest release"),
        Row::info("Note", "the running binary is replaced in place"),
    ]
}

pub fn activate(app: &mut App, index: usize) -> Action {
    match index {
        5 => Action::CheckUpdate,
        6 => {
            app.confirm = Some(Confirm {
                title: "Install update".to_string(),
                body: "Download and replace the ryoiki binary with the latest release?\n\
                       Restart ryoiki afterwards."
                    .to_string(),
                action: Action::SelfUpdate,
            });
            Action::None
        }
        _ => Action::None,
    }
}
