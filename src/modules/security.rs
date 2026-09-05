use crate::runner::Runner;
use anyhow::Result;
use std::path::Path;

pub fn setup(runner: &mut Runner) -> Result<()> {
    runner.apt_install(
        "Installing UFW firewall and Fail2Ban...",
        &["ufw", "fail2ban"],
    )?;

    // Configure UFW rules
    runner.exec_silent(
        "Setting UFW default deny incoming",
        "sudo",
        &["ufw", "default", "deny", "incoming"],
    )?;
    runner.exec_silent(
        "Setting UFW default allow outgoing",
        "sudo",
        &["ufw", "default", "allow", "outgoing"],
    )?;
    runner.exec_silent(
        "Allowing SSH through firewall",
        "sudo",
        &["ufw", "allow", "OpenSSH"],
    )?;
    runner.exec_silent(
        "Allowing HTTP (80/tcp)",
        "sudo",
        &["ufw", "allow", "80/tcp"],
    )?;
    runner.exec_silent(
        "Allowing HTTPS (443/tcp)",
        "sudo",
        &["ufw", "allow", "443/tcp"],
    )?;
    runner.exec_silent(
        "Enabling UFW firewall...",
        "sudo",
        &["ufw", "--force", "enable"],
    )?;

    // Fail2Ban configuration
    if !Path::new("/etc/fail2ban/jail.local").exists()
        && Path::new("/etc/fail2ban/jail.conf").exists()
    {
        runner.exec_silent(
            "Creating /etc/fail2ban/jail.local...",
            "sudo",
            &["cp", "/etc/fail2ban/jail.conf", "/etc/fail2ban/jail.local"],
        )?;
    }

    runner.exec_silent(
        "Enabling and starting Fail2Ban service...",
        "sudo",
        &["systemctl", "enable", "--now", "fail2ban"],
    )?;

    Ok(())
}
