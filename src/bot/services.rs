use anyhow::{bail, Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub name: String,
    pub status: String,
    pub is_running: bool,
    pub ports: String,
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub description: String,
    pub state: String,
    pub is_user: bool,
}

const MANAGED_USER_SERVICES: &[(&str, &str)] = &[
    ("ryoiki-bot.service", "Telegram Bot Daemon"),
    ("ryoiki-rclone.service", "Rclone Cloud Mount"),
    ("ryoiki-media-sync.timer", "Daily Media Sync Timer"),
    ("ryoiki-prune.timer", "Cloud Pruning Timer"),
    ("ryoiki-strip-retry.timer", "Dubstrip Retry Timer"),
];

const MANAGED_SYSTEM_SERVICES: &[(&str, &str)] = &[
    ("ryoiki-battery-watch.service", "Battery Watcher"),
    ("ryoiki-shutdown-notify.service", "Shutdown Hook"),
    ("docker.service", "Docker Engine"),
    ("tailscaled.service", "Tailscale Mesh VPN"),
];

pub fn get_docker_containers() -> Result<Vec<ContainerInfo>> {
    let out = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--format",
            "{{.Names}}\t{{.Status}}\t{{.State}}\t{{.Ports}}",
        ])
        .output()
        .context("Failed to execute docker ps")?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Docker daemon not accessible: {}", err.trim());
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut containers = Vec::new();

    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 3 {
            let name = cols[0].trim().to_string();
            let status = cols[1].trim().to_string();
            let state = cols[2].trim().to_lowercase();
            let ports = cols.get(3).unwrap_or(&"").trim().to_string();
            let is_running = state == "running";
            containers.push(ContainerInfo {
                name,
                status,
                is_running,
                ports,
            });
        }
    }
    Ok(containers)
}

pub fn restart_docker_container(name: &str) -> Result<String> {
    if !is_valid_name(name) {
        bail!("Invalid container identifier: {name}");
    }

    let out = Command::new("docker")
        .args(["restart", name])
        .output()
        .context("Failed to execute docker restart")?;

    if out.status.success() {
        Ok(format!(
            "Container <code>{name}</code> restarted successfully."
        ))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Failed to restart {name}: {}", err.trim())
    }
}

pub fn get_docker_logs(name: &str, tail: usize) -> Result<String> {
    if !is_valid_name(name) {
        bail!("Invalid container identifier: {name}");
    }

    let tail_str = tail.to_string();
    let out = Command::new("docker")
        .args(["logs", "--tail", &tail_str, name])
        .output()
        .context("Failed to execute docker logs")?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Failed to get logs for {name}: {}", err.trim());
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut combined = if stdout.is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };

    if combined.len() > 3000 {
        combined = combined[combined.len() - 3000..].to_string();
    }
    Ok(combined)
}

pub fn get_managed_services() -> Vec<ServiceInfo> {
    let mut services = Vec::new();

    for &(unit, desc) in MANAGED_USER_SERVICES {
        let state = check_service_state(unit, true);
        services.push(ServiceInfo {
            name: unit.to_string(),
            description: desc.to_string(),
            state,
            is_user: true,
        });
    }

    for &(unit, desc) in MANAGED_SYSTEM_SERVICES {
        let state = check_service_state(unit, false);
        services.push(ServiceInfo {
            name: unit.to_string(),
            description: desc.to_string(),
            state,
            is_user: false,
        });
    }

    services
}

pub fn resolve_managed_unit(name: &str) -> Option<(&'static str, bool)> {
    let resolved_user = MANAGED_USER_SERVICES.iter().find(|(u, _)| {
        *u == name
            || u.strip_suffix(".service").is_some_and(|base| base == name)
            || u.strip_suffix(".timer").is_some_and(|base| base == name)
    });
    if let Some((u, _)) = resolved_user {
        return Some((*u, true));
    }

    let resolved_system = MANAGED_SYSTEM_SERVICES.iter().find(|(u, _)| {
        *u == name
            || u.strip_suffix(".service").is_some_and(|base| base == name)
            || u.strip_suffix(".timer").is_some_and(|base| base == name)
    });
    if let Some((u, _)) = resolved_system {
        return Some((*u, false));
    }

    None
}

pub fn restart_managed_service(name: &str) -> Result<String> {
    let Some((unit, is_user)) = resolve_managed_unit(name) else {
        bail!("Service '{name}' is not in the managed services whitelist.");
    };

    let mut cmd = Command::new("systemctl");
    if is_user {
        cmd.args(["--user", "restart", unit]);
    } else {
        cmd.args(["restart", unit]);
    }

    let out = cmd
        .output()
        .context("Failed to execute systemctl restart")?;
    if out.status.success() {
        Ok(format!(
            "Service <code>{unit}</code> restarted successfully."
        ))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Failed to restart {unit}: {}", err.trim())
    }
}

fn check_service_state(unit: &str, is_user: bool) -> String {
    let mut cmd = Command::new("systemctl");
    if is_user {
        cmd.args(["--user", "is-active", unit]);
    } else {
        cmd.args(["is-active", unit]);
    }

    cmd.output().map_or_else(
        |_| "unknown".to_string(),
        |o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                "inactive".to_string()
            } else {
                s
            }
        },
    )
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_name_accepts_clean_identifiers() {
        assert!(is_valid_name("jellyfin"));
        assert!(is_valid_name("qbittorrent-1"));
        assert!(is_valid_name("ryoiki_bot.service"));
    }

    #[test]
    fn test_is_valid_name_rejects_injections() {
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("jellyfin; rm -rf /"));
        assert!(!is_valid_name("../../etc/shadow"));
        assert!(!is_valid_name("foo bar"));
        assert!(!is_valid_name("test$(whoami)"));
    }

    #[test]
    fn test_resolve_managed_unit() {
        assert_eq!(
            resolve_managed_unit("ryoiki-bot.service"),
            Some(("ryoiki-bot.service", true))
        );
        assert_eq!(
            resolve_managed_unit("ryoiki-bot"),
            Some(("ryoiki-bot.service", true))
        );
        assert_eq!(
            resolve_managed_unit("ryoiki-media-sync"),
            Some(("ryoiki-media-sync.timer", true))
        );
        assert_eq!(
            resolve_managed_unit("docker"),
            Some(("docker.service", false))
        );
        assert_eq!(
            resolve_managed_unit("docker.service"),
            Some(("docker.service", false))
        );
        assert_eq!(resolve_managed_unit("nonexistent"), None);
    }

    #[test]
    fn test_get_managed_services_populates_items() {
        let services = get_managed_services();
        assert!(!services.is_empty());
        assert!(services.iter().any(|s| s.name == "ryoiki-bot.service"));
        assert!(services.iter().any(|s| s.name == "docker.service"));
    }
}
