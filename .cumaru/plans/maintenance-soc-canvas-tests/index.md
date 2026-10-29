---
human_revised: false
scope:
  - modeler/canvas.md
  - modeler/code-editor.md
status: in-progress
summary: Move misplaced canvas headless tests to the file that owns their concern (code mode, empty state, document load) and drive handlers via actions.
targets: [platform]
aux: []
---

# Separation of concerns: canvas test files by concern

## Overview

The separation-of-concerns audit of `src/canvas` (2026-10-26) found headless
tests filed outside their concern:

- `view_tests_tabs.rs:326-567`: code-mode tests (mode button, `ToggleMode`, sync, invalid XML, revert, back to code, history) -> new `view_tests_code.rs`, matching the new `code-editor.md` concern. `mode_is_kept_per_tab` stays.
- `view_tests_tabs.rs:584-619`: empty-state tests -> `view_tests_empty.rs`.
- `view_tests_settings.rs:57-76` (`freshly_loaded_document_is_clean`): document load/dirty test in the Settings file -> `view_tests.rs`.
- `view_tests_empty.rs:49-59`: calls private handlers (`on_undo`, `on_save`, ...) directly while the same file already dispatches actions (lines 41-42) -> dispatch the actions.

Tests move verbatim; no assertion changes.

## Acceptance Criteria (EARS / RFC 2119)

- Each moved test MUST keep its name and assertions.
- `view_tests_code.rs` MUST be registered next to the other `view_tests_*` modules (`view_tests.rs`).
- The `cargo test` test count MUST be unchanged.
- `cargo test` MUST pass.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Move canvas tests to their concern's file | done | — |

## Out of scope

- Headless tests reading private `Canvas` / `Document` fields (`docs`, `active`, `tool`, `viewport`, ...): suite-wide convention enabled by child test modules; changing it needs a deliberate test-accessor decision, not a per-file fix.
- Test helpers re-deriving palette / context-pad geometry (use public GPUI-free layout fns).

## Risks

- Overlaps `maintenance-soc-canvas-document` if that plan changes field access used by the moved tests; run after it.
- `view_tests.rs` is also edited by `maintenance-soc-canvas-io` (moves `dialog_start_dir_*` / `check_openable_*` out); run after it. Global order: `.cumaru/notepad.md`.
- Also runs after `maintenance-soc-io` and `maintenance-soc-settings`, which edit `view_tests_settings.rs` (`io` switches `freshly_loaded_document_is_clean` to `persist::open` before this plan moves it). Waves: `.cumaru/notepad.md`.
