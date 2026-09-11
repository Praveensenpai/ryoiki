use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn install_hooks(bin_path: &Path) {
    install_boot_service(bin_path);
    install_pam_hook(bin_path);
    install_profile_hook(bin_path);
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
}
