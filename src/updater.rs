use anyhow::{bail, Context, Result};
use colored::Colorize;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

    let archive = download_asset(&client, &asset.browser_download_url)?;
    let self_path = std::env::current_exe().context("Failed to locate current executable")?;
    install_from_archive(&archive, &self_path)?;

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

/// Extracts the release tarball and atomically swaps in the contained binary.
///
/// The release asset is a gzipped tar holding a single `ryoiki` executable.
/// Writing the raw archive bytes to the binary path (the previous behaviour)
/// corrupted the install into gzip data, so every service using it failed with
/// `Exec format error`. Extract first, then rename the real ELF into place.
fn install_from_archive(archive: &[u8], self_path: &Path) -> Result<()> {
    let tmp_dir = unique_temp_dir()?;
    let result = extract_and_swap(archive, self_path, &tmp_dir);
    let _ = fs::remove_dir_all(&tmp_dir);
    result
}

fn unique_temp_dir() -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("ryoiki-update-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).context("Failed to create temporary update directory")?;
    Ok(dir)
}

fn extract_and_swap(archive: &[u8], self_path: &Path, tmp_dir: &Path) -> Result<()> {
    let tarball = tmp_dir.join("ryoiki.tar.gz");
    fs::write(&tarball, archive).context("Failed to write release archive")?;

    let output = Command::new("tar")
        .args(["-xzf"])
        .arg(&tarball)
        .arg("-C")
        .arg(tmp_dir)
        .output()
        .context("Failed to spawn tar (required for self-update)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to extract release archive: {}", stderr.trim());
    }

    let extracted = tmp_dir.join("ryoiki");
    if !extracted.is_file() {
        bail!("Release archive did not contain a 'ryoiki' binary");
    }

    replace_binary(self_path, &extracted)
}

fn replace_binary(self_path: &Path, new_binary: &Path) -> Result<()> {
    let tmp = self_path.with_extension("tmp");
    fs::copy(new_binary, &tmp).context("Failed to stage new binary")?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))
        .context("Failed to chmod new binary")?;
    fs::rename(&tmp, self_path).context("Failed to replace binary (try with sudo?)")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(prefix: &str) -> Result<PathBuf> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Builds a `.tar.gz` holding a single `ryoiki` file with the given payload.
    fn make_tarball(payload: &[u8]) -> Result<Vec<u8>> {
        let staging = scratch_dir("ryoiki-tar-src")?;
        fs::write(staging.join("ryoiki"), payload)?;

        let tarball = staging.with_extension("tar.gz");
        let status = Command::new("tar")
            .args(["-czf"])
            .arg(&tarball)
            .arg("-C")
            .arg(&staging)
            .arg("ryoiki")
            .status()?;
        assert!(status.success(), "tar creation failed");

        let bytes = fs::read(&tarball)?;
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_file(&tarball);
        Ok(bytes)
    }

    #[test]
    fn test_install_from_archive_extracts_binary() -> Result<()> {
        let payload = b"FAKE-ELF-PAYLOAD-NOT-GZIP";
        let archive = make_tarball(payload)?;

        let target_dir = scratch_dir("ryoiki-swap-target")?;
        let target = target_dir.join("ryoiki");
        fs::write(&target, b"OLD-BINARY")?;

        install_from_archive(&archive, &target)?;

        let installed = fs::read(&target)?;
        assert_eq!(
            installed, payload,
            "swapped binary must be the extracted payload, not the raw .tar.gz"
        );
        let mode = fs::metadata(&target)?.permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "installed binary must be executable");

        let _ = fs::remove_dir_all(&target_dir);
        Ok(())
    }

    #[test]
    fn test_install_from_archive_rejects_non_archive() -> Result<()> {
        let target_dir = scratch_dir("ryoiki-bad-target")?;
        let target = target_dir.join("ryoiki");
        fs::write(&target, b"ORIGINAL")?;

        let err = install_from_archive(b"this is not a gzip tarball", &target);
        assert!(
            err.is_err(),
            "garbage input must fail, not corrupt the binary"
        );
        // The original binary must be untouched on failure.
        assert_eq!(fs::read(&target)?, b"ORIGINAL");

        let _ = fs::remove_dir_all(&target_dir);
        Ok(())
    }

    /// Live end-to-end: downloads the real GitHub release asset, installs it into
    /// a temp path, and asserts the result is a runnable ELF reporting `--version`.
    /// Gated by `RYOIKI_UPDATE_LIVE` so it stays off the network in CI.
    #[test]
    fn test_live_update_from_github() -> Result<()> {
        if std::env::var("RYOIKI_UPDATE_LIVE").is_err() {
            return Ok(());
        }

        let client = Client::builder()
            .user_agent("ryoiki-updater-test")
            .build()?;
        let release = fetch_latest_release(&client)?;
        let asset = find_asset(&release.assets, detect_arch())?;
        let archive = download_asset(&client, &asset.browser_download_url)?;
        println!("live: downloaded {} bytes of {}", archive.len(), asset.name);

        let target_dir = scratch_dir("ryoiki-live-target")?;
        let target = target_dir.join("ryoiki");
        install_from_archive(&archive, &target)?;

        // Must be a real executable, not gzip data.
        let magic = fs::read(&target)?;
        assert_eq!(
            &magic[..4],
            b"\x7fELF",
            "installed file must be an ELF binary"
        );

        let output = Command::new(&target).arg("--version").output()?;
        assert!(output.status.success(), "installed binary failed to run");
        let version = String::from_utf8_lossy(&output.stdout);
        println!("live: installed binary reports: {}", version.trim());
        assert!(version.contains("ryoiki"), "unexpected --version output");

        let _ = fs::remove_dir_all(&target_dir);
        Ok(())
    }
}
