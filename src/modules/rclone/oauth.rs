use anyhow::Result;
use colored::Colorize;
use std::io::{self, BufRead, Write};
use std::process::Command;

/// Checks if the `gdrive:` remote is fully authenticated with a non-empty OAuth token.
#[must_use]
pub fn is_remote_configured() -> bool {
    let output = Command::new("rclone").args(["config", "dump"]).output();
    output.is_ok_and(|out| {
        if !out.status.success() {
            return false;
        }
        serde_json::from_slice::<serde_json::Value>(&out.stdout).is_ok_and(|val| {
            val.get("gdrive")
                .and_then(|g| g.get("token"))
                .and_then(|t| t.as_str())
                .is_some_and(|s| !s.trim().is_empty())
        })
    })
}

/// Turns rclone stderr into a short, actionable message.
///
/// rclone prefixes errors with a timestamp and a level, which is noise in a
/// status line or a TUI banner.
#[must_use]
pub fn describe_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("rclone not available") {
        return "rclone is not installed. Run: ryoiki rclone setup".to_string();
    }
    if lower.contains("invalid_grant") || lower.contains("couldn't fetch token") {
        return "Google Drive token is invalid or expired. Run: rclone config reconnect gdrive:"
            .to_string();
    }
    if lower.contains("no such remote") || lower.contains("didn't find section") {
        return "rclone remote 'gdrive:' is not configured. Run: ryoiki rclone setup".to_string();
    }
    format!("Cloud access failed: {}", first_stderr_line(raw))
}

/// Verifies the remote with a lightweight API call.
///
/// [`is_remote_configured`] only checks that a token string exists; this also
/// catches expired or revoked refresh tokens.
pub fn probe_remote() -> std::result::Result<(), String> {
    if !is_remote_configured() {
        return Err(
            "rclone remote 'gdrive:' is not configured. Run: ryoiki rclone setup".to_string(),
        );
    }

    let output = Command::new("rclone")
        .args(["about", "gdrive:", "--timeout", "10s", "--contimeout", "5s"])
        .output()
        .map_err(|err| describe_error(&format!("rclone not available: {err}")))?;

    if output.status.success() {
        return Ok(());
    }

    Err(describe_error(&String::from_utf8_lossy(&output.stderr)))
}

fn first_stderr_line(raw: &str) -> String {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown rclone error");
    strip_log_prefix(line).to_string()
}

fn strip_log_prefix(line: &str) -> &str {
    let mut parts = line.splitn(3, ' ');
    let (Some(date), Some(_time), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
        return strip_level(line);
    };
    if date.len() == 10 && date.as_bytes().get(4) == Some(&b'/') {
        strip_level(rest)
    } else {
        strip_level(line)
    }
}

fn strip_level(line: &str) -> &str {
    ["CRITICAL: ", "ERROR+4: ", "ERROR: ", "NOTICE: "]
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix))
        .unwrap_or(line)
}

/// Pre-creates the `gdrive` remote entry in `rclone.conf` if it does not yet exist.
pub fn ensure_remote_initialized() {
    let output = Command::new("rclone").arg("listremotes").output();
    let exists = output.is_ok_and(|out| {
        out.status.success()
            && String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|l| l.trim() == "gdrive:")
    });

    if !exists {
        let _ = Command::new("rclone")
            .args(["config", "create", "gdrive", "drive"])
            .status();
    }
}

/// Guides the user through streamlined Google Drive OAuth configuration.
pub fn prompt_and_configure_remote() -> Result<bool> {
    ensure_remote_initialized();

    println!(
        "\n  {} {}",
        "🔑".cyan(),
        "Google Drive OAuth Configuration".bold()
    );
    println!("  {}\n", "─".repeat(40).dimmed());
    println!("  Choose authentication mode:");
    println!("    [1] Interactive OAuth Reconnect (Guided Rclone browser/link flow)");
    println!("    [2] Paste Existing OAuth Token JSON");
    println!("    [3] Set Custom Client ID & Secret (Google Cloud Console)");
    println!("    [s] Skip for now");
    print!("\n  Select option [1/2/3/s]: ");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().lock().read_line(&mut choice)?;

    match choice.trim() {
        "1" => run_interactive_reconnect(),
        "2" => configure_remote_token()?,
        "3" => configure_custom_credentials()?,
        _ => {
            println!("  {} Skipping Google Drive authentication.", "•".dimmed());
            return Ok(false);
        }
    }

    if is_remote_configured() {
        println!(
            "\n  {} Google Drive remote authenticated successfully!",
            "✔".green().bold()
        );
        Ok(true)
    } else {
        println!(
            "\n  {} Google Drive token is still missing or unauthenticated.",
            "⚠".yellow().bold()
        );
        Ok(false)
    }
}

fn run_interactive_reconnect() {
    println!(
        "\n  {} Launching interactive OAuth session...\n",
        "▶".cyan()
    );
    let _ = Command::new("rclone")
        .args(["config", "reconnect", "gdrive:"])
        .status();
}

fn configure_remote_token() -> Result<()> {
    print!("  Paste raw token JSON: ");
    io::stdout().flush()?;
    let mut token = String::new();
    io::stdin().lock().read_line(&mut token)?;
    let trimmed = token.trim();

    if trimmed.is_empty() {
        return Ok(());
    }

    let _ = Command::new("rclone")
        .args(["config", "update", "gdrive", "token", trimmed])
        .status();
    Ok(())
}

fn configure_custom_credentials() -> Result<()> {
    print!("  Enter Google Client ID: ");
    io::stdout().flush()?;
    let mut client_id = String::new();
    io::stdin().lock().read_line(&mut client_id)?;

    print!("  Enter Google Client Secret: ");
    io::stdout().flush()?;
    let mut client_secret = String::new();
    io::stdin().lock().read_line(&mut client_secret)?;

    let id_trimmed = client_id.trim();
    let sec_trimmed = client_secret.trim();

    if !id_trimmed.is_empty() && !sec_trimmed.is_empty() {
        let _ = Command::new("rclone")
            .args([
                "config",
                "update",
                "gdrive",
                "client_id",
                id_trimmed,
                "client_secret",
                sec_trimmed,
            ])
            .status();
    }

    run_interactive_reconnect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_expired_token() {
        let raw = "2026/11/21 14:11:35 CRITICAL: Failed to create file system for \"gdrive:\": \
                   couldn't fetch token: invalid_grant: maybe token expired?";
        let message = describe_error(raw);
        assert!(message.contains("rclone config reconnect gdrive:"));
    }

    #[test]
    fn describes_unconfigured_remote() {
        let message = describe_error("didn't find section in config file");
        assert!(message.contains("ryoiki rclone setup"));
    }

    #[test]
    fn describes_missing_rclone() {
        let message = describe_error("rclone not available: No such file or directory");
        assert!(message.contains("ryoiki rclone setup"));
    }

    #[test]
    fn strips_timestamp_and_level() {
        assert_eq!(first_stderr_line("2026/11/21 14:11:35 ERROR: boom"), "boom");
        assert_eq!(first_stderr_line("plain error"), "plain error");
    }
}
