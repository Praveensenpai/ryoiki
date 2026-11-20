use std::process::Command;

use anyhow::{bail, Context};

use crate::tui::RunOptions;

use super::{suspend, Effect};

/// Restart or stop a managed systemd service.
pub fn service_action(name: String, is_user: bool, restart: bool, opts: RunOptions) -> Effect {
    let (label, done) = if restart {
        ("Restarting", "restarted")
    } else {
        ("Stopping", "stopped")
    };
    suspend(format!("{label} {name}"), false, move |app| {
        match (is_user, restart) {
            (true, true) => {
                crate::bot::services::restart_managed_service(&name)?;
            }
            (true, false) => run_user_stop(&name, opts)?,
            (false, true) => super::run_root("systemctl", &["restart", &name], opts)?,
            (false, false) => super::run_root("systemctl", &["stop", &name], opts)?,
        }
        app.request_refresh();
        Ok(format!("{name} {done}"))
    })
}

/// Restart or stop a Docker container. `docker stop` preserves volumes/data.
pub fn container_action(name: String, restart: bool) -> Effect {
    let (label, done) = if restart {
        ("Restarting", "restarted")
    } else {
        ("Stopping", "stopped")
    };
    suspend(format!("{label} container {name}"), false, move |app| {
        if restart {
            crate::bot::services::restart_docker_container(&name)?;
        } else {
            let out = Command::new("docker")
                .args(["stop", &name])
                .output()
                .context("Failed to execute docker stop")?;
            if !out.status.success() {
                bail!(
                    "Failed to stop {name}: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
        }
        app.request_refresh();
        Ok(format!("container {name} {done}"))
    })
}

fn run_user_stop(name: &str, opts: RunOptions) -> anyhow::Result<()> {
    super::run_root("systemctl", &["--user", "stop", name], opts)
}
