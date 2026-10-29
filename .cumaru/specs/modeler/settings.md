---
human_revised: false
name: settings
summary: App settings in settings.yaml (grid visibility, snapping, size; theme per appearance), the recent files list in recent.yaml, and the cmd-, Settings window applying them live.
depends-on: [canvas, theme]
relates: []
targets: [platform]
---

# Settings

## Overview

`mangue::settings` holds the `Settings` GPUI global, loaded from
`$XDG_CONFIG_HOME/mangue/settings.yaml` (default
`~/.config/mangue/settings.yaml`) at startup, and the Settings window
(`SettingsView`) that edits it. Canvases observe the global and repaint.

```yaml
grid:
  visible: true   # paint the dot grid
  snap: true      # snap create/move/paste to the grid
  size: 10        # diagram units; the window offers 5, 10, 20
theme:
  light: solarized-light   # optional; built-in or user theme name
  dark: tokyonight-night
```

## Requirements (EARS / RFC 2119)

- Missing keys MUST default to `grid: {visible: true, snap: true, size: 10}` and no theme pick; a missing file means all defaults.
- IF the file is invalid (YAML error, unknown key, non-positive size) THEN THE SYSTEM SHALL log to stderr and use the defaults.
- `theme.light` / `theme.dark` MUST win over the `theme.yaml` selection for that appearance; `theme.yaml` key overrides still merge over the pick.
- WHEN cmd-, or App menu "Settings..." is used THE SYSTEM SHALL open the Settings window (or focus it when open), themed by the active theme: theme list per appearance (built-ins then user themes), Show grid, Snap to grid, grid size 5/10/20.
- WHEN a document is opened or saved THE SYSTEM SHALL push its path to the recent list (deduplicated, newest first, max 10), persist it to `recent.yaml` beside `settings.yaml`, call `App::add_recent_document` and rebuild the menus.
- WHEN a setting changes THE SYSTEM SHALL apply it to every canvas window (re-resolving themes when the pick changed) and write `settings.yaml`.

## Decisions

- Persistence goes through a private `SettingsFile` global set only by `install`, so headless tests never write the user config.
- Canvases keep a copy of `GridSettings`, refreshed by an `observe_global_in::<Settings>` that also re-applies the theme for the window appearance.
- The Settings window uses gpui-component `Settings` / `SettingPage` / `SettingGroup`: a theme dropdown per appearance, Show grid / Snap to grid switches, a grid-size dropdown over `GRID_SIZES`; 720px wide because the kit always adds its search sidebar; wrapped in `Root` (`open` returns `WindowHandle<Root>`, `settings_view` reaches the view).

## Files

- `src/settings/mod.rs` — `Settings`, `GridSettings` (`snap`), `ThemeSettings`, `GRID_SIZES`, `config_path`, `install`, `update`, unit tests.
- `src/settings/view.rs` — `SettingsView` window and `open`.
- `src/canvas/actions.rs` — `OpenSettings` action, cmd-, binding, App menu entry.
- `src/settings/recent.rs` — `RecentFiles` (push, remove, clear, load/save `recent.yaml`), `record`, `forget`, `clear`, unit tests.
