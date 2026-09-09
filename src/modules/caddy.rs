use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;

/// Sets up Caddy reverse proxy configured for Tailscale `MagicDNS` HTTPS.
pub fn setup(runner: &mut Runner) -> Result<()> {
    install_caddy(runner)?;
    configure_tailscale_permission(runner)?;

    let (hostname, ts_ip) = detect_tailscale_info();
    let caddyfile = build_caddyfile(&hostname, &ts_ip);

    write_caddyfile(runner, &caddyfile)?;
    enable_caddy(runner)?;
    print_access_info(&hostname, &ts_ip);

    Ok(())
}

fn install_caddy(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("caddy") {
        println!("  {} Caddy is already installed.", "•".dimmed());
        return Ok(());
    }

    runner.exec_bash(
        "Adding Caddy official APT repository...",
        r"
        sudo apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
            | sudo gpg --dearmor --yes -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
            | sudo tee /etc/apt/sources.list.d/caddy-stable.list > /dev/null
        sudo apt-get update -y
        ",
    )?;

    runner.apt_install("Installing Caddy web server...", &["caddy"])?;
    Ok(())
}

fn configure_tailscale_permission(runner: &mut Runner) -> Result<()> {
    runner.exec_bash(
        "Permitting Caddy to fetch Tailscale TLS certificates...",
        r#"
        if [ -f /etc/default/tailscaled ]; then
            if ! grep -q "TS_PERMIT_CERT_UID=caddy" /etc/default/tailscaled; then
                echo 'TS_PERMIT_CERT_UID=caddy' | sudo tee -a /etc/default/tailscaled > /dev/null
                sudo systemctl restart tailscaled
            fi
        fi
        "#,
    )
}

fn detect_tailscale_info() -> (String, String) {
    let output = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output();

    let Ok(out) = output else {
        return (String::new(), String::new());
    };

    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);

    let dns_name = json["Self"]["DNSName"]
        .as_str()
        .map(|s| s.trim_end_matches('.').to_string())
        .unwrap_or_default();

    let ts_ip = json["Self"]["TailscaleIPs"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();

    (dns_name, ts_ip)
}

fn build_caddyfile(hostname: &str, ts_ip: &str) -> String {
    let mut http_targets = vec!["http://localhost".to_string()];
    if !ts_ip.is_empty() {
        http_targets.push(format!("http://{ts_ip}"));
    }
    if !hostname.is_empty() {
        http_targets.push(format!("http://{hostname}"));
    }
    let http_hosts = http_targets.join(", ");

    if hostname.is_empty() {
        return format!(
            r"{{
    auto_https disable_redirects
}}

{http_hosts} {{
    handle /jellyfin* {{
        reverse_proxy localhost:8096
    }}
    handle {{
        reverse_proxy localhost:6881 {{
            header_up Host localhost:6881
            header_up -X-Forwarded-Host
        }}
    }}
}}
"
        );
    }

    format!(
        r"{{
    auto_https disable_redirects
}}

{hostname} {{
    tls {{
        get_certificate tailscale
    }}
    handle /jellyfin* {{
        reverse_proxy localhost:8096
    }}
    handle {{
        reverse_proxy localhost:6881 {{
            header_up Host localhost:6881
            header_up -X-Forwarded-Host
        }}
    }}
}}

{http_hosts} {{
    handle /jellyfin* {{
        reverse_proxy localhost:8096
    }}
    handle {{
        reverse_proxy localhost:6881 {{
            header_up Host localhost:6881
            header_up -X-Forwarded-Host
        }}
    }}
}}
"
    )
}

fn write_caddyfile(runner: &Runner, content: &str) -> Result<()> {
    if runner.dry_run {
        println!("  {} [dry-run] Write /etc/caddy/Caddyfile", "•".dimmed());
        return Ok(());
    }

    let tmp = "/tmp/ryoiki_caddyfile";
    fs::write(tmp, content).context("Failed to write temp Caddyfile")?;

    std::process::Command::new("sudo")
        .args(["cp", tmp, "/etc/caddy/Caddyfile"])
        .status()
        .context("Failed to copy Caddyfile to /etc/caddy/")?;

    let _ = fs::remove_file(tmp);
    Ok(())
}

fn enable_caddy(runner: &mut Runner) -> Result<()> {
    runner.exec_silent(
        "Enabling and reloading Caddy service...",
        "sudo",
        &["systemctl", "enable", "--now", "caddy"],
    )?;
    runner.exec_silent(
        "Reloading Caddy configuration...",
        "sudo",
        &["systemctl", "reload", "caddy"],
    )
}

fn print_access_info(hostname: &str, ts_ip: &str) {
    if hostname.is_empty() {
        println!(
            "  {} Caddy running — http://localhost (qBittorrent: :6881, Jellyfin: :8096)",
            "✔".green().bold()
        );
    } else {
        println!("  {} Caddy reverse proxy active:", "✔".green().bold());
        println!(
            "    {} HTTPS: https://{} (Tailscale MagicDNS)",
            "•".dimmed(),
            hostname.cyan().bold()
        );
        if !ts_ip.is_empty() {
            println!(
                "    {} HTTP:  http://{} or http://{}",
                "•".dimmed(),
                hostname,
                ts_ip
            );
        }
        println!("    {} Jellyfin    → /jellyfin (port 8096)", "•".dimmed());
        println!("    {} qBittorrent → / (port 6881)", "•".dimmed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_caddyfile_with_tailscale() {
        let conf = build_caddyfile("mochi.ts.net", "100.65.1.1");
        assert!(conf.contains("get_certificate tailscale"));
        assert!(conf.contains("mochi.ts.net"));
        assert!(conf.contains("http://100.65.1.1"));
        assert!(conf.contains("header_up Host localhost:6881"));
        assert!(conf.contains("header_up -X-Forwarded-Host"));
        assert!(conf.contains("auto_https disable_redirects"));
    }

    #[test]
    fn test_build_caddyfile_empty_hostname() {
        let conf = build_caddyfile("", "");
        assert!(!conf.contains("get_certificate tailscale"));
        assert!(conf.contains("http://localhost"));
        assert!(conf.contains("header_up Host localhost:6881"));
    }
}
