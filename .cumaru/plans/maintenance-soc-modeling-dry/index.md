---
human_revised: false
scope:
  - modeler/modeling.md
status: in-progress
summary: Give fresh-id allocation one home in modeling - the factory new_id loop is copied in clipboard paste and lanes fresh_lane_ids.
targets: [platform]
aux: []
---

# Separation of concerns: single owner for fresh id allocation

## Overview

The separation-of-concerns audit of `src/modeling` (2026-10-26) found the
fresh-id rule living in three places (verified):

- `factory::new_id` (`factory.rs:27`) owns `<prefix>_<n>`.
- `clipboard.rs:167-172` copies the loop, adding a `taken` list because `defs`
  is not mutated between ids.
- `lanes.rs:312` `fresh_lane_ids` copies it for `Lane_<n>`.

Behavior stays identical; only ownership moves. The attach host search
duplicated in `canvas/view.rs` moved to `maintenance-soc-canvas-domain`
(2026-10-26 overlap review).

## Acceptance Criteria (EARS / RFC 2119)

- Fresh id allocation MUST have one implementation in `factory.rs`, used by `new_id`, clipboard paste and `fresh_lane_ids`.
- WHEN elements are pasted or lanes added THE SYSTEM SHALL generate the same ids as before (existing tests asserting `Lane_1`, `Lane_2` and paste ids stay green).
- `cargo test` MUST pass with no test weakened or deleted.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Single owner for id allocation | done | — |

## Out of scope

- Attach host search (`maintenance-soc-canvas-domain`).
- Repeated "refresh participant refs" step: shared helper introduced by `maintenance-soc-canvas-domain`.
- Lane vs auto-resize padding constants: different values, coincidental duplication.
- Test helper duplication (`pt`, `b`, `node`) across `tests_*.rs`: trivial constructors, kept (KISS).

## Risks

- Runs after `maintenance-soc-modeling-command`, which moves lane helpers into `lanes.rs` and edits `clipboard.rs` imports; `fresh_lane_ids` and the clipboard allocator are untouched by it. Global order: `.cumaru/notepad.md`.
