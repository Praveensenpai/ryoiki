use super::actions::handle_charge_limit;
use super::maintenance::{handle_bot_audio, handle_bot_check};
use super::router::extract_command;
use super::services::{
    get_docker_containers, get_docker_logs, get_managed_services, resolve_managed_unit,
};
use super::system::collect_system_metrics;
use super::torrents::handle_seedr_cmd;
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
        session_cooldown_mins: 60,
        timezone: None,
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
    let status = handle_seedr_cmd("status", 9119);
    assert!(status.contains("Seedr"));
}
