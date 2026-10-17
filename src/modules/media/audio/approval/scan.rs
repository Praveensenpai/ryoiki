//! Silence handling and the retroactive `[Multi]` library scan.

use anyhow::Result;
use std::path::Path;

use super::super::decisions::{self, now_secs, DecisionRecord};
use super::{register, StageOutcome};
use crate::modules::media::organizer::find_videos_recursive;
use crate::modules::media::organizer::pathing::is_video_file;
use crate::modules::media::probe;

/// Stops prompting for one pending decision, leaving every file untouched.
///
/// The title is recorded in history as silenced so the same file is never
/// staged again, and the `[Multi]` sidecar stays exactly as it is.
pub fn silence(id: &str) -> Result<String> {
    let mut store = decisions::load()?;
    let Some(pos) = store.pending.iter().position(|p| p.id == id) else {
        anyhow::bail!("No pending decision with id {id}");
    };
    let entry = store.pending.remove(pos);

    store.history.push(DecisionRecord {
        id: entry.id.clone(),
        title: entry.title.clone(),
        chosen: "kept [Multi]".to_string(),
        decided_secs: now_secs(),
        auto: false,
        silenced: true,
    });
    decisions::save(&store)?;

    Ok("Silenced — the [Multi] file is kept and will not be asked about again".to_string())
}

/// Stages every `[Multi]` file under `root` that still needs a language choice.
///
/// Files organized before the approval workflow existed never got a prompt.
/// This scan backfills them so the hourly reminder can pick them up.
pub fn scan_library(root: &Path) -> Result<usize> {
    let mut videos = Vec::new();
    if root.is_dir() {
        find_videos_recursive(root, &mut videos)?;
    } else if is_video_file(root) {
        videos.push(root.to_path_buf());
    }

    let mut staged = 0usize;
    for path in videos {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.to_ascii_lowercase().contains("[multi]") {
            continue;
        }
        let Some(probe) = probe::probe_media_file(&path) else {
            continue;
        };
        if !probe::needs_language_decision(&probe) {
            continue;
        }
        // A preserved `[Multi]` is both the source and the sidecar.
        if matches!(
            register(&path, &path, Some(&probe), None),
            StageOutcome::Pending
        ) {
            staged += 1;
        }
    }
    Ok(staged)
}
