use crate::modules::media::probe::normalize_resolution;
use crate::modules::media::MediaInfo;

/// Collapses the classifier's raw resolution strings into one canonical tag.
///
/// The model returns `1024x768` and `768p` interchangeably for the same encode,
/// so the same release ends up with two different labels. Normalizing here keeps
/// every file in a series consistent and matches the probe's thresholds.
pub fn apply_canonical_resolution(info: &mut MediaInfo) {
    let Some(canonical) = info
        .resolution
        .as_deref()
        .map(normalize_resolution)
        .filter(|res| !res.is_empty())
    else {
        return;
    };
    info.clean_name =
        super::super::ai::ensure_resolution_in_clean_name(&info.clean_name, &canonical);
    info.resolution = Some(canonical);
}

#[cfg(test)]
mod tests {
    use super::apply_canonical_resolution;
    use crate::modules::media::{ClassificationEngine, MediaInfo, MediaType};

    fn info(resolution: Option<&str>, clean_name: &str) -> MediaInfo {
        MediaInfo {
            media_type: MediaType::Anime,
            title: "Show".to_string(),
            year: None,
            season: Some(1),
            episode: Some(1),
            resolution: resolution.map(str::to_string),
            language: Some("Japanese".to_string()),
            clean_name: clean_name.to_string(),
            is_extra: false,
            engine: ClassificationEngine::Ai,
        }
    }

    #[test]
    fn test_apply_canonical_resolution_collapses_dimensions() {
        let mut item = info(Some("1024x768"), "Show - S01E01 [Japanese] [1024x768].mkv");
        apply_canonical_resolution(&mut item);
        assert_eq!(item.resolution.as_deref(), Some("720p"));
        assert_eq!(item.clean_name, "Show - S01E01 [Japanese] [720p].mkv");
    }

    #[test]
    fn test_apply_canonical_resolution_collapses_p_label() {
        let mut item = info(Some("768p"), "Show - S01E01 [Japanese] [768p].mkv");
        apply_canonical_resolution(&mut item);
        assert_eq!(item.resolution.as_deref(), Some("720p"));
        assert_eq!(item.clean_name, "Show - S01E01 [Japanese] [720p].mkv");
    }

    #[test]
    fn test_apply_canonical_resolution_is_noop_without_resolution() {
        let mut item = info(None, "Show - S01E01 [Japanese].mkv");
        apply_canonical_resolution(&mut item);
        assert_eq!(item.resolution, None);
        assert_eq!(item.clean_name, "Show - S01E01 [Japanese].mkv");
    }
}
