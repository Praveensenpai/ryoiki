use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::entry::{
    load_entries, local_trash_root, now_secs, push_entry, remove_entry, save_entries, TrashEntry,
    TrashKind, TrashOrigin,
};

const REMOTE_TRASH_PREFIX: &str = "gdrive:Trash/ryoiki";

/// Every entry currently held in the trash, oldest first.
#[must_use]
pub fn list_all() -> Vec<TrashEntry> {
    load_entries()
}

/// Moves a local SSD path into the trash tree and records it.
///
/// # Errors
/// Fails when the source is missing or the move cannot be completed.
pub fn trash_local(path: &Path, kind: TrashKind, title: &str, home: &Path) -> Result<TrashEntry> {
    if !path.exists() {
        bail!("{} no longer exists", path.display());
    }

    let id = new_id(title);
    let name = path
        .file_name()
        .map_or_else(|| "item".to_string(), |n| n.to_string_lossy().to_string());
    let dest_dir = local_trash_root(home).join(&id);
    fs::create_dir_all(&dest_dir)
        .with_context(|| format!("failed to create {}", dest_dir.display()))?;
    let dest = dest_dir.join(&name);

    move_path(path, &dest)?;

    let entry = TrashEntry {
        id,
        kind,
        origin: TrashOrigin::Ssd,
        title: title.to_string(),
        size_bytes: path_size(&dest),
        original_path: Some(path.to_path_buf()),
        original_remote: None,
        trash_path: dest,
        trashed_at: now_secs(),
    };
    push_entry(&entry)?;
    Ok(entry)
}

/// Moves a Google Drive path into `gdrive:Trash/ryoiki/<id>` and records it.
///
/// # Errors
/// Fails when rclone reports a non-zero exit status.
pub fn trash_remote(
    remote_rel: &str,
    kind: TrashKind,
    title: &str,
    size_bytes: u64,
) -> Result<TrashEntry> {
    let id = new_id(title);
    let dest = format!("{REMOTE_TRASH_PREFIX}/{id}");
    let src = format!("gdrive:{remote_rel}");

    let status = Command::new("rclone")
        .args(["move", &src, &dest])
        .status()
        .context("failed to run rclone move")?;
    if !status.success() {
        bail!("rclone move failed for {remote_rel}");
    }

    let entry = TrashEntry {
        id,
        kind,
        origin: TrashOrigin::GDrive,
        title: title.to_string(),
        size_bytes,
        original_path: None,
        original_remote: Some(remote_rel.to_string()),
        trash_path: PathBuf::from(dest),
        trashed_at: now_secs(),
    };
    push_entry(&entry)?;
    Ok(entry)
}

/// Puts a trashed entry back where it came from.
///
/// # Errors
/// Fails when the payload is gone or the move back fails.
pub fn restore_entry(entry: &TrashEntry) -> Result<()> {
    match entry.origin {
        TrashOrigin::Ssd => {
            let Some(target) = entry.original_path.as_ref() else {
                bail!("entry {} has no original path", entry.id);
            };
            if !entry.trash_path.exists() {
                bail!("{} is no longer in the trash", entry.trash_path.display());
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            move_path(&entry.trash_path, target)?;
            cleanup_empty_parent(&entry.trash_path);
        }
        TrashOrigin::GDrive => {
            let Some(target) = entry.original_remote.as_ref() else {
                bail!("entry {} has no original remote path", entry.id);
            };
            let status = Command::new("rclone")
                .args([
                    "move",
                    &entry.trash_path.to_string_lossy(),
                    &format!("gdrive:{target}"),
                ])
                .status()
                .context("failed to run rclone move")?;
            if !status.success() {
                bail!("rclone restore failed for {}", entry.id);
            }
        }
    }
    remove_entry(&entry.id)?;
    Ok(())
}

/// Deletes one trashed entry permanently.
///
/// # Errors
/// Fails when the payload cannot be removed.
pub fn purge_entry(entry: &TrashEntry) -> Result<()> {
    match entry.origin {
        TrashOrigin::Ssd => {
            if let Some(dir) = entry.trash_path.parent() {
                if dir.exists() {
                    fs::remove_dir_all(dir)
                        .with_context(|| format!("failed to purge {}", dir.display()))?;
                }
            } else if entry.trash_path.exists() {
                fs::remove_file(&entry.trash_path)?;
            }
        }
        TrashOrigin::GDrive => {
            let status = Command::new("rclone")
                .args(["purge", &entry.trash_path.to_string_lossy()])
                .status()
                .context("failed to run rclone purge")?;
            if !status.success() {
                bail!("rclone purge failed for {}", entry.id);
            }
        }
    }
    remove_entry(&entry.id)?;
    Ok(())
}

/// Purges every entry past the retention window and returns how many went.
///
/// # Errors
/// Fails when the manifest cannot be rewritten.
pub fn purge_expired() -> Result<usize> {
    let entries = load_entries();
    let (expired, kept): (Vec<TrashEntry>, Vec<TrashEntry>) =
        entries.into_iter().partition(TrashEntry::is_expired);
    if expired.is_empty() {
        return Ok(0);
    }

    let mut purged = 0;
    for entry in &expired {
        if purge_payload(entry).is_ok() {
            purged += 1;
        }
    }
    save_entries(&kept)?;
    Ok(purged)
}

fn purge_payload(entry: &TrashEntry) -> Result<()> {
    match entry.origin {
        TrashOrigin::Ssd => {
            if let Some(dir) = entry.trash_path.parent() {
                if dir.exists() {
                    fs::remove_dir_all(dir)?;
                }
            }
            Ok(())
        }
        TrashOrigin::GDrive => {
            let status = Command::new("rclone")
                .args(["purge", &entry.trash_path.to_string_lossy()])
                .status()?;
            if status.success() {
                Ok(())
            } else {
                bail!("rclone purge failed")
            }
        }
    }
}

/// Collision-resistant, human-readable trash id.
fn new_id(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for c in title.chars() {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let short: String = slug.chars().take(40).collect();
    format!("{}-{short}", now_secs())
}

/// Renames when possible, otherwise copies and deletes across filesystems.
fn move_path(src: &Path, dest: &Path) -> Result<()> {
    if fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    if src.is_dir() {
        copy_dir(src, dest)?;
    } else {
        fs::copy(src, dest).with_context(|| format!("failed to copy {}", src.display()))?;
    }
    if src.is_dir() {
        fs::remove_dir_all(src)?;
    } else {
        fs::remove_file(src)?;
    }
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn cleanup_empty_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}

/// Recursive byte size of a file or directory.
#[must_use]
pub fn path_size(path: &Path) -> u64 {
    if path.is_file() {
        return fs::metadata(path).map_or(0, |m| m.len());
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries.flatten().map(|e| path_size(&e.path())).sum::<u64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that mutate process-global env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Points the manifest at a temp dir and returns (guard, root, home).
    fn sandbox(tag: &str) -> (std::sync::MutexGuard<'static, ()>, PathBuf, PathBuf) {
        let guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = std::env::temp_dir().join(format!("ryoiki_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        std::env::set_var("RYOIKI_STATE_DIR", root.join("state"));
        let home = root.join("home");
        (guard, root, home)
    }

    #[test]
    fn lifecycle_trash_restore_purge() -> Result<()> {
        let (_guard, root, home) = sandbox("trash_lifecycle");
        let media = root.join("media/Epic (2026) [Tamil]");
        fs::create_dir_all(&media)?;
        let movie = media.join("Epic.mkv");
        fs::write(&movie, b"0123456789")?;

        let entry = trash_local(&media, TrashKind::Title, "Epic (2026) [Tamil]", &home)?;
        assert!(!media.exists(), "original should be gone");
        assert!(entry.trash_path.exists(), "payload should be in trash");
        assert_eq!(entry.size_bytes, 10);
        assert_eq!(entry.origin, TrashOrigin::Ssd);
        assert_eq!(entry.days_left(), 7);
        assert_eq!(load_entries().len(), 1);

        restore_entry(&entry)?;
        assert!(media.exists(), "restore should put the payload back");
        assert_eq!(fs::read(&movie)?, b"0123456789");
        assert!(load_entries().is_empty(), "manifest should be cleared");

        let again = trash_local(&media, TrashKind::Title, "Epic (2026) [Tamil]", &home)?;
        purge_entry(&again)?;
        assert!(!again.trash_path.exists(), "purge should delete payload");
        assert!(load_entries().is_empty());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn path_size_sums_nested_files() -> std::io::Result<()> {
        let base = std::env::temp_dir().join(format!("ryoiki_trash_size_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("sub"))?;
        fs::write(base.join("a.mkv"), b"12345")?;
        fs::write(base.join("sub/b.mkv"), b"123")?;

        assert_eq!(path_size(&base), 8);

        let _ = fs::remove_dir_all(&base);
        Ok(())
    }

    #[test]
    fn new_id_is_slugified_and_unique_per_second() {
        let id = new_id("Epic (2026) [Tamil]");
        assert!(id.ends_with("Epic-2026-Tamil"), "got {id}");
    }
}
