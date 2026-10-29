---
human_revised: false
scope:
  - modeler/settings.md
  - modeler/canvas.md
status: in-progress
summary: Break the settings -> canvas cycle - recent.rs persistence rebuilds the menu bar by calling canvas::actions::set_menus.
targets: [platform]
aux: []
---

# Separation of concerns: recent files must not call into canvas

## Overview

The separation-of-concerns audit of `src/settings` (2026-10-26) found
`src/settings/recent.rs:100` (`update`) calling
`crate::canvas::actions::set_menus(cx)` after persisting `recent.yaml`. Settings
storage thereby depends on the canvas menu bar, closing a cycle with
`canvas/view.rs` / `canvas/actions.rs`, which call `settings::recent`.

The menu rebuild is a canvas concern reacting to a change in the
`RecentFiles` global. GPUI's `observe_global` already drives the canvas from
`Settings` (`canvas/view.rs:212`), so the canvas side can observe
`RecentFiles` the same way instead of being called from storage.

## Acceptance Criteria (EARS / RFC 2119)

- `src/settings` MUST NOT reference `crate::canvas` (`rg -n "crate::canvas" src/settings` empty).
- WHEN a file is recorded, forgotten or the list cleared (Open, Save As, startup argument, Open Recent on a missing file, Clear Recent) THE SYSTEM SHALL rebuild the File > Open Recent menu, as today.
- The menu rebuild MUST be triggered from one place on the canvas/app side, not repeated at each `recent::*` call site.
- `cargo test` MUST pass with no test weakened or deleted.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Rebuild menus by observing RecentFiles | done | — |

## Out of scope

- Splitting `settings/mod.rs` persistence (`SettingsFile`, `load_from`, `save_to`) into a store module: audit called it a judgment call; the file is small and cohesive (KISS).
- `Settings::themes` calling `theme::load_picked`: private, two callers, direction settings -> theme is fine.

## Risks

- `observe_global` fires only for globals set via `cx.set_global`/`update_global`; `recent::update` uses `set_global`, verify the observer fires on every path, including before the first window exists (`main.rs:84` records before the window opens).
- Runs after `maintenance-soc-io`, which also edits `main.rs` and `view_tests_settings.rs`; `maintenance-soc-canvas-io` (`main.rs`) and `maintenance-soc-canvas-tests` (`view_tests_settings.rs`) run after this plan. Waves: `.cumaru/notepad.md`.
