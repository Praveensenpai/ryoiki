use std::fs;
use std::path::Path;
use anyhow::Result;
use crate::runner::Runner;

pub fn setup(runner: &mut Runner) -> Result<()> {
    // 1. Starship prompt
    if !runner.command_exists("starship") {
        runner.exec_bash(
            "Installing Starship prompt...",
            "curl -sS https://starship.rs/install.sh | sh -s -- -y",
        )?;
    }

    // 2. Fastfetch
    runner.apt_install(
        "Installing Fastfetch system banner...",
        &["fastfetch"],
    )?;

    // 3. Ensure Fastfetch runs on interactive login in ~/.bashrc
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bashrc_path = Path::new(&home).join(".bashrc");
    if bashrc_path.exists() {
        let bashrc = fs::read_to_string(&bashrc_path).unwrap_or_default();
        if !bashrc.contains("fastfetch") {
            let mut file = fs::OpenOptions::new().append(true).open(&bashrc_path)?;
            use std::io::Write;
            file.write_all(b"\n# Display fastfetch system banner on interactive terminal start\nif [[ $- == *i* ]] && command -v fastfetch &>/dev/null; then\n    fastfetch\nfi\n")?;
        }
    }

    Ok(())
}
