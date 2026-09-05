use anyhow::Result;
use crate::runner::Runner;

pub fn setup(runner: &mut Runner) -> Result<()> {
    if runner.command_exists("toss") {
        println!("  toss is already installed.");
        return Ok(());
    }

    runner.exec_bash(
        "Installing toss (toss-rs)...",
        "curl -fsSL -H \"Cache-Control: no-cache\" https://raw.githubusercontent.com/Praveensenpai/toss-rs/main/install.sh | bash",
    )?;

    Ok(())
}
