use std::path::Path;

use crate::modules::media::disk::{format_bytes, get_disk_usage};
use crate::modules::media::organizer::pathing::get_jellyfin_media_dir;
use crate::settings::{MAX_PRUNE, MIN_PRUNE, MIN_PRUNE_GAP, MIN_PRUNE_TARGET};
use crate::tui::app::{Action, App, Confirm};
use crate::tui::widgets::{step, Row};

pub fn rows(app: &App) -> Vec<Row> {
    let library = get_jellyfin_media_dir();
    let state = if library.exists() {
        "present"
    } else {
        "missing"
    };
    let disk = get_disk_usage(Path::new("/")).map_or_else(
        |_| "n/a".to_string(),
        |usage| {
            format!(
                "{} / {}  ({}%)",
                format_bytes(usage.used_bytes),
                format_bytes(usage.total_bytes),
                usage.used_pct
            )
        },
    );
    let storage = &app.settings.storage;
    let timer = if app.snapshot.prune_timer_active {
        "active"
    } else {
        "inactive"
    };

    vec![
        Row::info("Jellyfin library", library.display().to_string()),
        Row::info("Library state", state),
        Row::info("Root disk", disk),
        Row::divider(),
        Row::stepper("Auto-prune trigger", storage.prune_threshold, "%"),
        Row::stepper("Recover down to", storage.prune_target, "%"),
        Row::info("Auto-prune timer", timer),
        Row::action("Apply auto-prune policy"),
        Row::action("Disable auto-prune timer"),
        Row::divider(),
        Row::action("Open interactive media manager"),
        Row::action("Preview prune (dry-run)"),
        Row::action("Show storage report"),
        Row::action("Refresh storage"),
    ]
}

pub fn activate(app: &mut App, index: usize) -> Action {
    match index {
        7 => {
            let threshold = app.settings.storage.prune_threshold;
            let target = app.settings.storage.prune_target;
            app.confirm = Some(Confirm {
                title: "Apply auto-prune policy".to_string(),
                body: format!(
                    "Deploy the hourly prune timer at a {threshold}% trigger, recovering to {target}%?\n\
                     It runs as a systemd user timer; no sudo required."
                ),
                action: Action::ApplyPrunePolicy,
            });
            Action::None
        }
        8 => {
            app.confirm = Some(Confirm {
                title: "Disable auto-prune timer".to_string(),
                body: "Stop and disable the hourly media prune timer?\n\
                       The unit files stay on disk and can be reapplied later."
                    .to_string(),
                action: Action::DisablePruneTimer,
            });
            Action::None
        }
        10 => Action::LaunchMedia,
        11 => Action::PruneDryRun,
        12 => Action::ShowStorage,
        13 => Action::Refresh,
        _ => Action::None,
    }
}

pub fn adjust(app: &mut App, index: usize, delta: i32) {
    match index {
        4 => {
            let threshold = step(
                app.settings.storage.prune_threshold,
                delta,
                5,
                MIN_PRUNE,
                MAX_PRUNE,
            );
            app.settings.storage.prune_threshold = threshold;
            let ceiling = threshold.saturating_sub(MIN_PRUNE_GAP);
            if app.settings.storage.prune_target > ceiling {
                app.settings.storage.prune_target = ceiling;
            }
        }
        5 => {
            let ceiling = app
                .settings
                .storage
                .prune_threshold
                .saturating_sub(MIN_PRUNE_GAP);
            app.settings.storage.prune_target = step(
                app.settings.storage.prune_target,
                delta,
                5,
                MIN_PRUNE_TARGET,
                ceiling,
            );
        }
        _ => return,
    }
    app.save_settings();
}
