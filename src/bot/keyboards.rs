use super::types::{InlineKeyboardButton, InlineKeyboardMarkup};

fn primary_row(buttons: &[(&str, &str)]) -> Vec<InlineKeyboardButton> {
    buttons
        .iter()
        .map(|(label, data)| InlineKeyboardButton::primary(label, data))
        .collect()
}

pub fn status_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            primary_row(&[
                ("🖥 System", "cb:sys"),
                ("🧲 Torrents", "cb:torrents"),
                ("🐳 Docker", "cb:docker"),
            ]),
            primary_row(&[("💾 Storage", "cb:storage"), ("⚙️ Services", "cb:services")]),
            vec![
                InlineKeyboardButton::primary("🧰 Maintenance", "cb:maintenance"),
                InlineKeyboardButton::success("🔄 Refresh", "cb:status"),
            ],
        ],
    }
}

pub fn system_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            primary_row(&[("🏠 Overview", "cb:status"), ("💾 Storage", "cb:storage")]),
            vec![InlineKeyboardButton::success("🔄 Refresh", "cb:sys")],
        ],
    }
}

pub fn storage_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::primary("🏠 Overview", "cb:status"),
                InlineKeyboardButton::danger("🧹 Prune Cloud", "cb:prune"),
            ],
            vec![InlineKeyboardButton::success("🔄 Refresh", "cb:storage")],
        ],
    }
}

pub fn docker_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            primary_row(&[("🏠 Overview", "cb:status"), ("⚙️ Services", "cb:services")]),
            vec![InlineKeyboardButton::success("🔄 Refresh", "cb:docker")],
        ],
    }
}

pub fn services_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            primary_row(&[("🏠 Overview", "cb:status"), ("🐳 Docker", "cb:docker")]),
            vec![InlineKeyboardButton::success("🔄 Refresh", "cb:services")],
        ],
    }
}

/// Torrent navigation. Page controls appear only where a target page exists;
/// refresh always returns to the page the user is currently viewing.
pub fn torrents_keyboard(page: usize, total_pages: usize) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    if total_pages > 1 {
        let mut pager: Vec<(&str, String)> = Vec::new();
        if page > 1 {
            pager.push(("‹ Prev", format!("cb:torrents:{}", page - 1)));
        }
        if page < total_pages {
            pager.push(("Next ›", format!("cb:torrents:{}", page + 1)));
        }
        rows.push(
            pager
                .iter()
                .map(|(label, data)| InlineKeyboardButton::primary(label, data))
                .collect(),
        );
    }

    rows.push(vec![
        InlineKeyboardButton::primary("🏠 Overview", "cb:status"),
        InlineKeyboardButton::primary("🖥 System", "cb:sys"),
        InlineKeyboardButton::success("🔄 Refresh", &format!("cb:torrents:{page}")),
    ]);

    InlineKeyboardMarkup {
        inline_keyboard: rows,
    }
}

pub fn maintenance_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                InlineKeyboardButton::success("🚀 Organize", "cb:organize"),
                InlineKeyboardButton::danger("🧹 Prune", "cb:prune"),
            ],
            vec![
                InlineKeyboardButton::success("🔄 Sync", "cb:sync"),
                InlineKeyboardButton::primary("🔇 Dubstrip", "cb:audio"),
            ],
            vec![
                InlineKeyboardButton::primary("🔍 Audit Tools", "cb:check"),
                InlineKeyboardButton::primary("🏠 Overview", "cb:status"),
            ],
        ],
    }
}

/// One button per candidate language. The recommended entry carries a star.
/// Callback data stays `cb:al:<id>:<index>` to fit Telegram's 64-byte limit.
pub fn language_keyboard(
    id: &str,
    candidates: &[String],
    recommended: Option<&str>,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = candidates
        .iter()
        .enumerate()
        .map(|(idx, lang)| {
            let is_rec = recommended.is_some_and(|r| r.eq_ignore_ascii_case(lang));
            let label = if is_rec {
                format!("★ {lang} (recommended)")
            } else {
                lang.clone()
            };
            let btn = if is_rec {
                InlineKeyboardButton::success(&label, &format!("cb:al:{id}:{idx}"))
            } else {
                InlineKeyboardButton::primary(&label, &format!("cb:al:{id}:{idx}"))
            };
            vec![btn]
        })
        .collect();
    rows.push(vec![InlineKeyboardButton::danger(
        "Stop asking",
        &format!("cb:al:{id}:silent"),
    )]);
    InlineKeyboardMarkup {
        inline_keyboard: rows,
    }
}

pub fn seedr_queue_keyboard(hash: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![vec![
            InlineKeyboardButton::primary("Keep queued", &format!("cb:seedrq:keep:{hash}")),
            InlineKeyboardButton::success("Make priority", &format!("cb:seedrq:front:{hash}")),
        ]],
    }
}

/// Owner-only Approve/Deny buttons for a pending access request.
pub fn access_request_keyboard(user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![InlineKeyboardButton::success(
                "✅ Approve",
                &format!("cb:access:ok:{user_id}"),
            )],
            vec![InlineKeyboardButton::danger(
                "❌ Deny",
                &format!("cb:access:no:{user_id}"),
            )],
        ],
    }
}

pub fn reboot_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![InlineKeyboardButton::danger(
                "🔁 Confirm Reboot",
                "cb:reboot_confirm",
            )],
            vec![InlineKeyboardButton::primary("✖ Cancel", "cb:cancel")],
        ],
    }
}

pub fn poweroff_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![InlineKeyboardButton::danger(
                "⏻ Confirm Power Off",
                "cb:poweroff_confirm",
            )],
            vec![InlineKeyboardButton::primary("✖ Cancel", "cb:cancel")],
        ],
    }
}

/// Quick-nav keyboard for /help and /start.
pub fn help_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            primary_row(&[("📊 Status", "cb:status"), ("🧲 Torrents", "cb:torrents")]),
            primary_row(&[("🐳 Docker", "cb:docker"), ("💾 Storage", "cb:storage")]),
            vec![InlineKeyboardButton::primary(
                "🧰 Maintenance",
                "cb:maintenance",
            )],
        ],
    }
}

/// Re-run / back-nav keyboard appended to one-shot action result messages.
pub fn action_done_keyboard(back_cb: &str, back_label: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![vec![
            InlineKeyboardButton::primary("🏠 Overview", "cb:status"),
            InlineKeyboardButton::primary(back_label, back_cb),
        ]],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torrents_keyboard_page_controls() {
        let first = torrents_keyboard(1, 3);
        let first_labels: Vec<_> = first.inline_keyboard[0]
            .iter()
            .map(|b| b.text.as_str())
            .collect();
        assert_eq!(first_labels, vec!["Next ›"]);

        let middle = torrents_keyboard(2, 3);
        let middle_labels: Vec<_> = middle.inline_keyboard[0]
            .iter()
            .map(|b| b.text.as_str())
            .collect();
        assert_eq!(middle_labels, vec!["‹ Prev", "Next ›"]);
        assert_eq!(
            middle.inline_keyboard[1][2].callback_data.as_deref(),
            Some("cb:torrents:2")
        );

        let last = torrents_keyboard(3, 3);
        let last_labels: Vec<_> = last.inline_keyboard[0]
            .iter()
            .map(|b| b.text.as_str())
            .collect();
        assert_eq!(last_labels, vec!["‹ Prev"]);
    }

    #[test]
    fn test_torrents_keyboard_single_page_has_no_pager() {
        let kb = torrents_keyboard(1, 1);
        assert_eq!(kb.inline_keyboard.len(), 1);
        assert_eq!(
            kb.inline_keyboard[0][2].callback_data.as_deref(),
            Some("cb:torrents:1")
        );
    }

    #[test]
    fn test_colored_button_styles() {
        let status_kb = status_keyboard();
        let refresh_btn = &status_kb.inline_keyboard[2][1];
        assert_eq!(refresh_btn.style.as_deref(), Some("success"));

        let reboot_kb = reboot_keyboard();
        let confirm_btn = &reboot_kb.inline_keyboard[0][0];
        assert_eq!(confirm_btn.style.as_deref(), Some("danger"));
        let cancel_btn = &reboot_kb.inline_keyboard[1][0];
        assert_eq!(cancel_btn.style.as_deref(), Some("primary"));
    }
}
