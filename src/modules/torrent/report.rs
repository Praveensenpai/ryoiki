use super::api::TorrentInfo;
use super::notify::format_size;

pub fn format_status_report(torrents: &[TorrentInfo]) -> String {
    let seedr_tasks = super::seedr::get_active_seedr_tasks();
    let seedr_sec = super::seedr::format_seedr_tasks_section(&seedr_tasks);

    if torrents.is_empty() && seedr_tasks.is_empty() {
        return "🌊 <b>領域 RYOIKI • Torrents</b>\n━━━━━━━━━━━━━━━━━━━━━━━\nNo active or completed torrents found.".to_string();
    }

    let mut lines = vec!["🌊 <b>領域 RYOIKI • Torrents</b>\n━━━━━━━━━━━━━━━━━━━━━━━".to_string()];
    if !seedr_sec.is_empty() {
        lines.push(seedr_sec);
        lines.push(String::new());
    }

    if !torrents.is_empty() && !seedr_tasks.is_empty() {
        lines.push("📦 <b>qBittorrent Torrents:</b>".to_string());
    }

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
        let clean_name = crate::notify::client::escape_html(&t.name);
        let clean_state = crate::notify::client::escape_html(&t.state);
        let size_str = match u64::try_from(t.total_size) {
            Ok(sz) => format_size(sz),
            Err(_) => "Unknown".to_string(),
        };
        lines.push(format!(
            "📦 <b>{clean_name}</b>\n<code>{bar}</code> • <b>{clean_state}</b>\nSize: {size_str} | {speed} | ETA: {}\n",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status_report_escapes_html() {
        let torrents = vec![TorrentInfo {
            hash: "1234".into(),
            name: "Anime & Manga <Special> \"Edition\"".into(),
            progress: 0.5,
            dlspeed: 1024 * 1024,
            upspeed: 0,
            eta: 120,
            state: "downloading & active".into(),
            category: "anime".into(),
            total_size: 1024 * 1024 * 500,
            save_path: Some("/downloads".into()),
            content_path: Some("/downloads/file".into()),
            has_metadata: true,
        }];

        let report = format_status_report(&torrents);
        assert!(report.contains("Anime &amp; Manga &lt;Special&gt; &quot;Edition&quot;"));
        assert!(report.contains("downloading &amp; active"));
        assert!(!report.contains("Anime & Manga"));
    }

    #[test]
    fn test_format_status_report_meta_dl_negative_total_size() {
        let torrents = vec![TorrentInfo {
            hash: "meta1".into(),
            name: "Magnet Downloading Metadata".into(),
            progress: 0.0,
            dlspeed: 0,
            upspeed: 0,
            eta: 8_640_000,
            state: "metaDL".into(),
            category: String::new(),
            total_size: -1,
            save_path: Some("/downloads".into()),
            content_path: Some(String::new()),
            has_metadata: false,
        }];

        let report = format_status_report(&torrents);
        assert!(report.contains("Magnet Downloading Metadata"));
        assert!(report.contains("Size: Unknown"));
        assert!(report.contains("metaDL"));
    }
}
