---
human_revised: false
name: io
summary: GPUI-independent BPMN 2.0 XML import/export (quick-xml) mapping XML to the mangue model, bpmn-js compatible output.
depends-on: [modeler/model.md]
relates: []
targets: [platform]
---

# IO

## Overview

`mangue::io` reads and writes BPMN 2.0 XML for the [model](model.md), with no
GPUI dependency. Parsing uses `quick-xml`'s `NsReader` into a small
namespace-resolved tree, then maps it to `Definitions`. Export writes the same
shape bpmn-js produces (https://github.com/bpmn-io/bpmn-js), so files open in
bpmn.io tools.

API: `import_xml(&str) -> Result<Definitions, ImportError>`,
`export_xml(&Definitions) -> String`, `new_diagram() -> Definitions`.

## Requirements (EARS / RFC 2119)

- Import MUST match elements by namespace URI (BPMN MODEL, BPMNDI, DD DC, DD DI), accepting any prefix and default-namespace documents.
- WHEN the root is not `definitions` in the BPMN MODEL namespace THE SYSTEM SHALL return `ImportError::NotDefinitions`.
- WHEN the XML is malformed THE SYSTEM SHALL return `ImportError::Xml`.
- WHEN an element or DI entry is unknown or unmodeled THE SYSTEM SHALL skip it (and any DI pointing at it) without failing.
- WHEN a gateway or activity has a `default` attribute THE SYSTEM SHALL set `is_default` on that sequence flow; export MUST write it back as `default`.
- WHEN a lane lists a node in `flowNodeRef` THE SYSTEM SHALL set the node's `parent` to that lane (innermost lane wins); export MUST rebuild `laneSet`/`childLaneSet` and `flowNodeRef` from it.
- Data input associations MUST import with the owning node as target; data output associations with the owning node as source.
- Export MUST use prefixes `bpmn`, `bpmndi`, `dc`, `di`, `xsi`, set `targetNamespace="http://bpmn.io/schema/bpmn"` and `exporter="mangue"`, and write `bpmn:incoming`/`bpmn:outgoing` children on flow nodes.
- An import -> export -> import cycle MUST yield an equal `Definitions` (elements, kinds, names, documentation, `ElementDetails`, root elements, DI bounds, labels, waypoints and DI ids).
- Import/export MUST preserve `isInterrupting` (start events), `cancelActivity` (boundary events), `isExecutable` (processes), `isForCompensation` (activities), `isHorizontal` (pool and lane shapes, vertical ones included), `associationDirection`, event definition `messageRef`/`signalRef`/`errorRef`/`escalationRef`, timer `timeDate`/`timeDuration`/`timeCycle`, conditional `condition` text, root `message`/`signal`/`error`/`escalation` (id, name), `laneSet` ids and DI ids.
- Exported XML MUST parse in `bpmn-moddle` (pinned in `tools/validate`) with no warnings; `tests/moddle_validate.rs` (the `vertical-pool` fixture included) checks every fixture and `new_diagram`, skipping when `node` or `tools/validate/node_modules` is missing.
- Import MUST read every `BPMNDiagram` (first = root, later ones = sub-process planes, dropped when their `bpmnElement` is unknown); export MUST write them all in order. Missing ids default to `BPMNDiagram_1`/`BPMNPlane_1` (root) and `BPMNDiagram_<sub>`/`<sub>_plane` (sub-process plane, like the bpmn-js plane root id). Covered by `tests/fixtures/collapsed-subprocess.bpmn` (round trip + bpmn-moddle).
- `new_diagram` MUST return the bpmn-js initial diagram: `Process_1` with `StartEvent_1` at (173, 102), 36x36.
- WHEN an imported collapsed sub-process has no `BPMNDiagram` of its own THE SYSTEM SHALL create its plane and move the DI of its content from the plane it is drawn on there, top-left at (180, 160) (bpmn-js `SubprocessCompatibility`, `modeling::compat_planes`).

## Decisions

- Parse to an owned tree first, then map: simpler than a streaming state machine; BPMN files are small.
- Unqualified attributes only; `xsi:type` and vendor attributes are dropped.
- Export synthesizes ids for unmodeled wrappers: `DataObject_<ref>`, `Property_<activity>` (bpmn-js `__targetRef_placeholder`), `<Kind>EventDefinition_<event>`; `LaneSet_<owner>` and `<element>_di` only when no original id was imported (`ElementDetails::lane_set_id`, `Plane::di_ids`, which holds only ids differing from `<element>_di`).
- `isExecutable`, `isInterrupting`, `isForCompensation` and `associationDirection` are written only when imported/set; `new_diagram` sets `isExecutable="false"` and DI id `_BPMNShape_StartEvent_2` like bpmn-js.
- Root elements export after collaboration and processes, before the diagram.
- Known gaps (lost on round trip): event definition ids, `errorCode`/`escalationCode`/`structureRef`, `categoryValueRef`, `extensionElements`, nodes in more than one lane, `transaction`/`adHocSubProcess`.

## Files

- `src/io/mod.rs` — module root and public re-exports.
- `src/io/error.rs` — `ImportError`.
- `src/io/ns.rs` — namespace URIs and event-definition tag table.
- `src/io/tree.rs` — namespace-resolved XML tree (`NsReader`).
- `src/io/import.rs` — `import_xml`.
- `src/io/export.rs` — `export_xml` and its indenting writer.
- `src/io/new_diagram.rs` — `new_diagram`.
- `tests/io_roundtrip.rs` — integration tests over `tests/fixtures/*.bpmn`.
- `tests/moddle_validate.rs` — runs `tools/validate/validate.mjs` on exported XML.
- `tools/validate/` — Node project pinning `bpmn-moddle` 10.3.1 (`npm ci`), `validate.mjs <file>` exits non-zero on warnings or errors.
