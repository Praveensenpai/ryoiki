use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;

pub const TMUX_CONF: &str = include_str!("../configs/.tmux.conf");
pub const BASH_ALIASES: &str = include_str!("../configs/.bash_aliases");
pub const STARSHIP_TOML: &str = include_str!("../configs/starship.toml");

pub fn deploy_dotfiles(home: &str) -> Result<()> {
    let home_path = Path::new(home);

    fs::write(home_path.join(".tmux.conf"), TMUX_CONF).context("Failed to write ~/.tmux.conf")?;
    fs::write(home_path.join(".bash_aliases"), BASH_ALIASES)
        .context("Failed to write ~/.bash_aliases")?;

    let config_dir = home_path.join(".config");
    fs::create_dir_all(&config_dir).context("Failed to create ~/.config")?;
    fs::write(config_dir.join("starship.toml"), STARSHIP_TOML)
        .context("Failed to write ~/.config/starship.toml")?;

    ensure_bashrc_hooks(&home_path.join(".bashrc"))?;

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
