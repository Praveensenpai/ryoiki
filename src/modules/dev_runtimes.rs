use crate::runner::Runner;
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn setup(runner: &mut Runner) -> Result<()> {
    // 1. Golang, build essentials, unzip
    runner.apt_install(
        "Installing Go compiler, build-essentials, and unzip...",
        &["golang-go", "build-essential", "unzip"],
    )?;

    // 2. Rust via rustup
    if !Runner::command_exists("rustc") {
        runner.exec_bash(
            "Installing Rust toolchain (rustup)...",
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path",
        )?;
    }

    ensure_cargo_env()?;

    // 3. uv (Astral Python package manager)
    if !Runner::command_exists("uv") {
        runner.exec_bash(
            "Installing uv (Python package manager)...",
            "curl -LsSf https://astral.sh/uv/install.sh | sh",
        )?;
    }

    // 4. Bun (JavaScript/TypeScript runtime)
    if !Runner::command_exists("bun") {
        runner.exec_bash(
            "Installing Bun runtime & toolkit...",
            "curl -fsSL https://bun.sh/install | bash",
        )?;
    }

    Ok(())
}

fn ensure_cargo_env() -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bashrc_path = Path::new(&home).join(".bashrc");
    if bashrc_path.exists() {
        let bashrc = fs::read_to_string(&bashrc_path).unwrap_or_default();
        if !bashrc.contains("cargo/env") {
            let mut file = fs::OpenOptions::new().append(true).open(&bashrc_path)?;
            file.write_all(b"\n# Cargo environment\n[ -f \"$HOME/.cargo/env\" ] && source \"$HOME/.cargo/env\"\n")?;
        }
    }
    Ok(())
}
