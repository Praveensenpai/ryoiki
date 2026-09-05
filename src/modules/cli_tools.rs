use crate::runner::Runner;
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn setup(runner: &mut Runner) -> Result<()> {
    runner.apt_install(
        "Installing modern CLI tools (eza, bat, zoxide, fzf)...",
        &["eza", "bat", "zoxide", "fzf", "tar", "xz-utils"],
    )?;

    // Fix batcat -> bat symlink on Ubuntu if needed
    if Runner::command_exists("batcat") && !Runner::command_exists("bat") {
        runner.exec_silent(
            "Linking bat -> batcat...",
            "sudo",
            &["ln", "-sf", "/usr/bin/batcat", "/usr/local/bin/bat"],
        )?;
    }

    // ble.sh (Bash Line Editor)
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let blesh_dir = Path::new(&home).join(".local/share/blesh");

    if !blesh_dir.exists() {
        runner.exec_bash(
            "Installing ble.sh nightly release...",
            "mkdir -p \"$HOME/.local/share/blesh\" && curl -fsSL https://github.com/akinomyoga/ble.sh/releases/download/nightly/ble-nightly.tar.xz | tar -xJ -C \"$HOME/.local/share/blesh\" --strip-components=1",
        )?;
    }

    // Append ble.sh initialization to ~/.bashrc safely
    let bashrc_path = Path::new(&home).join(".bashrc");
    if bashrc_path.exists() {
        let bashrc = fs::read_to_string(&bashrc_path).unwrap_or_default();
        if !bashrc.contains("blesh/ble.sh") {
            let mut file = fs::OpenOptions::new().append(true).open(&bashrc_path)?;
            file.write_all(b"\n# ble.sh (Bash Line Editor)\n[[ $- == *i* ]] && source ~/.local/share/blesh/ble.sh --attach=none\n[[ ${BLE_VERSION-} ]] && ble-attach\n")?;
        }
    }

    Ok(())
}
