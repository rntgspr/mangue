---
human_revised: false
name: theme
summary: YAML themes for canvas colors, stroke widths and label font; built-in Solarized and Tokyo Night, chosen by macOS appearance, overridable by a user file.
depends-on: [canvas]
relates: []
targets: [platform]
---

# Theme

## Overview

`mangue::theme` holds every color, main stroke width and the label font the
canvas paints with. Themes are YAML mappings; built-ins live in
`assets/themes/*.yaml` and are embedded through `Assets`, so they use the
same format as user files. The active `Theme` is a GPUI `Global` read by the
painter, palette, context pad, replace menu, label editor and icons.

## Requirements (EARS / RFC 2119)

- The built-in themes MUST be `solarized-light`, `solarized-dark` (https://ethanschoonover.com/solarized/), `tokyonight-day` and `tokyonight-night` (https://github.com/folke/tokyonight.nvim, tag v4.14.1, `extras/lua/`).
- WHILE no user file pins a theme THE SYSTEM SHALL use `solarized-light` in light and `solarized-dark` in dark window appearance.
- WHEN the window appearance changes THE SYSTEM SHALL switch theme and repaint.
- The config dir MUST be `%APPDATA%\mangue` on Windows and `$XDG_CONFIG_HOME/mangue` (default `~/.config/mangue`) elsewhere (`config_dir_for`).
- WHEN `$XDG_CONFIG_HOME/mangue/theme.yaml` (default `~/.config/mangue/theme.yaml`) exists THE SYSTEM SHALL deep-merge its keys over the selected built-in; missing keys keep the built-in value.
- `extends: <name>` MUST pin one built-in for both appearances; `appearance: { light: <name>, dark: <name> }` MUST pick one per appearance (`extends` wins).
- WHEN `$XDG_CONFIG_HOME/mangue/themes/*.yaml` files exist THE SYSTEM SHALL load each as a named user theme (name = `name:` key, else file stem), selectable by `extends` / `appearance` like built-ins; a user theme named like a built-in MUST override it.
- A user theme MAY `extends:` a built-in or another user theme (its own name means the built-in; no `extends` means the same-named built-in, else `solarized-light`); IF the chain cycles THEN THE SYSTEM SHALL report the cycle and fall back to the built-ins.
- IF a file in `themes/` is unreadable or not a mapping THEN THE SYSTEM SHALL log it to stderr and skip it.
- WHEN `settings.yaml` picks `theme.light` / `theme.dark` THE SYSTEM SHALL use that theme for the appearance instead of the `theme.yaml` selection; `theme.yaml` key overrides still apply (see [settings](settings.md)).
- IF the user file is invalid (YAML error, unknown key, bad color, unknown built-in) THEN THE SYSTEM SHALL log to stderr and use the built-ins.
- Canvas, palette and overlay code MUST NOT hardcode colors.
- WHEN a theme is applied THE SYSTEM SHALL derive the gpui-component `Theme` global from the same YAML theme in the same update (`theme::component::sync`); the YAML theme MUST remain the single source of colors.

## Schema

Colors are `"#rrggbb"` or `"#rrggbbaa"`; `*_fill` keys may be `null`
(fall back to `shape.fill`). Example user file:

```yaml
extends: solarized-dark        # or: appearance: { light: tokyonight-day, dark: tokyonight-night }
canvas: { background: "#002b36" }
shape:
  stroke: "#839496"            # shape outlines
  fill: "#073642"
  label: "#93a1a1"             # all label text, vertical pool/lane labels
  event_fill: null
  task_fill: null              # tasks, call activities, sub-processes
  gateway_fill: null
  data_fill: null              # data objects and stores
  band_fill: "#002b36"         # pools and lanes
flow: { stroke: "#93a1a1" }    # connections
selection: "#268bd2"           # selection outline, connect preview, space guides
hover: "#2aa198"
lasso: "#268bd2"
grid: "#0b4150"               # snap grid dots
palette: { background: "#073642", border: "#586e75", icon: "#93a1a1", hover: "#2aa198" }
context_pad: { background: "#073642" }
replace_menu: { background: "#073642", border: "#586e75", text: "#93a1a1" }
label_editor: { background: "#073642", border: "#268bd2", text: "#93a1a1" }
overlay:
  badge: { background: "#073642", border: "#586e75", text: "#93a1a1" }
  error: { background: "#073642", border: "#dc322f", text: "#dc322f" }
stroke_width: { shape: 2.0, end_event: 4.0, flow: 2.0 }
font: { family: ".SystemUIFont", size: 12.0 }
```

## User themes (how-to)

1. Copy `assets/themes/example-custom.yaml` (documented, not embedded) to `~/.config/mangue/themes/<name>.yaml`.
2. Keep only the keys to change; set `extends:` to the base theme.
3. Select it in `theme.yaml`: `extends: <name>` or `appearance: { dark: <name> }`.

Key reference (all keys of a complete theme; user themes and `theme.yaml`
may give any subset):

| Key | Type | Used for |
|-----|------|----------|
| `name` | string | user theme files only; defaults to file stem |
| `extends` | string | base theme (built-in or user) |
| `appearance.light` / `appearance.dark` | string | `theme.yaml` only |
| `canvas.background` | color | canvas surface; message flow markers |
| `shape.stroke` / `shape.fill` / `shape.label` | color | outlines, default fill, all text |
| `shape.event_fill` / `task_fill` / `gateway_fill` / `data_fill` / `band_fill` | color or null | per-kind fills |
| `flow.stroke` | color | connections |
| `selection` / `hover` / `lasso` | color | overlays |
| `grid` | color | snap grid dots (keep low contrast) |
| `palette.background` / `border` / `icon` / `hover` | color | palette (icon also tints pad icons) |
| `context_pad.background` | color | context pad |
| `replace_menu.background` / `border` / `text` | color | replace menu |
| `label_editor.background` / `border` / `text` | color | inline label editor |
| `overlay.badge.*` / `overlay.error.*` (`background` / `border` / `text`) | color | canvas overlay badges; error tooltip and not-allowed marker |
| `stroke_width.shape` / `end_event` / `flow` | number | main stroke widths (diagram units) |
| `font.family` / `font.size` | string / number | labels |

User docs: `docs/themes.md`.

## Decisions

- YAML via `serde_norway =0.9.42` (maintained serde_yaml fork); `serde_yaml` is deprecated and `serde_yml` 0.0.13 is a deprecated shim. `serde =1.0.229` pinned with `derive`.
- Partial files deep-merge as YAML values over the base built-in, then deserialize with `deny_unknown_fields`; per-field `#[serde(default)]` was not used because the base differs per appearance (light vs dark), which a static `Default` cannot express.
- The painter clones the `Theme` global per paint call (`Theme::current`); no global means the light built-in (headless/unit contexts).
- The headless test platform always reports light appearance, so the dark switch is tested through `Canvas::on_appearance`, the appearance observer's handler.
- gpui-component token mapping (`src/theme/component.rs`); unmapped kit tokens keep the kit default for the mode; `sync` is a no-op until `gpui_component::init`; the mode is switched with `Theme::change` only when it differs, then colors are written through `Theme::update`.

  | Kit token(s) | YAML key |
  |---|---|
  | `background`, `list`, `tab_active`, `primary_foreground`, `sidebar_primary_foreground` | `canvas.background` |
  | `foreground`, `caret`, `accent_foreground`, `secondary_foreground`, `popover_foreground`, `sidebar_foreground`, `sidebar_accent_foreground`, `tab_active_foreground` | `replace_menu.text` |
  | `border`, `input` | `replace_menu.border` |
  | `popover` | `replace_menu.background` |
  | `accent`, `list_hover`, `secondary_hover`, `secondary_active`, `sidebar_accent` | `palette.hover` |
  | `secondary`, `muted`, `sidebar`, `tab_bar`, `tab` | `palette.background` |
  | `sidebar_border` | `palette.border` |
  | `muted_foreground`, `tab_foreground` | `palette.icon` |
  | `primary`, `ring`, `list_active_border`, `sidebar_primary` | `selection` |
  | `primary_hover` / `primary_active` / `list_active` | `selection` at 0.9 / 0.8 / 0.2 opacity |
  | `selection` (text) | `lasso` |
  | `danger` / `danger_foreground` | `overlay.error.border` / `overlay.error.text` |
  | `font_family` | `font.family` |

- Rotated pool/lane label SVGs stay monochrome sprites tinted with `shape.label`; their SVG font stays Arial (resvg cannot resolve `.SystemUIFont`), size follows `font.size`.
- Message flow start circles and arrowheads fill with `canvas.background` (hollow look).
- Tokyo Night night border `#15161e` is near-invisible on bg, so frames use `fg_gutter`.

## Files

- `src/theme/mod.rs` — module root and re-exports.
- `src/theme/schema.rs` — `Theme` (GPUI `Global`) and its sections, `fill_for`, `current`.
- `src/theme/color.rs` — `Color` hex parsing/deserialization.
- `src/theme/builtin.rs` — `LIGHT`, `DARK`, `NAMES`, `builtin`, `builtin_value`.
- `src/theme/user.rs` — `Themes` (GPUI `Global`), `config_dir`, `config_path`, `load`, `load_picked`, `parse`, `parse_picked`, `names`, deep merge.
- `src/theme/library.rs` — user theme library: `Library`, `load_dir`, `resolve` (extends chains, cycle detection).
- `src/theme/select.rs` — `for_appearance`, `apply`.
- `src/theme/component.rs` — `sync`, `map_colors`: YAML theme -> gpui-component `Theme` bridge.
- `assets/themes/*.yaml` — built-in themes; `example-custom.yaml` is a documented reference, not embedded.
- `docs/themes.md` — user guide.
