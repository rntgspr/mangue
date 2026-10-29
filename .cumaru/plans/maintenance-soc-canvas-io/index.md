---
human_revised: false
scope:
  - modeler/canvas.md
status: in-progress
summary: Move GPUI-free filesystem helpers out of canvas and main.rs into src/files (open checks, dialog start dir, tree lookup, file URL decoding).
targets: [platform]
aux: []
---

# Separation of concerns: filesystem helpers out of the view

## Overview

The separation-of-concerns audit of `src/canvas` and the `src/` root
(2026-10-26) found GPUI-free filesystem logic outside `src/files` (verified):

- `src/canvas/view.rs:2095-2130`: `home_dir`, `dialog_start_dir`,
  `check_openable` (file-extension policy).
- `src/canvas/sidebar.rs:67` `find_mut`: generic `files::tree::Node` lookup
  used only by `view.rs:292,317`, beside `Node::expand` in `src/files/tree.rs`.
- `src/main.rs:28` `url_to_path`: pure file-URL decoder (macOS Open Documents)
  in the binary, unreachable by lib unit tests.

Document save moved to `maintenance-soc-io` (2026-10-26 overlap review), so
this plan has no cross-plan dependency.

## Acceptance Criteria (EARS / RFC 2119)

- `home_dir`, `dialog_start_dir`, `check_openable` MUST live in `src/files/` (GPUI-free), with their existing tests.
- `find_mut` MUST live in `src/files/tree.rs`.
- `url_to_path` MUST live in `src/files/` with a unit test; `main.rs` only calls it.
- WHEN the user opens a file, picks a dialog folder, opens a document from Finder or browses the sidebar THE SYSTEM SHALL behave as today.
- `cargo test` MUST pass with no test weakened or deleted.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Move filesystem helpers into src/files | done | — |

## Out of scope

- Document save (`maintenance-soc-io`).
- Dialogs, alerts and unsaved-changes prompts: presentation, stay in the view.

## Risks

- Runs after `maintenance-soc-io` (`view.rs`, `main.rs`), `maintenance-soc-settings` (`main.rs`) and `maintenance-soc-canvas-document` (`view.rs`, `view_tests.rs`); `maintenance-soc-canvas-domain` (`view.rs`) and `maintenance-soc-canvas-tests` (`view_tests.rs`) run after it. Waves: `.cumaru/notepad.md`.
