//! Seedr active-download health watchdog.
//!
//! Two guards run against the single active slot:
//! 1. A whole-lifecycle deadline (`seedr_max_active_secs`, default 3h). On
//!    expiry the item is cancelled and handed to qBittorrent.
//! 2. A sustained slow-speed floor (`seedr_slow_speed_bps` held for
//!    `seedr_slow_grace_secs`). Recording the slow window lets [`super::scheduler`]
//!    preempt the slot when a newer magnet arrives.

use anyhow::Context;
use reqwest::blocking::Client;
use std::time::Duration;

use super::api;
use super::queue::{self, QueueState};
use super::scheduler;
use super::seedr::{load_pending_magnet, remove_pending_magnet};
use super::seedr_tasks;
use crate::notify::config::TelegramConfig;

/// Samples the active download: enforces the deadline and records slow periods.
pub fn record_active_health(config: &TelegramConfig) {
    queue::with_lock(|| record_active_health_locked(config));
}

fn record_active_health_locked(config: &TelegramConfig) {
    let mut q = queue::load();
    let Some(active) = q.active_entry().cloned() else {
        return;
    };
    let now = queue::now_secs();

    if is_expired(&active, now, config.seedr_max_active_secs) {
        expire_active(&mut q, &active, config);
        return;
    }

    if let Some(speed) = download_speed_for(&active.name) {
        if speed < config.seedr_slow_speed_bps {
            q.set_slow_since(&active.hash, now);
        } else {
            q.clear_slow_since(&active.hash);
        }
    }
    let _ = queue::save(&q);
}

/// Live download speed (bytes/sec) for a named local Seedr task.
///
/// Reads only local task JSON, so the watchdog never triggers a network probe.
fn download_speed_for(name: &str) -> Option<u64> {
    let target = seedr_tasks::normalize(name);
    seedr_tasks::load_local_tasks(&seedr_tasks::tasks_dir())
        .into_iter()
        .map(|(_, task)| task)
        .find(|task| {
            let candidate = seedr_tasks::normalize(&task.file_name);
            candidate == target || candidate.contains(&target) || target.contains(&candidate)
        })
        .map(|task| task.speed_bps)
}

fn is_expired(entry: &queue::QueueEntry, now: u64, max_secs: u64) -> bool {
    entry
        .activated_at
        .is_some_and(|started| now.saturating_sub(started) >= max_secs)
}

/// Cancels an expired Seedr item, forwards it to qBittorrent, and promotes next.
fn expire_active(q: &mut queue::SeedrQueue, active: &queue::QueueEntry, config: &TelegramConfig) {
    scheduler::cancel_seedr_item(active);
    let fallback_ok = forward_to_qbittorrent(active, config);
    q.finish(&active.hash, QueueState::Failed);
    let _ = queue::save(q);

    let detail = if fallback_ok {
        "Exceeded max Seedr time — forwarded to qBittorrent"
    } else {
        "Exceeded max Seedr time — qBittorrent fallback failed"
    };
    scheduler::notify_telegram("⏰ <b>SEEDR TIMEOUT — FALLBACK</b>", &active.name, detail);
    let _ = scheduler::promote_next_locked(config);
}

fn forward_to_qbittorrent(active: &queue::QueueEntry, config: &TelegramConfig) -> bool {
    let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build qBittorrent fallback client")
    else {
        return false;
    };
    let magnet = load_pending_magnet(&active.hash)
        .unwrap_or_else(|| format!("magnet:?xt=urn:btih:{}", active.hash));
    let ok = api::add_magnet(&client, &config.qbittorrent_url, &magnet).is_ok();
    remove_pending_magnet(&active.hash);
    ok
}
