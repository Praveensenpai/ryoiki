use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::path::Path;
use std::time::Duration;

use crate::notify::TelegramConfig;

/// Sends a library refresh signal to Jellyfin server via its REST API.
pub fn trigger_library_refresh(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<()> {
    let url = format!("{}/Library/Refresh", base_url.trim_end_matches('/'));
    let mut req = client.post(&url);

    if let Some(key) = api_key {
        let auth = format!("MediaBrowser Token=\"{key}\"");
        req = req.header("Authorization", auth);
    }

    let resp = req.send().context("Failed to connect to Jellyfin server")?;
    if !resp.status().is_success() {
        anyhow::bail!("Jellyfin Library/Refresh returned status {}", resp.status());
    }

    Ok(())
}

/// Automatically resolves configuration and triggers a background library refresh.
pub fn refresh_library_auto() -> Result<()> {
    let (url, key) = resolve_jellyfin_target();
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client for Jellyfin")?;

    trigger_library_refresh(&client, &url, key.as_deref())
}

/// Helper that spawns a thread to refresh library without blocking the caller.
pub fn refresh_library_async() {
    std::thread::spawn(|| {
        if let Err(e) = refresh_library_auto() {
            eprintln!("  ⚠️ Jellyfin library refresh signal failed: {e}");
        } else {
            println!("  ✔ Sent instant library sync signal to Jellyfin");
        }
    });
}

fn resolve_jellyfin_target() -> (String, Option<String>) {
    if let Ok(env_key) = std::env::var("JELLYFIN_API_KEY") {
        let trimmed = env_key.trim();
        if !trimmed.is_empty() {
            let url = std::env::var("JELLYFIN_URL")
                .unwrap_or_else(|_| "http://localhost:8096".to_string());
            return (url, Some(trimmed.to_string()));
        }
    }

    if let Ok(cfg) = TelegramConfig::load() {
        if let Some(k) = cfg.jellyfin_api_key {
            let trimmed = k.trim();
            if !trimmed.is_empty() {
                return (cfg.jellyfin_url, Some(trimmed.to_string()));
            }
        }
    }

    let default_url = "http://localhost:8096".to_string();
    let discovered = extract_key_from_local_db();
    (default_url, discovered)
}

const JELLYFIN_DB_QUERY: &str = r#"import sqlite3
try:
    c = sqlite3.connect("{db_path}")
    row = c.cursor().execute('SELECT AccessToken FROM ApiKeys ORDER BY Id DESC LIMIT 1').fetchone()
    if row:
        print(row[0])
except Exception:
    pass"#;

fn extract_key_from_local_db() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let db_path = Path::new(&home).join("jellyfin/config/data/jellyfin.db");
    if !db_path.exists() {
        return None;
    }

    let script = JELLYFIN_DB_QUERY.replace("{db_path}", &db_path.display().to_string());

    let output = std::process::Command::new("python3")
        .args(["-c", &script])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::JELLYFIN_DB_QUERY;

    #[test]
    fn jellyfin_db_query_is_valid_python() {
        let script = JELLYFIN_DB_QUERY.replace("{db_path}", "/tmp/does-not-exist.db");
        let compile = format!("compile({script:?}, '<test>', 'exec')");
        let Ok(output) = std::process::Command::new("python3")
            .args(["-c", &compile])
            .output()
        else {
            return;
        };
        assert!(
            output.status.success(),
            "generated Jellyfin DB query is not valid Python:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
