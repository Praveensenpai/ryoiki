use std::path::Path;

use crate::modules::media::disk::{format_bytes, get_disk_usage};
use crate::modules::media::organizer::pathing::get_jellyfin_media_dir;
use crate::tui::app::{Action, App};
use crate::tui::widgets::Row;

pub fn rows(_app: &App) -> Vec<Row> {
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

    vec![
        Row::info("Jellyfin library", library.display().to_string()),
        Row::info("Library state", state),
        Row::info("Root disk", disk),
        Row::divider(),
        Row::action("Open interactive media manager"),
        Row::action("Preview prune (dry-run)"),
        Row::action("Show storage report"),
        Row::action("Refresh storage"),
    ]
}

pub fn activate(_app: &mut App, index: usize) -> Action {
    match index {
        4 => Action::LaunchMedia,
        5 => Action::PruneDryRun,
        6 => Action::ShowStorage,
        7 => Action::Refresh,
        _ => Action::None,
    }
}
