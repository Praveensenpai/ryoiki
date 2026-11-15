use crate::notify::config::TelegramConfig;
use crate::tui::app::{mask_secret, Action, App, Confirm, InputTarget};
use crate::tui::widgets::Row;

pub fn rows(app: &App) -> Vec<Row> {
    let config = app.telegram.as_ref();
    let token = config.map_or_else(|| "not set".to_string(), |cfg| mask_secret(&cfg.bot_token));
    let chat = config.map_or_else(|| "not set".to_string(), |cfg| mask_secret(&cfg.chat_id));
    let server = config
        .and_then(|cfg| cfg.server_name.clone())
        .unwrap_or_else(|| "hostname".to_string());
    let timezone = config
        .and_then(|cfg| cfg.timezone.clone())
        .unwrap_or_else(|| "auto".to_string());

    let mut rows = vec![
        Row::info("Status", app.telegram_status()),
        Row::info("Bot token", token),
        Row::info("Chat id", chat),
        Row::info("Server name", server),
        Row::info("Timezone", timezone),
        Row::info(
            "Config",
            TelegramConfig::primary_config_path().display().to_string(),
        ),
        Row::divider(),
        Row::action("Edit bot token"),
        Row::action("Edit chat id"),
        Row::action("Edit server name"),
        Row::action("Edit timezone"),
        Row::action("Send test notification"),
        Row::action("Install system hooks"),
    ];

    if let Some(error) = &app.telegram_error {
        rows.push(Row::info("Note", error.as_str()));
    }
    rows
}

pub fn activate(app: &mut App, index: usize) -> Action {
    match index {
        7 => Action::Edit(InputTarget::BotToken),
        8 => Action::Edit(InputTarget::ChatId),
        9 => Action::Edit(InputTarget::ServerName),
        10 => Action::Edit(InputTarget::Timezone),
        11 => Action::TestNotification,
        12 => {
            app.confirm = Some(Confirm {
                title: "Install system hooks".to_string(),
                body: "Install boot, login and shutdown notification hooks?\n\
                       This writes systemd units and PAM snippets (sudo required)."
                    .to_string(),
                action: Action::InstallHooks,
            });
            Action::None
        }
        _ => Action::None,
    }
}
