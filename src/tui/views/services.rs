use crate::tui::app::{Action, App, Choice, ChoiceOption};
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
            Row::action_state(label, format!("{} · {} · enter ↵", service.state, scope))
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
        let name = service.name.clone();
        let is_user = service.is_user;
        let container = service.container;
        let label = if container {
            name.clone()
        } else {
            service.description.clone()
        };

        let restart = if container {
            Action::RestartContainer(name.clone())
        } else {
            Action::RestartService {
                name: name.clone(),
                is_user,
            }
        };
        let stop = if container {
            Action::StopContainer(name.clone())
        } else {
            Action::StopService {
                name: name.clone(),
                is_user,
            }
        };

        app.choice = Some(Choice {
            title: format!("Action for {label}"),
            options: vec![
                ChoiceOption {
                    label: format!("Restart {label}"),
                    action: restart,
                },
                ChoiceOption {
                    label: format!("Stop {label}"),
                    action: stop,
                },
                ChoiceOption {
                    label: "Cancel".to_string(),
                    action: Action::None,
                },
            ],
            index: 0,
        });
        return Action::None;
    }
    if index + 1 == rows(app).len() {
        return Action::Refresh;
    }
    Action::None
}
