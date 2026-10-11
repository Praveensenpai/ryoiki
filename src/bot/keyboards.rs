use super::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub fn status_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::callback("📊 System", "cb:sys"),
                InlineKeyboardButton::callback("📦 Torrents", "cb:torrents"),
            ],
            vec![
                InlineKeyboardButton::callback("🐳 Docker", "cb:docker"),
                InlineKeyboardButton::callback("💾 Storage", "cb:storage"),
            ],
            vec![
                InlineKeyboardButton::callback("🛠 Services", "cb:services"),
                InlineKeyboardButton::callback("⚙ Maintenance", "cb:maintenance"),
            ],
            vec![InlineKeyboardButton::callback("🔄 Refresh", "cb:status")],
        ],
    }
}

pub fn system_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::callback("🏠 Overview", "cb:status"),
                InlineKeyboardButton::callback("💾 Storage", "cb:storage"),
            ],
            vec![InlineKeyboardButton::callback("🔄 Refresh", "cb:sys")],
        ],
    }
}

pub fn storage_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::callback("🏠 Overview", "cb:status"),
                InlineKeyboardButton::callback("🧹 Prune Cloud", "cb:prune"),
            ],
            vec![InlineKeyboardButton::callback("🔄 Refresh", "cb:storage")],
        ],
    }
}

pub fn docker_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::callback("🏠 Overview", "cb:status"),
                InlineKeyboardButton::callback("🛠 Services", "cb:services"),
            ],
            vec![InlineKeyboardButton::callback("🔄 Refresh", "cb:docker")],
        ],
    }
}

pub fn services_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::callback("🏠 Overview", "cb:status"),
                InlineKeyboardButton::callback("🐳 Docker", "cb:docker"),
            ],
            vec![InlineKeyboardButton::callback("🔄 Refresh", "cb:services")],
        ],
    }
}

pub fn torrents_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::callback("🏠 Overview", "cb:status"),
                InlineKeyboardButton::callback("📊 System", "cb:sys"),
            ],
            vec![InlineKeyboardButton::callback("🔄 Refresh", "cb:torrents")],
        ],
    }
}

pub fn maintenance_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::callback("🎬 Organize", "cb:organize"),
                InlineKeyboardButton::callback("🧹 Prune", "cb:prune"),
            ],
            vec![
                InlineKeyboardButton::callback("🔄 Sync", "cb:sync"),
                InlineKeyboardButton::callback("🗡️ Dubstrip", "cb:audio"),
            ],
            vec![
                InlineKeyboardButton::callback("🔍 Audit Tools", "cb:check"),
                InlineKeyboardButton::callback("🏠 Overview", "cb:status"),
            ],
        ],
    }
}

/// Inline keyboard offering every candidate language for a pending audio
/// decision. The AI recommendation carries a star so it stands out.
///
/// Callback data is `cb:al:<id>:<index>` to stay well under Telegram's
/// 64-byte limit regardless of how long a language name is.
pub fn language_keyboard(
    id: &str,
    candidates: &[String],
    recommended: Option<&str>,
) -> InlineKeyboardMarkup {
    let rows = candidates
        .iter()
        .enumerate()
        .map(|(idx, lang)| {
            let is_rec = recommended.is_some_and(|r| r.eq_ignore_ascii_case(lang));
            let label = if is_rec {
                format!("⭐ {lang} (recommended)")
            } else {
                lang.clone()
            };
            vec![InlineKeyboardButton::callback(
                &label,
                &format!("cb:al:{id}:{idx}"),
            )]
        })
        .collect();
    InlineKeyboardMarkup {
        inline_keyboard: rows,
    }
}

pub fn seedr_queue_keyboard(hash: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![vec![
            InlineKeyboardButton::callback("✅ Keep queued", &format!("cb:seedrq:keep:{hash}")),
            InlineKeyboardButton::callback("⚡ Download first", &format!("cb:seedrq:front:{hash}")),
        ]],
    }
}

pub fn reboot_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![InlineKeyboardButton::callback(
                "⚠️ Yes, Reboot Server",
                "cb:reboot_confirm",
            )],
            vec![InlineKeyboardButton::callback("✖ Cancel", "cb:cancel")],
        ],
    }
}

pub fn poweroff_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![InlineKeyboardButton::callback(
                "🔴 Yes, Power Off",
                "cb:poweroff_confirm",
            )],
            vec![InlineKeyboardButton::callback("✖ Cancel", "cb:cancel")],
        ],
    }
}
