use crate::runner::Runner;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

mod telegram;

/// Sets up qBittorrent server directly with Docker without compose files.
pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    ensure_docker(runner)?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let config_dir = Path::new(&home).join(".config/qbittorrent");
    let download_dir = Path::new(&home).join("torrents");

    create_directories(runner, &config_dir, &download_dir)?;

    let creds = prompt_credentials(runner, non_interactive)?;
    if let Some((user, pass)) = &creds {
        let hash = hash_password(pass)?;
        apply_credentials(&config_dir, user, &hash)?;
    }

    let tg_config = telegram::prompt_telegram_config(runner, non_interactive)?;
    let tg_installed = if let Some(cfg) = &tg_config {
        telegram::install_notification_script(&config_dir, cfg)?;
        true
    } else {
        config_dir.join("scripts").join("telegram_notify.sh").exists()
    };

    if !runner.dry_run {
        apply_default_preferences(&config_dir, tg_installed)?;
    }

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    start_container(runner, &config_dir, &download_dir, (uid, gid))?;
    configure_firewall(runner);
    print_access_info(&home, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())));

    Ok(())
}

fn ensure_docker(runner: &mut Runner) -> Result<()> {
    if !Runner::command_exists("docker") {
        super::docker::setup(runner)?;
    }
    Ok(())
}

fn create_directories(runner: &Runner, config_dir: &Path, download_dir: &Path) -> Result<()> {
    for dir in [config_dir, download_dir] {
        if !runner.dry_run && !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }
    }
    Ok(())
}

fn prompt_credentials(runner: &Runner, non_interactive: bool) -> Result<Option<(String, String)>> {
    if non_interactive || runner.dry_run {
        return Ok(None);
    }

    print!("  Configure custom WebUI credentials? [y/N]: ");
    io::stdout().flush()?;
    let mut choice = String::new();
    io::stdin().lock().read_line(&mut choice)?;
    if !choice.trim().eq_ignore_ascii_case("y") {
        return Ok(None);
    }

    print!("  Enter WebUI username [{}]: ", "admin".cyan());
    io::stdout().flush()?;
    let mut user = String::new();
    io::stdin().lock().read_line(&mut user)?;
    let user = user.trim();
    let final_user = if user.is_empty() { "admin" } else { user };

    print!("  Enter WebUI password: ");
    io::stdout().flush()?;
    let mut pass = String::new();
    io::stdin().lock().read_line(&mut pass)?;
    let pass = pass.trim();
    if pass.is_empty() {
        return Ok(None);
    }

    Ok(Some((final_user.to_string(), pass.to_string())))
}

fn hash_password(password: &str) -> Result<String> {
    let script = format!(
        "import hashlib, os, base64; salt=os.urandom(16); dk=hashlib.pbkdf2_hmac('sha512', {password:?}.encode(), salt, 100000); print(f'@ByteArray({{base64.b64encode(salt).decode()}}:{{base64.b64encode(dk).decode()}})')"
    );
    let output = std::process::Command::new("python3")
        .args(["-c", &script])
        .output()
        .context("Failed to run python3 for password hash")?;

    if !output.status.success() {
        bail!("Password hash generation failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn insert_into_section(lines: &mut Vec<String>, section: &str, entries: &[&str]) {
    if let Some(idx) = lines.iter().position(|l| l.trim() == section) {
        for (offset, entry) in entries.iter().enumerate() {
            lines.insert(idx + 1 + offset, (*entry).to_string());
        }
    } else {
        lines.push(section.to_string());
        for entry in entries {
            lines.push((*entry).to_string());
        }
    }
}

fn apply_default_preferences(config_dir: &Path, tg_installed: bool) -> Result<()> {
    let conf_dir = config_dir.join("qBittorrent");
    fs::create_dir_all(&conf_dir)?;
    let conf_path = conf_dir.join("qBittorrent.conf");

    let existing = if conf_path.exists() {
        fs::read_to_string(&conf_path).unwrap_or_default()
    } else {
        String::new()
    };

    let bt_defaults = [
        "Session\\DefaultSavePath=/downloads/",
        "Session\\TempPath=/downloads/incomplete/",
        "Session\\TempPathEnabled=true",
        "Session\\Preallocation=true",
        "Session\\AddExtensionToIncompleteFiles=true",
        "Session\\UseUnwantedFolder=true",
        "Session\\DisableAutoTMMByDefault=false",
        "Session\\DisableAutoTMMTriggers\\CategoryChanged=false",
        "Session\\DisableAutoTMMTriggers\\DefaultSavePathChanged=false",
        "Session\\DisableAutoTMMTriggers\\CategorySavePathChanged=false",
        "Session\\UseCategoryPathsInManualMode=false",
        "Session\\TorrentBackupEnabled=false",
        "Session\\FinishedTorrentBackupDirectoryEnabled=false",
    ];

    let pref_defaults = [
        "Downloads\\PreAllocation=true",
        "Downloads\\SavePath=/downloads/",
        "Downloads\\TempPath=/downloads/incomplete/",
        "Downloads\\TempPathEnabled=true",
        "Downloads\\UseIncompleteExtension=true",
        "WebUI\\AuthSubnetWhitelist=127.0.0.1/32, ::1/128, 172.17.0.1/32",
        "WebUI\\AuthSubnetWhitelistEnabled=true",
    ];

    let mut lines: Vec<String> = existing
        .lines()
        .filter(|l| {
            !bt_defaults.iter().any(|d| {
                let prefix = d.split('=').next().unwrap_or("");
                l.starts_with(prefix)
            }) && !pref_defaults.iter().any(|d| {
                let prefix = d.split('=').next().unwrap_or("");
                l.starts_with(prefix)
            })
        })
        .map(ToString::to_string)
        .collect();

    insert_into_section(&mut lines, "[BitTorrent]", &bt_defaults);
    insert_into_section(&mut lines, "[Preferences]", &pref_defaults);
    telegram::configure_autorun(&mut lines, tg_installed);

    fs::write(&conf_path, lines.join("\n") + "\n")?;
    Ok(())
}

fn apply_credentials(config_dir: &Path, username: &str, password_hash: &str) -> Result<()> {
    let conf_dir = config_dir.join("qBittorrent");
    fs::create_dir_all(&conf_dir)?;
    let conf_path = conf_dir.join("qBittorrent.conf");

    let existing = if conf_path.exists() {
        fs::read_to_string(&conf_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut lines: Vec<String> = existing
        .lines()
        .filter(|l| !l.starts_with("WebUI\\Username=") && !l.starts_with("WebUI\\Password_PBKDF2="))
        .map(ToString::to_string)
        .collect();

    let u_entry = format!("WebUI\\Username={username}");
    let p_entry = format!("WebUI\\Password_PBKDF2=\"{password_hash}\"");
    insert_into_section(&mut lines, "[Preferences]", &[&u_entry, &p_entry]);

    fs::write(&conf_path, lines.join("\n") + "\n")?;
    Ok(())
}

fn start_container(
    runner: &mut Runner,
    config: &Path,
    download: &Path,
    (uid, gid): (u32, u32),
) -> Result<()> {
    if runner.dry_run {
        println!("  • [dry-run] docker rm -f qbittorrent");
        println!("  • [dry-run] docker run -d --name qbittorrent --restart unless-stopped ...");
        return Ok(());
    }

    let _ = std::process::Command::new("docker")
        .args(["rm", "-f", "qbittorrent"])
        .output();

    let cfg_vol = format!("{}:/config", config.display());
    let dl_vol = format!("{}:/downloads", download.display());
    let user_id_env = format!("PUID={uid}");
    let group_id_env = format!("PGID={gid}");

    runner.exec_silent(
        "Starting qBittorrent container...",
        "docker",
        &[
            "run",
            "-d",
            "--name",
            "qbittorrent",
            "--restart",
            "unless-stopped",
            "-e",
            &user_id_env,
            "-e",
            &group_id_env,
            "-e",
            "TZ=Etc/UTC",
            "-e",
            "WEBUI_PORT=6881",
            "-e",
            "TORRENTING_PORT=6882",
            "-p",
            "6881:6881",
            "-p",
            "6882:6882",
            "-p",
            "6882:6882/udp",
            "-v",
            &cfg_vol,
            "-v",
            &dl_vol,
            "lscr.io/linuxserver/qbittorrent:latest",
        ],
    )?;

    std::thread::sleep(std::time::Duration::from_millis(1500));
    Ok(())
}

fn configure_firewall(runner: &mut Runner) {
    if !Runner::command_exists("ufw") {
        return;
    }

    let can_sudo = std::process::Command::new("sudo")
        .args(["-n", "true"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if can_sudo {
        let _ = runner.exec_silent(
            "Allowing qBittorrent Web UI port (6881/tcp) in UFW...",
            "sudo",
            &["ufw", "allow", "6881/tcp"],
        );
        let _ = runner.exec_silent(
            "Allowing BitTorrent peer ports (6882) in UFW...",
            "sudo",
            &["ufw", "allow", "6882"],
        );
    }
}

fn get_access_urls() -> (String, String) {
    let ts_ip = std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .map_or_else(
            |_| String::new(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
        );

    let hostname = std::process::Command::new("hostname").output().map_or_else(
        |_| String::new(),
        |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
    );

    (ts_ip, hostname)
}

fn get_initial_password() -> Option<String> {
    let output = std::process::Command::new("docker")
        .args(["logs", "qbittorrent"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for line in stdout.lines().chain(stderr.lines()) {
        if line.contains("temporary password is provided for this session:") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                return Some(parts[parts.len() - 1].trim().to_string());
            }
        }
    }
    None
}

fn print_access_info(home: &str, custom_creds: Option<(&str, &str)>) {
    let (ts_ip, host) = get_access_urls();
    let url = if ts_ip.is_empty() {
        if host.is_empty() {
            "http://<server-ip>:6881".to_string()
        } else {
            format!("http://{host}:6881")
        }
    } else {
        format!("http://{ts_ip}:6881")
    };

    println!("  {} qBittorrent web UI: {url}", "✔".green().bold());
    if !host.is_empty() && !ts_ip.is_empty() {
        println!("  {} MagicDNS URL: http://{host}:6881", "•".dimmed());
    }

    if let Some((user, pass)) = custom_creds {
        println!("  {} Username: {}", "•".dimmed(), user.cyan().bold());
        println!("  {} Password: {}", "✔".green().bold(), pass.cyan().bold());
    } else {
        println!("  {} Default username: admin", "•".dimmed());
        if let Some(pwd) = get_initial_password() {
            println!(
                "  {} Temporary password: {}",
                "✔".green().bold(),
                pwd.cyan().bold()
            );
        } else {
            println!(
                "  {} Check password: docker logs qbittorrent | grep -i password",
                "•".dimmed()
            );
        }
    }

    println!("  {} Downloads directory: {home}/torrents", "•".dimmed());
}
