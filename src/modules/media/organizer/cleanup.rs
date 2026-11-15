use std::fs;
use std::path::{Path, PathBuf};

use super::junk::is_skippable_extra_name;
use crate::modules::media::organizer::find_videos_recursive;
use crate::modules::torrent::api::TorrentInfo;

/// Resolves the filesystem source path for a torrent.
pub fn resolve_torrent_source(torrent: &TorrentInfo, default_dl: &Path) -> PathBuf {
    if let Some(content_path) = &torrent.content_path {
        if !content_path.trim().is_empty() {
            let rel = content_path
                .trim_start_matches("/downloads/")
                .trim_start_matches('/');
            let p = default_dl.join(rel);
            if p.exists() {
                return p;
            }
        }
    }

    if let Some(save_path) = &torrent.save_path {
        if !save_path.trim().is_empty() {
            let rel = save_path
                .trim_start_matches("/downloads/")
                .trim_start_matches('/');
            let p = default_dl.join(rel).join(&torrent.name);
            if p.exists() {
                return p;
            }
        }
    }

    default_dl.join(&torrent.name)
}

/// Cleans up leftover directories and extra files from an organized torrent download.
///
/// Once media files (episodes, movies) have been moved to the library, leftover
/// extras (soundtracks, scans, artwork, .nfo) and empty folders remain orphaned
/// on disk. This function removes the source directory if all legitimate media has
/// been organized.
pub fn cleanup_torrent_source(source: &Path, default_dl: &Path) {
    let incomplete = default_dl.join("incomplete");
    if !source.exists()
        || source == default_dl
        || source == incomplete
        || !source.starts_with(default_dl)
    {
        return;
    }

    if source.is_file() {
        let _ = fs::remove_file(source);
        return;
    }

    if !source.is_dir() {
        return;
    }

    if has_unorganized_videos(source) {
        eprintln!(
            "  ⚠️ Keeping leftover torrent dir {}: unorganized video files remain",
            source.display()
        );
        return;
    }

    if let Err(e) = fs::remove_dir_all(source) {
        eprintln!(
            "  ⚠️ Failed to remove leftover torrent dir {}: {e}",
            source.display()
        );
        return;
    }

    prune_empty_parents(source, default_dl);
}

/// Checks whether any non-extra video files remain in the directory.
pub fn has_unorganized_videos(dir: &Path) -> bool {
    let mut videos = Vec::new();
    if find_videos_recursive(dir, &mut videos).is_err() {
        return true;
    }
    videos.into_iter().any(|p| !is_skippable_extra_name(&p))
}

/// Ascends up the directory tree and prunes any empty parent directories
/// up to (but excluding) `default_dl`.
fn prune_empty_parents(source: &Path, default_dl: &Path) {
    let mut curr = source.parent();
    while let Some(parent) = curr {
        if parent == default_dl || !parent.starts_with(default_dl) {
            break;
        }
        if fs::remove_dir(parent).is_err() {
            break;
        }
        curr = parent.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::time::SystemTime;

    fn make_test_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "ryoiki_cleanup_{tag}_{}_{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_cleanup_guards_against_default_dl() {
        let dl = make_test_dir("guard_default");
        cleanup_torrent_source(&dl, &dl);
        assert!(dl.exists(), "default_dl must never be deleted");
        let _ = fs::remove_dir_all(&dl);
    }

    #[test]
    fn test_cleanup_guards_against_incomplete() {
        let dl = make_test_dir("guard_inc");
        let inc = dl.join("incomplete");
        let _ = fs::create_dir(&inc);
        cleanup_torrent_source(&inc, &dl);
        assert!(inc.exists(), "incomplete must never be deleted");
        let _ = fs::remove_dir_all(&dl);
    }

    #[test]
    fn test_cleanup_guards_against_outside_path() {
        let dl = make_test_dir("dl_out");
        let outside = make_test_dir("outside");
        let child = outside.join("external");
        let _ = fs::create_dir(&child);
        cleanup_torrent_source(&child, &dl);
        assert!(child.exists(), "paths outside default_dl must be untouched");
        let _ = fs::remove_dir_all(&dl);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn test_cleanup_removes_leftovers_and_extras() {
        let dl = make_test_dir("leftovers");
        let torrent_dir = dl.join("Heya Camp BD");
        let bonus_dir = torrent_dir.join("Bonus");
        let _ = fs::create_dir_all(&bonus_dir);

        let _ = File::create(torrent_dir.join("soundtrack.flac"));
        let _ = File::create(bonus_dir.join("scan.jpg"));
        let _ = File::create(bonus_dir.join("Heya Camp S00 - OP.mkv"));

        cleanup_torrent_source(&torrent_dir, &dl);
        assert!(
            !torrent_dir.exists(),
            "torrent_dir should be completely removed"
        );
        let _ = fs::remove_dir_all(&dl);
    }

    #[test]
    fn test_cleanup_preserves_dir_with_unorganized_videos() {
        let dl = make_test_dir("preserve");
        let torrent_dir = dl.join("Heya Camp BD");
        let _ = fs::create_dir_all(&torrent_dir);

        let _ = File::create(torrent_dir.join("Heya Camp - S01E01.mkv"));

        cleanup_torrent_source(&torrent_dir, &dl);
        assert!(
            torrent_dir.exists(),
            "torrent_dir must be preserved when real videos remain"
        );
        let _ = fs::remove_dir_all(&dl);
    }

    #[test]
    fn test_prune_empty_parents() {
        let dl = make_test_dir("prune");
        let sub = dl.join("anime").join("subfolder");
        let _ = fs::create_dir_all(&sub);
        let target = sub.join("release");
        let _ = fs::create_dir(&target);

        cleanup_torrent_source(&target, &dl);
        assert!(
            !dl.join("anime").exists(),
            "empty parent directories should be pruned up to dl"
        );
        assert!(dl.exists(), "default_dl must remain");
        let _ = fs::remove_dir_all(&dl);
    }
}
