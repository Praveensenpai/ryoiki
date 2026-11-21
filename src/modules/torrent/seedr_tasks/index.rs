//! Live Seedr account index and name normalization.

use std::collections::HashSet;

use super::SeedrCloudList;

pub(crate) fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Normalized comparison key: lowercased, trimmed, without a `folder-` prefix.
pub(crate) fn base_key(name: &str) -> String {
    let n = normalize(name);
    n.strip_prefix("folder-").unwrap_or(&n).to_string()
}

/// Set of names, titles, and hashes currently present in the Seedr cloud account.
pub(crate) struct LiveIndex {
    names: HashSet<String>,
    titles: HashSet<String>,
    hashes: HashSet<String>,
}

impl LiveIndex {
    pub(crate) fn from_list(list: &SeedrCloudList) -> Self {
        let mut names = HashSet::new();
        let mut titles = HashSet::new();
        let mut hashes = HashSet::new();
        for t in &list.torrents {
            let n = normalize(&t.name);
            names.insert(n);
            let title = normalize(
                &crate::modules::media::heuristic::classify_media_heuristic(&t.name).title,
            );
            if !title.is_empty() {
                titles.insert(title);
            }
            if let Some(ref h) = t.hash {
                hashes.insert(normalize(h));
            }
        }
        for f in &list.folders {
            let n = normalize(&f.name);
            names.insert(n);
            let title = normalize(
                &crate::modules::media::heuristic::classify_media_heuristic(&f.name).title,
            );
            if !title.is_empty() {
                titles.insert(title);
            }
        }
        for f in &list.files {
            let n = normalize(&f.name);
            names.insert(n);
            let title = normalize(
                &crate::modules::media::heuristic::classify_media_heuristic(&f.name).title,
            );
            if !title.is_empty() {
                titles.insert(title);
            }
        }
        Self {
            names,
            titles,
            hashes,
        }
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        let target = normalize(name);
        if self.names.contains(&target) || self.hashes.contains(&target) {
            return true;
        }
        if let Some(stripped) = target.strip_prefix("folder-") {
            if self.names.contains(stripped) {
                return true;
            }
        }
        for n in &self.names {
            let n_clean = n.strip_prefix("folder-").unwrap_or(n);
            let t_clean = target.strip_prefix("folder-").unwrap_or(&target);
            if n_clean == t_clean || n_clean.contains(t_clean) || t_clean.contains(n_clean) {
                return true;
            }
        }
        let target_title =
            normalize(&crate::modules::media::heuristic::classify_media_heuristic(name).title);
        if !target_title.is_empty() && target_title.len() >= 3 {
            if self.titles.contains(&target_title) {
                return true;
            }
            if self.names.iter().any(|n| n.contains(&target_title)) {
                return true;
            }
        }
        false
    }
}
