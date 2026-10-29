---
human_revised: false
scope:
  - modeler/modeling.md
status: in-progress
summary: Split the concerns bundled in src/modeling/command.rs (sub-process planes, routing policy, lane geometry, graph closure) into their owning modeling modules.
targets: [platform]
aux: []
---

# Separation of concerns: decompose modeling/command.rs

## Overview

The separation-of-concerns audit of `src/modeling` (2026-10-26) found
`src/modeling/command.rs` (1368 lines) owning, besides command execution, four
concerns that change for independent reasons and partly already have a home:

- **Sub-process planes** (lines 537-745): `drop_plane`, `split_plane`,
  `compat_planes`, `sub_process_diagram`, `content_bounds`, `translate_plane`,
  `PLANE_ORIGIN`. `compat_planes` is an import normalization reached from `io`
  (see `maintenance-soc-io`).
- **Routing policy** (lines 975-1188): `reroute`, `reroute_keeping`, `route`,
  `layout_rule`, `obstacles`, `repair_or_reroute`, `orthogonal`, while
  `layout.rs` owns the `Rule` / `Side` vocabulary they consume.
- **Lane geometry and membership** (lines 868-905, 1267-1354):
  `close_lane_gap`, `update_flow_node_refs`, `lane_slot`, `lane_depth`, next to
  `lanes.rs`, which owns lane editing.
- **Graph closure** (line 909): `pub(super) fn closure` imported by
  `arrange.rs:6`, `clipboard.rs:7`, `snapping.rs:8`.

Also: `layout.rs:673` `is_direction_horizontal` walks the `Definitions`
hierarchy (pool/lane DI, `process_ref`), a model query beside pure routing
geometry; `rules.rs` already owns `owner`, `participant_of`, `plane_process`.

Pure move refactor: no behavior, public API or undo semantics change.

## Acceptance Criteria (EARS / RFC 2119)

- Plane functions MUST live in one modeling module (e.g. `src/modeling/planes.rs`); `command.rs` MUST only call them.
- Routing policy (`route`, `layout_rule`, `obstacles`) MUST live in `layout.rs` (or a routing module next to it); `command.rs` MUST call one routing entry point.
- Lane geometry/membership helpers MUST live in `lanes.rs`.
- `closure` MUST NOT be imported from `command` by other modules (`rg -n "command::closure" src` empty).
- `is_direction_horizontal` MUST live with the other model-hierarchy queries in `rules.rs`.
- `src/modeling` MUST still not reference GPUI, `io` or `canvas` outside `#[cfg(test)]`.
- `cargo test` MUST pass with no test weakened or deleted.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Move bundled concerns out of command.rs | done | — |

## Out of scope

- `Refusal::message` (`rules.rs:444`) and `factory::kind_label` (`factory.rs:466`) returning English UI text from `modeling`: real presentation-in-domain, deferred until a second locale or UI wording change makes them move independently (YAGNI).
- `undo.rs` plane repair in `restore`, `ordering::gfx_parent` lookup, `snapping::targets` policy, `export.rs` writer split: audited as cohesive, no change.
- Duplicated id allocation and attach host search (`maintenance-soc-modeling-dry`); test-file cohesion (`maintenance-soc-modeling-tests`).

## Risks

- Overlaps `maintenance-soc-io` on `compat_planes`: run `maintenance-soc-io` T1 first, or move `compat_planes` here and let that plan only re-point callers. Do not run both concurrently on `src/modeling/mod.rs`.
- Moving lane helpers into `lanes.rs` adds `lanes -> command` and `command -> lanes` edges inside one module; acceptable in Rust, but keep `Command` the only shared type.
- Large mechanical diff: keep moves verbatim (no logic edits) so review is a cut/paste check.
- Runs before `maintenance-soc-modeling-dry` (both edit `lanes.rs` and `clipboard.rs`, different functions) and before `maintenance-soc-canvas-domain` (adds functions to `rules.rs`, `command.rs`, `arrange.rs`). Global order: `.cumaru/notepad.md`.
