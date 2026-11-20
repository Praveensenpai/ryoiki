use crate::tui::app::{Action, App};
use crate::tui::widgets::Row;

pub fn rows(app: &App) -> Vec<Row> {
    let mut rows: Vec<Row> = app
        .services
        .iter()
        .map(|service| {
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
            Row::action_state(label, format!("{} · {scope}", service.state))
        })
        .collect();

    if rows.is_empty() {
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
    if index < app.services.len() {
        let service = &app.services[index];
        if service.container {
            return Action::StopContainer(service.name.clone());
        }
        return Action::StopService {
            name: service.name.clone(),
            is_user: service.is_user,
        };
    }
    if index + 1 == rows(app).len() {
        return Action::Refresh;
    }
    Action::None
}
