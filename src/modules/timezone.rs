use crate::notify::TelegramConfig;
use crate::runner::Runner;
use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use colored::Colorize;
use serde::Deserialize;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Supported public IP geolocation providers for timezone lookup.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum IpProvider {
    #[value(name = "ip-api")]
    Api,
    #[value(name = "ipapi")]
    ApiCo,
    #[value(name = "ipinfo")]
    Info,
}

impl IpProvider {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Api => "ip-api.com",
            Self::ApiCo => "ipapi.co",
            Self::Info => "ipinfo.io",
        }
    }

    #[must_use]
    pub const fn endpoint(self) -> &'static str {
        match self {
            Self::Api => "http://ip-api.com/json",
            Self::ApiCo => "https://ipapi.co/json/",
            Self::Info => "https://ipinfo.io/json",
        }
    }
}

/// CLI arguments for the `timezone` subcommand.
#[derive(Args, Debug, Clone)]
pub struct TimezoneArgs {
    /// Desired IANA timezone to set (e.g. `Asia/Kolkata`, `America/New_York`)
    #[arg(value_name = "TIMEZONE")]
    pub timezone: Option<String>,

    /// Auto-detect timezone via IP geolocation and apply it
    #[arg(long)]
    pub auto: bool,

    /// Explicitly select IP geolocation provider
    #[arg(long, short = 'p', value_enum)]
    pub provider: Option<IpProvider>,

    /// Inspect current system and detected timezone without modifying
    #[arg(long, short = 's')]
    pub status: bool,
}

#[derive(Deserialize)]
struct GeoTimezoneResponse {
    timezone: Option<String>,
}

/// Detects local timezone using public IP geolocation.
pub fn detect_timezone(provider: Option<IpProvider>) -> Result<(String, &'static str)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(4))
        .user_agent("ryoiki")
        .build()
        .context("Failed to build HTTP client for timezone detection")?;

    if let Some(p) = provider {
        let tz = query_provider(&client, p)?;
        return Ok((tz, p.name()));
    }

    let candidates = [IpProvider::Api, IpProvider::ApiCo, IpProvider::Info];
    for p in candidates {
        if let Ok(tz) = query_provider(&client, p) {
            return Ok((tz, p.name()));
        }
    }

    bail!("Failed to auto-detect timezone across all IP providers")
}

fn query_provider(client: &reqwest::blocking::Client, provider: IpProvider) -> Result<String> {
    let resp = client
        .get(provider.endpoint())
        .send()
        .with_context(|| format!("Request to {} failed", provider.name()))?;

    let body: GeoTimezoneResponse = resp
        .json()
        .with_context(|| format!("Failed to parse response from {}", provider.name()))?;

    let tz = body
        .timezone
        .filter(|s| !s.trim().is_empty())
        .with_context(|| format!("No timezone field returned by {}", provider.name()))?;

    Ok(tz)
}

/// Resolves current system timezone from `/etc/localtime` or `timedatectl`.
#[must_use]
pub fn get_current_timezone() -> String {
    if let Ok(link) = fs::read_link("/etc/localtime") {
        let path_str = link.to_string_lossy();
        if let Some(idx) = path_str.find("zoneinfo/") {
            let tz = &path_str[idx + "zoneinfo/".len()..];
            if !tz.is_empty() {
                return tz.to_string();
            }
        }
    }

    if let Ok(out) = Command::new("timedatectl")
        .args(["show", "--property=Timezone", "--value"])
        .output()
    {
        let tz = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !tz.is_empty() {
            return tz;
        }
    }

    "Etc/UTC".to_string()
}

/// Checks if a timezone name is valid according to standard Linux zoneinfo.
#[must_use]
pub fn is_valid_timezone(tz: &str) -> bool {
    if tz.is_empty() || tz.contains("..") || tz.starts_with('/') {
        return false;
    }

    let zoneinfo = Path::new("/usr/share/zoneinfo");
    if zoneinfo.exists() {
        return zoneinfo.join(tz).exists();
    }

    tz.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
}

/// Sets the host operating system timezone via `timedatectl`.
pub fn set_system_timezone(tz: &str, runner: &mut Runner) -> Result<()> {
    if !is_valid_timezone(tz) {
        bail!("Invalid IANA timezone '{tz}'. Example: Asia/Kolkata or America/New_York");
    }

    let current = get_current_timezone();
    if current == tz {
        println!(
            "  {} System timezone already set to {}",
            "ℹ".cyan(),
            tz.bold()
        );
        persist_configured_timezone(tz);
        return Ok(());
    }

    let desc = format!("Setting system timezone to {tz}...");
    let is_root = unsafe { libc::geteuid() } == 0;
    if is_root {
        runner.exec_silent(&desc, "timedatectl", &["set-timezone", tz])?;
    } else {
        runner.exec_silent(&desc, "sudo", &["timedatectl", "set-timezone", tz])?;
    }

    persist_configured_timezone(tz);
    println!(
        "  {} System timezone updated to {}",
        "✔".green().bold(),
        tz.cyan().bold()
    );
    Ok(())
}

fn persist_configured_timezone(tz: &str) {
    if let Ok(mut cfg) = TelegramConfig::load() {
        cfg.timezone = Some(tz.to_string());
        let _ = cfg.save();
    }
}

/// Interactive and automated setup handler for provisioning module.
pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    let detected = detect_timezone(None).ok();
    let current = get_current_timezone();
    let default_tz = detected
        .as_ref()
        .map_or(current.as_str(), |(tz, _)| tz.as_str());

    let chosen_tz = if non_interactive {
        default_tz.to_string()
    } else {
        prompt_user_timezone(default_tz, detected.as_ref().map(|(_, p)| *p))?
    };

    set_system_timezone(&chosen_tz, runner)
}

fn prompt_user_timezone(default_tz: &str, provider_name: Option<&str>) -> Result<String> {
    if let Some(provider) = provider_name {
        println!(
            "  {} Auto-detected timezone via {}: {}",
            "🌐".cyan(),
            provider.bold(),
            default_tz.cyan().bold()
        );
    }

    print!("  Configure system timezone [{default_tz}]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let trimmed = input.trim();

    if trimmed.is_empty() {
        Ok(default_tz.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

/// Dispatches CLI `timezone` command.
pub fn handle_cli(args: TimezoneArgs, runner: &mut Runner) -> Result<()> {
    if args.status {
        print_status_overview(args.provider);
        return Ok(());
    }

    if let Some(tz) = args.timezone {
        return set_system_timezone(&tz, runner);
    }

    if args.auto {
        let (detected, provider) = detect_timezone(args.provider)?;
        println!(
            "  {} Detected timezone via {}: {}",
            "🌐".cyan(),
            provider.bold(),
            detected.bold()
        );
        return set_system_timezone(&detected, runner);
    }

    setup(runner, false)
}

fn print_status_overview(provider: Option<IpProvider>) {
    let current = get_current_timezone();
    let detected = detect_timezone(provider);
    println!("  {} System Timezone Status:\n", "🕒".bold());
    println!("    Current Timezone : {}", current.cyan().bold());

    match detected {
        Ok((tz, p)) => {
            println!(
                "    IP Auto-Detected : {} {}",
                tz.green().bold(),
                format!("(via {p})").dimmed()
            );
        }
        Err(err) => {
            println!(
                "    IP Auto-Detected : {} {}",
                "Unavailable".yellow(),
                format!("({err})").dimmed()
            );
        }
    }

    if let Ok(out) = Command::new("date").output() {
        let date_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        println!("    Local System Date: {}\n", date_str.bold());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_metadata() {
        assert_eq!(IpProvider::Api.name(), "ip-api.com");
        assert_eq!(IpProvider::ApiCo.name(), "ipapi.co");
        assert_eq!(IpProvider::Info.name(), "ipinfo.io");
        assert!(IpProvider::Api.endpoint().starts_with("http"));
    }

    #[test]
    fn test_valid_timezones() {
        assert!(is_valid_timezone("UTC"));
        assert!(is_valid_timezone("Etc/UTC"));
        assert!(!is_valid_timezone(""));
        assert!(!is_valid_timezone("/etc/shadow"));
        assert!(!is_valid_timezone("../../../root"));
    }

    #[test]
    fn test_get_current_timezone_non_empty() {
        let tz = get_current_timezone();
        assert!(!tz.is_empty());
    }
}
