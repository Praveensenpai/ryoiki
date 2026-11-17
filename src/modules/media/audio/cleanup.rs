//! Automatic cleanup of stale `[Multi]` sidecars after 14 days.

use colored::Colorize;
use std::fs;

use super::decisions;
use super::decisions::DecisionRecord;

/// Removes `[Multi]` sidecars for decisions that have been pending (or
/// resolved) for at least 14 days. Only distinct sidecars are deleted; a
/// preserved `[Multi]` that is the only copy of a file is left on disk but
/// stops being prompted about.
///
/// Returns the number of sidecars removed.
pub fn cleanup_stale_multis() -> usize {
    const AUTO_REMOVE_DAYS: u64 = 14;
    let Ok(mut store) = decisions::load() else {
        return 0;
    };
    let now = decisions::now_secs();
    let mut removed = 0usize;
    let mut drop_indices: Vec<usize> = Vec::new();

    for (i, entry) in store.pending.iter().enumerate() {
        if decisions::days_pending(entry) < AUTO_REMOVE_DAYS {
            continue;
        }
        let distinct = entry.stripped_path != entry.multi_path;
        if distinct && entry.multi_path.exists() {
            let _ = fs::remove_file(&entry.multi_path);
            println!(
                "  {} Auto-removed stale [Multi] ({} days): {}",
                "🧹".cyan(),
                decisions::days_pending(entry),
                entry.multi_path.display()
            );
            removed += 1;
        }
        drop_indices.push(i);
    }

    for i in drop_indices.into_iter().rev() {
        let entry = store.pending.remove(i);
        let id = entry.id.clone();
        let created = entry.created_secs;
        if let Some(rec) = store.history.iter_mut().find(|r| r.id == id) {
            rec.chosen = "removed [Multi]".to_string();
            rec.decided_secs = now;
            rec.auto = true;
            rec.silenced = true;
            if rec.created_secs == 0 {
                rec.created_secs = created;
            }
            rec.multi_path = Some(entry.multi_path.clone());
        } else {
            store.history.push(DecisionRecord {
                id,
                title: entry.title.clone(),
                chosen: "removed [Multi]".to_string(),
                decided_secs: now,
                auto: true,
                silenced: true,
                created_secs: created,
                multi_path: Some(entry.multi_path.clone()),
            });
        }
    }

    // History entries resolved at least 14 days ago: drop their [Multi] sidecar.
    for rec in &mut store.history {
        let Some(mp) = rec.multi_path.clone() else {
            continue;
        };
        let base_secs = if rec.created_secs > 0 {
            rec.created_secs
        } else {
            rec.decided_secs
        };
        if base_secs == 0
            || now.saturating_sub(base_secs) < AUTO_REMOVE_DAYS * decisions::SECS_PER_DAY
        {
            continue;
        }
        // Only sidecars.
        let is_sidecar = mp
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.to_ascii_lowercase().contains("[multi]"));
        if !is_sidecar {
            continue;
        }
        if mp.exists() {
            let _ = fs::remove_file(&mp);
            println!(
                "  {} Auto-removed stale [Multi]: {}",
                "🧹".cyan(),
                mp.display()
            );
            removed += 1;
        }
        rec.multi_path = None;
    }

    let _ = decisions::save(&store);
    removed
}
