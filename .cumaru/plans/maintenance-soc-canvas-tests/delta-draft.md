---
human_revised: false
plan: maintenance-soc-canvas-tests
status: draft
date: 2026-10-26
summary: Draft canvas specification changes for headless test ownership.
---

# Delta draft

- `modeler/canvas.md`: `view_tests_code.rs` owns code-mode tests; `view_tests_empty.rs` owns empty-state tests; the document load test lives in `view_tests.rs`.
- `modeler/code-editor.md`: the test file references already identify `view_tests_code.rs` and the per-tab mode test in `view_tests_tabs.rs`.
