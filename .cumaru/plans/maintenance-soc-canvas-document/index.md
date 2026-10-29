---
human_revised: false
scope:
  - modeler/canvas.md
  - modeler/code-editor.md
status: in-progress
summary: Encapsulate the per-tab dirty-state revision stack in Document so view.rs and view_tabs.rs stop mutating history, undone and next_revision directly.
targets: [platform]
aux: []
---

# Separation of concerns: Document owns its revision bookkeeping

## Overview

The separation-of-concerns audit of `src/canvas` (2026-10-26) found the
dirty-state revision stack (`Document::history`, `undone`, `next_revision`,
`saved`; `src/canvas/document.rs:24-33`) mutated from outside `Document`:

- `src/canvas/view.rs:471-473` (execute), `:1392` (undo), `:1408` (redo).
- `src/canvas/view_tabs.rs:241-261` (`replace_definitions`): revision
  renumbering after a code sync plus model swap, order-cache, selection,
  viewport and plane resets.

`canvas.md` records the decision to mirror revisions next to the
`CommandStack` in the view because modeling exposes no stack position; this
plan keeps that decision and only gives the mirror one owner: `Document`
methods, with private fields.

## Acceptance Criteria (EARS / RFC 2119)

- `history`, `undone`, `next_revision` and `saved` MUST NOT be accessed outside `document.rs` in production code (`rg -n "\.(history|undone|next_revision)\b" src/canvas --glob '!*tests*'` only hits `document.rs`).
- `Document` MUST expose the transitions used today: record an executed command, undo, redo, replace the model after a code sync, mark saved, is dirty.
- WHEN a command executes, is undone or redone THE SYSTEM SHALL report dirty state exactly as today.
- WHEN code mode syncs accepted XML THE SYSTEM SHALL keep the document dirty after undoing back past the sync (existing behavior and `code-editor.md` decision).
- `cargo test` MUST pass with no test weakened or deleted.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Move revision transitions into Document | done | — |

## Out of scope

- Moving revision tracking into `modeling::CommandStack` (would reverse a recorded `canvas.md` decision; separate design change).
- Tests reading `doc.history` (`view_tests_tabs.rs`): repo-wide headless-test convention, see `maintenance-soc-canvas-tests`.

## Risks

- `replace_definitions` also touches view-level state (`fit_pending`, `hover`); keep those in `Canvas`, move only document state.
- Runs after `maintenance-soc-io`, which edits the `import_xml` call in `leave_code_mode` right before `replace_definitions` (`view_tabs.rs`); this plan only touches `replace_definitions` and the revision fields. Global order: `.cumaru/notepad.md`.
