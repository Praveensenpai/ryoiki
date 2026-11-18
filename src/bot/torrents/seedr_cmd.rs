use anyhow::Result;
use reqwest::blocking::Client;

use crate::modules::torrent::api;
use crate::modules::torrent::scheduler::{self, SubmitOutcome};
use crate::modules::torrent::seedr;
use crate::notify::config::TelegramConfig;
use crate::notify::style;

use super::{
    check_and_restore, duplicate_message, event_view, seedr_magnet_target, started_message,
};

pub fn handle_seedr_cmd(client: &Client, config: &TelegramConfig, target: &str) -> String {
    if target.is_empty() || target == "status" {
        let tasks = seedr::get_active_seedr_tasks();
        let sec = seedr::format_seedr_tasks_section(&tasks);
        if sec.is_empty() {
            event_view(
                "Seedr",
                &style::accent("·", "No active downloads"),
                &["<code>/seedr &lt;magnet&gt;</code> to start one.".to_string()],
            )
        } else {
            format!("{}\n\n{}", style::view("Seedr"), sec)
        }
    } else {
        if let Some(msg) = check_and_restore(target) {
            return msg;
        }

        // Magnets carry a hash, so they belong in the single-slot Seedr queue
        // exactly like a raw magnet message. Previously this spawned seedr-dl
        // directly, bypassing the queue: no single-slot guard, no completion
        // promotion, and a duplicate of an already-queued magnet could run.
        if let Some((hash, name)) = seedr_magnet_target(target) {
            return match scheduler::submit(&hash, target, &name, config) {
                SubmitOutcome::Started => {
                    let _ = api::add_magnet(client, &config.qbittorrent_url, target);
                    started_message()
                }
                SubmitOutcome::Duplicate => duplicate_message(),
                SubmitOutcome::Queued(pos) => {
                    let _ = api::add_magnet(client, &config.qbittorrent_url, target);
                    event_view(
                        "SEEDR QUEUE",
                        &style::accent("…", &format!("Queue slot busy · position {pos}")),
                        &[
                            "Downloading in qBittorrent meanwhile.".to_string(),
                            "<i>Seedr starts when the cloud slot frees.</i>".to_string(),
                        ],
                    )
                }
                SubmitOutcome::FailedStart => {
                    let _ = api::add_magnet(client, &config.qbittorrent_url, target);
                    event_view(
                        "QBITTORRENT",
                        &style::accent("↓", "Magnet queued"),
                        &["<i>Seedr rejected the download; forwarded locally.</i>".to_string()],
                    )
                }
            };
        }

        // Non-magnet target (torrent URL): the queue cannot track it by hash,
        // so hand it straight to seedr-dl as before.
        match seedr::spawn_seedr_download(target, config.api_port) {
            Ok(()) => event_view(
                "SEEDR",
                &style::accent("↓", "Download queued"),
                &["Seedr cloud caching initiated.".to_string()],
            ),
            Err(e) => format!(
                "{} Failed to spawn Seedr download:\n<code>{e}</code>",
                style::accent("!", "Error")
            ),
        }
    }
}

pub fn handle_torrent_file(
    client: &Client,
    config: &TelegramConfig,
    fname: &str,
    bytes: Vec<u8>,
) -> Result<String> {
    if let Some(msg) = check_and_restore(fname) {
        return Ok(msg);
    }

    api::add_torrent_file(client, &config.qbittorrent_url, fname, bytes)?;
    Ok(event_view(
        "QBITTORRENT",
        &style::accent("↓", "Torrent file queued"),
        &[format!("<code>{fname}</code> added to downloads.")],
    ))
}
