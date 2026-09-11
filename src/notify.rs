pub mod client;
pub mod config;
pub mod hooks;
pub mod power;
pub mod server;
pub mod system;

use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

pub use config::TelegramConfig;

#[derive(Subcommand, Debug)]
pub enum NotifySubcommand {
    /// Send custom alert via Telegram
    Send {
        /// Alert message body
        message: String,
        /// Optional title or category
        #[arg(short, long)]
        title: Option<String>,
        /// Alert level: info, success, warn, error
        #[arg(short, long, default_value = "info")]
        level: String,
    },
    /// Send system boot notification
    Boot,
    /// Send SSH or user login security notification
    Login {
        /// Logged in username
        #[arg(long)]
        user: Option<String>,
        /// Client remote IP
        #[arg(long)]
        ip: Option<String>,
        /// Service name (e.g. sshd)
        #[arg(long)]
        service: Option<String>,
        /// TTY name (e.g. pts/0)
        #[arg(long)]
        tty: Option<String>,
    },
    /// Send torrent event notification (qBittorrent `AutoRun` hook)
    Torrent {
        /// Event type ("started" or "completed")
        event: String,
        /// Torrent info hash
        hash: String,
    },
    /// Start local HTTP notification webhook listener
    Serve {
        /// Local port to listen on
        #[arg(short, long, default_value_t = 9119)]
        port: u16,
    },
    /// Send AC power plug/unplug notification (called by udev rule)
    Power {
        /// Event status: "plugged" or "unplugged"
        status: String,
    },
    /// Start battery watch daemon — fires Telegram alert at each low-battery threshold
    BatteryWatch,
    /// Install systemd boot service and PAM/profile login hooks
    InstallHooks,
    /// Legacy alias for torrent started event
    #[command(hide = true)]
    Started { hash: String },
    /// Legacy alias for torrent completed event
    #[command(hide = true)]
    Completed { hash: String },
}

pub fn handle_cli(cmd: NotifySubcommand) -> Result<()> {
    let config = TelegramConfig::load()?;
    match cmd {
        NotifySubcommand::Send {
            message,
            title,
            level,
        } => {
            system::send_custom_notification(&config, &message, title.as_deref(), &level)?;
            println!("  {} Telegram notification sent", "✔".green().bold());
        }
        NotifySubcommand::Boot => {
            system::send_boot_notification(&config)?;
            println!(
                "  {} Boot notification sent to Telegram",
                "✔".green().bold()
            );
        }
        NotifySubcommand::Login {
            user,
            ip,
            service,
            tty,
        } => {
            system::send_login_notification(
                &config,
                user.as_deref(),
                ip.as_deref(),
                service.as_deref(),
                tty.as_deref(),
            )?;
        }
        NotifySubcommand::Torrent { event, hash } => {
            crate::modules::torrent::notify::execute(&event, &hash)?;
        }
        NotifySubcommand::Started { hash } => {
            crate::modules::torrent::notify::execute("started", &hash)?;
        }
        NotifySubcommand::Completed { hash } => {
            crate::modules::torrent::notify::execute("completed", &hash)?;
        }
        NotifySubcommand::Serve { port } => {
            server::run_server(config, port)?;
        }
        NotifySubcommand::InstallHooks => {
            let exe = std::env::current_exe()?;
            hooks::install_hooks(&exe);
        }
        NotifySubcommand::Power { status } => {
            power::send_power_event(&config, &status)?;
            println!("  {} Power event ({status}) sent to Telegram", "✔".green().bold());
        }
        NotifySubcommand::BatteryWatch => {
            println!("  {} Starting battery watcher…", "▶".cyan().bold());
            power::run_battery_watch(&config)?;
        }
    }
    Ok(())
}
