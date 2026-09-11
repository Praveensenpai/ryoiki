use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn install_hooks(bin_path: &Path) {
    install_boot_service(bin_path);
    install_pam_hook(bin_path);
    install_profile_hook(bin_path);
    install_power_hooks(bin_path);
}

fn render_system_unit(bin: &str) -> String {
    format!(
        "[Unit]\n\
        Description=Ryoiki System Boot Telegram Notification\n\
        After=network-online.target\n\
        Wants=network-online.target\n\n\
        [Service]\n\
        Type=oneshot\n\
        ExecStart={bin} notify boot\n\
        RemainAfterExit=yes\n\n\
        [Install]\n\
        WantedBy=multi-user.target\n"
    )
}

fn render_user_unit(bin: &str) -> String {
    format!(
        "[Unit]\n\
        Description=Ryoiki System Boot Telegram Notification\n\
        After=network.target network-online.target\n\
        Wants=network-online.target\n\n\
        [Service]\n\
        Type=oneshot\n\
        ExecStart={bin} notify boot\n\
        RemainAfterExit=yes\n\n\
        [Install]\n\
        WantedBy=default.target\n"
    )
}

fn render_login_script(bin: &str) -> String {
    format!(
        "#!/bin/sh\n\
        if [ -n \"$SSH_CLIENT\" ] && [ -z \"$RYOIKI_LOGIN_NOTIFIED\" ]; then\n\
            export RYOIKI_LOGIN_NOTIFIED=1\n\
            {bin} notify login 2>/dev/null || true\n\
        fi\n"
    )
}

fn render_bashrc_hook(bin: &str) -> String {
    format!(
        "\n# Ryoiki SSH login alert hook\n\
        if [ -n \"$SSH_CLIENT\" ] && [ -z \"$RYOIKI_LOGIN_NOTIFIED\" ]; then\n\
            export RYOIKI_LOGIN_NOTIFIED=1\n\
            ({bin} notify login >/dev/null 2>&1 &)\n\
        fi\n"
    )
}

fn install_boot_service(bin_path: &Path) {
    let path = Path::new("/etc/systemd/system/ryoiki-boot-notify.service");
    let bin = bin_path.display().to_string();
    let unit = render_system_unit(&bin);

    if fs::write(path, unit).is_ok() {
        let _ = Command::new("systemctl").args(["daemon-reload"]).output();
        let _ = Command::new("systemctl")
            .args(["enable", "ryoiki-boot-notify.service"])
            .output();
        println!("  ✔ Installed and enabled system ryoiki-boot-notify.service");
        return;
    }

    install_user_boot_service(bin_path);
}

fn install_user_boot_service(bin_path: &Path) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let user_dir = Path::new(&home).join(".config/systemd/user");
    let user_path = user_dir.join("ryoiki-boot-notify.service");
    let bin = bin_path.display().to_string();
    let user_unit = render_user_unit(&bin);

    if fs::create_dir_all(&user_dir).is_ok() && fs::write(&user_path, user_unit).is_ok() {
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "enable", "ryoiki-boot-notify.service"])
            .output();
        println!("  ✔ Installed and enabled user ryoiki-boot-notify.service");
    }
}

fn install_pam_hook(bin_path: &Path) {
    let pam_sshd = Path::new("/etc/pam.d/sshd");
    if pam_sshd.exists() {
        let content = fs::read_to_string(pam_sshd).unwrap_or_default();
        let entry = format!(
            "session optional pam_exec.so quiet {} notify login",
            bin_path.display()
        );
        if !content.contains("ryoiki notify login") {
            let mut new_content = content;
            let _ = writeln!(new_content, "\n# Ryoiki SSH login alert\n{entry}");
            if fs::write(pam_sshd, new_content).is_ok() {
                println!("  ✔ Configured PAM login hook in /etc/pam.d/sshd");
            }
        }
    }
}

fn install_profile_hook(bin_path: &Path) {
    let profile_d = Path::new("/etc/profile.d/ryoiki-login-notify.sh");
    let bin = bin_path.display().to_string();
    let script = render_login_script(&bin);
    if fs::write(profile_d, &script).is_ok() {
        let _ = Command::new("chmod")
            .args(["+x", "/etc/profile.d/ryoiki-login-notify.sh"])
            .output();
        println!(
            "  ✔ Configured interactive profile hook in /etc/profile.d/ryoiki-login-notify.sh"
        );
        return;
    }

    install_user_bashrc_hook(bin_path);
}

fn install_user_bashrc_hook(bin_path: &Path) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bashrc = Path::new(&home).join(".bashrc");
    let Ok(content) = fs::read_to_string(&bashrc) else {
        return;
    };
    if content.contains("RYOIKI_LOGIN_NOTIFIED") {
        return;
    }
    let bin = bin_path.display().to_string();
    let entry = render_bashrc_hook(&bin);
    if fs::write(&bashrc, format!("{content}{entry}")).is_ok() {
        println!("  ✔ Configured interactive login hook in ~/.bashrc");
    }
}

/// Install udev rule (power plug/unplug) + battery-watch systemd service.
fn install_power_hooks(bin_path: &Path) {
    install_udev_power_rule(bin_path);
    install_battery_watch_service(bin_path);
}

fn render_udev_power_rule(bin: &str) -> String {
    format!(
        "# Ryoiki power event notifications\n\
        SUBSYSTEM==\"power_supply\", ATTR{{type}}==\"Mains\", \
        ATTR{{online}}==\"1\", \
        RUN+=\"/bin/sh -c '{bin} notify power plugged &'\"\n\
        SUBSYSTEM==\"power_supply\", ATTR{{type}}==\"Mains\", \
        ATTR{{online}}==\"0\", \
        RUN+=\"/bin/sh -c '{bin} notify power unplugged &'\"\n"
    )
}

fn render_battery_watch_service(bin: &str) -> String {
    format!(
        "[Unit]\n\
        Description=Ryoiki Battery Watch Telegram Notification\n\
        After=network-online.target\n\
        Wants=network-online.target\n\n\
        [Service]\n\
        Type=simple\n\
        ExecStart={bin} notify battery-watch\n\
        Restart=on-failure\n\
        RestartSec=30\n\n\
        [Install]\n\
        WantedBy=default.target\n"
    )
}

fn install_udev_power_rule(bin_path: &Path) {
    let rule_path = Path::new("/etc/udev/rules.d/99-ryoiki-power.rules");
    let bin = bin_path.display().to_string();
    let rule = render_udev_power_rule(&bin);
    if fs::write(rule_path, rule).is_ok() {
        let _ = Command::new("udevadm").args(["control", "--reload-rules"]).output();
        let _ = Command::new("udevadm").args(["trigger"]).output();
        println!("  ✔ Installed udev power rule at /etc/udev/rules.d/99-ryoiki-power.rules");
    } else {
        println!("  ✖ Could not write udev rule (run as root); skipping power plug hook");
    }
}

fn install_battery_watch_service(bin_path: &Path) {
    let bin = bin_path.display().to_string();
    let unit = render_battery_watch_service(&bin);

    // Try system-wide first, fall back to user unit
    let sys_path = Path::new("/etc/systemd/system/ryoiki-battery-watch.service");
    if fs::write(sys_path, &unit).is_ok() {
        let _ = Command::new("systemctl").args(["daemon-reload"]).output();
        let _ = Command::new("systemctl")
            .args(["enable", "--now", "ryoiki-battery-watch.service"])
            .output();
        println!("  ✔ Installed and started system ryoiki-battery-watch.service");
        return;
    }

    install_user_battery_watch_service(&unit);
}

fn install_user_battery_watch_service(unit: &str) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let user_dir = Path::new(&home).join(".config/systemd/user");
    let user_path = user_dir.join("ryoiki-battery-watch.service");
    if fs::create_dir_all(&user_dir).is_ok() && fs::write(&user_path, unit).is_ok() {
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "enable", "--now", "ryoiki-battery-watch.service"])
            .output();
        println!("  ✔ Installed and started user ryoiki-battery-watch.service");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_system_unit() {
        let unit = render_system_unit("/usr/bin/ryoiki");
        assert!(unit.contains("ExecStart=/usr/bin/ryoiki notify boot"));
        assert!(unit.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn test_render_user_unit() {
        let unit = render_user_unit("/home/user/.local/bin/ryoiki");
        assert!(unit.contains("ExecStart=/home/user/.local/bin/ryoiki notify boot"));
        assert!(unit.contains("WantedBy=default.target"));
    }

    #[test]
    fn test_render_bashrc_hook() {
        let hook = render_bashrc_hook("/usr/local/bin/ryoiki");
        assert!(hook.contains("RYOIKI_LOGIN_NOTIFIED"));
        assert!(hook.contains("/usr/local/bin/ryoiki notify login"));
    }

    #[test]
    fn test_render_udev_power_rule() {
        let rule = render_udev_power_rule("/usr/local/bin/ryoiki");
        assert!(rule.contains("notify power plugged"));
        assert!(rule.contains("notify power unplugged"));
        assert!(rule.contains("power_supply"));
    }

    #[test]
    fn test_render_battery_watch_service() {
        let unit = render_battery_watch_service("/usr/local/bin/ryoiki");
        assert!(unit.contains("ExecStart=/usr/local/bin/ryoiki notify battery-watch"));
        assert!(unit.contains("WantedBy=default.target"));
    }
}
