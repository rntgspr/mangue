//! Recently opened or saved files (`recent.yaml` beside `settings.yaml`),
//! newest first, held as the `RecentFiles` global; see
//! `specs/modeler/settings.md`.

use std::path::{Path, PathBuf};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};

/// Most entries kept.
pub const MAX_RECENT: usize = 10;

/// The recent files list, newest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RecentFiles {
    files: Vec<PathBuf>,
}

impl Global for RecentFiles {}

/// Where the list is persisted; absent (tests) means it is not saved.
struct RecentFile(PathBuf);

impl Global for RecentFile {}

impl RecentFiles {
    /// The entries, newest first.
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Moves `path` to the front (deduplicated), dropping the oldest beyond
    /// `MAX_RECENT`.
    pub fn push(&mut self, path: &Path) {
        self.files.retain(|p| p != path);
        self.files.insert(0, path.to_path_buf());
        self.files.truncate(MAX_RECENT);
    }

    /// Drops `path` from the list.
    pub fn remove(&mut self, path: &Path) {
        self.files.retain(|p| p != path);
    }

    /// Empties the list.
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// The list read from `path`; empty when missing or invalid.
    pub fn load_from(path: &Path) -> RecentFiles {
        let yaml = std::fs::read_to_string(path).unwrap_or_default();

        serde_norway::from_str(&yaml).unwrap_or_default()
    }

    /// Writes the list as YAML to `path`, creating its directory.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        }
        let yaml = serde_norway::to_string(self).map_err(|err| err.to_string())?;

        std::fs::write(path, yaml).map_err(|err| err.to_string())
    }
}

/// Loads `recent.yaml` next to the settings file and installs it as globals.
pub fn install(cx: &mut App) {
    let recent = match super::config_path().and_then(|p| Some(p.parent()?.join("recent.yaml"))) {
        Some(path) => {
            let recent = RecentFiles::load_from(&path);
            cx.set_global(RecentFile(path));
            recent
        }
        None => RecentFiles::default(),
    };

    cx.set_global(recent);
}

/// The current list (empty when none was installed).
pub fn current(cx: &App) -> RecentFiles {
    cx.try_global::<RecentFiles>().cloned().unwrap_or_default()
}

/// Applies `change` and persists the list.
fn update(cx: &mut App, change: impl FnOnce(&mut RecentFiles)) {
    let mut recent = current(cx);
    change(&mut recent);

    if let Some(file) = cx.try_global::<RecentFile>()
        && let Err(err) = recent.save_to(&file.0)
    {
        eprintln!("mangue: could not save recent files: {err}");
    }

    cx.set_global(recent);
}

/// Records a successfully opened or saved file, also in the OS recent list.
pub fn record(path: &Path, cx: &mut App) {
    cx.add_recent_document(path);
    update(cx, |recent| recent.push(path));
}

/// Forgets `path` (for instance after it went missing).
pub fn forget(path: &Path, cx: &mut App) {
    update(cx, |recent| recent.remove(path));
}

/// Empties the list.
pub fn clear(cx: &mut App) {
    update(cx, RecentFiles::clear);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pushing deduplicates, orders newest first and caps at `MAX_RECENT`.
    #[test]
    fn push_dedups_orders_and_caps() {
        let mut recent = RecentFiles::default();
        for i in 0..12 {
            recent.push(Path::new(&format!("/f{i}.bpmn")));
        }
        recent.push(Path::new("/f5.bpmn"));

        assert_eq!(recent.files().len(), MAX_RECENT);
        assert_eq!(recent.files()[0], Path::new("/f5.bpmn"));
        assert_eq!(recent.files()[1], Path::new("/f11.bpmn"));
        assert_eq!(
            recent.files().iter().filter(|p| *p == "/f5.bpmn").count(),
            1
        );
    }

    /// Remove and clear drop entries.
    #[test]
    fn remove_and_clear() {
        let mut recent = RecentFiles::default();
        recent.push(Path::new("/a.bpmn"));
        recent.push(Path::new("/b.bpmn"));
        recent.remove(Path::new("/a.bpmn"));
        assert_eq!(recent.files(), [PathBuf::from("/b.bpmn")]);

        recent.clear();
        assert!(recent.files().is_empty());
    }

    /// The list round-trips through disk; a missing file loads empty.
    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("mangue-recent-{}", std::process::id()));
        let path = dir.join("recent.yaml");
        assert!(RecentFiles::load_from(&path).files().is_empty());

        let mut recent = RecentFiles::default();
        recent.push(Path::new("/a.bpmn"));
        recent.save_to(&path).unwrap();

        assert_eq!(RecentFiles::load_from(&path), recent);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
