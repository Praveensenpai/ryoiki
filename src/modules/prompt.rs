use crate::runner::Runner;
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn setup(runner: &mut Runner) -> Result<()> {
    // 1. Starship prompt
    if !Runner::command_exists("starship") {
        runner.exec_bash(
            "Installing Starship prompt...",
            "curl -sS https://starship.rs/install.sh | sh -s -- -y",
        )?;
    }

    // 2. Fastfetch
    runner.apt_install("Installing Fastfetch system banner...", &["fastfetch"])?;

    // 3. Ensure Fastfetch runs on interactive login in ~/.bashrc
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bashrc_path = Path::new(&home).join(".bashrc");
    if bashrc_path.exists() {
        let bashrc = fs::read_to_string(&bashrc_path).unwrap_or_default();
        if !bashrc.contains("fastfetch") {
            let mut file = fs::OpenOptions::new().append(true).open(&bashrc_path)?;
            file.write_all(b"\n# Display fastfetch system banner on interactive terminal start\nif [[ $- == *i* ]] && command -v fastfetch &>/dev/null; then\n    fastfetch\nfi\n")?;
        }
    }

    Ok(())
}
