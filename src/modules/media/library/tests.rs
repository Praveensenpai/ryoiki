use super::*;

fn temp_base(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ryoiki_library_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn render_all(report: &LibraryReport) -> String {
    report.render_pages().join("\n")
}

#[test]
fn test_scan_lists_titles_and_flags_empty() {
    let base = temp_base("scan");
    let movie = base.join("movies/Epic (2026)");
    let _ = fs::create_dir_all(&movie);
    let _ = fs::write(movie.join("Epic.mkv"), b"x");

    let broken = base.join("movies/Ghost (2024)");
    let _ = fs::create_dir_all(&broken);
    let _ = fs::write(broken.join("readme.txt"), b"x");

    let season = base.join("shows/Some Show/Season 01");
    let _ = fs::create_dir_all(&season);
    let _ = fs::write(season.join("ep1.mp4"), b"x");
    let _ = fs::write(season.join("ep2.mp4"), b"x");

    let report = scan_library(&base);
    let movies = &report.sections[0];
    assert_eq!(movies.titles.len(), 1);
    assert_eq!(movies.video_count(), 1);
    assert_eq!(movies.empty.len(), 1);
    assert!(movies.empty[0].contains("Ghost (2024)"));

    let shows = &report.sections[1];
    assert_eq!(shows.titles.len(), 1);
    assert_eq!(shows.video_count(), 2);

    assert!(!report.sections[2].exists);
    assert_eq!(report.total_titles(), 2);
    assert_eq!(report.total_videos(), 3);

    let html = render_all(&report);
    assert!(html.contains("Media Library"));
    assert!(html.contains("Epic (2026)"));
    assert!(html.contains("Some Show"));
    assert!(html.contains("Ghost (2024)"));
    assert!(html.contains("not found"));

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_title_line_shows_languages() {
    let entry = TitleEntry::new(
        "Peddi (2026)".to_string(),
        1,
        vec!["Telugu".to_string(), "Tamil".to_string()],
    );
    let line = entry.render_line();
    assert!(line.contains("Peddi (2026)"));
    assert!(line.contains("Telugu, Tamil"));
    assert!(line.contains("1 file"));

    let unknown = TitleEntry::new("Mystery".to_string(), 2, Vec::new());
    let line = unknown.render_line();
    assert!(line.contains("2 files"));
    assert!(line.contains("audio ?"));
}

#[test]
fn test_languages_from_filenames() {
    let files = vec![
        PathBuf::from("Epic (2026) [Tamil] [1080p].mkv"),
        PathBuf::from("Epic (2026) [Telugu] [1080p].mkv"),
        PathBuf::from("Epic (2026) [Tamil] [720p].mkv"),
    ];
    let languages = languages_from_files(&files);
    assert_eq!(languages, vec!["Tamil".to_string(), "Telugu".to_string()]);

    let no_tags = vec![PathBuf::from("Some Show - S01E01.mp4")];
    assert_eq!(languages_from_files(&no_tags), Vec::<String>::new());

    let resolution_only = vec![PathBuf::from("Movie [1080p] [x264].mkv")];
    assert_eq!(languages_from_files(&resolution_only), Vec::<String>::new());
}

#[test]
fn test_bracket_tags_and_language_detection() {
    assert_eq!(
        bracket_tags("Epic (2026) [Tamil] [1080p].mkv"),
        vec!["Tamil".to_string(), "1080p".to_string()]
    );
    assert_eq!(bracket_tags("no tags here"), Vec::<String>::new());
    assert!(is_language_tag("tamil"));
    assert!(is_language_tag("Japanese"));
    assert!(!is_language_tag("1080p"));
    assert!(!is_language_tag("Multi"));
}

#[test]
fn test_scan_reads_language_from_filenames() {
    let base = temp_base("langs");
    let movie = base.join("movies/Epic (2026)");
    let _ = fs::create_dir_all(&movie);
    let _ = fs::write(movie.join("Epic (2026) [Tamil] [1080p].mkv"), b"x");
    let _ = fs::write(movie.join("Epic (2026) [Telugu] [1080p].mkv"), b"x");

    let report = scan_library(&base);
    let movies = &report.sections[0];
    assert_eq!(movies.titles.len(), 1);
    let entry = &movies.titles[0];
    assert_eq!(
        entry.languages,
        vec!["Tamil".to_string(), "Telugu".to_string()]
    );
    let html = render_all(&report);
    assert!(html.contains("Tamil, Telugu"));

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_paginate_splits_long_lists() {
    let mut lines = vec!["header".to_string()];
    for i in 0..400 {
        lines.push(format!("  • Title {i} — 1 file · Tamil"));
    }
    let pages = paginate(&lines);
    assert!(pages.len() > 1);
    assert!(pages[0].starts_with("header"));
    assert!(pages[1].starts_with(CONTINUED_HEADER));
    for page in &pages {
        assert!(page.chars().count() <= PAGE_BUDGET + CONTINUED_HEADER.chars().count());
    }
    let joined: String = pages.concat();
    assert!(joined.contains("Title 399"));
}

#[test]
fn test_paginate_single_page() {
    let lines = vec!["header".to_string(), "body".to_string()];
    let pages = paginate(&lines);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0], "header\nbody");
}

#[test]
fn test_missing_base_is_graceful() {
    let base = temp_base("missing");
    let report = scan_library(&base);
    assert!(report.sections.iter().all(|s| !s.exists));
    assert_eq!(report.empty_folders(), Vec::<&str>::new());
    assert!(render_all(&report).contains("Media Library"));
    assert_eq!(report.render_pages().len(), 1);
}

#[test]
#[ignore = "reads the real Jellyfin library on this machine"]
fn smoke_real_library() {
    let base =
        std::path::Path::new(&std::env::var("HOME").unwrap_or_default()).join("jellyfin/media");
    let report = scan_library(&base);
    println!("SMOKE_TITLES={}", report.total_titles());
    println!("SMOKE_VIDEOS={}", report.total_videos());
    println!("SMOKE_EMPTY={}", report.empty_folders().len());
    println!("SMOKE_PAGES={}", report.render_pages().len());
    println!("{}", render_all(&report));
}
