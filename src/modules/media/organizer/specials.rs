use std::path::PathBuf;

use super::super::probe::MediaProbe;
use super::super::MediaInfo;

/// Assigns unique season-0 episode numbers across a classified series group.
///
/// A large series is split into several AI chunks, and the model restarts its
/// special numbering in each chunk. That produces duplicate `S00EXX` numbers —
/// the descriptive names stay unique, but Jellyfin keys on the episode number.
/// Renumber specials deterministically so every file in the group is distinct.
pub fn assign_unique_special_numbers(classified: &mut [(PathBuf, MediaInfo, Option<MediaProbe>)]) {
    let mut specials: Vec<usize> = classified
        .iter()
        .enumerate()
        .filter(|(_, (_, info, _))| info.season == Some(0))
        .map(|(i, _)| i)
        .collect();

    if specials.len() < 2 {
        return;
    }

    // Preserve the model's intended grouping, then fall back to filename order.
    specials.sort_by_cached_key(|&i| {
        let (path, info, _) = &classified[i];
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        (info.episode.unwrap_or(u32::MAX), name)
    });

    for (offset, &idx) in specials.iter().enumerate() {
        let Ok(new_ep) = u32::try_from(offset + 1) else {
            break;
        };
        let (_, info, _) = &mut classified[idx];
        if info.episode != Some(new_ep) {
            info.clean_name = replace_episode_number(&info.clean_name, 0, new_ep);
            info.episode = Some(new_ep);
        }
    }
}

/// Rewrites the `S{season:02}EXX` token in a clean name to `new_ep`.
fn replace_episode_number(name: &str, season: u32, new_ep: u32) -> String {
    let marker = format!("S{season:02}E");
    let Some(pos) = name.find(&marker) else {
        return name.to_string();
    };
    let digits_start = pos + marker.len();
    let digits_end = name[digits_start..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(name.len(), |off| digits_start + off);
    format!(
        "{}{new_ep:02}{}",
        &name[..digits_start],
        &name[digits_end..]
    )
}

#[cfg(test)]
mod tests {
    use super::assign_unique_special_numbers;
    use crate::modules::media::ClassificationEngine;
    use crate::modules::media::MediaInfo;
    use crate::modules::media::MediaType;
    use std::path::PathBuf;

    fn special_info(episode: u32) -> MediaInfo {
        MediaInfo {
            media_type: MediaType::Anime,
            title: "Show".to_string(),
            year: None,
            season: Some(0),
            episode: Some(episode),
            resolution: Some("1080p".to_string()),
            language: Some("Japanese".to_string()),
            clean_name: format!("Show - S00E{episode:02} - Extra [Japanese] [1080p].mkv"),
            is_extra: true,
            engine: ClassificationEngine::Ai,
        }
    }

    #[test]
    fn test_assign_unique_special_numbers_dedupes_chunk_restarts() {
        // Two chunks each numbered their specials from E01, so E04..E06 repeat.
        let mut classified = vec![
            (PathBuf::from("/t/Show/EXTRA/a.mkv"), special_info(1), None),
            (PathBuf::from("/t/Show/EXTRA/b.mkv"), special_info(4), None),
            (PathBuf::from("/t/Show/EXTRA/c.mkv"), special_info(4), None),
            (PathBuf::from("/t/Show/EXTRA/d.mkv"), special_info(6), None),
            (PathBuf::from("/t/Show/EXTRA/e.mkv"), special_info(6), None),
        ];

        assign_unique_special_numbers(&mut classified);

        let mut episodes: Vec<u32> = classified
            .iter()
            .filter_map(|(_, info, _)| info.episode)
            .collect();
        episodes.sort_unstable();
        assert_eq!(episodes, vec![1, 2, 3, 4, 5], "specials must be unique");

        for (_, info, _) in &classified {
            let Some(ep) = info.episode else {
                panic!("episode must be assigned");
            };
            assert!(
                info.clean_name.contains(&format!("S00E{ep:02}")),
                "clean_name {} must match episode {ep}",
                info.clean_name
            );
        }
    }

    #[test]
    fn test_assign_unique_special_numbers_keeps_regular_episodes() {
        let mut regular = special_info(3);
        regular.season = Some(1);
        regular.episode = Some(7);
        regular.clean_name = "Show - S01E07 [Japanese] [1080p].mkv".to_string();

        let mut classified = vec![
            (PathBuf::from("/t/Show/01.mkv"), regular, None),
            (PathBuf::from("/t/Show/EXTRA/a.mkv"), special_info(5), None),
        ];

        assign_unique_special_numbers(&mut classified);

        assert_eq!(
            classified[0].1.episode,
            Some(7),
            "regular episode untouched"
        );
        assert_eq!(
            classified[0].1.clean_name, "Show - S01E07 [Japanese] [1080p].mkv",
            "regular clean_name untouched"
        );
        assert_eq!(
            classified[1].1.episode,
            Some(5),
            "a lone special is left as classified"
        );
    }
}
