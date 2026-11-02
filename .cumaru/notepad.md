---
human_revised: false
summary: Scratchpad for working notes shared by the user and the agent; transient drafts, never a source of truth.
---

# Notepad

Scratch area for drafts, reminders and loose notes between sessions. Not loaded by
default and not canonical: anything durable moves to `specs/`, `plans/` or
`exploring/`, and is deleted from here.

## Notes

### Separation-of-concerns plans: execution waves (2026-10-26)

Task `depends-on:` is intra-plan only (schema), so cross-plan order lives here and in each plan's Risks. Dispatch rule: never run two plans that share a `files:` entry at the same time; plans inside one wave share none (checked by script against every `t1.md`).

1. `io`, `theme`, `canvas-text`.
2. `modeling-command` (after `io`: `compat_planes`, `modeling/mod.rs`), `settings` (after `io`: `main.rs`, `view_tests_settings.rs`), `canvas-document` (after `io`: `view_tabs.rs`, `view.rs`).
3. `modeling-dry` (after `modeling-command`: `lanes.rs`, `clipboard.rs`), `canvas-io` (after `io`, `settings`, `canvas-document`: `view.rs`, `main.rs`, `view_tests.rs`).
4. `canvas-domain` (after `modeling-command`: `rules.rs`, `command.rs`, `arrange.rs`; after `canvas-io`: `view.rs`).
5. `modeling-tests` (after `io`, `modeling-command`), `canvas-tests` (after `io`, `settings`, `canvas-document`, `canvas-io`).

All plan names carry the `maintenance-soc-` prefix.

Wave 1 completed on 2026-10-26: `io`, `theme`, and `canvas-text` T1 tasks are done. Verification: `cargo test` passed (320 unit, 17 roundtrip, 3 moddle); `cumaru doctor` passed with 0 errors and 0 warnings.

Wave 2 completed on 2026-10-26: `modeling-command`, `settings`, and `canvas-document` T1 tasks are done. Consolidated verification: `cargo test` passed (321 unit, 17 roundtrip, 3 moddle), `cargo build` passed, `git diff --check` passed, and `cumaru doctor` reported 0 errors and 0 warnings.

Wave 3 completed on 2026-10-26: `modeling-dry` and `canvas-io` T1 tasks are done. Consolidated verification: `cargo test` passed (323 unit, 17 roundtrip, 3 moddle), `cargo build` passed, `git diff --check` passed, and `cumaru doctor` reported 0 errors and 0 warnings.

Wave 4 completed on 2026-10-26: `canvas-domain` T1 is done. Consolidated verification: `cargo test` passed (326 unit, 17 roundtrip, 3 moddle), `cargo build` and `git diff --check` passed, and `cumaru doctor` reported 0 errors and 0 warnings. Its test files contract was extended to include `src/modeling/mod.rs` and `src/modeling/tests_rules.rs`.

Wave 5 completed on 2026-10-26: `modeling-tests` and `canvas-tests` T1 tasks are done. The test count remained 326 unit, 17 roundtrip, and 3 moddle. `cargo test`, `cargo build`, `cargo fmt --check`, `git diff --check`, and `cumaru doctor` passed; doctor reported 0 errors and 0 warnings.

Resolutions applied before starting:

- Attach host search moved from `modeling-dry` to `canvas-domain`; document save moved from `canvas-io` to `io`.
- Load/save/code-import composition home fixed: `src/canvas/persist.rs` (`open`, `import_code`, `save`), created by `io`.
- Test boundary: modeling tests call `io::import_xml` + `modeling::compat_planes` explicitly and never `canvas::persist`; the composition is tested in canvas.
- `canvas-domain` reuses the participant-refresh tail of `arrange.rs` `moves_command` (lines 162-169) via one shared helper.
- Rules tests: `can_connect` / `container_at` go to a new `tests_rules.rs`; refusal tests stay in `tests_refusal.rs`.
- `settings` menu test asserts the installed menu (`App::get_menus`, backed by the GPUI test platform), not the `RecentFiles` global.
