---
human_revised: false
plan: maintenance-connection-docking-slots
task: T2
status: complete
date: 2026-11-02
summary: Hand-off for T2 - redock gesture on the connection's own shape.
---

# Hand-off - maintenance-connection-docking-slots / T2

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/modeling/docking.rs`](../../../src/modeling/docking.rs) | modified - project, redock |
| [`src/modeling/layout.rs`](../../../src/modeling/layout.rs) | modified - side_route (join or detour) |
| [`src/canvas/view.rs`](../../../src/canvas/view.rs) | modified - endpoint drop on own shape issues redock |
| [`src/canvas/view_tests_parity.rs`](../../../src/canvas/view_tests_parity.rs) | modified - drag_end_along_own_shape_redocks |
<!-- /cumaru:touched -->

## Decisions made during implementation

- Every side of the own shape is valid; no refusal case was found, so the planned invalid-side refusal was dropped (plan criterion amended).
- Changing side drops the user bendpoints of that connection (re-routed from the new side).

## Commands run / verification

- `cargo test --lib redocks` - fails with the canvas branch disabled, passes enabled.
- `cargo test` - green.

## Pending / follow-ups

- Not smoke-tested in the running app (headless GPUI tests only).

## Suggestions for the Lead

- None.
