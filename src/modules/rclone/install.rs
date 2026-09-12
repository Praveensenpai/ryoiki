use crate::runner::Runner;
use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Installs the official Rclone binary if not already present in PATH.
pub fn install_rclone_binary(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("rclone") {
        println!("  {} Rclone binary is already installed", "✔".green());
        return Ok(());
    }

    runner.exec_bash(
        "Installing official Rclone binary...",
        "curl -fsSL https://rclone.org/install.sh | sudo bash",
    )
}

/// Installs FUSE3 filesystem tools required for mounting cloud remotes.
pub fn install_fuse_support(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("fusermount3") {
        println!("  {} FUSE3 filesystem tools installed", "✔".green());
        return Ok(());
    }

    runner.apt_install("Installing FUSE3 filesystem support...", &["fuse3"])
}

/// Ensures `user_allow_other` is enabled in `/etc/fuse.conf` so user mounts are accessible.
pub fn configure_fuse_allow_other(runner: &mut Runner) -> Result<()> {
    let fuse_conf = Path::new("/etc/fuse.conf");
    if !fuse_conf.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(fuse_conf).unwrap_or_default();
    let already_enabled = content
        .lines()
        .any(|line| line.trim() == "user_allow_other");

    if already_enabled {
        return Ok(());
    }

    runner.exec_bash(
        "Enabling user_allow_other in /etc/fuse.conf...",
        "sudo sed -i 's/#user_allow_other/user_allow_other/' /etc/fuse.conf || echo 'user_allow_other' | sudo tee -a /etc/fuse.conf >/dev/null",
    )
}
