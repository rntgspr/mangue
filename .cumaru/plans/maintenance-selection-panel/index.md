---
human_revised: false
scope:
  - modeler/canvas
  - modeler/modeling
  - modeler/parity
status: drafting
summary: Add a right-side selection panel to the canvas showing a per-kind set of options (properties and actions) for the selected element, or batch actions for many.
targets: [platform]
aux: []
---

# Selection panel on the right of the canvas

## Overview

The canvas offers selection-dependent actions only as floating affordances: the
context pad next to a single shape, the replace menu, and the Arrange menu for
align/distribute. Attributes the model already imports and exports
(`ElementDetails`, `SequenceFlow { is_default, condition }`, `documentation`)
have no editing surface at all.

This plan adds a fixed-width panel on the right of the canvas surface (mirror of
the left folder `Sidebar`). **Each element kind has its own option set**: the
properties and actions that make sense for it. The panel renders the option set
of the current selection; with several elements selected it shows batch actions.

It is a mangue addition, not a bpmn-js core module (the bpmn-js properties panel
is the separate `bpmn-js-properties-panel` package,
https://github.com/bpmn-io/bpmn-js-properties-panel), so it extends `parity.md`
Decisions instead of the feature table.

Design rules:

- The catalog `kind -> option set` is a pure function of `(Definitions, selection)`, GPUI-independent and unit-tested, following the `context_pad::entries` pattern. It is the single home of "which option applies to which kind".
- Options only edit attributes the model already has; no new model field.
- Actions reuse existing commands (`ReplaceElement`, `ToggleCollapse`, lane tools, `arrange::align` / `distribute`, `DeleteElements`, `UpdateLabel`). Property edits go through one new command, `UpdateProperties`, modeled on bpmn-js `UpdatePropertiesHandler` (https://github.com/bpmn-io/bpmn-js/blob/v17.11.1/lib/features/modeling/cmd/UpdatePropertiesHandler.js), with exact undo via the existing element snapshot.

### Option catalog (proposal)

Every single element: **name** (text; annotation edits `text`), **id** (read-only), **documentation** (multi-line text).

| Kind | Options |
|---|---|
| Process (empty selection) | `isExecutable` (toggle); element count (info). |
| Collaboration (empty selection) | element count (info). |
| Participant | process ref (read-only); lane tools: add above/below, split 2/3. |
| Lane | lane tools: add above/below, split 2/3. |
| Start event | event type (select -> replace); `isInterrupting` (toggle, only inside an event sub-process); event detail (below). |
| Intermediate catch/throw, end event | event type (select -> replace); event detail. |
| Boundary event | event type (select -> replace); interrupting (`cancel_activity`, toggle); host (read-only); event detail. |
| Event detail | timer: kind (date/duration/cycle) + expression; conditional: condition; message/signal/error/escalation: `event_ref` (read-only). |
| Task, typed tasks, call activity | task type (select -> replace); `isForCompensation` (toggle). |
| Sub-process | event sub-process (`triggered_by_event`, toggle -> replace); collapse/expand; open plane (drill-down); `isForCompensation`. |
| Exclusive / inclusive / complex gateway | gateway type (select -> replace); default flow (select among outgoing sequence flows). |
| Parallel / event-based gateway | gateway type (select -> replace). |
| Sequence flow | condition (text); default (toggle, only when the source is an exclusive/inclusive/complex gateway or an activity); source -> target (read-only). |
| Message flow, data input/output association | source -> target (read-only). |
| Association | direction (none / one / both). |
| Data object, data store, group, text annotation | common options only. |
| Many selected | count by kind; align left/center/right/top/middle/bottom; distribute horizontally/vertically; delete. |

Every single-element set ends with **delete** (except root).

## Acceptance Criteria (EARS / RFC 2119)

- WHILE a document is open in canvas mode and the panel is visible THE SYSTEM SHALL show a fixed-width panel to the right of the canvas surface, themed from the same tokens as the folder sidebar.
- WHEN the View "Toggle Inspector" action runs THE SYSTEM SHALL show or hide the panel; it MUST be visible by default and MUST NOT appear in code mode or in the empty state.
- WHEN the selection is empty, one element, or several elements THE SYSTEM SHALL show exactly the option set the catalog defines for that selection, and no option the catalog excludes for that kind.
- The kind -> option set catalog MUST live in one GPUI-independent function with a unit test per catalog row.
- WHEN an action option is used THE SYSTEM SHALL apply exactly the same command as the equivalent context pad entry or Arrange menu item, as one undoable step.
- WHEN a property option is committed with a changed value THE SYSTEM SHALL apply one `UpdateProperties` command; one undo SHALL restore the element exactly.
- WHEN a sequence flow is made default (from the flow or from its gateway) THE SYSTEM SHALL clear `is_default` on every other outgoing sequence flow of the same source in the same command.
- WHEN a property edited in the panel is exported and re-imported THE SYSTEM SHALL read back the same value.
- WHEN a text field is committed (Enter, or focus leaving it; shift+Enter for a newline in multi-line fields) with a changed value THE SYSTEM SHALL commit it; Escape SHALL restore the current value without a command.
- WHILE a panel field has focus, canvas key bindings (Delete, Backspace, letters, arrows) MUST NOT act on the selection.
- WHEN the selection, the active tab, the drill-down plane or the document changes (including undo/redo) THE SYSTEM SHALL re-derive the panel in the same frame and discard uncommitted field edits.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Pure per-kind option catalog | pending | — |
| [T2](t2.md) | `UpdateProperties` command with exact undo | pending | — |
| [T3](t3.md) | Panel element, layout slot and toggle action (read-only) | pending | T1 |
| [T4](t4.md) | Action, toggle and select options wired to commands | pending | T2, T3 |
| [T5](t5.md) | Text options with focus isolation | pending | T4 |

T1 and T2 may run in parallel (disjoint files). T3-T5 are sequential: all touch `src/canvas/view.rs` and the panel files.

## Out of scope

- Managing root definitions (messages, signals, errors, escalations): `event_ref` is read-only.
- Engine-specific properties (Camunda/Zeebe extension elements, input/output mappings, forms, listeners).
- Editing the id; multi-element property editing.
- Adding model fields not already in `ElementKind` / `ElementDetails` (e.g. `loopCharacteristics`, `completionQuantity`).
- Persisting panel visibility or width in `settings.yaml`; resizing the panel.
- Removing the context pad or the Arrange menu; the panel complements them.

## Risks

- **Concurrent plans.** Eleven uncommitted `maintenance-soc-*` plans restructure `src/canvas/view.rs`, `document.rs`, `text_input.rs` and split `src/modeling/command.rs` (`soc-modeling-command`). Start T2 after `soc-modeling-command` and T3 after the `soc-canvas-*` plans are absorbed, or rebase the task contracts on their result.
- **Catalog drift (DRY).** Replace, collapse and lane-tool applicability already live in `context_pad::entries` and `view_planes::collapse_state`; T1 must reuse them, not re-encode them.
- **Replace vs. property.** Event type, task type, gateway type and event sub-process are kind changes, so they go through `ReplaceElement` (connections it no longer allows are deleted); only attributes inside a kind go through `UpdateProperties`.
- **Canvas width shrinks.** The panel must sit outside `surface` so `apply_fit`, `origin`/`size` and `palette::covers` keep the surface's bounds.
- **Focus contention.** A key leaking from a panel field to the canvas would delete or move the selection; T5 proves isolation with headless tests.
- **Panel length.** Event elements carry the most options; the body must scroll.
- **Kit choice.** If `maintenance-gpui-component-chrome` T1 (theme bridge, `Root`) lands first, build the panel controls with gpui-component `Input`, `Select`, `Switch` and `Button` instead of hand-built controls and `TextInput`; T3-T5 shrink accordingly.
