use crate::tui::app::{Action, App};
use crate::tui::widgets::Row;

/// Two action rows (Stop, Restart) per service, following an info row.
const ROWS_PER_SERVICE: usize = 3;

pub fn rows(app: &App) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();

    for service in &app.services {
        let label = if service.container {
            service.name.as_str()
        } else {
            service.description.as_str()
        };
        let scope = if service.container {
            "docker"
        } else if service.is_user {
            "user"
        } else {
            "system"
        };

        rows.push(Row::info(label.to_string(), format!("{} · {scope}", service.state)));
        rows.push(Row::action(format!("Stop {label}")));
        rows.push(Row::action(format!("Restart {label}")));
    }

    if app.services.is_empty() {
        if app.is_refreshing() {
            rows.push(Row::info(
                "Loading managed services",
                "probing systemd and docker",
            ));
        } else {
            rows.push(Row::info(
                "No managed services detected",
                "install hooks or start Docker",
            ));
        }
    }
    rows.push(Row::divider());
    rows.push(Row::action("Refresh service list"));
    rows
}

pub fn activate(app: &mut App, index: usize) -> Action {
    let count = app.services.len();
    if count > 0 && index < count * ROWS_PER_SERVICE {
        let service = &app.services[index / ROWS_PER_SERVICE];
        let slot = index % ROWS_PER_SERVICE;
        if slot == 0 {
            return Action::None; // info row
        }
        let stop = slot == 1;
        if service.container {
            return if stop {
                Action::StopContainer(service.name.clone())
            } else {
                Action::RestartContainer(service.name.clone())
            };
        }
        return if stop {
            Action::StopService {
                name: service.name.clone(),
                is_user: service.is_user,
            }
        } else {
            Action::RestartService {
                name: service.name.clone(),
                is_user: service.is_user,
            }
        };
    }
    if index + 1 == rows(app).len() {
        return Action::Refresh;
    }
    Action::None
}
