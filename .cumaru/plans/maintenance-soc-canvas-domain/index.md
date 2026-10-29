---
human_revised: false
scope:
  - modeler/canvas.md
  - modeler/modeling.md
status: in-progress
summary: Move BPMN rules decided in canvas (attach host search, reconnect refusal, move + participant refresh, connect-from predicate, Command introspection) into modeling.
targets: [platform]
aux: []
---

# Separation of concerns: domain rules out of canvas

## Overview

The separation-of-concerns audit of `src/canvas` (2026-10-26) found modeling
decisions made in the presentation layer (all verified):

- `src/canvas/view_feedback.rs:149-154`: reconnect refusal decided inline
  (`factory::reconnect(..).is_none().then_some(Refusal::NotConnectable)`)
  while sibling refusals live in `modeling::rules` (`connect_refusal`,
  `create_refusal`, `drop_refusal`).
- `src/canvas/view.rs:1024-1053` (`move_command`): builds `MoveShapes` plus one
  `UpdateFlowNodeRefs` per participant, the same lane-membership step
  `modeling/arrange.rs:162`, `lanes.rs` and `clipboard.rs:327` apply.
- `src/canvas/context_pad.rs:107`: `!matches!(kind, Lane | Group)` decides
  which shapes may start a connection, beside `rules::can_connect`.
- `src/canvas/view.rs:1691-1714` (`created_id`, `created_shape`): pattern
  match on `Command` variants; also used by `view_aids.rs:128`.
- `src/canvas/view.rs:1062-1078` (`attach_command`) re-implements the attach
  host search of `src/modeling/attach.rs:67-80` (`move_boundary`): 15px pad,
  self exclusion, bounds test, `can_attach`, topmost (moved here from
  `maintenance-soc-modeling-dry` by the 2026-10-26 overlap review, since
  `move_command` calls `attach_command`).

Behavior is unchanged; rules gain one owner in `modeling`.

## Acceptance Criteria (EARS / RFC 2119)

- `modeling::rules` MUST expose `reconnect_refusal` used by `view_feedback.rs`; covered by a test in `tests_refusal.rs`.
- `modeling` MUST expose one move builder (selection move + participant ref refresh) used by `view.rs` `move_command`; the per-participant `UpdateFlowNodeRefs` loop MUST NOT appear in `src/canvas`.
- The connect-entry predicate MUST live in `modeling::rules` and be used by `context_pad.rs`.
- `created_id` / `created_shape` MUST live in `modeling` (`command.rs`), not `src/canvas`.
- `src/modeling/attach.rs` MUST expose one host lookup (e.g. `host_at(defs, id, point) -> Option<ElementId>`) used by `move_boundary` and `canvas/view.rs` `attach_command`; the 15px attachment padding MUST have one shared definition.
- WHEN the user moves, reconnects or opens the context pad THE SYSTEM SHALL behave as today (existing headless tests green).
- `cargo test` MUST pass with no test weakened or deleted.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Move canvas-side rules into modeling | done | — |

## Out of scope

- `search::find`, `drilldown::crumbs`, `view_aids` what-if previews (clone + execute), `append_menu` catalog: audited as UI feature logic over the public model API (bpmn-js keeps search/drilldown in features), kept.
- `auto_resize::with_auto_resize` applied at three call sites: note only.

## Risks

- `move_command` interleaves the attach/detach choice with the move: extract `host_at` first, then the plain-move builder, so the attach branch stays a thin call.
- Runs after `maintenance-soc-modeling-command`, which also edits `rules.rs` (gains `closure`, `is_direction_horizontal`), `command.rs` (extractions) and `arrange.rs` (`closure` import); add the new functions on top of its result.
- Do not add a second participant-refresh loop: extract the tail of `arrange.rs` `moves_command` (lines 162-169, `UpdateFlowNodeRefs` per participant) into one `pub(super)` helper and use it from both `moves_command` and the new move builder. `moves_command` itself is not reusable as-is (one `MoveShapes` per id with per-id deltas, skips zero moves).
- Test homes: `reconnect_refusal` goes to `tests_refusal.rs` (refusal rules); non-refusal rules tests go to `tests_rules.rs`, created by `maintenance-soc-modeling-tests`. Global order: `.cumaru/notepad.md`.
- Runs after `maintenance-soc-canvas-io` (`view.rs`), not only `maintenance-soc-modeling-command`. Waves: `.cumaru/notepad.md`.
