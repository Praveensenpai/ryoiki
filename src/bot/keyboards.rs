use super::types::{InlineKeyboardButton, InlineKeyboardMarkup};

fn row(buttons: &[(&str, &str)]) -> Vec<InlineKeyboardButton> {
    buttons
        .iter()
        .map(|(label, data)| InlineKeyboardButton::callback(label, data))
        .collect()
}

pub fn status_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[
                ("System", "cb:sys"),
                ("Torrents", "cb:torrents"),
                ("Docker", "cb:docker"),
            ]),
            row(&[("Storage", "cb:storage"), ("Services", "cb:services")]),
            row(&[("Maintenance", "cb:maintenance"), ("Refresh", "cb:status")]),
        ],
    }
}

pub fn system_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Overview", "cb:status"), ("Storage", "cb:storage")]),
            row(&[("Refresh", "cb:sys")]),
        ],
    }
}

pub fn storage_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Overview", "cb:status"), ("Prune Cloud", "cb:prune")]),
            row(&[("Refresh", "cb:storage")]),
        ],
    }
}

pub fn docker_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Overview", "cb:status"), ("Services", "cb:services")]),
            row(&[("Refresh", "cb:docker")]),
        ],
    }
}

pub fn services_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Overview", "cb:status"), ("Docker", "cb:docker")]),
            row(&[("Refresh", "cb:services")]),
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
                .map(|(label, data)| InlineKeyboardButton::callback(label, data))
                .collect(),
        );
    }

    rows.push(row(&[
        ("Overview", "cb:status"),
        ("System", "cb:sys"),
        ("Refresh", &format!("cb:torrents:{page}")),
    ]));

    InlineKeyboardMarkup {
        inline_keyboard: rows,
    }
}

pub fn maintenance_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Organize", "cb:organize"), ("Prune", "cb:prune")]),
            row(&[("Sync", "cb:sync"), ("Dubstrip", "cb:audio")]),
            row(&[("Audit Tools", "cb:check"), ("Overview", "cb:status")]),
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
            vec![InlineKeyboardButton::callback(
                &label,
                &format!("cb:al:{id}:{idx}"),
            )]
        })
        .collect();
    rows.push(row(&[("Stop asking", &format!("cb:al:{id}:silent"))]));
    InlineKeyboardMarkup {
        inline_keyboard: rows,
    }
}

pub fn seedr_queue_keyboard(hash: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![row(&[
            ("Keep queued", &format!("cb:seedrq:keep:{hash}")),
            ("Download first", &format!("cb:seedrq:front:{hash}")),
        ])],
    }
}

/// Owner-only Approve/Deny buttons for a pending access request.
pub fn access_request_keyboard(user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Approve", &format!("cb:access:ok:{user_id}"))]),
            row(&[("Deny", &format!("cb:access:no:{user_id}"))]),
        ],
    }
}

pub fn reboot_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Confirm reboot", "cb:reboot_confirm")]),
            row(&[("Cancel", "cb:cancel")]),
        ],
    }
}

pub fn poweroff_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            row(&[("Confirm power off", "cb:poweroff_confirm")]),
            row(&[("Cancel", "cb:cancel")]),
        ],
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
}
