---
human_revised: false
plan: maintenance-connection-docking-slots
status: draft
date: 2026-11-02
summary: Delta draft for maintenance-connection-docking-slots - docking slots, redock and task resize.
---

# Delta draft - maintenance-connection-docking-slots

## specs/modeler/modeling.md

### Added Requirements

- WHEN N connection ends dock on the same side of a shape THE SYSTEM SHALL place the i-th (0-based) at `(i + 0.5) / N` of that side, ordered by current position along the side, then by the other end's docking position; incoming and outgoing share the slots (`modeling::docking`).
- WHEN any command executes THE SYSTEM SHALL redistribute slots in the same undoable step; WHEN a file is opened THE SYSTEM SHALL redistribute every plane, keeping imported label bounds.
- `docking::redock` MUST move one end to the nearest border point of its own shape on any side, without changing source or target.
- Tasks (`Task` and its seven typed variants) MUST be resizable with a minimum of 6 grid cells (settings grid spacing) in width and height.

### Modified Requirements

- Reconnect and move keep the side of the end that did not change, but slot distribution takes precedence over its exact docking point (was: the other end keeps its docking point, `preserveDocking`).

## specs/modeler/canvas.md

### Added Requirements

- WHEN the first or last waypoint of the selected connection is dropped on its own shape THE SYSTEM SHALL redock that end at the nearest border point (any side) and redistribute the slots.

### Modified Requirements

- Selected tasks show resize corners (was: tasks have no resize handles).

## specs/modeler/parity.md

### Modified Requirements

- Connection reconnect row and Decisions: `preserveDocking` is overridden by mangue docking slots; tasks are resizable (a mangue addition to bpmn-js `canResize`).
