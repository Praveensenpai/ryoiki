//! Seedr single-slot scheduling: queue promotion, space guard, and race resolution.

use std::path::{Path, PathBuf};

use super::api;
use super::queue::{self, QueueEntry, QueuePolicy, QueueState};
use super::seedr;
use crate::notify::config::TelegramConfig;

/// Result of submitting a magnet to the Seedr pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitOutcome {
    Started,
    Queued(usize),
    Duplicate,
}

/// Decision produced by [`pick_promotion`].
pub enum Promotion {
    Start(String),
    SpaceBlocked {
        name: String,
        needed: u64,
        free: u64,
    },
    Idle,
}

/// Picks the next magnet to run under the policy, honoring the single slot and
/// the Seedr free-space guard. Pure so it can be unit tested without I/O.
#[must_use]
pub fn pick_promotion(
    entries: &[QueueEntry],
    policy: QueuePolicy,
    free_bytes: Option<u64>,
    needed_bytes: Option<u64>,
) -> Promotion {
    if entries.iter().any(|e| e.state == QueueState::Active) {
        return Promotion::Idle;
    }
    let mut queued: Vec<&QueueEntry> = entries
        .iter()
        .filter(|e| e.state == QueueState::Queued)
        .collect();
    queued.sort_by_key(|e| e.seq);
    let next = match policy {
        QueuePolicy::Fifo => queued.first().copied(),
        QueuePolicy::Lifo => queued.last().copied(),
    };
    let Some(entry) = next else {
        return Promotion::Idle;
    };
    if let (Some(free), Some(needed)) = (free_bytes, needed_bytes) {
        if needed > free {
            return Promotion::SpaceBlocked {
                name: entry.name.clone(),
                needed,
                free,
            };
        }
    }
    Promotion::Start(entry.hash.clone())
}

/// Records a magnet and starts it in Seedr when the slot is free.
#[must_use]
pub fn submit(hash: &str, magnet: &str, name: &str, config: &TelegramConfig) -> SubmitOutcome {
    queue::with_lock(|| submit_locked(hash, magnet, name, config))
}

fn submit_locked(hash: &str, magnet: &str, name: &str, config: &TelegramConfig) -> SubmitOutcome {
    let mut q = queue::load();
    q.prune_terminal();
    if q.is_tracked(hash) {
        // The entry may be a phantom (cloud gone, worker dead). Verify against
        // the live cloud before refusing, then re-check.
        prune_stale_entries(&mut q);
        let _ = queue::save(&q);
        if q.is_tracked(hash) {
            return SubmitOutcome::Duplicate;
        }
    }
    q.enqueue(hash, magnet, name);

    // A sustained-slow active download yields the slot to this newer magnet.
    if q.has_active() && preempt_slow_active(&mut q, config) {
        if start_entry_now(&mut q, hash, magnet, config) {
            return SubmitOutcome::Started;
        }
        let pos = q.position(hash);
        let _ = queue::save(&q);
        return SubmitOutcome::Queued(pos);
    }

    if !q.has_active() && start_entry_now(&mut q, hash, magnet, config) {
        return SubmitOutcome::Started;
    }

    let pos = q.position(hash);
    let _ = queue::save(&q);
    SubmitOutcome::Queued(pos)
}

/// Activates one queued entry and spawns its Seedr worker. Caller holds the lock.
fn start_entry_now(
    q: &mut queue::SeedrQueue,
    hash: &str,
    magnet: &str,
    config: &TelegramConfig,
) -> bool {
    if !q.activate(hash) {
        return false;
    }
    let _ = queue::save(q);
    if seedr::spawn_seedr_download(magnet, config.api_port).is_ok() {
        true
    } else {
        q.finish(hash, QueueState::Queued);
        let _ = queue::save(q);
        false
    }
}

/// Demotes the active download when it has been slow past the grace window.
///
/// Returns true when the slot was freed for the incoming magnet.
fn preempt_slow_active(q: &mut queue::SeedrQueue, config: &TelegramConfig) -> bool {
    let Some(active) = q.active_entry().cloned() else {
        return false;
    };
    let Some(since) = active.slow_since else {
        return false;
    };
    if queue::now_secs().saturating_sub(since) < config.seedr_slow_grace_secs {
        return false;
    }
    cancel_seedr_item(&active);
    q.demote_to_queued(&active.hash);
    notify_telegram(
        "⚡ <b>SEEDR SLOW — PREEMPTED</b>",
        &active.name,
        "Newer magnet took the slot; slow download requeued at the front",
    );
    true
}

/// Starts the next queued magnet once the slot frees. Returns its hash if started.
///
/// The queue lock is held across the space probe and process spawn so the
/// single-slot decision stays atomic. Callers must not already hold the lock;
/// internal callers use [`promote_next_locked`].
pub(crate) fn promote_next_locked(config: &TelegramConfig) -> Option<String> {
    let mut q = queue::load();
    let policy = QueuePolicy::parse(&config.seedr_queue_policy);

    let Promotion::Start(candidate) = pick_promotion(&q.entries, policy, None, None) else {
        return None;
    };
    let needed = api::get_torrent_size(&candidate, &config.qbittorrent_url);
    let free = super::seedr_tasks::seedr_available_bytes();

    match pick_promotion(&q.entries, policy, free, needed) {
        Promotion::Start(hash) => start_entry(&mut q, &hash, config),
        Promotion::SpaceBlocked { name, needed, free } => {
            notify_space_blocked(&name, needed, free);
            None
        }
        Promotion::Idle => None,
    }
}

/// Activates and spawns one entry. Caller holds the queue lock.
fn start_entry(q: &mut queue::SeedrQueue, hash: &str, config: &TelegramConfig) -> Option<String> {
    let entry = q.entries.iter().find(|e| e.hash == hash).cloned()?;
    if !q.activate(hash) {
        return None;
    }
    let _ = queue::save(q);
    if seedr::spawn_seedr_download(&entry.magnet, config.api_port).is_ok() {
        notify_telegram(
            "🚀 <b>SEEDR QUEUE PROMOTED</b>",
            &entry.name,
            "Started in Seedr cloud",
        );
        Some(entry.hash)
    } else {
        q.finish(hash, QueueState::Queued);
        let _ = queue::save(q);
        None
    }
}

/// Resolves the qBittorrent-wins race: cancel Seedr, drop the entry, promote next.
pub fn handle_qb_completion(hash: &str, config: &TelegramConfig) {
    queue::with_lock(|| {
        let mut q = queue::load();
        let Some(entry) = q.entries.iter().find(|e| e.hash == hash).cloned() else {
            return;
        };
        if entry.state == QueueState::Done {
            return;
        }
        delete_seedr_cloud(&entry);
        q.remove(hash);
        let _ = queue::save(&q);
        let _ = promote_next_locked(config);
    });
}

/// Moves a queued entry to the front so it runs next.
pub fn promote_front(hash: &str) -> bool {
    queue::with_lock(|| {
        let mut q = queue::load();
        if q.promote_front(hash) {
            let _ = queue::save(&q);
            true
        } else {
            false
        }
    })
}

/// Marks a Seedr download complete and promotes the next queued magnet.
pub fn handle_seedr_done(hash: &str, config: &TelegramConfig) {
    queue::with_lock(|| finish_seedr_locked(hash, QueueState::Done, config));
}

/// Marks a Seedr download failed and promotes the next queued magnet.
pub fn handle_seedr_failure(hash: &str, config: &TelegramConfig) {
    queue::with_lock(|| finish_seedr_locked(hash, QueueState::Failed, config));
}

fn finish_seedr_locked(hash: &str, state: QueueState, config: &TelegramConfig) {
    let mut q = queue::load();
    if q.entries.iter().any(|e| e.hash == hash) {
        q.finish(hash, state);
        let _ = queue::save(&q);
    }
    let _ = promote_next_locked(config);
}

/// Stops Seedr's background *download task* for a preempted magnet.
///
/// The cloud item is intentionally left in place so a later run can resume it.
pub(crate) fn cancel_seedr_item(entry: &QueueEntry) {
    if let Some(id) = seedr::resolve_cloud_id(&entry.name) {
        let _ = std::process::Command::new("seedr-dl")
            .args(["cancel", &id.to_string()])
            .output();
    }
    remove_partial(&entry.name);
}

/// Deletes Seedr's copy of a magnet qBittorrent already finished locally.
///
/// Uses the live cloud list (not transient task JSON) and `delete -y`, so the
/// cloud item is actually removed instead of merely having its task cancelled.
pub(crate) fn delete_seedr_cloud(entry: &QueueEntry) {
    if let Some(id) = seedr::resolve_cloud_id(&entry.name) {
        let _ = seedr::run_seedr_delete(id);
    }
    remove_partial(&entry.name);
}

/// Drops `Active` queue entries whose Seedr cloud item and worker are both gone.
///
/// This clears the phantom "active" rows (dead pid, empty cloud) that used to
/// keep reporting `ALREADY TRACKED` for magnets that no longer exist anywhere.
/// Queued entries are kept untouched: a magnet waiting for the single slot has
/// no cloud presence yet by design. Only prunes when the cloud is reachable, so
/// a network blip cannot wipe live state.
pub(crate) fn prune_stale_entries(q: &mut queue::SeedrQueue) {
    let keep: Vec<QueueEntry> = q
        .entries
        .iter()
        .filter(|e| {
            e.state != QueueState::Active
                || super::seedr_tasks::cloud_contains(&e.name) != Some(false)
                || super::seedr_tasks::worker_alive_by_name(&e.name)
        })
        .cloned()
        .collect();
    q.entries = keep;
}

fn remove_partial(name: &str) {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let safe = Path::new(name)
        .file_name()
        .map_or_else(|| name.to_string(), |n| n.to_string_lossy().to_string());
    let base = Path::new(&home).join("torrents").join(&safe);
    let part = PathBuf::from(format!("{}.part", base.display()));
    let _ = std::fs::remove_file(&part);
}

pub(crate) fn notify_telegram(badge: &str, name: &str, detail: &str) {
    let Ok(config) = crate::notify::config::TelegramConfig::load() else {
        return;
    };
    let clean = crate::notify::client::escape_html(name);
    let card = crate::notify::client::format_card(
        "Seedr Queue",
        badge,
        &[("File:", &clean), ("Status:", detail)],
    );
    let _ = crate::notify::client::send_alert(&config.bot_token, &config.chat_id, &card);
}

fn notify_space_blocked(name: &str, needed: u64, free: u64) {
    let mb = |b: u64| b / 1_048_576;
    let clean = crate::notify::client::escape_html(name);
    let detail = format!(
        "Waiting for space — needs {} MB, {} MB free",
        mb(needed),
        mb(free)
    );
    notify_telegram("⏳ <b>SEEDR QUEUE HELD</b>", &clean, &detail);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(hash: &str, seq: u64, state: QueueState) -> QueueEntry {
        QueueEntry {
            hash: hash.to_string(),
            magnet: format!("magnet:?xt=urn:btih:{hash}"),
            name: hash.to_string(),
            seq,
            state,
            activated_at: None,
            slow_since: None,
        }
    }

    #[test]
    fn test_pick_promotion_idle_when_active() {
        let entries = vec![
            entry("a", 1, QueueState::Active),
            entry("b", 2, QueueState::Queued),
        ];
        assert!(matches!(
            pick_promotion(&entries, QueuePolicy::Fifo, None, None),
            Promotion::Idle
        ));
    }

    #[test]
    fn test_pick_promotion_fifo_and_lifo() {
        let entries = vec![
            entry("a", 1, QueueState::Queued),
            entry("b", 2, QueueState::Queued),
        ];
        let Promotion::Start(f) = pick_promotion(&entries, QueuePolicy::Fifo, None, None) else {
            panic!("expected start");
        };
        assert_eq!(f, "a");
        let Promotion::Start(l) = pick_promotion(&entries, QueuePolicy::Lifo, None, None) else {
            panic!("expected start");
        };
        assert_eq!(l, "b");
    }

    #[test]
    fn test_pick_promotion_space_blocks() {
        let entries = vec![entry("a", 1, QueueState::Queued)];
        assert!(matches!(
            pick_promotion(&entries, QueuePolicy::Fifo, Some(100), Some(500)),
            Promotion::SpaceBlocked { .. }
        ));
    }

    #[test]
    fn test_pick_promotion_allows_when_fits() {
        let entries = vec![entry("a", 1, QueueState::Queued)];
        assert!(matches!(
            pick_promotion(&entries, QueuePolicy::Fifo, Some(500), Some(100)),
            Promotion::Start(_)
        ));
    }

    #[test]
    fn test_pick_promotion_ignores_space_when_unknown() {
        let entries = vec![entry("a", 1, QueueState::Queued)];
        assert!(matches!(
            pick_promotion(&entries, QueuePolicy::Fifo, None, Some(100)),
            Promotion::Start(_)
        ));
    }
}
