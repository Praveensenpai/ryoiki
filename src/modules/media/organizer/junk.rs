use crate::modules::media::{MediaInfo, MediaType};

/// Filename fragments that mark a non-episode anime extra (creditless OP/ED,
/// menus, commercials, bumpers, previews, scans).
const SKIP_MARKERS: [&str; 17] = [
    "ncop",
    "nced",
    "creditless",
    "textless",
    "non-credit",
    "noncredit",
    "tv-cm",
    "tvcm",
    "bd scan",
    "scan",
    "preview",
    "commercial",
    "menu",
    "warning",
    "trailer",
    "opening",
    "ending",
];

/// Junk anime extras that must never be moved into the library.
///
/// Creditless openings/endings, menus, commercials, bumpers, previews and scans
/// have no playback value, so they are left in the source directory. Detection
/// is gated on the classifier's `is_extra` flag and anime media type, so a main
/// episode that happens to contain a marker word can never be skipped.
pub fn is_skippable_extra(info: &MediaInfo, raw_name: &str) -> bool {
    if !info.is_extra || info.media_type != MediaType::Anime {
        return false;
    }
    let lower = raw_name.to_ascii_lowercase();
    if SKIP_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| matches!(token, "op" | "ed"))
}

#[cfg(test)]
mod tests {
    use super::is_skippable_extra;
    use crate::modules::media::{ClassificationEngine, MediaInfo, MediaType};

    fn info(media_type: MediaType, is_extra: bool, raw: &str) -> (MediaInfo, String) {
        (
            MediaInfo {
                media_type,
                title: "Show".to_string(),
                year: None,
                season: Some(0),
                episode: Some(1),
                resolution: Some("1080p".to_string()),
                language: Some("Japanese".to_string()),
                clean_name: format!("Show - S00E01 - {raw}.mkv"),
                is_extra,
                engine: ClassificationEngine::Ai,
            },
            raw.to_string(),
        )
    }

    #[test]
    fn test_is_skippable_extra_drops_creditless_and_bumpers() {
        for raw in [
            "[Moozzi2] Show [SP01] NCOP [Kirikirisu]",
            "[Moozzi2] Show [SP02] NCED - 01",
            "[Moozzi2] Show [SP00] Menu",
            "[Moozzi2] Show [SP03] Warning - 01",
            "[Moozzi2] Show [SP07] TV-CM",
            "Show Creditless Opening",
            "Show BD Scans",
        ] {
            let (i, name) = info(MediaType::Anime, true, raw);
            assert!(is_skippable_extra(&i, &name), "{raw} should be skipped");
        }
    }

    #[test]
    fn test_is_skippable_extra_keeps_real_episodes_and_ovas() {
        let (episode, name) = info(MediaType::Anime, false, "Show - 01");
        assert!(!is_skippable_extra(&episode, &name), "main episode kept");

        let (ova, name) = info(MediaType::Anime, true, "Show OVA - 01");
        assert!(!is_skippable_extra(&ova, &name), "real OVA kept");

        let (movie_extra, name) = info(MediaType::Movie, true, "Show Trailer");
        assert!(
            !is_skippable_extra(&movie_extra, &name),
            "movie extras untouched"
        );

        // Gated on is_extra: a non-extra file with a marker word is never skipped.
        let (not_extra, name) = info(MediaType::Anime, false, "Show NCOP");
        assert!(
            !is_skippable_extra(&not_extra, &name),
            "must require is_extra"
        );
    }
}
