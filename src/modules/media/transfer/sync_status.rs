use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{MediaCategory, MediaItem, MediaSeason};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaFile {
    pub name: String,
    pub rel_path: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub exists_in_other: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SyncStatus {
    pub total_files: usize,
    pub other_files: usize,
    pub total_bytes: u64,
    pub missing_bytes: u64,
}

impl SyncStatus {
    pub fn is_all_in_other(&self) -> bool {
        self.total_files > 0 && self.other_files == self.total_files
    }

    pub fn is_partial(&self) -> bool {
        self.other_files > 0 && self.other_files < self.total_files
    }

    pub fn from_files(files: &[MediaFile]) -> Self {
        let total_files = files.len();
        let other_files = files.iter().filter(|f| f.exists_in_other).count();
        let total_bytes = files.iter().map(|f| f.size_bytes).sum();
        let missing_bytes = files
            .iter()
            .filter(|f| !f.exists_in_other)
            .map(|f| f.size_bytes)
            .sum();

        Self {
            total_files,
            other_files,
            total_bytes,
            missing_bytes,
        }
    }

    pub fn combine(statuses: impl IntoIterator<Item = SyncStatus>) -> Self {
        let mut combined = Self::default();
        for s in statuses {
            combined.total_files = combined.total_files.saturating_add(s.total_files);
            combined.other_files = combined.other_files.saturating_add(s.other_files);
            combined.total_bytes = combined.total_bytes.saturating_add(s.total_bytes);
            combined.missing_bytes = combined.missing_bytes.saturating_add(s.missing_bytes);
        }
        combined
    }
}

pub fn cross_reference_libraries(local: &mut [MediaItem], remote: &mut [MediaItem]) {
    let mut remote_lookup: HashMap<(MediaCategory, String), usize> = HashMap::new();
    for (idx, item) in remote.iter().enumerate() {
        let key = (item.category, item.title.to_lowercase());
        remote_lookup.insert(key, idx);
    }

    for local_item in local.iter_mut() {
        let key = (local_item.category, local_item.title.to_lowercase());
        if let Some(&rem_idx) = remote_lookup.get(&key) {
            let remote_item = &mut remote[rem_idx];
            reconcile_items(local_item, remote_item);
        } else {
            finalize_unmatched_item(local_item);
        }
    }

    for remote_item in remote.iter_mut() {
        if remote_item.sync_status.total_files == 0
            && (!remote_item.files.is_empty() || remote_item.has_seasons())
        {
            finalize_unmatched_item(remote_item);
        }
    }
}

fn reconcile_items(local_item: &mut MediaItem, remote_item: &mut MediaItem) {
    if local_item.has_seasons() || remote_item.has_seasons() {
        reconcile_seasons(local_item, remote_item);
    } else {
        reconcile_file_lists(&mut local_item.files, &mut remote_item.files);
        local_item.sync_status = SyncStatus::from_files(&local_item.files);
        remote_item.sync_status = SyncStatus::from_files(&remote_item.files);
    }
}

fn reconcile_seasons(local_item: &mut MediaItem, remote_item: &mut MediaItem) {
    let mut remote_seasons: HashMap<String, usize> = HashMap::new();
    for (idx, s) in remote_item.seasons.iter().enumerate() {
        remote_seasons.insert(s.title.to_lowercase(), idx);
    }

    for local_season in &mut local_item.seasons {
        let key = local_season.title.to_lowercase();
        if let Some(&rs_idx) = remote_seasons.get(&key) {
            let remote_season = &mut remote_item.seasons[rs_idx];
            reconcile_file_lists(&mut local_season.files, &mut remote_season.files);
            local_season.sync_status = SyncStatus::from_files(&local_season.files);
            remote_season.sync_status = SyncStatus::from_files(&remote_season.files);
        } else {
            finalize_unmatched_season(local_season);
        }
    }

    for remote_season in &mut remote_item.seasons {
        if remote_season.sync_status.total_files == 0 && !remote_season.files.is_empty() {
            finalize_unmatched_season(remote_season);
        }
    }

    local_item.sync_status = SyncStatus::combine(local_item.seasons.iter().map(|s| s.sync_status));
    remote_item.sync_status =
        SyncStatus::combine(remote_item.seasons.iter().map(|s| s.sync_status));
}

fn reconcile_file_lists(local_files: &mut [MediaFile], remote_files: &mut [MediaFile]) {
    let mut remote_map: HashMap<(String, u64), usize> = HashMap::new();
    for (idx, f) in remote_files.iter().enumerate() {
        remote_map.insert((f.name.to_lowercase(), f.size_bytes), idx);
    }

    for local_file in local_files.iter_mut() {
        let key = (local_file.name.to_lowercase(), local_file.size_bytes);
        if let Some(&r_idx) = remote_map.get(&key) {
            local_file.exists_in_other = true;
            remote_files[r_idx].exists_in_other = true;
        }
    }
}

fn finalize_unmatched_item(item: &mut MediaItem) {
    if item.has_seasons() {
        for season in &mut item.seasons {
            finalize_unmatched_season(season);
        }
        item.sync_status = SyncStatus::combine(item.seasons.iter().map(|s| s.sync_status));
    } else {
        item.sync_status = SyncStatus::from_files(&item.files);
    }
}

fn finalize_unmatched_season(season: &mut MediaSeason) {
    season.sync_status = SyncStatus::from_files(&season.files);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_status_from_files() {
        let files = vec![
            MediaFile {
                name: "ep1.mkv".into(),
                rel_path: "ep1.mkv".into(),
                size_bytes: 100,
                exists_in_other: true,
            },
            MediaFile {
                name: "ep2.mkv".into(),
                rel_path: "ep2.mkv".into(),
                size_bytes: 200,
                exists_in_other: false,
            },
        ];
        let status = SyncStatus::from_files(&files);
        assert_eq!(status.total_files, 2);
        assert_eq!(status.other_files, 1);
        assert_eq!(status.total_bytes, 300);
        assert_eq!(status.missing_bytes, 200);
        assert!(status.is_partial());
        assert!(!status.is_all_in_other());
    }

    #[test]
    fn test_cross_reference_reconciliation() {
        let mut local = vec![MediaItem {
            title: "Movie A".into(),
            category: MediaCategory::Movie,
            size_bytes: 500,
            is_watched: false,
            local_path: None,
            remote_path: "media/movie/Movie A".into(),
            seasons: Vec::new(),
            files: vec![MediaFile {
                name: "Movie A.mkv".into(),
                rel_path: "Movie A.mkv".into(),
                size_bytes: 500,
                exists_in_other: false,
            }],
            sync_status: SyncStatus::default(),
        }];

        let mut remote = vec![MediaItem {
            title: "Movie A".into(),
            category: MediaCategory::Movie,
            size_bytes: 500,
            is_watched: false,
            local_path: None,
            remote_path: "media/movie/Movie A".into(),
            seasons: Vec::new(),
            files: vec![MediaFile {
                name: "Movie A.mkv".into(),
                rel_path: "Movie A.mkv".into(),
                size_bytes: 500,
                exists_in_other: false,
            }],
            sync_status: SyncStatus::default(),
        }];

        cross_reference_libraries(&mut local, &mut remote);
        assert!(local[0].sync_status.is_all_in_other());
        assert!(remote[0].sync_status.is_all_in_other());
    }

    #[test]
    fn test_sync_status_combine() {
        let s1 = SyncStatus {
            total_files: 2,
            other_files: 2,
            total_bytes: 400,
            missing_bytes: 0,
        };
        let s2 = SyncStatus {
            total_files: 2,
            other_files: 1,
            total_bytes: 600,
            missing_bytes: 300,
        };
        let combined = SyncStatus::combine([s1, s2]);
        assert_eq!(combined.total_files, 4);
        assert_eq!(combined.other_files, 3);
        assert_eq!(combined.total_bytes, 1000);
        assert_eq!(combined.missing_bytes, 300);
        assert!(combined.is_partial());
    }

    #[test]
    fn test_cross_reference_partial_seasons() {
        let mut local = vec![MediaItem {
            title: "Anime Show".into(),
            category: MediaCategory::Anime,
            size_bytes: 600,
            is_watched: false,
            local_path: None,
            remote_path: "media/anime/Anime Show".into(),
            seasons: vec![MediaSeason {
                title: "Season 01".into(),
                size_bytes: 600,
                local_path: None,
                remote_path: "media/anime/Anime Show/Season 01".into(),
                is_selected: false,
                files: vec![
                    MediaFile {
                        name: "ep1.mkv".into(),
                        rel_path: "Season 01/ep1.mkv".into(),
                        size_bytes: 300,
                        exists_in_other: false,
                    },
                    MediaFile {
                        name: "ep2.mkv".into(),
                        rel_path: "Season 01/ep2.mkv".into(),
                        size_bytes: 300,
                        exists_in_other: false,
                    },
                ],
                sync_status: SyncStatus::default(),
            }],
            files: Vec::new(),
            sync_status: SyncStatus::default(),
        }];

        let mut remote = vec![MediaItem {
            title: "Anime Show".into(),
            category: MediaCategory::Anime,
            size_bytes: 900,
            is_watched: false,
            local_path: None,
            remote_path: "media/anime/Anime Show".into(),
            seasons: vec![
                MediaSeason {
                    title: "Season 01".into(),
                    size_bytes: 600,
                    local_path: None,
                    remote_path: "media/anime/Anime Show/Season 01".into(),
                    is_selected: false,
                    files: vec![
                        MediaFile {
                            name: "ep1.mkv".into(),
                            rel_path: "Season 01/ep1.mkv".into(),
                            size_bytes: 300,
                            exists_in_other: false,
                        },
                        MediaFile {
                            name: "ep3.mkv".into(),
                            rel_path: "Season 01/ep3.mkv".into(),
                            size_bytes: 300,
                            exists_in_other: false,
                        },
                    ],
                    sync_status: SyncStatus::default(),
                },
                MediaSeason {
                    title: "Season 02".into(),
                    size_bytes: 300,
                    local_path: None,
                    remote_path: "media/anime/Anime Show/Season 02".into(),
                    is_selected: false,
                    files: vec![MediaFile {
                        name: "ep1.mkv".into(),
                        rel_path: "Season 02/ep1.mkv".into(),
                        size_bytes: 300,
                        exists_in_other: false,
                    }],
                    sync_status: SyncStatus::default(),
                },
            ],
            files: Vec::new(),
            sync_status: SyncStatus::default(),
        }];

        cross_reference_libraries(&mut local, &mut remote);
        // local season 1 has ep1 in other, ep2 missing in other
        assert!(local[0].seasons[0].sync_status.is_partial());
        assert_eq!(local[0].seasons[0].sync_status.other_files, 1);
        assert_eq!(local[0].seasons[0].sync_status.missing_bytes, 300);

        // remote season 1 has ep1 in other, ep3 missing
        assert!(remote[0].seasons[0].sync_status.is_partial());
        assert_eq!(remote[0].seasons[0].sync_status.missing_bytes, 300);

        // remote season 2 is completely absent locally
        assert_eq!(remote[0].seasons[1].sync_status.other_files, 0);
        assert_eq!(remote[0].seasons[1].sync_status.missing_bytes, 300);
    }
}
