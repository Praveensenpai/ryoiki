use super::actions::handle_charge_limit;
use super::maintenance::{handle_bot_audio, handle_bot_check};
use super::router::extract_command;
use super::services::{
    get_docker_containers, get_docker_logs, get_managed_services, resolve_managed_unit,
};
use super::system::collect_system_metrics;
use super::torrents::{handle_seedr_cmd, render_torrent_report, seedr_magnet_target};
use super::ui::{
    render_docker_view, render_maintenance_view, render_poweroff_confirm, render_reboot_confirm,
    render_services_view, render_storage_view, render_system_view,
};
use crate::notify::config::TelegramConfig;

fn dummy_config() -> TelegramConfig {
    TelegramConfig {
        bot_token: "123:ABC".to_string(),
        chat_id: "999".to_string(),
        qbittorrent_url: "http://localhost:6881".to_string(),
        server_name: Some("test-server".to_string()),
        api_port: 9119,
        gemini_api_key: None,
        gemini_model: None,
        deepseek_url: None,
        deepseek_model: None,
        deepseek_api_key: None,
        enable_deepseek: None,
        jellyfin_url: "http://localhost:8096".to_string(),
        jellyfin_api_key: None,
        session_cooldown_mins: crate::notify::config::DEFAULT_LOGIN_WINDOW_MINS,
        timezone: None,
        seedr_queue_policy: "fifo".to_string(),
        seedr_prompt_timeout_secs: 120,
        seedr_max_active_secs: 9_000,
        seedr_slow_speed_bps: 307_200,
        seedr_slow_grace_secs: 300,
        seedr_max_torrent_mb: 4_999,
    }
}

#[test]
fn test_smoke_system_telemetry_live() {
    let cfg = dummy_config();
    let sys = collect_system_metrics(&cfg);

    assert!(!sys.host.is_empty(), "Hostname should be populated");
    assert!(!sys.uptime.is_empty(), "Uptime should be populated");
    assert!(sys.mem_total > 0, "Total memory should be non-zero");

    let (text, kb) = render_system_view(&sys);
    assert!(text.contains("Host Telemetry"));
    assert!(text.contains(&sys.host));
    assert!(!kb.inline_keyboard.is_empty());
}

#[test]
fn test_smoke_storage_view_live() {
    let cfg = dummy_config();
    let sys = collect_system_metrics(&cfg);
    let (text, kb) = render_storage_view(&sys.disks, sys.gdrive_info.as_deref());

    assert!(text.contains("Storage Health"));
    assert!(!kb.inline_keyboard.is_empty());
}

#[test]
fn test_smoke_docker_and_services_live() {
    let containers_res = get_docker_containers();
    assert!(containers_res.is_ok(), "Docker command should succeed");
    let containers = containers_res.unwrap_or_default();
    let (docker_text, docker_kb) = render_docker_view(&containers);
    assert!(docker_text.contains("Docker Containers"));
    assert!(!docker_kb.inline_keyboard.is_empty());

    let services = get_managed_services();
    assert!(!services.is_empty(), "Managed services must be configured");
    let (svc_text, svc_kb) = render_services_view(&services);
    assert!(svc_text.contains("Managed Services"));
    assert!(!svc_kb.inline_keyboard.is_empty());
}

#[test]
fn test_smoke_docker_logs_live_or_bail() {
    let res = get_docker_logs("qbittorrent", 5);
    if let Ok(logs) = res {
        assert!(!logs.is_empty(), "Logs should contain output if running");
    }

    let bad = get_docker_logs("nonexistent-container-xyz-123", 5);
    assert!(bad.is_err(), "Nonexistent container logs must return error");
}

#[test]
fn test_smoke_maintenance_and_server_controls() {
    let check = handle_bot_check();
    assert!(check.contains("System Tool Audit"));

    let audio = handle_bot_audio("");
    assert!(audio.is_ok());

    let charge = handle_charge_limit("status");
    assert!(charge.contains("Battery Charge Control"));

    let (maint_text, maint_kb) = render_maintenance_view();
    assert!(maint_text.contains("Server Maintenance"));
    assert!(!maint_kb.inline_keyboard.is_empty());

    let (reboot_text, reboot_kb) = render_reboot_confirm();
    assert!(reboot_text.contains("CONFIRM SYSTEM REBOOT"));
    assert!(!reboot_kb.inline_keyboard.is_empty());

    let (power_text, power_kb) = render_poweroff_confirm();
    assert!(power_text.contains("CONFIRM SYSTEM POWER OFF"));
    assert!(!power_kb.inline_keyboard.is_empty());
}

#[test]
fn test_smoke_command_extraction_matrix() {
    let commands = [
        ("/status", "/status"),
        ("/system", "/system"),
        ("/sys", "/sys"),
        ("/storage", "/storage"),
        ("/disk", "/disk"),
        ("/docker", "/docker"),
        ("/ps", "/ps"),
        ("/services", "/services"),
        ("/service", "/service"),
        ("/torrent", "/torrent"),
        ("/torrents", "/torrents"),
        ("/seedr", "/seedr"),
        ("/pause", "/pause"),
        ("/resume", "/resume"),
        ("/organize", "/organize"),
        ("/organise", "/organise"),
        ("/prune", "/prune"),
        ("/sync", "/sync"),
        ("/refresh", "/refresh"),
        ("/audio", "/audio"),
        ("/charge", "/charge"),
        ("/check", "/check"),
        ("/reboot", "/reboot"),
        ("/poweroff", "/poweroff"),
        ("/update", "/update"),
        ("/help", "/help"),
        ("/start", "/start"),
    ];

    for (cmd, expected) in commands {
        let parsed = extract_command(cmd);
        assert_eq!(parsed.map(|(c, _)| c), Some(expected));

        let with_bot = format!("{cmd}@ryoiki_bot");
        let parsed_bot = extract_command(&with_bot);
        assert_eq!(parsed_bot.map(|(c, _)| c), Some(expected));
    }
}

#[test]
fn test_smoke_service_whitelist_resolution() {
    assert_eq!(
        resolve_managed_unit("ryoiki-bot.service"),
        Some(("ryoiki-bot.service", true))
    );
    assert_eq!(
        resolve_managed_unit("docker"),
        Some(("docker.service", false))
    );
    assert_eq!(
        resolve_managed_unit("tailscaled"),
        Some(("tailscaled.service", false))
    );
    assert_eq!(resolve_managed_unit("random_service"), None);
}

#[test]
fn test_smoke_seedr_command() {
    let config = dummy_config();
    let client = reqwest::blocking::Client::new();
    let status = handle_seedr_cmd(&client, &config, "status");
    assert!(status.contains("Seedr"));
}

#[test]
fn test_seedr_cmd_routes_magnets_to_queue() {
    // A magnet must be recognized as queueable so `/seedr <magnet>` takes the
    // scheduler path instead of bypassing the single-slot queue.
    let mag = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=Route.Test.mkv";
    let (hash, name) = seedr_magnet_target(mag).unwrap_or_else(|| panic!("magnet must parse"));
    assert_eq!(hash, "0123456789abcdef0123456789abcdef01234567");
    assert_eq!(name, "Route.Test.mkv");

    // Raw infohash also qualifies.
    assert!(seedr_magnet_target("0123456789abcdef0123456789abcdef01234567").is_some());

    // A plain torrent URL has no hash to track, so it must not be queued.
    assert!(seedr_magnet_target("https://example.com/file.torrent").is_none());
}

/// Live end-to-end render of `/torrents` against the running qBittorrent
/// instance. Prints the exact Telegram message body and inline keyboard JSON
/// for every reachable page so pagination can be inspected with real data.
#[test]
fn test_live_torrent_pagination_render() {
    let Ok(cfg) = TelegramConfig::load() else {
        eprintln!("SKIP: no Telegram/qBittorrent config on this machine");
        return;
    };
    let client = reqwest::blocking::Client::new();
    let torrents =
        match crate::modules::torrent::api::get_torrents(&client, &cfg.qbittorrent_url, None) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("SKIP: qBittorrent unreachable: {e}");
                return;
            }
        };

    let pages = crate::modules::torrent::report::torrent_page_count(torrents.len());
    eprintln!("LIVE torrents={} pages={pages}", torrents.len());
    assert!(pages >= 1, "at least one page must exist");

    for page in 1..=pages {
        let (text, kb) = match render_torrent_report(&client, &cfg.qbittorrent_url, page) {
            Ok(rendered) => rendered,
            Err(e) => panic!("render must succeed against live qBittorrent: {e}"),
        };
        let keyboard_json = match serde_json::to_string(&kb) {
            Ok(json) => json,
            Err(e) => panic!("keyboard must serialize: {e}"),
        };

        eprintln!("\n===== /torrents PAGE {page}/{pages} =====\n{text}\n----- keyboard -----\n{keyboard_json}");

        assert!(
            !text.contains("more torrents"),
            "overflow text must be gone"
        );
        assert!(
            text.contains(&format!("</code>{page}/{pages} • ")),
            "PAGE row must show the current page"
        );
        for row in &kb.inline_keyboard {
            for btn in row {
                assert!(
                    btn.callback_data.as_ref().is_some_and(|d| d.len() <= 64),
                    "callback data must fit Telegram's 64-byte limit"
                );
            }
        }
    }

    // Out-of-range page clamps to the last page.
    let clamped = match render_torrent_report(&client, &cfg.qbittorrent_url, pages + 50) {
        Ok((text, _)) => text,
        Err(e) => panic!("clamped render must succeed: {e}"),
    };
    assert!(clamped.contains(&format!("</code>{pages}/{pages} • ")));
}

/// Live read-only inspection of the hourly Seedr retry decision. Uses the real
/// qBittorrent list and real Seedr cloud list; never submits anything.
#[test]
fn test_live_hourly_seedr_candidate() {
    let Ok(cfg) = TelegramConfig::load() else {
        eprintln!("SKIP: no Telegram/qBittorrent config on this machine");
        return;
    };
    let client = reqwest::blocking::Client::new();
    let torrents =
        match crate::modules::torrent::api::get_torrents(&client, &cfg.qbittorrent_url, None) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("SKIP: qBittorrent unreachable: {e}");
                return;
            }
        };

    let live = crate::modules::torrent::seedr_tasks::fetch_live_list();
    let cloud_names: std::collections::HashSet<String> = live
        .as_ref()
        .map(|list| {
            use crate::modules::torrent::seedr_tasks::base_key;
            list.torrents
                .iter()
                .map(|t| base_key(&t.name))
                .chain(list.folders.iter().map(|f| base_key(&f.name)))
                .chain(list.files.iter().map(|f| base_key(&f.name)))
                .collect()
        })
        .unwrap_or_default();
    let queued = crate::modules::torrent::queue::load();
    let queued_hashes: std::collections::HashSet<String> =
        queued.entries.iter().map(|e| e.hash.clone()).collect();

    let max_bytes = cfg.seedr_max_torrent_mb.saturating_mul(1_048_576);
    let picked = crate::modules::torrent::seedr_retry::pick_candidate(
        &torrents,
        max_bytes,
        &cloud_names,
        &queued_hashes,
    );
    let available = crate::modules::torrent::seedr_tasks::seedr_available_bytes();

    eprintln!(
        "\nLIVE hourly retry: qb_torrents={} seedr_list={} free={:?} ceiling={}MB",
        torrents.len(),
        live.is_some(),
        available.map(|b| b / 1_048_576),
        cfg.seedr_max_torrent_mb
    );
    match picked {
        Some(t) => eprintln!(
            "  candidate: {} · {} MB · {} · metadata={}",
            t.name,
            t.total_size / 1_048_576,
            t.state,
            t.has_metadata
        ),
        None => eprintln!("  candidate: none (all skipped)"),
    }
    eprintln!(
        "  sweep would {}: {}",
        if available.is_some() { "run" } else { "pause" },
        if available.is_some() {
            "Seedr account reachable"
        } else {
            "Seedr quota unavailable"
        }
    );

    if let Some(t) = picked {
        assert!(t.has_metadata && t.total_size > 0);
        assert!(!t.is_completed());
        assert!(u64::try_from(t.total_size).is_ok_and(|s| s <= max_bytes));
    }
    assert_eq!(cfg.seedr_max_torrent_mb, 4_999);
}

/// Real magnet captured from a drushti-sourced 1TamilMV WEB-DL thread. Verifies
/// the admission decision path against genuine forum data, not fixtures.
#[test]
fn test_live_real_magnet_size_gate() {
    let magnet = concat!(
        "magnet:?xt=urn:btih:5d664cf513399dc489528368cad99d0acb07fae3",
        "&dn=www.1TamilMV.style%20-%20Sarala%20Subbarao%20%282026%29%20Kannada%20WEB-DL",
        "&xl=3391383552",
    );
    let hash = crate::modules::torrent::seedr::extract_btih_hash(magnet);
    let size = crate::modules::torrent::seedr::extract_magnet_size(magnet);
    assert_eq!(
        hash.as_deref(),
        Some("5d664cf513399dc489528368cad99d0acb07fae3")
    );
    assert_eq!(size, Some(3_391_383_552));

    let cfg = dummy_config();
    let ceiling = cfg.seedr_max_torrent_mb.saturating_mul(1_048_576);
    assert!(size.is_some_and(|s| s <= ceiling));

    eprintln!(
        "\nLIVE real magnet: hash={} size={} MB ceiling={} MB → {}",
        hash.as_deref().unwrap_or("?"),
        size.unwrap_or(0) / 1_048_576,
        cfg.seedr_max_torrent_mb,
        if size.is_some_and(|s| s <= ceiling) {
            "under ceiling, Seedr-eligible"
        } else {
            "oversized, qBittorrent only"
        }
    );
}
