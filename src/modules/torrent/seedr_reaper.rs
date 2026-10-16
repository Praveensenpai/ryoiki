//! Reaper for Seedr cloud items ryoiki does not own.
//!
//! Torrents added straight on the Seedr site are invisible to the single-slot
//! queue and to the local task state. They keep consuming the account quota, so
//! the available-space guard sees a full account and reroutes every magnet to
//! qBittorrent. This module clears those orphans before each space probe.

use std::collections::HashSet;

use super::seedr_tasks::{base_key, fetch_live_list, load_local_tasks, tasks_dir};

/// Names ryoiki currently owns: queue entries plus local download tasks.
///
/// Snapshotted under the queue lock so a concurrent writer cannot hand us a
/// half-read document (which would look like "nothing is tracked"). Callers
/// that already hold the lock must use [`tracked_names_from`] instead, since
/// the lock is not reentrant.
#[must_use]
pub fn tracked_names() -> HashSet<String> {
    super::queue::with_lock(|| tracked_names_from(&super::queue::load().entries))
}

/// Tracked names from an already-loaded queue plus local task files.
#[must_use]
pub fn tracked_names_from(entries: &[super::queue::QueueEntry]) -> HashSet<String> {
    let mut names = HashSet::new();
    for entry in entries {
        names.insert(base_key(&entry.name));
    }
    for (_, task) in load_local_tasks(&tasks_dir()) {
        names.insert(base_key(&task.file_name));
    }
    names
}

/// Deletes Seedr cloud items ryoiki does not own.
///
/// Completed payloads (`folders`/`files`) matching no tracked name are removed.
/// Cloud `torrents` still caching are skipped so an in-flight manual download is
/// never destroyed.
///
/// Returns `None` when the cloud list cannot be fetched, so a network blip can
/// never trigger deletions.
#[must_use]
pub fn reap_untracked_cloud() -> Option<Vec<String>> {
    let tracked = tracked_names();
    reap_untracked_cloud_with(&tracked)
}

/// [`reap_untracked_cloud`] with the tracked-name set supplied by the caller.
///
/// Used by callers already holding the queue lock, which must pass
/// [`tracked_names_from`] to avoid re-entering a non-reentrant lock.
#[must_use]
pub fn reap_untracked_cloud_with(tracked: &HashSet<String>) -> Option<Vec<String>> {
    let list = fetch_live_list()?;
    let mut removed = Vec::new();

    let candidates = list
        .folders
        .iter()
        .map(|f| (f.id, f.name.clone()))
        .chain(list.files.iter().map(|f| (f.id, f.name.clone())))
        .chain(
            list.torrents
                .iter()
                .filter(|t| t.progress.unwrap_or(0.0) >= 100.0)
                .map(|t| (t.id, t.name.clone())),
        );

    for (id, name) in candidates {
        if tracked.contains(&base_key(&name)) {
            continue;
        }
        if super::seedr::run_seedr_delete(id).is_ok() {
            removed.push(name);
        }
    }

    Some(removed)
}

/// Alerts the user when manual Seedr items are cleared to free space.
pub fn notify_reaped(names: &[String]) {
    use crate::notify::client::{escape_html, format_card, send_alert};

    let Ok(config) = crate::notify::config::TelegramConfig::load() else {
        return;
    };
    let list = names
        .iter()
        .map(|n| format!("• <code>{}</code>", escape_html(n)))
        .collect::<Vec<_>>()
        .join("\n");
    let card = format_card(
        "Seedr",
        "🧹 <b>MANUAL SEEDR ITEMS CLEARED</b>",
        &[
            ("Removed:", &list),
            ("Reason:", "Not tracked by ryoiki — freeing Seedr space"),
        ],
    );
    let _ = send_alert(&config.bot_token, &config.chat_id, &card);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_key_strips_folder_prefix_and_normalizes() {
        assert_eq!(base_key("folder-Some Release"), "some release");
        assert_eq!(base_key("  MY.SHOW "), "my.show");
    }

    #[test]
    fn test_tracked_names_from_includes_queue_entries() {
        let mut q = super::super::queue::SeedrQueue::default();
        q.enqueue("abc", "magnet:?xt=urn:btih:abc", "folder-Manual.Release");
        let tracked = tracked_names_from(&q.entries);
        assert!(tracked.contains("manual.release"));
    }
}
