pub mod events;
pub mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use super::disk::{self, DiskUsage};
use super::transfer::{MediaCategory, MediaItem, TransferDirection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemFilter {
    All,
    Movies,
    Shows,
}

impl ItemFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Movies,
            Self::Movies => Self::Shows,
            Self::Shows => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All Media",
            Self::Movies => "Movies Only",
            Self::Shows => "Series / Anime",
        }
    }

    pub fn matches(self, cat: MediaCategory) -> bool {
        match self {
            Self::All => true,
            Self::Movies => cat == MediaCategory::Movie,
            Self::Shows => cat == MediaCategory::Show || cat == MediaCategory::Anime,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Main,
    SubView { item_idx: usize, cursor: usize },
}

pub struct AppState {
    pub direction: TransferDirection,
    pub local_items: Vec<MediaItem>,
    pub remote_items: Vec<MediaItem>,
    pub local_selected: Vec<bool>,
    pub remote_selected: Vec<bool>,
    pub filter: ItemFilter,
    pub cursor: usize,
    pub view_mode: ViewMode,
    pub local_disk: Option<DiskUsage>,
    pub warning_msg: Option<String>,
}

impl AppState {
    pub fn new(
        local: Vec<MediaItem>,
        remote: Vec<MediaItem>,
        dir: TransferDirection,
        local_disk: Option<DiskUsage>,
    ) -> Self {
        let local_len = local.len();
        let remote_len = remote.len();
        Self {
            direction: dir,
            local_items: local,
            remote_items: remote,
            local_selected: vec![false; local_len],
            remote_selected: vec![false; remote_len],
            filter: ItemFilter::All,
            cursor: 0,
            view_mode: ViewMode::Main,
            local_disk,
            warning_msg: None,
        }
    }

    pub fn current_items(&self) -> &[MediaItem] {
        match self.direction {
            TransferDirection::Push => &self.local_items,
            TransferDirection::Pull => &self.remote_items,
        }
    }

    pub fn current_items_mut(&mut self) -> &mut [MediaItem] {
        match self.direction {
            TransferDirection::Push => &mut self.local_items,
            TransferDirection::Pull => &mut self.remote_items,
        }
    }

    pub fn is_selected(&self, idx: usize) -> bool {
        let mask = match self.direction {
            TransferDirection::Push => &self.local_selected,
            TransferDirection::Pull => &self.remote_selected,
        };
        mask.get(idx).copied().unwrap_or(false)
    }

    pub fn current_selected(&mut self) -> &mut [bool] {
        match self.direction {
            TransferDirection::Push => &mut self.local_selected,
            TransferDirection::Pull => &mut self.remote_selected,
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let items = self.current_items();
        items
            .iter()
            .enumerate()
            .filter(|(_, it)| self.filter.matches(it.category))
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn total_selected_bytes(&self) -> u64 {
        let (items, selected_mask) = match self.direction {
            TransferDirection::Push => (&self.local_items, &self.local_selected),
            TransferDirection::Pull => (&self.remote_items, &self.remote_selected),
        };
        items
            .iter()
            .enumerate()
            .map(|(idx, it)| it.selected_bytes(selected_mask.get(idx).copied().unwrap_or(false)))
            .sum()
    }

    pub fn can_add_bytes(&self, additional_bytes: u64) -> bool {
        if self.direction != TransferDirection::Pull {
            return true;
        }
        let Some(disk) = self.local_disk else {
            return true;
        };
        self.total_selected_bytes().saturating_add(additional_bytes) <= disk.free_bytes
    }

    pub fn remaining_free_bytes(&self) -> Option<u64> {
        let disk = self.local_disk?;
        match self.direction {
            TransferDirection::Push => {
                Some(disk.free_bytes.saturating_add(self.total_selected_bytes()))
            }
            TransferDirection::Pull => {
                Some(disk.free_bytes.saturating_sub(self.total_selected_bytes()))
            }
        }
    }

    pub fn toggle_main_item(&mut self, real_idx: usize) {
        let has_seasons = self
            .current_items()
            .get(real_idx)
            .is_some_and(MediaItem::has_seasons);

        if has_seasons {
            self.toggle_show_seasons(real_idx);
        } else {
            self.toggle_single_item(real_idx);
        }
    }

    fn toggle_show_seasons(&mut self, real_idx: usize) {
        let (all_sel, needed) = {
            let Some(item) = self.current_items().get(real_idx) else {
                return;
            };
            let all = item.are_all_seasons_selected();
            let n: u64 = item
                .seasons
                .iter()
                .filter(|s| !s.is_selected)
                .map(|s| s.size_bytes)
                .sum();
            (all, n)
        };

        if all_sel {
            if let Some(item) = self.current_items_mut().get_mut(real_idx) {
                for s in &mut item.seasons {
                    s.is_selected = false;
                }
            }
            self.warning_msg = None;
            return;
        }

        if !self.can_add_bytes(needed) {
            let free = self.remaining_free_bytes().unwrap_or(0);
            self.warning_msg = Some(format!(
                "⚠️ Show exceeds free space (free: {}, need: {})",
                disk::format_bytes(free),
                disk::format_bytes(needed)
            ));
            return;
        }

        if let Some(item) = self.current_items_mut().get_mut(real_idx) {
            for s in &mut item.seasons {
                s.is_selected = true;
            }
        }
        self.warning_msg = None;
    }

    fn toggle_single_item(&mut self, real_idx: usize) {
        let item_size = self
            .current_items()
            .get(real_idx)
            .map_or(0, |it| it.size_bytes);
        let is_sel = self
            .current_selected()
            .get(real_idx)
            .copied()
            .unwrap_or(false);

        if is_sel {
            if let Some(val) = self.current_selected().get_mut(real_idx) {
                *val = false;
            }
            self.warning_msg = None;
            return;
        }

        if !self.can_add_bytes(item_size) {
            let free = self.remaining_free_bytes().unwrap_or(0);
            self.warning_msg = Some(format!(
                "⚠️ Exceeds free storage (free: {}, need: {})",
                disk::format_bytes(free),
                disk::format_bytes(item_size)
            ));
            return;
        }

        if let Some(val) = self.current_selected().get_mut(real_idx) {
            *val = true;
        }
        self.warning_msg = None;
    }

    pub fn collect_transfer_items(&self) -> Vec<MediaItem> {
        let mut result = Vec::new();
        let (items, selected_mask) = match self.direction {
            TransferDirection::Push => (&self.local_items, &self.local_selected),
            TransferDirection::Pull => (&self.remote_items, &self.remote_selected),
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
                        });
                    }
                }
            } else if selected_mask.get(idx).copied().unwrap_or(false) {
                result.push(item.clone());
            }
        }
        result
    }
}

pub fn run_interactive_tui(
    local: Vec<MediaItem>,
    remote: Vec<MediaItem>,
    dir: TransferDirection,
    local_disk: Option<DiskUsage>,
) -> Result<Option<(TransferDirection, Vec<MediaItem>)>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(local, remote, dir, local_disk);
    let confirmed = run_event_loop(&mut terminal, &mut state)?;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if !confirmed {
        return Ok(None);
    }

    let chosen = state.collect_transfer_items();
    if chosen.is_empty() {
        Ok(None)
    } else {
        Ok(Some((state.direction, chosen)))
    }
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> Result<bool> {
    loop {
        terminal.draw(|f| ui::render_ui(f, state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if let Some(res) = events::handle_key(key.code, state) {
                    return Ok(res);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cycle() {
        assert_eq!(ItemFilter::All.next(), ItemFilter::Movies);
        assert_eq!(ItemFilter::Movies.next(), ItemFilter::Shows);
        assert_eq!(ItemFilter::Shows.next(), ItemFilter::All);
    }

    #[test]
    fn test_filter_matches() {
        assert!(ItemFilter::All.matches(MediaCategory::Movie));
        assert!(ItemFilter::All.matches(MediaCategory::Show));
        assert!(ItemFilter::Movies.matches(MediaCategory::Movie));
        assert!(!ItemFilter::Movies.matches(MediaCategory::Show));
        assert!(ItemFilter::Shows.matches(MediaCategory::Anime));
    }
}
