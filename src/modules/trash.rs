use crate::runner::Runner;
use anyhow::Result;

pub fn setup(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("toss") {
        println!("  toss is already installed.");
        return Ok(());
    }

    runner.exec_bash(
        "Installing toss (toss-rs)...",
        "curl -fsSL -H \"Cache-Control: no-cache\" https://raw.githubusercontent.com/Praveensenpai/toss-rs/main/install.sh | bash",
    )?;

    Ok(())
}
