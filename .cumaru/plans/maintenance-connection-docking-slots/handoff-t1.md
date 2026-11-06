---
human_revised: false
plan: maintenance-connection-docking-slots
task: T1
status: complete
date: 2026-11-02
summary: Hand-off for T1 - slot distribution wired into commands and load.
---

# Hand-off - maintenance-connection-docking-slots / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/modeling/docking.rs`](../../../src/modeling/docking.rs) | created - slot geometry, distribute, apply, distribute_all |
| [`src/modeling/tests_docking.rs`](../../../src/modeling/tests_docking.rs) | created - slot, order, command, undo and load tests |
| [`src/modeling/command.rs`](../../../src/modeling/command.rs) | modified - execute = apply + slot pass as a trailing undo step |
| [`src/modeling/layout.rs`](../../../src/modeling/layout.rs) | modified - border_side visible to modeling |
| [`src/modeling/mod.rs`](../../../src/modeling/mod.rs) | modified - docking and tests_docking modules |
| [`src/canvas/persist.rs`](../../../src/canvas/persist.rs) | modified - distribute_all on open |
| [`src/modeling/tests_vertical.rs`](../../../src/modeling/tests_vertical.rs) | modified - preserveDocking tests now expect the lone end at its slot |
<!-- /cumaru:touched -->

## Decisions made during implementation

- User order needs no DI field: it is the current docking position along the side, so it survives save/load.
- A lone end is moved to its side middle, overriding preserveDocking (user-approved precedence).
- Load keeps imported label bounds; command-time moves reset the label to its default spot.
- Code-editor import (`persist::import_code`) does not distribute, like it does not orthogonalize.

## Commands run / verification

- `cargo test --lib docking` - connect test failed before the execute wrapper, passes after.
- `cargo test` - 357 + 17 + 3 passed.

## Pending / follow-ups

- Slots sit on the bounding box: on gateways and events a non-middle slot leaves a visible gap to the outline.
- No obstacle check after shifting an end into its slot.

## Suggestions for the Lead

- None.
