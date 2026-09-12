use crossterm::event::KeyCode;

use super::{AppState, MediaItem, TransferDirection, ViewMode};
use crate::modules::media::disk;

pub fn handle_key(code: KeyCode, state: &mut AppState) -> Option<bool> {
    match state.view_mode {
        ViewMode::SubView { item_idx, cursor } => handle_subview_key(code, state, item_idx, cursor),
        ViewMode::Main => handle_main_key(code, state),
    }
}

fn handle_subview_key(
    code: KeyCode,
    state: &mut AppState,
    item_idx: usize,
    mut cursor: usize,
) -> Option<bool> {
    let seasons_len = state
        .current_items()
        .get(item_idx)
        .map_or(0, |it| it.seasons.len());

    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.warning_msg = None;
            if seasons_len > 0 {
                cursor = if cursor > 0 {
                    cursor - 1
                } else {
                    seasons_len - 1
                };
                state.view_mode = ViewMode::SubView { item_idx, cursor };
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.warning_msg = None;
            if seasons_len > 0 {
                cursor = if cursor + 1 < seasons_len {
                    cursor + 1
                } else {
                    0
                };
                state.view_mode = ViewMode::SubView { item_idx, cursor };
            }
            None
        }
        KeyCode::Char(' ') => {
            toggle_subview_season(state, item_idx, cursor);
            None
        }
        KeyCode::Char('a') => {
            select_all_subview_seasons(state, item_idx);
            None
        }
        KeyCode::Char('n') => {
            if let Some(item) = state.current_items_mut().get_mut(item_idx) {
                for s in &mut item.seasons {
                    s.is_selected = false;
                }
            }
            state.warning_msg = None;
            None
        }
        KeyCode::Enter | KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') => {
            state.warning_msg = None;
            state.view_mode = ViewMode::Main;
            None
        }
        _ => None,
    }
}

fn toggle_subview_season(state: &mut AppState, item_idx: usize, cursor: usize) {
    let (is_selected, season_size) = {
        let Some(item) = state.current_items().get(item_idx) else {
            return;
        };
        let Some(season) = item.seasons.get(cursor) else {
            return;
        };
        (season.is_selected, season.size_bytes)
    };

    if is_selected {
        if let Some(item) = state.current_items_mut().get_mut(item_idx) {
            if let Some(season) = item.seasons.get_mut(cursor) {
                season.is_selected = false;
            }
        }
        state.warning_msg = None;
        return;
    }

    if !state.can_add_bytes(season_size) {
        let free = state.remaining_free_bytes().unwrap_or(0);
        state.warning_msg = Some(format!(
            "⚠️ Season exceeds free storage (free: {}, need: {})",
            disk::format_bytes(free),
            disk::format_bytes(season_size)
        ));
        return;
    }

    if let Some(item) = state.current_items_mut().get_mut(item_idx) {
        if let Some(season) = item.seasons.get_mut(cursor) {
            season.is_selected = true;
        }
    }
    state.warning_msg = None;
}

fn select_all_subview_seasons(state: &mut AppState, item_idx: usize) {
    let unselected: Vec<(usize, u64)> = {
        let Some(item) = state.current_items().get(item_idx) else {
            return;
        };
        item.seasons
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_selected)
            .map(|(idx, s)| (idx, s.size_bytes))
            .collect()
    };

    let mut hit_limit = false;
    let mut to_select = Vec::new();
    let mut accumulated: u64 = 0;
    for (s_idx, sz) in unselected {
        let needed = accumulated.saturating_add(sz);
        if state.can_add_bytes(needed) {
            accumulated = needed;
            to_select.push(s_idx);
        } else {
            hit_limit = true;
            break;
        }
    }

    if let Some(item) = state.current_items_mut().get_mut(item_idx) {
        for s_idx in to_select {
            if let Some(s) = item.seasons.get_mut(s_idx) {
                s.is_selected = true;
            }
        }
    }

    if hit_limit {
        state.warning_msg =
            Some("⚠️ Limit reached: remaining seasons exceed free storage".to_string());
    } else {
        state.warning_msg = None;
    }
}

fn handle_main_key(code: KeyCode, state: &mut AppState) -> Option<bool> {
    let indices = state.filtered_indices();
    let total = indices.len();

    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.warning_msg = None;
            if total > 0 {
                state.cursor = if state.cursor > 0 {
                    state.cursor - 1
                } else {
                    total - 1
                };
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.warning_msg = None;
            if total > 0 {
                state.cursor = if state.cursor + 1 < total {
                    state.cursor + 1
                } else {
                    0
                };
            }
            None
        }
        KeyCode::Char(' ') => {
            if let Some(&real_idx) = indices.get(state.cursor) {
                state.toggle_main_item(real_idx);
            }
            None
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('o') => {
            if let Some(&real_idx) = indices.get(state.cursor) {
                let has_seasons = state
                    .current_items()
                    .get(real_idx)
                    .is_some_and(MediaItem::has_seasons);
                if has_seasons {
                    state.view_mode = ViewMode::SubView {
                        item_idx: real_idx,
                        cursor: 0,
                    };
                } else {
                    state.toggle_main_item(real_idx);
                }
            }
            None
        }
        KeyCode::Char('p') => Some(true),
        KeyCode::Tab => {
            state.direction = match state.direction {
                TransferDirection::Push => TransferDirection::Pull,
                TransferDirection::Pull => TransferDirection::Push,
            };
            state.cursor = 0;
            state.warning_msg = None;
            None
        }
        KeyCode::Char('a') => {
            select_all_main_items(state, &indices);
            None
        }
        KeyCode::Char('n') => {
            deselect_all_main_items(state, &indices);
            None
        }
        KeyCode::Char('f') => {
            state.filter = state.filter.next();
            state.cursor = 0;
            state.warning_msg = None;
            None
        }
        KeyCode::Char('q') | KeyCode::Esc => Some(false),
        _ => None,
    }
}

fn select_all_main_items(state: &mut AppState, indices: &[usize]) {
    let mut hit_limit = false;
    let mut accumulated: u64 = 0;
    let mut items_to_select = Vec::new();
    let mut seasons_to_select: Vec<(usize, usize)> = Vec::new();

    for &idx in indices {
        let Some(item) = state.current_items().get(idx) else {
            continue;
        };
        if item.has_seasons() {
            for (s_idx, s) in item.seasons.iter().enumerate() {
                if !s.is_selected {
                    let needed = accumulated.saturating_add(s.size_bytes);
                    if state.can_add_bytes(needed) {
                        accumulated = needed;
                        seasons_to_select.push((idx, s_idx));
                    } else {
                        hit_limit = true;
                        break;
                    }
                }
            }
        } else {
            let is_sel = state.is_selected(idx);
            if !is_sel {
                let needed = accumulated.saturating_add(item.size_bytes);
                if state.can_add_bytes(needed) {
                    accumulated = needed;
                    items_to_select.push(idx);
                } else {
                    hit_limit = true;
                }
            }
        }
    }

    for (item_idx, s_idx) in seasons_to_select {
        if let Some(item) = state.current_items_mut().get_mut(item_idx) {
            if let Some(s) = item.seasons.get_mut(s_idx) {
                s.is_selected = true;
            }
        }
    }

    for idx in items_to_select {
        if let Some(val) = state.current_selected().get_mut(idx) {
            *val = true;
        }
    }

    if hit_limit {
        state.warning_msg =
            Some("⚠️ Limit reached: selected items up to available storage".to_string());
    } else {
        state.warning_msg = None;
    }
}

fn deselect_all_main_items(state: &mut AppState, indices: &[usize]) {
    for &idx in indices {
        if let Some(item) = state.current_items_mut().get_mut(idx) {
            for s in &mut item.seasons {
                s.is_selected = false;
            }
        }
    }
    let sel = state.current_selected();
    for &idx in indices {
        if let Some(val) = sel.get_mut(idx) {
            *val = false;
        }
    }
    state.warning_msg = None;
}
