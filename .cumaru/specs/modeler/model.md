---
human_revised: false
name: model
summary: GPUI-independent BPMN 2.0 semantic model and DI types (bpmn-js core element set) in the mangue library crate.
depends-on: []
relates: []
targets: [platform]
---

# Model

## Overview

`mangue::model` is a pure-Rust module (no GPUI) holding the BPMN 2.0 semantic
model and its diagram interchange (BPMNDI). `Definitions` is the document root:
an insertion-ordered map of `Element`s plus `diagrams`: the root `Diagram`
(process or collaboration) first, then one per collapsed sub-process with its
own plane, each a `Plane` of `Shape`s and `Edge`s. Element kinds cover the bpmn-js core set
(https://github.com/bpmn-io/bpmn-js); no vendor extensions.

## Requirements (EARS / RFC 2119)

- The model MUST NOT depend on GPUI.
- `Definitions` MUST keep elements in insertion (document) order.
- WHEN an element with an existing id is added THE SYSTEM SHALL reject it and leave the model unchanged.
- WHEN a node is removed THE SYSTEM SHALL also remove every connection whose source or target is that node, and the DI of all removed elements.
- WHEN a connection is removed THE SYSTEM SHALL keep its endpoints.
- `ElementKind::default_size` MUST return bpmn-js `ElementFactory` sizes: task/call activity/collapsed sub-process 100x80, event 36x36, gateway 50x50, expanded sub-process 350x200, participant 600x250, data object 36x50, data store 50x50, text annotation 100x30, group 300x300.
- `Definitions` MUST always hold at least the root diagram; `active` indexes the plane being edited and `shapes`/`edges`/`shape`/`add_shape`/`add_edge`/`plane`/`plane_mut` work on it. `root_plane`, `plane_index(sub)`, `plane_owner` and `open_plane(Option<&id>)` navigate planes (unknown owner falls back to root).
- WHEN an element is removed THE SYSTEM SHALL drop its DI from every plane and drop the planes of removed sub-processes.
- Connections (`SequenceFlow`, `MessageFlow`, `Association`, `DataInputAssociation`, `DataOutputAssociation`) MUST expose `(source, target)` via `ElementKind::endpoints`.

## Decisions

- Single `ElementKind` enum with per-variant attributes instead of a trait hierarchy; `Process` and `Collaboration` are kinds too, containment via `Element::parent`.
- Elements stored in `indexmap::IndexMap` for O(1) lookup plus stable order (round-trip friendly).
- Incoming/outgoing are computed by scanning connections, not cached; optimize only when profiling demands.
- Sequence-flow default flag lives on the flow (`is_default`); the XML layer maps it to the source's `default` attribute.
- Optional round-trip attributes (`isInterrupting`, `isExecutable`, event refs, timer, condition, `associationDirection`, `isForCompensation`, lane set id) live in `Element::details: ElementDetails`, not in `ElementKind`, so kind constructors stay unchanged.
- Root `message`/`signal`/`error`/`escalation` live in `Definitions::root_elements` (`RootElement`, `RootKind`); event refs point at them by id. Original DI ids live in `Plane::di_ids`; `remove_element` drops them too.
- Active plane lives in the model (not the view) so commands, rules and factories stay plane-agnostic; the command stack records the plane per entry and undo/redo re-open it.
- `Lane` has no default size (bpmn-js sizes lanes from the participant); removal does not cascade to children or attached boundary events yet.

## Files

- `src/lib.rs` — library root, exposes `model`.
- `src/model/mod.rs` — module root and public re-exports.
- `src/model/id.rs` — `ElementId`.
- `src/model/kind.rs` — `ElementKind`, `EventDefinition`, `endpoints`, `default_size`.
- `src/model/details.rs` — `ElementDetails`, `TimerExpression`, `AssociationDirection`, `RootElement`, `RootKind`.
- `src/model/di.rs` — `Bounds`, `Waypoint`, `Shape`, `Edge`, `Plane`, `Diagram`.
- `src/model/definitions.rs` — `Element`, `Definitions` and its operations.
- `src/model/tests.rs` — unit tests.
