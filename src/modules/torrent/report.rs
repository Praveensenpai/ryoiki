use std::ffi::CString;
use std::mem::MaybeUninit;

use super::api::TorrentInfo;
use super::notify::format_size;

pub fn format_status_report(torrents: &[TorrentInfo]) -> String {
    if torrents.is_empty() {
        return "🌊 <b>領域 RYOIKI • Torrents</b>\n━━━━━━━━━━━━━━━━━━━━━━━\nNo active or completed torrents found.".to_string();
    }

    let mut lines = vec!["🌊 <b>領域 RYOIKI • Torrents</b>\n━━━━━━━━━━━━━━━━━━━━━━━".to_string()];
    for t in torrents.iter().take(5) {
        let pct = (t.progress * 100.0).clamp(0.0, 100.0);
        let blocks = format!("{:.0}", pct / 10.0)
            .parse::<usize>()
            .unwrap_or(0)
            .min(10);
        let bar = format!(
            "[{}{}] {pct:.1}%",
            "█".repeat(blocks),
            "░".repeat(10 - blocks)
        );
        let speed = format!(
            "DL: {}/s | UL: {}/s",
            format_size(t.dlspeed),
            format_size(t.upspeed)
        );
        lines.push(format!(
            "📦 <b>{}</b>\n<code>{bar}</code> • <b>{}</b>\nSize: {} | {speed} | ETA: {}\n",
            t.name,
            t.state,
            format_size(t.total_size),
            format_eta(t.eta)
        ));
    }

    if torrents.len() > 5 {
        lines.push(format!(
            "<i>...and {} more torrents</i>",
            torrents.len() - 5
        ));
    }
    lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    lines.join("\n")
}

pub fn format_disk_report() -> String {
    let (total, used, free) = get_disk_info("/home/neko/torrents").unwrap_or((0, 0, 0));
    let pct = (used * 100).checked_div(total).unwrap_or(0);

    format!(
        "💾 <b>Disk Usage • mochi</b>\n━━━━━━━━━━━━━━━━━━━━━━━\n<b>Total:</b> {}\n<b>Used:</b>  {} ({pct}%)\n<b>Free:</b>  {}\n━━━━━━━━━━━━━━━━━━━━━━━",
        format_size(total),
        format_size(used),
        format_size(free)
    )
}

fn format_eta(eta: i64) -> String {
    if eta <= 0 || eta >= 8_640_000 {
        "∞".to_string()
    } else if eta < 60 {
        format!("{eta}s")
    } else if eta < 3600 {
        format!("{}m {}s", eta / 60, eta % 60)
    } else {
        format!("{}h {}m", eta / 3600, (eta % 3600) / 60)
    }
}

fn get_disk_info(path: &str) -> Option<(u64, u64, u64)> {
    let c_path = CString::new(path).ok()?;
    let mut stat = MaybeUninit::<libc::statvfs>::uninit();
    let res = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if res == 0 {
        let s = unsafe { stat.assume_init() };
        let total = s.f_blocks * s.f_frsize;
        let free = s.f_bavail * s.f_frsize;
        let used = total.saturating_sub(free);
        Some((total, used, free))
    } else {
        None
    }
}
