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
    let key = match api_key {
        Some(k) if !k.trim().is_empty() => k.trim(),
        _ => anyhow::bail!(
            "No Jellyfin API key found (configure jellyfin_api_key in telegram.json, set JELLYFIN_API_KEY, or log into Jellyfin)"
        ),
    };

    let url = format!("{}/Library/Refresh", base_url.trim_end_matches('/'));
    let auth = format!("MediaBrowser Token=\"{key}\"");
    let resp = client
        .post(&url)
        .header("Authorization", auth)
        .header("X-Emby-Token", key)
        .send()
        .context("Failed to connect to Jellyfin server")?;

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
    if let Some(ref token) = discovered {
        if let Ok(mut cfg) = TelegramConfig::load() {
            if cfg.jellyfin_api_key.as_deref() != Some(token.as_str()) {
                cfg.jellyfin_api_key = Some(token.clone());
                let _ = cfg.save();
            }
        }
    }
    (default_url, discovered)
}

const JELLYFIN_DB_QUERY: &str = r#"import sqlite3
try:
    c = sqlite3.connect("{db_path}")
    token = None
    try:
        row = c.cursor().execute('SELECT AccessToken FROM ApiKeys WHERE AccessToken IS NOT NULL AND AccessToken != "" ORDER BY Id DESC LIMIT 1').fetchone()
        if row and row[0]:
            token = row[0]
    except Exception:
        pass
    if not token:
        try:
            row = c.cursor().execute('SELECT AccessToken FROM Devices WHERE AccessToken IS NOT NULL AND AccessToken != "" ORDER BY DateLastActivity DESC, Id DESC LIMIT 1').fetchone()
            if row and row[0]:
                token = row[0]
        except Exception:
            pass
    if token:
        print(token)
except Exception:
    pass"#;

fn extract_key_from_local_db() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let home_path = Path::new(&home);
    let candidates = [
        home_path.join("jellyfin/config/data/jellyfin.db"),
        Path::new("/var/lib/jellyfin/data/jellyfin.db").to_path_buf(),
        home_path.join(".config/jellyfin/data/jellyfin.db"),
    ];

    let db_path = candidates.into_iter().find(|p| p.exists())?;
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
    use super::*;

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

    #[test]
    fn test_trigger_refresh_missing_api_key_err() {
        let Ok(client) = Client::builder().build() else {
            panic!("failed to build reqwest client");
        };
        let res = trigger_library_refresh(&client, "http://localhost:8096", None);
        match res {
            Err(e) => assert!(e.to_string().contains("No Jellyfin API key found")),
            Ok(()) => panic!("expected error on missing API key"),
        }
    }

    #[test]
    fn test_smoke_live_refresh_library_auto() {
        let Ok(probe) = Client::builder()
            .timeout(Duration::from_millis(500))
            .build()
        else {
            return;
        };
        if probe.get("http://localhost:8096/health").send().is_err() {
            return;
        }

        let res = refresh_library_auto();
        match res {
            Ok(()) => {}
            Err(e) => panic!("refresh_library_auto failed live: {e}"),
        }
    }
}
