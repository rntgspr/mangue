---
human_revised: false
plan: maintenance-soc-canvas-document
task: T1
status: complete
date: 2026-10-26
summary: Document owns revision transitions and code-sync model replacement.
---

# Hand-off — maintenance-soc-canvas-document / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/canvas/document.rs`](../../../src/canvas/document.rs) | Owns private revision state and transitions. |
| [`src/canvas/view.rs`](../../../src/canvas/view.rs) | Calls document revision transitions. |
| [`src/canvas/view_tabs.rs`](../../../src/canvas/view_tabs.rs) | Delegates model replacement to Document. |
| [`src/canvas/view_tests_tabs.rs`](../../../src/canvas/view_tests_tabs.rs) | Uses a test-only accessor to retain history assertions. |
<!-- /cumaru:touched -->

## Verification

- Before moving code, `cargo test` filters for `code_sync_keeps_canvas_history`, `save_marks_clean`, and `switching_tabs` passed (1, 1, and 2 tests).
- After moving code, `cargo test canvas::view::tests::tabs::` passed 30 tests.
- `cargo test` passed 321 unit tests, 17 roundtrip tests, and 3 moddle tests, plus doc tests.
- Revision field searches find accesses only in `document.rs` outside tests.

## Pending

- Absorb the delta when the plan is closed.
