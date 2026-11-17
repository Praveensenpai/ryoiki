use super::api::TorrentInfo;
use super::notify::format_size;
use crate::notify::client::escape_html;
use crate::notify::style;

/// Number of qBittorrent entries rendered per page.
pub const TORRENTS_PER_PAGE: usize = 5;

/// Returns the number of pages needed for `total` torrents, never below one.
#[must_use]
pub fn torrent_page_count(total: usize) -> usize {
    total.div_ceil(TORRENTS_PER_PAGE).max(1)
}

/// Clamps a requested 1-based page into the valid range.
#[must_use]
pub fn clamp_torrent_page(page: usize, total: usize) -> usize {
    page.clamp(1, torrent_page_count(total))
}

/// Renders the torrent report for a single 1-based page.
#[must_use]
pub fn format_status_report(torrents: &[TorrentInfo], page: usize) -> String {
    let seedr_tasks = super::seedr::get_active_seedr_tasks();
    let seedr_sec = super::seedr::format_seedr_tasks_section(&seedr_tasks);
    let page = clamp_torrent_page(page, torrents.len());
    let mut lines = vec![style::view("TORRENTS"), String::new()];

    if !seedr_sec.is_empty() {
        lines.push(style::eyebrow("SEEDR CLOUD"));
        lines.push(seedr_sec);
        lines.push(String::new());
    }

    if torrents.is_empty() {
        if seedr_tasks.is_empty() {
            lines.push("No active or completed torrents found.".to_string());
        }
        return lines.join("\n");
    }

    let start = (page - 1) * TORRENTS_PER_PAGE;
    for torrent in torrents.iter().skip(start).take(TORRENTS_PER_PAGE) {
        lines.push(render_torrent_entry(torrent));
    }

    lines.push(String::new());
    lines.push(style::row(
        "PAGE",
        &format!(
            "{page}/{} • {} torrents",
            torrent_page_count(torrents.len()),
            torrents.len()
        ),
    ));
    lines.join("\n")
}

/// Renders one torrent as a compact three-line block.
fn render_torrent_entry(torrent: &TorrentInfo) -> String {
    let pct = (torrent.progress * 100.0).clamp(0.0, 100.0);
    let blocks = format!("{:.0}", pct / 10.0)
        .parse::<usize>()
        .unwrap_or(0)
        .min(10);
    let bar = format!(
        "[{}{}] {pct:.1}%",
        "█".repeat(blocks),
        "░".repeat(10 - blocks)
    );
    let size = match u64::try_from(torrent.total_size) {
        Ok(bytes) => format_size(bytes),
        Err(_) => "Unknown".to_string(),
    };
    let name = escape_html(&torrent.name);
    let state = escape_html(&torrent.state);
    let state_key = style::key("STATE");
    let size_key = style::key("SIZE");
    format!(
        "<b>{name}</b>\n{state_key}<code>{bar}</code>  {state}\n\
         {size_key}{size}  ↓ {}  ↑ {}  η {}",
        format_size(torrent.dlspeed),
        format_size(torrent.upspeed),
        format_eta(torrent.eta)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn torrent(name: &str) -> TorrentInfo {
        TorrentInfo {
            hash: name.into(),
            name: name.into(),
            progress: 0.5,
            dlspeed: 1024 * 1024,
            upspeed: 0,
            eta: 120,
            state: "downloading".into(),
            category: "anime".into(),
            total_size: 1024 * 1024 * 500,
            save_path: Some("/downloads".into()),
            content_path: Some("/downloads/file".into()),
            has_metadata: true,
        }
    }

    #[test]
    fn test_page_count_and_clamp() {
        assert_eq!(torrent_page_count(0), 1);
        assert_eq!(torrent_page_count(5), 1);
        assert_eq!(torrent_page_count(6), 2);
        assert_eq!(torrent_page_count(11), 3);
        assert_eq!(clamp_torrent_page(9, 11), 3);
        assert_eq!(clamp_torrent_page(0, 11), 1);
    }

    #[test]
    fn test_report_paginates_five_per_page() {
        let torrents: Vec<_> = (1..=12)
            .map(|i| torrent(&format!("Release.{i:02}")))
            .collect();
        let first = format_status_report(&torrents, 1);
        let second = format_status_report(&torrents, 2);
        let third = format_status_report(&torrents, 3);

        assert!(first.contains("Release.01"));
        assert!(first.contains("Release.05"));
        assert!(!first.contains("Release.06"));
        assert!(first.contains("1/3 • 12 torrents"));

        assert!(second.contains("Release.06"));
        assert!(second.contains("Release.10"));
        assert!(!second.contains("Release.01"));
        assert!(second.contains("2/3 • 12 torrents"));

        assert!(third.contains("Release.11"));
        assert!(third.contains("Release.12"));
        assert!(third.contains("3/3 • 12 torrents"));
        assert!(!third.contains("...and"));
        assert!(!third.contains("more torrents"));
    }

    #[test]
    fn test_report_clamps_out_of_range_page() {
        let torrents: Vec<_> = (1..=6)
            .map(|i| torrent(&format!("Release.{i:02}")))
            .collect();
        let out = format_status_report(&torrents, 99);
        assert!(out.contains("2/2 • 6 torrents"));
        assert!(out.contains("Release.06"));
    }

    #[test]
    fn test_format_status_report_escapes_html() {
        let mut item = torrent("x");
        item.name = "Anime & Manga <Special> \"Edition\"".into();
        item.state = "downloading & active".into();
        let report = format_status_report(&[item], 1);
        assert!(report.contains("Anime &amp; Manga &lt;Special&gt; &quot;Edition&quot;"));
        assert!(report.contains("downloading &amp; active"));
        assert!(!report.contains("Anime & Manga"));
    }

    #[test]
    fn test_format_status_report_meta_dl_negative_total_size() {
        let mut item = torrent("Magnet Downloading Metadata");
        item.total_size = -1;
        item.state = "metaDL".into();
        let report = format_status_report(&[item], 1);
        assert!(report.contains("Magnet Downloading Metadata"));
        assert!(report.contains("<code>SIZE       </code>Unknown"));
        assert!(report.contains("metaDL"));
    }
}
