---
human_revised: false
plan: maintenance-soc-settings
task: T1
status: complete
date: 2026-10-26
summary: Recent files changes rebuild the installed menus through an app observer.
---

# Hand-off — maintenance-soc-settings / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/settings/recent.rs`](../../../src/settings/recent.rs) | Removed the canvas dependency from recent-file updates. |
| [`src/canvas/actions.rs`](../../../src/canvas/actions.rs) | Observes recent-file updates and rebuilds menus. |
| [`src/canvas/view_tests_settings.rs`](../../../src/canvas/view_tests_settings.rs) | Checks installed menus across initial, record, forget and clear states. |
<!-- /cumaru:touched -->

## Verification

- `cargo test recent_files_rebuild_installed_menu`: failed on the forget assertion without an observer, then passed with the observer.
- `cargo test`: 321 unit, 17 roundtrip and 3 moddle tests passed; doc tests passed.
- `rg -n "crate::canvas" src/settings`: no matches.

## Pending

- Absorb the delta into the settings and canvas specs when the plan closes.
