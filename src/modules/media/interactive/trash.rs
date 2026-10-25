//! Trash confirmation modal and the actions it can trigger.

use crossterm::event::KeyCode;
use std::path::PathBuf;

use super::{AppState, MediaItem, TransferDirection, ViewMode};
use crate::modules::media::trash::{self, TrashEntry, TrashKind};

#[derive(Debug, Clone)]
pub enum PendingAction {
    TrashLocal {
        path: PathBuf,
        kind: TrashKind,
        title: String,
    },
    TrashRemote {
        remote: String,
        kind: TrashKind,
        title: String,
        size_bytes: u64,
    },
    Restore(TrashEntry),
    Purge(TrashEntry),
}

#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub prompt: String,
    pub detail: String,
    /// Verb shown on the affirmative button, e.g. "Yes, trash it".
    pub yes_label: String,
    /// `false` means the safe "No" button is focused.
    pub yes_focused: bool,
    pub action: PendingAction,
}

impl ConfirmState {
    fn new(
        prompt: impl Into<String>,
        detail: impl Into<String>,
        yes_label: impl Into<String>,
        action: PendingAction,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            detail: detail.into(),
            yes_label: yes_label.into(),
            yes_focused: false,
            action,
        }
    }
}

/// Builds a confirmation for trashing whatever the cursor points at.
pub fn request_from_current(state: &mut AppState) {
    if state.modal.is_some() {
        return;
    }
    let Some((path, remote, kind, title, size)) = target_of_cursor(state) else {
        state.warning_msg = Some("⚠️ Nothing to trash here".to_string());
        return;
    };

    let action = if state.direction == TransferDirection::Pull {
        PendingAction::TrashRemote {
            remote,
            kind,
            title,
            size_bytes: size,
        }
    } else {
        PendingAction::TrashLocal { path, kind, title }
    };

    state.modal = Some(ConfirmState::new(
        "Move to trash?",
        "Recoverable for 7 days, then deleted for good.",
        "Yes, trash it",
        action,
    ));
}

/// Asks for confirmation before restoring a trashed entry.
pub fn request_restore(state: &mut AppState) {
    if state.modal.is_some() {
        return;
    }
    let Some(entry) = selected_trash_entry(state) else {
        return;
    };
    state.modal = Some(ConfirmState::new(
        "Restore from trash?",
        format!("{} returns to its original location.", entry.title),
        "Yes, restore it",
        PendingAction::Restore(entry),
    ));
}

/// Asks for confirmation before deleting a trashed entry permanently.
pub fn request_purge(state: &mut AppState) {
    if state.modal.is_some() {
        return;
    }
    let Some(entry) = selected_trash_entry(state) else {
        return;
    };
    state.modal = Some(ConfirmState::new(
        "Delete permanently?",
        format!("{} cannot be recovered after this.", entry.title),
        "Yes, delete forever",
        PendingAction::Purge(entry),
    ));
}

fn selected_trash_entry(state: &AppState) -> Option<TrashEntry> {
    state.trash_entries.get(state.trash_cursor).cloned()
}

/// Resolves the cursor into (local path, remote path, kind, title, size).
fn target_of_cursor(state: &AppState) -> Option<(PathBuf, String, TrashKind, String, u64)> {
    let item_idx = match state.view_mode {
        ViewMode::Main => {
            let indices = state.filtered_indices();
            *indices.get(state.cursor)?
        }
        ViewMode::SubView { item_idx, .. } | ViewMode::FilesView { item_idx, .. } => item_idx,
    };
    let item = state.current_items().get(item_idx)?;

    match state.view_mode {
        ViewMode::FilesView {
            season_idx, cursor, ..
        } => file_target(item, season_idx, cursor),
        ViewMode::SubView { cursor, .. } => {
            let season = item.seasons.get(cursor)?;
            Some((
                season.local_path.clone().unwrap_or_default(),
                season.remote_path.clone(),
                TrashKind::Season,
                format!("{} — {}", item.title, season.title),
                season.size_bytes,
            ))
        }
        ViewMode::Main => Some((
            item.local_path.clone().unwrap_or_default(),
            item.remote_path.clone(),
            TrashKind::Title,
            item.title.clone(),
            item.size_bytes,
        )),
    }
}

fn file_target(
    item: &MediaItem,
    season_idx: Option<usize>,
    cursor: usize,
) -> Option<(PathBuf, String, TrashKind, String, u64)> {
    let (files, local_base, remote_base) = match season_idx {
        Some(s_idx) => {
            let season = item.seasons.get(s_idx)?;
            (
                &season.files,
                season.local_path.clone().unwrap_or_default(),
                season.remote_path.clone(),
            )
        }
        None => (
            &item.files,
            item.local_path.clone().unwrap_or_default(),
            item.remote_path.clone(),
        ),
    };
    let file = files.get(cursor)?;
    Some((
        local_base.join(&file.rel_path),
        format!("{remote_base}/{}", file.rel_path),
        TrashKind::File,
        format!("{} — {}", item.title, file.name),
        file.size_bytes,
    ))
}

/// Handles keys while a confirmation modal is open.
///
/// Returns `Some(false)` when the user cancels out of the whole TUI.
pub fn handle_modal_key(code: KeyCode, state: &mut AppState) -> Option<bool> {
    let modal = state.modal.as_mut()?;

    match code {
        KeyCode::Left | KeyCode::Char('h') => {
            modal.yes_focused = false;
            None
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            modal.yes_focused = true;
            None
        }
        KeyCode::Char('y' | 'Y') => {
            modal.yes_focused = true;
            confirm(state)
        }
        KeyCode::Enter => {
            if modal.yes_focused {
                confirm(state)
            } else {
                state.modal = None;
                None
            }
        }
        KeyCode::Esc | KeyCode::Char('q' | 'n' | 'N') => {
            state.modal = None;
            None
        }
        _ => None,
    }
}

fn confirm(state: &mut AppState) -> Option<bool> {
    let modal = state.modal.take()?;

    let outcome = match &modal.action {
        PendingAction::TrashLocal { path, kind, title } => {
            trash::trash_local(path, *kind, title, &state.home_path).map(|_| "Moved to trash")
        }
        PendingAction::TrashRemote {
            remote,
            kind,
            title,
            size_bytes,
        } => trash::trash_remote(remote, *kind, title, *size_bytes).map(|_| "Moved to trash"),
        PendingAction::Restore(entry) => trash::restore_entry(entry).map(|()| "Restored"),
        PendingAction::Purge(entry) => trash::purge_entry(entry).map(|()| "Deleted permanently"),
    };

    match outcome {
        Ok(msg) => {
            state.warning_msg = Some(format!("✨ {msg}"));
            state.refresh_after_mutation();
        }
        Err(err) => {
            state.warning_msg = Some(format!("⚠️ {err}"));
        }
    }
    None
}
