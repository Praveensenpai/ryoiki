use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const TMUX_CONF: &str = include_str!("../configs/.tmux.conf");
pub const BASH_ALIASES: &str = include_str!("../configs/.bash_aliases");
pub const STARSHIP_TOML: &str = include_str!("../configs/starship.toml");

/// Deploys embedded configuration dotfiles to the specified home directory.
pub fn deploy_dotfiles(home: &str) -> Result<()> {
    let home_path = Path::new(home);

    write_dotfile(
        &home_path.join(".tmux.conf"),
        TMUX_CONF,
        "Failed to write ~/.tmux.conf",
    )?;
    write_dotfile(
        &home_path.join(".bash_aliases"),
        BASH_ALIASES,
        "Failed to write ~/.bash_aliases",
    )?;

    let config_dir = home_path.join(".config");
    fs::create_dir_all(&config_dir).context("Failed to create ~/.config")?;
    write_dotfile(
        &config_dir.join("starship.toml"),
        STARSHIP_TOML,
        "Failed to write ~/.config/starship.toml",
    )?;

    ensure_bashrc_hooks(&home_path.join(".bashrc"))?;
    ensure_tmux_plugins(home_path);

    Ok(())
}

fn ensure_tmux_plugins(home_path: &Path) {
    let tpm_dir = home_path.join(".tmux/plugins/tpm");
    if !tpm_dir.exists() && crate::runner::Runner::command_exists("git") {
        let _ = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "https://github.com/tmux-plugins/tpm",
            ])
            .arg(&tpm_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    let installer = tpm_dir.join("bin/install_plugins");
    if installer.exists() && crate::runner::Runner::command_exists("tmux") {
        let tmux_conf = home_path.join(".tmux.conf");
        let _ = Command::new("tmux")
            .arg("start-server")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = Command::new("tmux")
            .arg("source-file")
            .arg(&tmux_conf)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = Command::new(&installer)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn write_dotfile(path: &Path, content: &str, error_msg: &'static str) -> Result<()> {
    if path.is_symlink() {
        let _ = fs::remove_file(path);
    }
    fs::write(path, content).context(error_msg)?;
    Ok(())
}

fn ensure_bashrc_hooks(bashrc_path: &Path) -> Result<()> {
    if !bashrc_path.exists() {
        return Ok(());
    }
    let bashrc = fs::read_to_string(bashrc_path).unwrap_or_default();
    let mut to_append = String::new();

    if !bashrc.contains(".bash_aliases") {
        to_append.push_str("\n# Source bash_aliases if available\nif [ -f ~/.bash_aliases ]; then\n    . ~/.bash_aliases\nfi\n");
    }

    if !bashrc.contains("starship init bash") {
        to_append.push_str("\n# Starship cross-shell prompt\nif command -v starship &>/dev/null; then\n    eval \"$(starship init bash)\"\nfi\n");
    }

    if !to_append.is_empty() {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(bashrc_path)
            .context("Failed to open ~/.bashrc for appending")?;
        file.write_all(to_append.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn test_write_dotfile_overwrites_broken_symlink() -> Result<()> {
        let tmp_dir = std::env::temp_dir().join("ryoiki_test_broken_symlink");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir)?;

        let symlink_path = tmp_dir.join(".tmux.conf");
        let nonexistent_target = tmp_dir.join("nonexistent/dir/.tmux.conf");
        symlink(&nonexistent_target, &symlink_path)?;

        assert!(symlink_path.is_symlink());

        write_dotfile(&symlink_path, "test content", "Failed to write")?;

        assert!(!symlink_path.is_symlink());
        assert_eq!(fs::read_to_string(&symlink_path)?, "test content");

        let _ = fs::remove_dir_all(&tmp_dir);
        Ok(())
    }

    #[test]
    fn test_write_dotfile_regular_file() -> Result<()> {
        let tmp_dir = std::env::temp_dir().join("ryoiki_test_regular_file");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir)?;

        let file_path = tmp_dir.join(".bash_aliases");
        fs::write(&file_path, "initial")?;

        write_dotfile(&file_path, "updated", "Failed to write")?;

        assert_eq!(fs::read_to_string(&file_path)?, "updated");

        let _ = fs::remove_dir_all(&tmp_dir);
        Ok(())
    }

    #[test]
    fn test_ensure_tmux_plugins_does_not_panic() {
        let tmp_dir = std::env::temp_dir().join("ryoiki_test_tmux_plugins_run");
        let _ = fs::create_dir_all(&tmp_dir);
        ensure_tmux_plugins(&tmp_dir);
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
