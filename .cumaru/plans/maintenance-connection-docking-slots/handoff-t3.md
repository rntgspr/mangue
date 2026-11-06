---
human_revised: false
plan: maintenance-connection-docking-slots
task: T3
status: complete
date: 2026-11-02
summary: Hand-off for T3 - task resize with a 6 grid cell minimum.
---

# Hand-off - maintenance-connection-docking-slots / T3

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/modeling/rules.rs`](../../../src/modeling/rules.rs) | modified - TASK_MIN_GRIDS, grid-aware min_size, tasks in can_resize |
| [`src/modeling/tests_parity.rs`](../../../src/modeling/tests_parity.rs) | modified - task resize and clamp |
| [`src/canvas/view_tests.rs`](../../../src/canvas/view_tests.rs) | modified - task_corner_drag_resizes replaces task_corner_drag_moves |
<!-- /cumaru:touched -->

## Decisions made during implementation

- Scope is the eight task kinds; call activities and collapsed sub-processes stay fixed.
- The minimum is 6 grid cells of the settings spacing (user correction; first draft used one label line). Auto-resize passes a fixed `CONTAINER_GRID`, unused by container minimums.

## Commands run / verification

- `cargo test` - green.

## Pending / follow-ups

- Resizing a task does not move its boundary events (same as expanded sub-processes today).
- Task labels are not clipped when the box is one line tall.

## Suggestions for the Lead

- None.
