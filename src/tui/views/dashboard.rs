use crate::tui::app::{Action, App};
use crate::tui::widgets::{bar, Row};

pub fn rows(app: &App) -> Vec<Row> {
    let snapshot = &app.snapshot;
    let battery = snapshot.battery_percent.map_or_else(
        || "no battery".to_string(),
        |pct| format!("{}  {pct}%", bar(pct, 10)),
    );
    let cutoff = if app.settings.power.poweroff_on_low {
        format!("on at {}%", app.settings.power.poweroff_percent)
    } else {
        "off".to_string()
    };
    let watcher = format!(
        "{} · {}",
        snapshot.watcher_state,
        if snapshot.watcher_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );

    vec![
        Row::info("Host", snapshot.host.as_str()),
        Row::info("Kernel", snapshot.kernel.as_str()),
        Row::info("Uptime", snapshot.uptime.as_str()),
        Row::info("CPU", snapshot.cpu.as_str()),
        Row::info("Memory", snapshot.memory.as_str()),
        Row::info("Disk /", snapshot.disk.as_str()),
        Row::info("Battery", format!("{battery}  {}", snapshot.battery_status)),
        Row::info("Charge backend", snapshot.charge_backend.as_str()),
        Row::info(
            "Max charge",
            format!("{}%", app.settings.battery.max_charge_percent),
        ),
        Row::info("Auto poweroff", cutoff),
        Row::info("Watcher", watcher),
        Row::divider(),
        Row::action("Refresh snapshot"),
    ]
}

pub fn activate(app: &mut App, index: usize) -> Action {
    if index == rows(app).len().saturating_sub(1) {
        Action::Refresh
    } else {
        Action::None
    }
}
