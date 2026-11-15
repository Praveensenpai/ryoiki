use crate::settings::{MAX_CHARGE, MAX_POWEROFF, MIN_CHARGE, MIN_POWEROFF};
use crate::tui::app::{Action, App, Confirm, InputTarget};
use crate::tui::widgets::{bar, step, Row};

pub fn rows(app: &App) -> Vec<Row> {
    let battery = &app.settings.battery;
    let power = &app.settings.power;
    let thresholds = battery
        .alert_thresholds
        .iter()
        .map(|value| format!("{value}%"))
        .collect::<Vec<_>>()
        .join(" · ");
    let now = app.snapshot.battery_percent.map_or_else(
        || "n/a".to_string(),
        |pct| format!("{}  {pct}%  {}", bar(pct, 7), app.snapshot.battery_status),
    );
    let watcher_active = matches!(app.snapshot.watcher_state.as_str(), "active" | "activating");

    vec![
        Row::toggle("Max charge ceiling", battery.max_charge_enabled),
        Row::stepper("Ceiling percent", battery.max_charge_percent, "%"),
        Row::divider(),
        Row::toggle("Auto poweroff", power.poweroff_on_low),
        Row::stepper("Shutdown at", power.poweroff_percent, "%"),
        Row::divider(),
        Row::action("Apply ceiling to hardware"),
        Row::action("Restart battery watcher"),
        Row::action(if watcher_active {
            "Stop battery watcher"
        } else {
            "Start battery watcher"
        }),
        Row::action("Edit alert thresholds"),
        Row::action("Reset alert thresholds"),
        Row::info("Alert thresholds", thresholds),
        Row::info("Charge backend", app.snapshot.charge_backend.as_str()),
        Row::info("Battery now", now),
    ]
}

pub fn activate(app: &mut App, index: usize) -> Action {
    match index {
        0 => {
            app.settings.battery.max_charge_enabled = !app.settings.battery.max_charge_enabled;
            app.save_settings();
            Action::None
        }
        1 | 4 => Action::SaveSettings,
        3 => {
            if app.settings.power.poweroff_on_low {
                app.settings.power.poweroff_on_low = false;
                app.save_settings();
                Action::None
            } else {
                let pct = app.settings.power.poweroff_percent;
                app.confirm = Some(Confirm {
                    title: "Enable auto poweroff".to_string(),
                    body: format!(
                        "Power off the host when the battery reaches {pct}% while on battery?\n\
                         It fires once per discharge cycle. Plugging in resets it."
                    ),
                    action: Action::SetPoweroffEnabled(true),
                });
                Action::None
            }
        }
        6 => {
            let limit = app.settings.battery.max_charge_percent;
            app.confirm = Some(Confirm {
                title: "Apply charge ceiling".to_string(),
                body: format!(
                    "Write a {limit}% charge ceiling to the battery controller?\n\
                     This may prompt for sudo."
                ),
                action: Action::ApplyChargeLimit,
            });
            Action::None
        }
        7 => Action::RestartWatcher,
        8 => {
            if matches!(app.snapshot.watcher_state.as_str(), "active" | "activating") {
                Action::DisableWatcher
            } else {
                Action::EnableWatcher
            }
        }
        9 => Action::Edit(InputTarget::AlertThresholds),
        10 => Action::ResetAlerts,
        _ => Action::None,
    }
}

pub fn adjust(app: &mut App, index: usize, delta: i32) {
    match index {
        1 => {
            app.settings.battery.max_charge_percent = step(
                app.settings.battery.max_charge_percent,
                delta,
                5,
                MIN_CHARGE,
                MAX_CHARGE,
            );
            app.save_settings();
        }
        4 => {
            app.settings.power.poweroff_percent = step(
                app.settings.power.poweroff_percent,
                delta,
                1,
                MIN_POWEROFF,
                MAX_POWEROFF,
            );
            app.save_settings();
        }
        _ => {}
    }
}
