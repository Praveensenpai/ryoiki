use super::*;

#[test]
fn test_heuristic_movie_cleaning() {
    let raw = "www.1TamilMV.center - Love Mocktail 3 (2026) Kannada TRUE WEB-DL - 1080p - AVC - (DD_5.1 - 192Kbps _ AAC 2.0) - 2GB - ESub.mkv";
    let info = classify_media_heuristic(raw);
    assert_eq!(
        (
            info.media_type,
            info.title.as_str(),
            info.year,
            info.language.as_deref(),
            info.resolution.as_deref()
        ),
        (
            MediaType::Movie,
            "Love Mocktail 3",
            Some(2026),
            Some("Kannada"),
            Some("1080p")
        )
    );
    assert_eq!(
        info.clean_name,
        "Love Mocktail 3 (2026) [Kannada] [1080p].mkv"
    );
}

#[test]
fn test_heuristic_tamilmv_prefix_with_kannada_language() {
    let raw = "www.1TamilMV.cards - The Rise of Ashoka (2026) Kannada TRUE WEB-DL - 1080p - AVC - (DD_5.1 - 192Kbps _ AAC 2.0) - 2GB - ESub.mkv";
    let info = classify_media_heuristic(raw);
    assert_eq!(
        (
            info.media_type,
            info.title.as_str(),
            info.year,
            info.language.as_deref(),
            info.resolution.as_deref()
        ),
        (
            MediaType::Movie,
            "The Rise of Ashoka",
            Some(2026),
            Some("Kannada"),
            Some("1080p")
        )
    );
    assert_eq!(
        info.clean_name,
        "The Rise of Ashoka (2026) [Kannada] [1080p].mkv"
    );
}

#[test]
fn test_heuristic_show_cleaning() {
    let raw = "House.of.the.Dragon.S02E04.1080p.WEB.H264-SUCCESS.mkv";
    let info = classify_media_heuristic(raw);
    assert_eq!(
        (
            info.media_type,
            info.title.as_str(),
            info.season,
            info.episode
        ),
        (MediaType::Show, "House of the Dragon", Some(2), Some(4))
    );
    assert_eq!(info.clean_name, "House of the Dragon - S02E04 [1080p].mkv");
}

#[test]
fn test_heuristic_anime_cleaning() {
    let raw = "[Moozzi2] Yuru Camp S3 - 01 (BD 1920x1080 x265-10Bit Flac).mkv";
    let info = classify_media_heuristic(raw);
    assert_eq!(
        (
            info.media_type,
            info.title.as_str(),
            info.season,
            info.episode
        ),
        (MediaType::Anime, "Yuru Camp", Some(3), Some(1))
    );
    assert_eq!(info.clean_name, "Yuru Camp - S03E01 [1080p].mkv");
    assert!(!info.is_extra);
}

#[test]
fn test_heuristic_anime_specials() {
    let s2 = classify_media_heuristic(
        "[Moozzi2] Yuru Camp S2 [SP01] NCOP (BD 1920x1080 x265-10Bit Flac).mkv",
    );
    assert_eq!(
        (s2.title.as_str(), s2.season, s2.is_extra),
        ("Yuru Camp", Some(2), true)
    );
    assert_eq!(s2.clean_name, "Yuru Camp - S02 [SP01] NCOP [1080p].mkv");

    let s3 = classify_media_heuristic(
        "[Moozzi2] Yuru Camp S3 [SP01] NCED (BD 1920x1080 x265-10Bit Flac).mkv",
    );
    assert_eq!(
        (s3.title.as_str(), s3.season, s3.is_extra),
        ("Yuru Camp", Some(3), true)
    );
    assert_eq!(s3.clean_name, "Yuru Camp - S03 [SP01] NCED [1080p].mkv");

    let menu = classify_media_heuristic(
        "[Moozzi2] Yuru Camp S3 [SP00] Menu - 01 (BD 1920x1080 x265-10Bit Flac).mkv",
    );
    assert_eq!(
        menu.clean_name,
        "Yuru Camp - S03 [SP00] Menu - 01 [1080p].mkv"
    );
}
