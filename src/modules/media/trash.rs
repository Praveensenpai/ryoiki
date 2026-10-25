//! Reversible media trash with a 7-day retention window.
//!
//! Items are moved to `~/jellyfin/.trash/ssd/<id>` on the local SSD and to
//! `gdrive:Trash/ryoiki/<id>` in the cloud, so nothing is ever hard-deleted
//! on the first action. A manifest in `$XDG_STATE_HOME/ryoiki/media_trash.json`
//! records where each entry came from so it can be restored or purged later.

pub mod entry;
pub mod ops;

pub use entry::{TrashEntry, TrashKind};
pub use ops::{list_all, purge_entry, purge_expired, restore_entry, trash_local, trash_remote};
