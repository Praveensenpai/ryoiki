//! ffmpeg remux that reduces a `[Multi]` sidecar to a single chosen audio track.
//!
//! The `[Multi]` file is never modified; the single-language copy is written
//! alongside it so the full multi-track version stays available.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Remuxes the `[Multi]` sidecar down to a single chosen audio language.
///
/// Writes the single-language file and leaves the `[Multi]` intact.
pub fn apply_language(multi_path: &Path, language: &str) -> Result<PathBuf> {
    let index = audio_stream_index(multi_path, language).with_context(|| {
        format!(
            "No audio track tagged '{language}' in {}",
            multi_path.display()
        )
    })?;

    let parent = multi_path
        .parent()
        .context("multi file has no parent dir")?;
    let multi_name = multi_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("multi file has no name")?;
    let target_name =
        crate::modules::media::ai::ensure_language_in_clean_name(multi_name, language);
    let target = parent.join(&target_name);

    // Keep the real extension on the temp path so ffmpeg can infer the muxer;
    // a `.remux_tmp` suffix alone leaves it unable to choose an output format.
    let ext = target.extension().and_then(|e| e.to_str()).unwrap_or("mkv");
    let temp = parent.join(format!(".{target_name}.remux_tmp.{ext}"));

    run_ffmpeg(multi_path, index, &temp)?;

    if target.exists() {
        let _ = std::fs::remove_file(&target);
    }
    std::fs::rename(&temp, &target).context("Failed to move remuxed file into place")?;
    Ok(target)
}

/// Runs the copy remux into a temporary path, cleaning up on failure.
fn run_ffmpeg(input: &Path, index: u32, temp: &Path) -> Result<()> {
    let status = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-i"])
        .arg(input)
        .args(["-map", "0:v", "-map"])
        .arg(format!("0:{index}"))
        .args(["-map", "0:s?", "-c", "copy"])
        .arg(temp)
        .status()
        .context("Failed to spawn ffmpeg")?;

    if !status.success() {
        let _ = std::fs::remove_file(temp);
        anyhow::bail!("ffmpeg remux failed for {}", input.display());
    }
    Ok(())
}

/// Finds the global ffmpeg stream index of the audio track matching `language`.
fn audio_stream_index(path: &Path, language: &str) -> Option<u32> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=index:stream_tags=language",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    parsed["streams"].as_array()?.iter().find_map(|s| {
        let tag = s["tags"]["language"].as_str().unwrap_or("");
        if crate::modules::media::probe::map_language_code(tag).eq_ignore_ascii_case(language) {
            u32::try_from(s["index"].as_u64()?).ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_dir(tag: &str) -> PathBuf {
        let base = if Path::new("/tmp/opencode").is_dir() {
            PathBuf::from("/tmp/opencode")
        } else {
            std::env::temp_dir()
        };
        let dir = base.join(format!("ryoiki-remux-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Builds a two-audio-track MKV tagged Tamil (tam) and Telugu (tel).
    ///
    /// Returns `None` when ffmpeg is unavailable or the encode fails.
    fn make_two_track_fixture(dir: &Path) -> Option<PathBuf> {
        let path = dir.join("Show S01E01 [Multi].mkv");
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=128x72:rate=5",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:duration=1",
                "-map",
                "0:v",
                "-map",
                "1:a",
                "-map",
                "2:a",
                "-metadata:s:a:0",
                "language=tam",
                "-metadata:s:a:1",
                "language=tel",
                "-c:v",
                "mpeg4",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&path)
            .status()
            .ok()?;
        status.success().then_some(path)
    }

    #[test]
    fn test_apply_language_remuxes_live() {
        let dir = work_dir("live");
        let Some(multi) = make_two_track_fixture(&dir) else {
            return; // ffmpeg unavailable in this environment; skip
        };

        let before = crate::modules::media::probe::probe_media_file(&multi);
        assert_eq!(before.map(|p| p.audio_stream_count), Some(2));

        let out = apply_language(&multi, "Telugu").unwrap_or_else(|e| panic!("remux failed: {e}"));
        assert!(out.exists(), "remuxed output must exist");
        assert!(multi.exists(), "[Multi] sidecar must be preserved");
        assert_eq!(
            out.file_name().and_then(|n| n.to_str()),
            Some("Show S01E01 [Telugu].mkv")
        );

        let after = crate::modules::media::probe::probe_media_file(&out);
        assert_eq!(after.map(|p| p.audio_stream_count), Some(1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_apply_language_missing_track_errors() {
        let dir = work_dir("missing");
        let Some(multi) = make_two_track_fixture(&dir) else {
            return;
        };
        assert!(apply_language(&multi, "Korean").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
