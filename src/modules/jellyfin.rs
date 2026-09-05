use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Sets up Jellyfin media server with Docker Compose and Intel GPU hardware acceleration.
pub fn setup(runner: &mut Runner) -> Result<()> {
    ensure_docker(runner)?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let jellyfin_dir = Path::new(&home).join("jellyfin");

    create_directories(runner, &jellyfin_dir)?;
    deploy_compose_file(runner, &jellyfin_dir)?;
    start_container(runner, &jellyfin_dir)?;
    configure_firewall(runner)?;
    print_access_info(&home);

    Ok(())
}

fn ensure_docker(runner: &mut Runner) -> Result<()> {
    if !Runner::command_exists("docker") {
        super::docker::setup(runner)?;
    }
    Ok(())
}

fn create_directories(runner: &Runner, base: &Path) -> Result<()> {
    let dirs = [
        base.join("config"),
        base.join("cache"),
        base.join("media/movies"),
        base.join("media/shows"),
        base.join("media/music"),
    ];

    for dir in &dirs {
        if !runner.dry_run && !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }
    }
    Ok(())
}

fn deploy_compose_file(runner: &Runner, base: &Path) -> Result<()> {
    let compose_file = base.join("docker-compose.yml");
    if compose_file.exists() {
        return Ok(());
    }

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let has_dri = Path::new("/dev/dri").exists();

    let compose_content = generate_compose_yaml(uid, gid, has_dri);

    if runner.dry_run {
        println!("  • [dry-run] Create {}", compose_file.display());
    } else {
        fs::write(&compose_file, compose_content)
            .with_context(|| format!("Failed to write: {}", compose_file.display()))?;
    }
    Ok(())
}

fn generate_compose_yaml(uid: u32, gid: u32, has_dri: bool) -> String {
    let gpu_config = if has_dri {
        r#"    group_add:
      - "991"
      - "44"
    devices:
      - /dev/dri:/dev/dri
"#
    } else {
        ""
    };

    format!(
        r#"services:
  jellyfin:
    image: jellyfin/jellyfin:latest
    container_name: jellyfin
    user: "{uid}:{gid}"
{gpu_config}    volumes:
      - ./config:/config
      - ./cache:/cache
      - ./media:/media
    ports:
      - "8096:8096"
    restart: unless-stopped
"#
    )
}

fn start_container(runner: &mut Runner, base: &Path) -> Result<()> {
    let dir_str = base.to_string_lossy();
    runner.exec_bash(
        "Starting Jellyfin media server...",
        &format!("cd '{dir_str}' && docker compose up -d"),
    )
}

fn configure_firewall(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("ufw") {
        runner.exec_silent(
            "Allowing Jellyfin port (8096/tcp) in UFW...",
            "sudo",
            &["ufw", "allow", "8096/tcp"],
        )?;
    }
    Ok(())
}

fn print_access_info(home: &str) {
    println!(
        "  {} Jellyfin web UI: http://<server-ip>:8096",
        "✔".green().bold()
    );
    println!("  {} Media directory: {home}/jellyfin/media", "•".dimmed());
}
