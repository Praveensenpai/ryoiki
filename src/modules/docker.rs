use anyhow::Result;
use crate::runner::Runner;

pub fn setup(runner: &mut Runner) -> Result<()> {
    if runner.command_exists("docker") {
        println!("  Docker is already installed.");
        return Ok(());
    }

    runner.apt_install(
        "Installing Docker prerequisites...",
        &["ca-certificates", "curl", "gnupg", "lsb-release"],
    )?;

    runner.exec_bash(
        "Configuring official Docker APT repository...",
        r#"
        sudo install -m 0755 -d /etc/apt/keyrings
        curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor --yes -o /etc/apt/keyrings/docker.gpg
        sudo chmod a+r /etc/apt/keyrings/docker.gpg
        UBUNTU_CODENAME="$(lsb_release -cs 2>/dev/null || grep VERSION_CODENAME /etc/os-release | cut -d= -f2)"
        echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu ${UBUNTU_CODENAME} stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
        sudo apt-get update -y
        "#,
    )?;

    runner.apt_install(
        "Installing Docker Engine & Docker Compose...",
        &[
            "docker-ce",
            "docker-ce-cli",
            "containerd.io",
            "docker-buildx-plugin",
            "docker-compose-plugin",
        ],
    )?;

    let user = std::env::var("USER").unwrap_or_default();
    if !user.is_empty() && user != "root" {
        runner.exec_silent(
            &format!("Adding {user} to docker group..."),
            "sudo",
            &["usermod", "-aG", "docker", &user],
        )?;
    }

    runner.exec_silent(
        "Enabling and starting Docker service...",
        "sudo",
        &["systemctl", "enable", "--now", "docker"],
    )?;

    Ok(())
}
