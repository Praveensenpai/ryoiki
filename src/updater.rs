use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const REPO: &str = "Praveensenpai/ryoiki";
const API_BASE: &str = "https://api.github.com";

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Checks the latest GitHub release and self-updates the binary if a newer version is available.
pub fn run_self_update(current_version: &str) -> Result<()> {
    println!("\n  {} Checking for updates...", "🔍".bold());

    let client = Client::builder()
        .user_agent("ryoiki-updater")
        .build()
        .context("Failed to build HTTP client")?;

    let release = fetch_latest_release(&client)?;
    let latest = release.tag_name.trim_start_matches('v');

    if latest == current_version {
        println!(
            "  {} Already up to date ({}).",
            "✔".green().bold(),
            current_version.cyan()
        );
        return Ok(());
    }

    println!(
        "  {} Update available: {} → {}",
        "▶".cyan().bold(),
        current_version.dimmed(),
        latest.cyan().bold()
    );

    let arch = detect_arch();
    let asset = find_asset(&release.assets, arch)?;

    println!("  {} Downloading {}...", "⬇".cyan(), asset.name.bold());

    let bytes = download_asset(&client, &asset.browser_download_url)?;
    let self_path = std::env::current_exe().context("Failed to locate current executable")?;
    replace_binary(&self_path, &bytes)?;

    println!(
        "  {} Updated to {} successfully. Restart ryoiki to use new version.",
        "✔".green().bold(),
        latest.cyan().bold()
    );
    Ok(())
}

fn fetch_latest_release(client: &Client) -> Result<GithubRelease> {
    let url = format!("{API_BASE}/repos/{REPO}/releases/latest");
    client
        .get(&url)
        .send()
        .context("Failed to reach GitHub API")?
        .json::<GithubRelease>()
        .context("Failed to parse GitHub release JSON")
}

fn detect_arch() -> &'static str {
    let output = std::process::Command::new("uname")
        .arg("-m")
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"x86_64".to_vec(),
            stderr: vec![],
        });
    let arch = String::from_utf8_lossy(&output.stdout);
    if arch.trim() == "aarch64" {
        "aarch64"
    } else {
        "x86_64"
    }
}

fn find_asset<'a>(assets: &'a [GithubAsset], arch: &str) -> Result<&'a GithubAsset> {
    assets
        .iter()
        .find(|a| a.name.contains(arch) && !a.name.ends_with(".sha256"))
        .with_context(|| format!("No release asset found for arch: {arch}"))
}

fn download_asset(client: &Client, url: &str) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .context("Failed to download release asset")?;
    let bytes = resp.bytes().context("Failed to read asset bytes")?;
    Ok(bytes.to_vec())
}

fn replace_binary(self_path: &PathBuf, bytes: &[u8]) -> Result<()> {
    let tmp = self_path.with_extension("tmp");
    fs::write(&tmp, bytes).context("Failed to write temporary binary")?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))
        .context("Failed to chmod new binary")?;
    fs::rename(&tmp, self_path).context("Failed to replace binary (try with sudo?)")?;
    Ok(())
}
