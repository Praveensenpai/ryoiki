use std::time::Duration;

use super::{MediaCategory, MediaItem, TransferDirection};
use crate::modules::media::disk;
use crate::notify::client::{escape_html, send_alert};
use crate::notify::TelegramConfig;
use crate::runner::format_duration;

#[must_use]
pub fn format_transfer_rate(bytes: u64, duration: Duration) -> String {
    const BYTES_PER_MIB: u64 = 1024 * 1024;
    let secs = duration.as_secs_f64();
    if secs <= 0.05 {
        return "—".to_string();
    }
    let mib_val = bytes / BYTES_PER_MIB;
    let rem_bytes = bytes % BYTES_PER_MIB;
    let mib_u32 = u32::try_from(mib_val.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
    let rem_u32 = u32::try_from((rem_bytes * 10) / BYTES_PER_MIB).unwrap_or(0);
    let mib_f = f64::from(mib_u32) + (f64::from(rem_u32) / 10.0);
    let mib_per_sec = mib_f / secs;
    if mib_per_sec >= 1024.0 {
        format!("{:.2} GiB/s", mib_per_sec / 1024.0)
    } else {
        format!("{mib_per_sec:.1} MiB/s")
    }
}

pub fn send_item_notification(
    dir: TransferDirection,
    item: &MediaItem,
    index: usize,
    total_count: usize,
    duration: Duration,
) {
    let Ok(cfg) = TelegramConfig::load() else {
        return;
    };

    let title_esc = escape_html(&item.title);
    let size_str = disk::format_bytes(item.size_bytes);
    let dur_str = format_duration(duration);
    let rate_str = format_transfer_rate(item.size_bytes, duration);
    let pct = (index * 100).checked_div(total_count).unwrap_or(100);

    let (badge, size_label) = match dir {
        TransferDirection::Push => (
            format!("📤 <b>OFFLOADED [{index}/{total_count}]</b> • <code>{pct}%</code>"),
            format!("{size_str} freed"),
        ),
        TransferDirection::Pull => (
            format!("📥 <b>DOWNLOADED [{index}/{total_count}]</b> • <code>{pct}%</code>"),
            size_str,
        ),
    };

    let cat_tag = match item.category {
        MediaCategory::Movie => "Movie",
        MediaCategory::Show => "TV Show",
        MediaCategory::Anime => "Anime",
    };

    let card = format!(
        "🌊 <b>領域 RYOIKI</b> • <i>Media Manager</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        {badge}\n\n\
        🎬 <b>{title_esc}</b>\n\
         ├ 📂 <b>Type:</b> <code>{cat_tag}</code>\n\
         ├ 💾 <b>Size:</b> <code>{size_label}</code>\n\
         ├ ⏱ <b>Time:</b> <code>{dur_str}</code>\n\
         └ ⚡ <b>Rate:</b> <code>{rate_str}</code>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━"
    );

    let _ = send_alert(&cfg.bot_token, &cfg.chat_id, &card);
}

pub fn send_batch_initiated_notification(
    dir: TransferDirection,
    items: &[MediaItem],
    total_bytes: u64,
) {
    let Ok(cfg) = TelegramConfig::load() else {
        return;
    };

    let count = items.len();
    let total_size_str = disk::format_bytes(total_bytes);
    let dir_desc = match dir {
        TransferDirection::Push => "Local SSD ➔ Google Drive (Offload)",
        TransferDirection::Pull => "Google Drive ➔ Local SSD (Restore)",
    };

    let header = format!(
        "🌊 <b>領域 RYOIKI</b> • <i>Media Manager</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        🚀 <b>TRANSFER INITIATED</b>\n\n\
        📦 <b>Direction:</b> <code>{dir_desc}</code>\n\
        🎬 <b>Queue:</b> {count} titles • {total_size_str}\n\n\
        📋 <b>Queued Media:</b>\n"
    );

    let footer = "\n━━━━━━━━━━━━━━━━━━━━━━━";

    let item_lines: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(idx, it)| {
            let num = idx + 1;
            let title = escape_html(&it.title);
            let size = disk::format_bytes(it.size_bytes);
            let cat = it.category.as_str();
            format!(" {num:02}. <b>{title}</b> • <code>{size}</code> [{cat}]")
        })
        .collect();

    let chunks = build_chunked_messages(&header, &item_lines, footer);
    send_chunks(&cfg.bot_token, &cfg.chat_id, &chunks);
}

pub fn send_batch_completed_notification(
    dir: TransferDirection,
    timings: &[(&MediaItem, Duration)],
    total_bytes: u64,
    total_duration: Duration,
) {
    let Ok(cfg) = TelegramConfig::load() else {
        return;
    };

    let count = timings.len();
    let total_size_str = disk::format_bytes(total_bytes);
    let total_dur_str = format_duration(total_duration);
    let avg_rate = format_transfer_rate(total_bytes, total_duration);

    let dir_desc = match dir {
        TransferDirection::Push => "Local SSD ➔ Google Drive",
        TransferDirection::Pull => "Google Drive ➔ Local SSD",
    };

    let header = format!(
        "🌊 <b>領域 RYOIKI</b> • <i>Media Manager</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        ✅ <b>BATCH TRANSFER COMPLETED</b>\n\n\
        📦 <b>Direction:</b> <code>{dir_desc}</code>\n\
        🎬 <b>Total Items:</b> {count} / {count} transferred\n\
        📊 <b>Total Volume:</b> {total_size_str}\n\
        ⏱ <b>Total Time:</b> {total_dur_str} (⚡ {avg_rate})\n\n\
        📋 <b>Transferred Titles:</b>\n"
    );

    let footer = "\n━━━━━━━━━━━━━━━━━━━━━━━\n✨ <i>Jellyfin library refreshed successfully</i>";

    let item_lines: Vec<String> = timings
        .iter()
        .enumerate()
        .map(|(idx, (it, dur))| {
            let num = idx + 1;
            let title = escape_html(&it.title);
            let size = disk::format_bytes(it.size_bytes);
            let dur_str = format_duration(*dur);
            let rate = format_transfer_rate(it.size_bytes, *dur);
            format!(" {num:02}. <b>{title}</b>\n      └ 💾 <code>{size}</code> • ⏱ <code>{dur_str}</code> ({rate})")
        })
        .collect();

    let chunks = build_chunked_messages(&header, &item_lines, footer);
    send_chunks(&cfg.bot_token, &cfg.chat_id, &chunks);
}

fn build_chunked_messages(header: &str, items: &[String], footer: &str) -> Vec<String> {
    const MAX_LEN: usize = 3800;
    let mut messages = Vec::new();
    let mut current_body = String::new();

    for item_line in items {
        if header.len() + current_body.len() + item_line.len() + footer.len() > MAX_LEN
            && !current_body.is_empty()
        {
            messages.push(format!("{header}{current_body}{footer}"));
            current_body.clear();
        }
        current_body.push_str(item_line);
        current_body.push('\n');
    }

    if !current_body.is_empty() || messages.is_empty() {
        messages.push(format!("{header}{current_body}{footer}"));
    }

    if messages.len() > 1 {
        let total = messages.len();
        for (i, msg) in messages.iter_mut().enumerate() {
            *msg = format!("<b>[Part {}/{}]</b>\n{}", i + 1, total, msg);
        }
    }
    messages
}

fn send_chunks(token: &str, chat_id: &str, chunks: &[String]) {
    for chunk in chunks {
        let _ = send_alert(token, chat_id, chunk);
    }
}
