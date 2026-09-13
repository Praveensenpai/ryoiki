use crate::runner::Runner;
use anyhow::Result;

pub fn setup(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("dubstrip") {
        println!("  dubstrip is already installed.");
        return Ok(());
    }

    runner.exec_bash(
        "Installing dubstrip (Praveensenpai/dubstrip)...",
        "curl -fsSL -H \"Cache-Control: no-cache\" https://raw.githubusercontent.com/Praveensenpai/dubstrip/main/install.sh | bash",
    )?;

    Ok(())
}
