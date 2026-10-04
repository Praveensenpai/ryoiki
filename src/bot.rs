pub mod actions;
pub mod callbacks;
pub mod client;
pub mod keyboards;
pub mod maintenance;
pub mod prompts;
pub mod router;
pub mod services;
pub mod system;
pub mod torrents;
pub mod types;
pub mod ui;

#[cfg(test)]
mod smoke_tests;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::time::Duration;

use crate::notify::config::TelegramConfig;

pub fn run_bot() -> Result<()> {
    let config = TelegramConfig::load()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(40))
        .build()
        .context("Failed to initialize HTTP client for bot")?;

    println!("  ⚡ Ryoiki Pure-Rust Command Center & Bot active (Listening for commands)...");
    crate::notify::server::spawn_background_server(config.clone(), config.api_port);
    torrents::start_torrent_monitor(config.clone());
    let prompts = prompts::new_registry();

    let mut offset: i64 = 0;

    loop {
        let Ok(updates) = client::fetch_updates(&client, &config.bot_token, offset) else {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        };

        for u in updates {
            offset = u.id + 1;
            if let Some(msg) = u.message {
                router::route_message(&client, &config, &prompts, msg);
            }
            if let Some(cb) = u.callback_query {
                router::route_callback(&client, &config, &prompts, cb);
            }
        }
    }
}
