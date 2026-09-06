use crate::runner::Runner;
use anyhow::Result;

pub fn setup(runner: &mut Runner) -> Result<()> {
    runner.apt_install("Installing UFW firewall...", &["ufw"])?;

    configure_ufw(runner)?;
    purge_unneeded_services(runner)?;

    Ok(())
}

fn configure_ufw(runner: &mut Runner) -> Result<()> {
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
    )
}

fn purge_unneeded_services(runner: &mut Runner) -> Result<()> {
    runner.exec_bash(
        "Purging unneeded daemons (fail2ban, unattended-upgrades, networkd-dispatcher)...",
        r"
        sudo systemctl disable --now fail2ban unattended-upgrades networkd-dispatcher 2>/dev/null || true
        sudo apt-get purge --autoremove -y fail2ban unattended-upgrades networkd-dispatcher 2>/dev/null || true
        ",
    )
}
