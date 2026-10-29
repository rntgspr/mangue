//! App settings (`$XDG_CONFIG_HOME/mangue/settings.yaml`, default
//! `~/.config/mangue/settings.yaml`): snap grid visibility, snapping and
//! size, and the theme per appearance (which wins over the selection of
//! `theme.yaml`). Held as the `Settings` global; see
//! `specs/modeler/settings.md`.

pub mod recent;
pub mod view;

use std::path::{Path, PathBuf};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};

use crate::theme;

/// Grid sizes offered by the settings window, in diagram units.
pub const GRID_SIZES: [f64; 3] = [5.0, 10.0, 20.0];

/// Snap grid settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridSettings {
    /// Whether the dot grid is painted.
    pub visible: bool,
    /// Whether moves, creates and pastes snap to the grid.
    pub snap: bool,
    /// Grid spacing in diagram units.
    pub size: f64,
}

impl Default for GridSettings {
    /// Visible, snapping, 10 units (diagram-js `GridUtil.SPACING`).
    fn default() -> Self {
        GridSettings {
            visible: true,
            snap: true,
            size: 10.0,
        }
    }
}

impl GridSettings {
    /// `value` snapped to this grid, unchanged when snapping is off
    /// (diagram-js `GridSnapping` `quantize`).
    pub fn snap(&self, value: f64) -> f64 {
        if !self.snap {
            return value;
        }

        (value / self.size).round() * self.size
    }
}

/// Theme picked per appearance; `None` keeps the `theme.yaml` selection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dark: Option<String>,
}

/// The whole settings file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub grid: GridSettings,
    pub theme: ThemeSettings,
}

impl Global for Settings {}

/// Where settings are persisted; absent (tests) means changes are not saved.
struct SettingsFile(PathBuf);

impl Global for SettingsFile {}

impl Settings {
    /// Parses a settings file; empty text yields the defaults. A grid size
    /// that is not positive is an error.
    pub fn parse(yaml: &str) -> Result<Settings, String> {
        if yaml.trim().is_empty() {
            return Ok(Settings::default());
        }

        let settings: Settings = serde_norway::from_str(yaml).map_err(|err| err.to_string())?;
        if settings.grid.size.is_nan() || settings.grid.size <= 0.0 {
            return Err(format!(
                "grid size must be positive, got {}",
                settings.grid.size
            ));
        }

        Ok(settings)
    }

    /// Settings read from `path`, the defaults when it is missing; an
    /// invalid file is reported on stderr and ignored.
    pub fn load_from(path: &Path) -> Settings {
        let yaml = std::fs::read_to_string(path).unwrap_or_default();

        Settings::parse(&yaml).unwrap_or_else(|err| {
            eprintln!("mangue: ignoring settings {}: {err}", path.display());
            Settings::default()
        })
    }

    /// Writes these settings as YAML to `path`, creating its directory.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        }
        let yaml = serde_norway::to_string(self).map_err(|err| err.to_string())?;

        std::fs::write(path, yaml).map_err(|err| err.to_string())
    }

    /// The app settings (the defaults when none were installed).
    pub fn current(cx: &App) -> Settings {
        cx.try_global::<Settings>().cloned().unwrap_or_default()
    }

    /// The themes these settings select, resolved with the user theme files.
    fn themes(&self) -> theme::Themes {
        theme::load_picked(self.theme.light.as_deref(), self.theme.dark.as_deref())
    }
}

/// Path of the settings file, `None` when no config dir can be found.
pub fn config_path() -> Option<PathBuf> {
    Some(theme::config_dir()?.join("settings.yaml"))
}

/// Loads the settings file and installs it, with the themes it selects,
/// as globals; later `update`s are saved back to that file.
pub fn install(cx: &mut App) {
    let settings = match config_path() {
        Some(path) => {
            let settings = Settings::load_from(&path);
            cx.set_global(SettingsFile(path));
            settings
        }
        None => Settings::default(),
    };

    recent::install(cx);

    cx.set_global(settings.themes());
    cx.set_global(settings);
}

/// Replaces the settings: re-resolves the themes when the theme choice
/// changed, sets the global (observers such as canvases repaint) and
/// persists to the installed settings file.
pub fn update(settings: Settings, cx: &mut App) {
    let old = Settings::current(cx);
    if old == settings {
        return;
    }

    if old.theme != settings.theme {
        cx.set_global(settings.themes());
    }

    if let Some(SettingsFile(path)) = cx.try_global::<SettingsFile>()
        && let Err(err) = settings.save_to(path)
    {
        eprintln!("mangue: cannot save settings {}: {err}", path.display());
    }

    cx.set_global(settings);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty and partial files fill the defaults (visible, snap, 10).
    #[test]
    fn parse_defaults() {
        assert_eq!(Settings::parse("").unwrap(), Settings::default());

        let partial = Settings::parse("grid:\n  snap: false\n").unwrap();
        assert!(partial.grid.visible);
        assert!(!partial.grid.snap);
        assert_eq!(partial.grid.size, 10.0);
        assert_eq!(partial.theme, ThemeSettings::default());
    }

    /// Theme names and grid values parse; bad keys and sizes fail.
    #[test]
    fn parse_values() {
        let yaml = "grid: {visible: false, size: 20}\ntheme: {light: tokyonight-day}\n";
        let settings = Settings::parse(yaml).unwrap();

        assert!(!settings.grid.visible);
        assert_eq!(settings.grid.size, 20.0);
        assert_eq!(settings.theme.light.as_deref(), Some("tokyonight-day"));
        assert_eq!(settings.theme.dark, None);

        assert!(Settings::parse("grid: {colour: 1}\n").is_err());
        assert!(Settings::parse("grid: {size: 0}\n").is_err());
        assert!(Settings::parse("- 1\n").is_err());
    }

    /// Snapping quantizes to the configured size, or not at all when off.
    #[test]
    fn grid_snap() {
        let mut grid = GridSettings {
            size: 20.0,
            ..GridSettings::default()
        };
        assert_eq!(grid.snap(29.0), 20.0);
        assert_eq!(grid.snap(31.0), 40.0);

        grid.snap = false;
        assert_eq!(grid.snap(31.0), 31.0);
    }

    /// Saving into a fresh directory and loading back is lossless; a
    /// missing file loads the defaults.
    #[test]
    fn save_round_trip() {
        let dir = std::env::temp_dir().join(format!("mangue-settings-{}", std::process::id()));
        let path = dir.join("nested").join("settings.yaml");
        let settings = Settings {
            grid: GridSettings {
                visible: false,
                snap: false,
                size: 5.0,
            },
            theme: ThemeSettings {
                light: None,
                dark: Some("tokyonight-night".into()),
            },
        };

        assert_eq!(Settings::load_from(&path), Settings::default());

        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(loaded, settings);
    }
}
