use super::{AppState, MediaItem, TransferDirection};
use crate::modules::media::disk;

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
                }
            }
        } else if selected_mask.get(idx).copied().unwrap_or(false) {
            result.push(item.clone());
        }
    }
    result
}
