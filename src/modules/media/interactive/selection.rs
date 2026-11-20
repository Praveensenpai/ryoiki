use super::{AppState, MediaFile, MediaItem, TransferDirection};
use crate::modules::media::disk;
use std::path::PathBuf;

pub fn toggle_main_item(state: &mut AppState, real_idx: usize) {
    let has_seasons = state
        .current_items()
        .get(real_idx)
        .is_some_and(MediaItem::has_seasons);

    if has_seasons {
        toggle_show_seasons(state, real_idx);
    } else {
        toggle_single_item(state, real_idx);
    }
}

fn toggle_show_seasons(state: &mut AppState, real_idx: usize) {
    let (all_sel, needed) = {
        let Some(item) = state.current_items().get(real_idx) else {
            return;
        };
        let all = item.are_all_seasons_selected();
        let n: u64 = if state.direction == TransferDirection::Pull {
            item.seasons
                .iter()
                .filter(|s| !s.is_selected)
                .map(|s| s.sync_status.missing_bytes)
                .sum()
        } else {
            item.seasons
                .iter()
                .filter(|s| !s.is_selected)
                .map(|s| s.size_bytes)
                .sum()
        };
        (all, n)
    };

    if all_sel {
        if let Some(item) = state.current_items_mut().get_mut(real_idx) {
            for s in &mut item.seasons {
                s.is_selected = false;
            }
        }
        state.warning_msg = None;
        return;
    }

    if !state.can_add_bytes(needed) {
        let free = state.remaining_free_bytes().unwrap_or(0);
        state.warning_msg = Some(format!(
            "⚠️ Show exceeds free space (free: {}, need: {})",
            disk::format_bytes(free),
            disk::format_bytes(needed)
        ));
        return;
    }

    if let Some(item) = state.current_items_mut().get_mut(real_idx) {
        for s in &mut item.seasons {
            s.is_selected = true;
        }
    }
    state.warning_msg = None;
}

fn toggle_single_item(state: &mut AppState, real_idx: usize) {
    let item_needed = if state.direction == TransferDirection::Pull {
        state
            .current_items()
            .get(real_idx)
            .map_or(0, |it| it.sync_status.missing_bytes)
    } else {
        state
            .current_items()
            .get(real_idx)
            .map_or(0, |it| it.size_bytes)
    };
    let is_sel = state
        .current_selected()
        .get(real_idx)
        .copied()
        .unwrap_or(false);

    if is_sel {
        if let Some(val) = state.current_selected().get_mut(real_idx) {
            *val = false;
        }
        state.warning_msg = None;
        return;
    }

    if !state.can_add_bytes(item_needed) {
        let free = state.remaining_free_bytes().unwrap_or(0);
        state.warning_msg = Some(format!(
            "⚠️ Exceeds free storage (free: {}, need: {})",
            disk::format_bytes(free),
            disk::format_bytes(item_needed)
        ));
        return;
    }

    if let Some(val) = state.current_selected().get_mut(real_idx) {
        *val = true;
    }
    state.warning_msg = None;
}

/// Toggles selection of a single file inside a movie (or season folder).
pub fn toggle_file(
    state: &mut AppState,
    item_idx: usize,
    season_idx: Option<usize>,
    file_idx: usize,
) {
    let (is_selected, cost) = {
        let Some((file, is_pull)) = file_ref(state, item_idx, season_idx, file_idx) else {
            return;
        };
        // On Pull, only files missing locally consume disk space.
        let cost = if is_pull && file.exists_in_other {
            0
        } else {
            file.size_bytes
        };
        (file.is_selected, cost)
    };

    if is_selected {
        if let Some(file) = file_mut(state, item_idx, season_idx, file_idx) {
            file.is_selected = false;
        }
        state.warning_msg = None;
        return;
    }

    if !state.can_add_bytes(cost) {
        let free = state.remaining_free_bytes().unwrap_or(0);
        state.warning_msg = Some(format!(
            "⚠️ File exceeds free storage (free: {}, need: {})",
            disk::format_bytes(free),
            disk::format_bytes(cost)
        ));
        return;
    }

    if let Some(file) = file_mut(state, item_idx, season_idx, file_idx) {
        file.is_selected = true;
    }
    // Selecting individual files supersedes whole-item selection.
    if let Some(val) = state.current_selected().get_mut(item_idx) {
        *val = false;
    }
    state.warning_msg = None;
}

/// Selects all files inside a movie (or season folder), respecting disk space.
pub fn select_all_files(state: &mut AppState, item_idx: usize, season_idx: Option<usize>) {
    let is_pull = state.direction == TransferDirection::Pull;
    let candidates: Vec<(usize, u64, bool)> = {
        let files = files_ref(state, item_idx, season_idx);
        let Some(files) = files else { return };
        files
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.is_selected)
            .map(|(idx, f)| {
                let cost = if is_pull && f.exists_in_other {
                    0
                } else {
                    f.size_bytes
                };
                (idx, cost, f.exists_in_other)
            })
            .collect()
    };

    let mut accumulated = 0u64;
    let mut to_select = Vec::new();
    let mut hit_limit = false;
    for (idx, cost, _) in candidates {
        let needed = accumulated.saturating_add(cost);
        if state.can_add_bytes(needed) {
            accumulated = needed;
            to_select.push(idx);
        } else {
            hit_limit = true;
            break;
        }
    }
    for idx in to_select {
        if let Some(file) = file_mut(state, item_idx, season_idx, idx) {
            file.is_selected = true;
        }
    }
    if let Some(val) = state.current_selected().get_mut(item_idx) {
        *val = false;
    }
    state.warning_msg =
        hit_limit.then(|| "⚠️ Limit reached: selected files up to available storage".to_string());
}

/// Clears all per-file selections inside a movie (or season folder).
pub fn clear_all_files(state: &mut AppState, item_idx: usize, season_idx: Option<usize>) {
    if let Some(files) = files_mut(state, item_idx, season_idx) {
        for f in files {
            f.is_selected = false;
        }
    }
    state.warning_msg = None;
}

fn file_ref<'a>(
    state: &'a AppState,
    item_idx: usize,
    season_idx: Option<usize>,
    file_idx: usize,
) -> Option<(&'a MediaFile, bool)> {
    let file = files_ref(state, item_idx, season_idx)?.get(file_idx)?;
    Some((file, state.direction == TransferDirection::Pull))
}

fn file_mut<'a>(
    state: &'a mut AppState,
    item_idx: usize,
    season_idx: Option<usize>,
    file_idx: usize,
) -> Option<&'a mut MediaFile> {
    files_mut(state, item_idx, season_idx)?.get_mut(file_idx)
}

fn files_ref<'a>(
    state: &'a AppState,
    item_idx: usize,
    season_idx: Option<usize>,
) -> Option<&'a [MediaFile]> {
    let item = state.current_items().get(item_idx)?;
    Some(match season_idx {
        Some(s) => &item.seasons.get(s)?.files,
        None => &item.files,
    })
}

fn files_mut<'a>(
    state: &'a mut AppState,
    item_idx: usize,
    season_idx: Option<usize>,
) -> Option<&'a mut Vec<MediaFile>> {
    let item = state.current_items_mut().get_mut(item_idx)?;
    Some(match season_idx {
        Some(s) => &mut item.seasons.get_mut(s)?.files,
        None => &mut item.files,
    })
}

/// Builds a transfer item for a single individually-selected file.
fn file_item(
    item: &MediaItem,
    local_base: &Option<PathBuf>,
    remote_base: &str,
    file: &MediaFile,
) -> MediaItem {
    let local_path = local_base.as_ref().map(|p| p.join(&file.rel_path));
    MediaItem {
        title: format!("{} — {}", item.title, file.name),
        category: item.category,
        size_bytes: file.size_bytes,
        is_watched: false,
        local_path,
        remote_path: format!("{remote_base}/{}", file.rel_path),
        seasons: Vec::new(),
        files: vec![file.clone()],
        sync_status: crate::modules::media::transfer::SyncStatus::from_files(&[file.clone()]),
    }
}

pub fn collect_transfer_items(state: &AppState) -> Vec<MediaItem> {
    let mut result = Vec::new();
    let (items, selected_mask) = match state.direction {
        TransferDirection::Push => (&state.local_items, &state.local_selected),
        TransferDirection::Pull => (&state.remote_items, &state.remote_selected),
    };

    for (idx, item) in items.iter().enumerate() {
        if item.has_seasons() {
            for season in &item.seasons {
                if season.is_selected {
                    result.push(MediaItem {
                        title: format!("{} - {}", item.title, season.title),
                        category: item.category,
                        size_bytes: season.size_bytes,
                        is_watched: false,
                        local_path: season.local_path.clone(),
                        remote_path: season.remote_path.clone(),
                        seasons: Vec::new(),
                        files: season.files.clone(),
                        sync_status: season.sync_status,
                    });
                } else {
                    for file in season.files.iter().filter(|f| f.is_selected) {
                        result.push(file_item(
                            item,
                            &season.local_path,
                            &season.remote_path,
                            file,
                        ));
                    }
                }
            }
        } else if selected_mask.get(idx).copied().unwrap_or(false) {
            result.push(item.clone());
        } else {
            for file in item.files.iter().filter(|f| f.is_selected) {
                result.push(file_item(item, &item.local_path, &item.remote_path, file));
            }
        }
    }
    result
}
