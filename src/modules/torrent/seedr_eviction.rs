//! Frees Seedr quota by evicting cloud items that can no longer be useful.
//!
//! A space-blocked admission is not a reason to give up on Seedr. Completed
//! items still occupying the account are candidates: manual ones are safe to
//! drop immediately, while ryoiki-owned ones are only dropped once their local
//! pull is provably finished. Anything still downloading is left untouched.

use std::collections::HashSet;
use std::path::Path;

use super::scheduler;
use super::seedr;
use super::seedr_tasks::{self, base_key};

/// Frees room for `required` bytes using the current tracked-name set.
///
/// Returns remaining free bytes, or `None` when the Seedr cloud is unreachable.
#[must_use]
pub fn free_space_for(required: u64) -> Option<u64> {
    let tracked = super::seedr_reaper::tracked_names();
    free_space_for_with(&tracked, required)
}

/// [`free_space_for`] for callers that already hold the queue lock.
#[must_use]
pub(crate) fn free_space_for_with(tracked: &HashSet<String>, required: u64) -> Option<u64> {
    let list = seedr_tasks::fetch_live_list()?;
    let max = list.space_max?;
    let before = max.saturating_sub(list.space_used.unwrap_or(0));
    if before >= required {
        return Some(before);
    }

    let mut removed: Vec<String> = Vec::new();
    for target in eviction_targets(&list, tracked) {
        if seedr::run_seedr_delete(target.id).is_ok() {
            removed.push(target.name);
        }
    }

    if !removed.is_empty() {
        let detail = format!("Evicted {} completed item(s) to make room", removed.len());
        scheduler::notify_telegram("·", &removed.join(", "), &detail);
    }

    seedr_tasks::seedr_available_bytes()
}

/// One cloud item chosen for eviction.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EvictionTarget {
    pub(crate) id: u64,
    pub(crate) name: String,
}

/// Completed cloud items eligible for eviction. Anything still caching is kept.
pub(crate) fn eviction_targets(
    list: &seedr_tasks::SeedrCloudList,
    tracked: &HashSet<String>,
) -> Vec<EvictionTarget> {
    let mut targets = Vec::new();
    for torrent in &list.torrents {
        if torrent.progress.unwrap_or(0.0) >= 100.0 && should_evict(&torrent.name, tracked) {
            targets.push(EvictionTarget {
                id: torrent.id,
                name: torrent.name.clone(),
            });
        }
    }
    for item in list.folders.iter().chain(list.files.iter()) {
        if should_evict(&item.name, tracked) {
            targets.push(EvictionTarget {
                id: item.id,
                name: item.name.clone(),
            });
        }
    }
    targets
}

/// Manual items are safe to evict at any time. Owned items must have finished
/// pulling to local disk first.
fn should_evict(name: &str, tracked: &HashSet<String>) -> bool {
    if !tracked.contains(&base_key(name)) {
        return true;
    }
    local_pull_done(name)
}

/// A local pull is done when no worker is alive, no local task still has bytes
/// remaining, and the payload exists on disk.
fn local_pull_done(name: &str) -> bool {
    if seedr_tasks::worker_alive_by_name(name) {
        return false;
    }
    let target = seedr_tasks::normalize(name);
    let has_partial = seedr_tasks::load_local_tasks(&seedr_tasks::tasks_dir())
        .into_iter()
        .any(|(_, task)| {
            let key = seedr_tasks::normalize(&task.file_name);
            let matches = key == target || key.contains(&target) || target.contains(&key);
            matches
                && !task.status.eq_ignore_ascii_case("failed")
                && seedr_tasks::worker_alive(task.pid)
                && task.downloaded_bytes < task.total_bytes
        });
    !has_partial && local_copy_exists(name)
}

/// Checks both the raw cloud name and its `folder-`-stripped form under
/// `~/torrents`, or anywhere in the Jellyfin media library via download history.
fn local_copy_exists(name: &str) -> bool {
    if let Ok(home) = std::env::var("HOME") {
        let base = Path::new(&home).join("torrents");
        if base.join(name).exists()
            || base
                .join(name.strip_prefix("folder-").unwrap_or(name))
                .exists()
        {
            return true;
        }
    }

    let history = super::history::load_history();
    let target = seedr_tasks::normalize(name);
    let stripped = target.strip_prefix("folder-").unwrap_or(&target);

    history.entries.values().any(|record| {
        let title_norm = seedr_tasks::normalize(&record.title);
        let clean_norm = seedr_tasks::normalize(&record.clean_name);
        let matches = (!title_norm.is_empty()
            && (stripped.contains(&title_norm) || title_norm.contains(stripped)))
            || (!clean_norm.is_empty()
                && (stripped.contains(&clean_norm) || clean_norm.contains(stripped)));
        matches
            && (record.paths.iter().any(|p| p.exists())
                || record.files.iter().any(|f| f.path.exists()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_items_are_evictable_without_a_local_copy() {
        let tracked = HashSet::new();
        assert!(should_evict("folder-Manual.Release", &tracked));
    }

    #[test]
    fn tracked_items_require_a_finished_local_pull() {
        let mut tracked = HashSet::new();
        tracked.insert(base_key("folder-Owned.Release"));
        // No local copy, no task: not proven safe.
        assert!(!should_evict("folder-Owned.Release", &tracked));
    }

    #[test]
    fn only_completed_cloud_items_become_targets() {
        let json = r#"{
            "space_max": 1000, "space_used": 1000,
            "torrents": [
                {"id": 1, "name": "folder-Manual.Done", "progress": 100.0},
                {"id": 2, "name": "Still.Caching", "progress": 42.0}
            ],
            "folders": [{"id": 3, "name": "folder-Manual.Payload"}],
            "files": []
        }"#;
        let list: seedr_tasks::SeedrCloudList = match serde_json::from_str(json) {
            Ok(list) => list,
            Err(e) => panic!("valid list: {e}"),
        };
        let targets = eviction_targets(&list, &HashSet::new());
        assert!(targets.iter().any(|t| t.name == "folder-Manual.Done"));
        assert!(targets.iter().any(|t| t.name == "folder-Manual.Payload"));
        assert!(!targets.iter().any(|t| t.name == "Still.Caching"));
    }
}
