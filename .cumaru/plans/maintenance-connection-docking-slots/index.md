---
human_revised: false
scope:
  - modeler/modeling
  - modeler/canvas
  - modeler/model
  - modeler/parity
status: in-progress
summary: Distribute connection docking points into equal slots per shape side, let the user reorder them, and bound task resize by 6 grid cells.
targets: [platform]
aux: []
---

# Connection docking slots

## Overview

Today the layout docks every connection at the middle of the chosen side
(`layout::dock`, `preferred_keeping` in `src/modeling/layout.rs`), and reconnect
or move keeps the other end's docking point (`preserveDocking`). When a shape
receives several connections on one side they converge on one point and become
hard to read and to edit.

This plan divides each side of a connectable shape into N equal slots, where N
is the number of connections docked on that side, and docks the i-th connection
at the middle of slot i, i.e. at `(i + 0.5) / N` of the side length. The initial
order follows the position of each connection's other end; the user may then
drag a docking point to another valid slot on any side of the same shape, without disconnecting, and that choice is
kept. Slot distribution replaces `preserveDocking` wherever they conflict.

Tasks become resizable, with a minimum of 6 grid cells (settings grid
spacing) in both width and height.

## Acceptance Criteria (EARS / RFC 2119)

- WHEN N connections dock on the same side of a connectable shape THE SYSTEM SHALL place the i-th docking point (0-based) at `(i + 0.5) / N` of that side's length.
- WHEN slots are first assigned on a side THE SYSTEM SHALL order them by the other end's position along that side (top-to-bottom for left/right sides, left-to-right for top/bottom sides).
- Incoming and outgoing connections on the same side MUST share one slot set.
- Every connection kind (sequence flow, message flow, association, data input/output association) MUST count toward and use the slots of every shape kind it can connect to.
- WHEN a connection is created, reconnected or deleted, or a shape is moved or resized THE SYSTEM SHALL recompute the slots of every affected side and re-route the connections docked there, as one undoable command.
- WHEN a diagram is loaded THE SYSTEM SHALL redistribute docking points into slots.
- WHEN the user drags a connection's end onto another valid slot of the same shape, on the same or another side, THE SYSTEM SHALL move it there without disconnecting, re-route it and redistribute both affected sides; the chosen side and order MUST persist across later recomputations.
- Every side of the connection's own shape MUST be a valid redock target (no refusal case was found; amended from an invalid-side refusal during T2).
- Slot distribution MUST take precedence over `preserveDocking` (`layout::Keep`) when they disagree.
- WHEN a task is resized THE SYSTEM SHALL keep its width and height at least 6 cells of the settings grid spacing (`rules::TASK_MIN_GRIDS`).
- Routes MUST stay orthogonal-only; every existing modeling and canvas test MUST still pass or be updated with a recorded reason.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Slot distribution in commands and load | done | - |
| [T2](t2.md) | Redock gesture on the own shape | done | T1 |
| [T3](t3.md) | Task resize with 6-grid minimum | done | - |

Spec and parity updates are carried by [delta-draft.md](delta-draft.md) for absorb.

## Out of scope

- Connect/docking snapping of the pointer (`BpmnConnectSnapping`); see parity Decisions.
- Non-orthogonal routes.
- Growing a shape automatically when slots get crowded.

## Risks

- Deviates from bpmn-js `preserveDocking`; must be recorded in `parity.md` Decisions.
- Persisting a user slot order has no BPMN DI field; it must be derivable from waypoints on load (order of docking points along the side), otherwise it is lost on round-trip.
- Redistribution on load rewrites imported waypoints, changing exported XML of untouched files.
- Obstacle avoidance may fail for a slot that is not the side middle; the fallback must still dock inside the slot.
- Very small tasks with many connections yield slots closer than the 6 px grab tolerance.
