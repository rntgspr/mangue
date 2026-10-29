---
human_revised: false
plan: maintenance-soc-canvas-tests
task: T1
status: complete
date: 2026-10-26
summary: Canvas headless tests organized by concern and empty state actions dispatched.
---

# Hand-off — maintenance-soc-canvas-tests / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/canvas/view_tests.rs`](../../../src/canvas/view_tests.rs) | Registers code tests and owns the document load test. |
| [`src/canvas/view_tests_tabs.rs`](../../../src/canvas/view_tests_tabs.rs) | Retains tab-specific tests. |
| [`src/canvas/view_tests_code.rs`](../../../src/canvas/view_tests_code.rs) | Owns code-mode tests. |
| [`src/canvas/view_tests_empty.rs`](../../../src/canvas/view_tests_empty.rs) | Owns empty-state tests and dispatches actions. |
| [`src/canvas/view_tests_settings.rs`](../../../src/canvas/view_tests_settings.rs) | Retains settings tests. |
| [Code editor spec](../../specs/modeler/code-editor.md) | Identifies the code-mode and tab test files. |
<!-- /cumaru:touched -->

## Verification

- Before and after `cargo test -q`: 326 unit, 17 roundtrip and 3 moddle tests passed; counts were unchanged.
- The test moves were performed together and verified with a single full suite; intermediate per-group runs were not performed.

## Pending

- Absorb the canvas test ownership delta when closing the plan.
