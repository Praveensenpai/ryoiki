pub mod execute;
pub mod notify;
pub mod scan;

pub use execute::{confirm_transfer, execute_transfer};
pub use scan::{scan_local_media, scan_remote_media};

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Push,
    Pull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCategory {
    Movie,
    Show,
    Anime,
}

impl MediaCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "Movie",
            Self::Show => "Show",
            Self::Anime => "Anime",
        }
    }

    pub fn remote_folder(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Show => "shows",
            Self::Anime => "anime",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaSeason {
    pub title: String,
    pub size_bytes: u64,
    pub local_path: Option<PathBuf>,
    pub remote_path: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone)]
pub struct MediaItem {
    pub title: String,
    pub category: MediaCategory,
    pub size_bytes: u64,
    pub is_watched: bool,
    pub local_path: Option<PathBuf>,
    pub remote_path: String,
    pub seasons: Vec<MediaSeason>,
}

impl MediaItem {
    pub fn has_seasons(&self) -> bool {
        !self.seasons.is_empty()
    }

    pub fn selected_seasons_count(&self) -> usize {
        self.seasons.iter().filter(|s| s.is_selected).count()
    }

    pub fn selected_bytes(&self, is_item_selected: bool) -> u64 {
        if self.has_seasons() {
            self.seasons
                .iter()
                .filter(|s| s.is_selected)
                .map(|s| s.size_bytes)
                .sum()
        } else if is_item_selected {
            self.size_bytes
        } else {
            0
        }
    }

    pub fn are_all_seasons_selected(&self) -> bool {
        self.has_seasons() && self.seasons.iter().all(|s| s.is_selected)
    }

    pub fn are_some_seasons_selected(&self) -> bool {
        self.has_seasons() && self.seasons.iter().any(|s| s.is_selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_category_mappings() {
        assert_eq!(MediaCategory::Movie.as_str(), "Movie");
        assert_eq!(MediaCategory::Movie.remote_folder(), "movie");
        assert_eq!(MediaCategory::Show.remote_folder(), "shows");
        assert_eq!(MediaCategory::Anime.remote_folder(), "anime");
    }

    #[test]
    fn test_media_item_season_selection_helpers() {
        let mut item = MediaItem {
            title: "Test Anime".into(),
            category: MediaCategory::Anime,
            size_bytes: 1000,
            is_watched: false,
            local_path: None,
            remote_path: "media/anime/Test Anime".into(),
            seasons: vec![
                MediaSeason {
                    title: "Season 01".into(),
                    size_bytes: 400,
                    local_path: None,
                    remote_path: "media/anime/Test Anime/Season 01".into(),
                    is_selected: false,
                },
                MediaSeason {
                    title: "Season 02".into(),
                    size_bytes: 600,
                    local_path: None,
                    remote_path: "media/anime/Test Anime/Season 02".into(),
                    is_selected: true,
                },
            ],
        };
        assert!(item.has_seasons());
        assert_eq!(item.selected_seasons_count(), 1);
        assert_eq!(item.selected_bytes(false), 600);
        assert!(!item.are_all_seasons_selected());
        assert!(item.are_some_seasons_selected());

        item.seasons[0].is_selected = true;
        assert!(item.are_all_seasons_selected());
        assert_eq!(item.selected_bytes(false), 1000);
    }
}
