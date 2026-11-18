//! Hourly retry: promotes eligible qBittorrent torrents into the Seedr queue.
//!
//! Torrents that fell back to qBittorrent are not forgotten. Once an hour the
//! monitor looks for the smallest eligible local download that still fits the
//! configured size ceiling, frees space if possible, and re-submits it to Seedr.

use std::collections::HashSet;

use reqwest::blocking::Client;

use super::api::{self, TorrentInfo};
use super::queue::{self, QueueState};
use super::seedr_tasks::{self, base_key};
use super::{scheduler, seedr_eviction};
use crate::notify::config::TelegramConfig;

/// Eligibility is decided from qBittorrent's own metadata: dead torrents report
/// `has_metadata == false` (and `total_size == -1`) and are dropped here.
#[must_use]
pub(crate) fn pick_candidate<'a>(
    torrents: &'a [TorrentInfo],
    max_bytes: u64,
    cloud_names: &HashSet<String>,
    queued_hashes: &HashSet<String>,
) -> Option<&'a TorrentInfo> {
    let mut eligible: Vec<&TorrentInfo> = torrents
        .iter()
        .filter(|t| !t.is_completed() && t.has_metadata)
        .filter_map(|t| u64::try_from(t.total_size).ok().map(|size| (t, size)))
        .filter(|(_, size)| *size > 0 && *size <= max_bytes)
        .filter(|(t, _)| !cloud_names.contains(&base_key(&t.name)))
        .filter(|(t, _)| !queued_hashes.contains(&t.hash))
        .map(|(t, _)| t)
        .collect();
    // Smallest first: the most likely to fit the remaining Seedr quota.
    eligible.sort_by_key(|t| t.total_size);
    eligible.into_iter().next()
}

/// Runs one retry pass. Called on the hourly tick from the bot monitor.
pub fn retry_candidates(client: &Client, config: &TelegramConfig) {
    // A failed cloud probe (missing binary, expired auth, network) pauses the
    // sweep. It resumes automatically on the next tick once Seedr is reachable.
    let Some(live) = seedr_tasks::fetch_live_list() else {
        return;
    };
    let cloud_names: HashSet<String> = live
        .torrents
        .iter()
        .map(|t| base_key(&t.name))
        .chain(live.folders.iter().map(|f| base_key(&f.name)))
        .chain(live.files.iter().map(|f| base_key(&f.name)))
        .collect();

    // One candidate per hour, never a second concurrent slot.
    let q = queue::load();
    if q.has_active() || q.pending_count() > 0 {
        return;
    }
    let queued_hashes: HashSet<String> = q
        .entries
        .iter()
        .filter(|e| matches!(e.state, QueueState::Queued | QueueState::Active))
        .map(|e| e.hash.clone())
        .collect();

    let Ok(torrents) = api::get_torrents(client, &config.qbittorrent_url, None) else {
        return;
    };
    let max_bytes = config.seedr_max_torrent_mb.saturating_mul(1_048_576);
    let Some(candidate) = pick_candidate(&torrents, max_bytes, &cloud_names, &queued_hashes) else {
        return;
    };

    let Ok(size) = u64::try_from(candidate.total_size) else {
        return;
    };
    let Some(avail) = seedr_tasks::seedr_available_bytes() else {
        return;
    };
    if size > avail && seedr_eviction::free_space_for(size).is_none_or(|freed| freed < size) {
        return;
    }

    let hash = candidate.hash.clone();
    let name = candidate.name.clone();
    let magnet = format!("magnet:?xt=urn:btih:{hash}");
    let outcome = scheduler::submit(&hash, &magnet, &name, config);
    if matches!(
        outcome,
        scheduler::SubmitOutcome::Started | scheduler::SubmitOutcome::Queued(_)
    ) {
        scheduler::notify_telegram("·", &name, "Hourly Seedr retry accepted");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn torrent(hash: &str, name: &str, size: i64, progress: f64, metadata: bool) -> TorrentInfo {
        TorrentInfo {
            name: name.to_string(),
            total_size: size,
            progress,
            dlspeed: 0,
            upspeed: 0,
            eta: 0,
            state: "downloading".to_string(),
            category: String::new(),
            has_metadata: metadata,
            hash: hash.to_string(),
            content_path: None,
            save_path: None,
        }
    }

    #[test]
    fn dead_and_completed_torrents_are_skipped() {
        let items = [
            torrent("a", "Dead (no metadata)", -1, 0.0, false),
            torrent("b", "Already done", 1_000, 1.0, true),
        ];
        assert!(
            pick_candidate(&items, 4_999 * 1_048_576, &HashSet::new(), &HashSet::new()).is_none()
        );
    }

    #[test]
    fn smallest_eligible_torrent_wins() {
        let items = [
            torrent("a", "Big", 4_900 * 1_048_576, 0.1, true),
            torrent("b", "Small", 900 * 1_048_576, 0.2, true),
            torrent("c", "Oversized", 5_100 * 1_048_576, 0.3, true),
        ];
        let picked = pick_candidate(&items, 4_999 * 1_048_576, &HashSet::new(), &HashSet::new());
        assert_eq!(picked.map(|t| t.hash.as_str()), Some("b"));
    }

    #[test]
    fn cloud_and_queued_torrents_are_skipped() {
        let items = [torrent("a", "Cached", 900 * 1_048_576, 0.2, true)];
        let cloud = HashSet::from(["cached".to_string()]);
        assert!(pick_candidate(&items, 4_999 * 1_048_576, &cloud, &HashSet::new()).is_none());
        let queued = HashSet::from(["a".to_string()]);
        assert!(pick_candidate(&items, 4_999 * 1_048_576, &HashSet::new(), &queued).is_none());
    }
}
