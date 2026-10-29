---
human_revised: false
name: code-editor
summary: Per-document code mode showing the BPMN XML in gpui-component's code editor, with sync into the model on leave, invalid-XML banner and history kept apart from the canvas.
depends-on: [canvas, io, modeling]
relates: []
targets: [platform]
---

# Code editor

## Overview

Each document can switch between preview mode (the diagram canvas, see
[canvas](canvas.md)) and code mode, which shows the document's BPMN XML in
`gpui-component`'s code editor (`EditorState` / `Editor`). The XML shown is
`io::export_xml` of the model; on leaving code mode the edited text is parsed
with `io::import_xml` (see [io](io.md)) and, when accepted, replaces the
document's definitions. A code sync is not a canvas command: it keeps the
canvas undo history and invalidates only the entries it touched (see
[modeling](modeling.md)).

The mode button lives in the document header (`DocHeader`, see
[canvas](canvas.md)); this concern owns the mode state, the editor lifecycle,
the sync and the invalid-XML banner.

## Requirements (EARS / RFC 2119)

### Mode

- Each tab MUST keep its own mode (`Document::code_mode`, default preview); switching tabs MUST NOT change any tab's mode.
- WHEN the mode button is clicked or cmd-b (`ToggleMode`) is pressed THE SYSTEM SHALL toggle the active document's mode between preview (the canvas) and code (its BPMN XML in a code editor), one at a time.
- The mode button MUST read the mode a click switches to: "Code" in preview mode, "Preview" in code mode.

### Editor

- WHEN a document enters code mode THE SYSTEM SHALL show `io::export_xml` of its model in `gpui-component`'s code editor (`code_view.rs`) in place of the canvas, highlighted as markup, readable and editable like a normal code editor.
- The editor MUST be created and focused on the first code-mode render and dropped once its text is synced or reverted; `init_editor` (gpui-component globals) MUST run once at app start and in test setup before the first code-mode render.

### Sync on leave

- WHEN a document leaves code mode with text equal to the export of its model THE SYSTEM SHALL leave the model untouched and drop the editor.
- WHEN a document leaves code mode with text `io::import_xml` accepts THE SYSTEM SHALL replace the definitions with the imported ones, mark the document dirty, drop the editor, clear the selection and per-plane viewports, reset the active plane and fit the diagram.
- WHEN a document leaves code mode with text `io::import_xml` rejects THE SYSTEM SHALL show the canvas with the model unchanged and a banner over it (`XmlError`) reading "Invalid XML: <import error>" with "Revert" (drop the code edits; the model stays as it was on entering code mode) and "Back to code" (return to code mode with the edited text intact); re-entering code mode while the banner shows behaves as "Back to code".
- WHEN a document leaves code mode THE SYSTEM SHALL focus the canvas, so canvas shortcuts (cmd-z, cmd-shift-z) apply immediately.

### History

- The code editor and the canvas MUST keep separate undo/redo histories: the editor's is its own and a code sync is not a canvas command, so canvas undo never reverts code edits; while code mode is open the canvas cannot change, so no editor-side conflict exists.
- WHEN a code sync changes content a canvas undo/redo entry touched THE SYSTEM SHALL make undo/redo of that entry a no-op; untouched entries keep working (`CommandStack::invalidate`, see [modeling](modeling.md)).

## Decisions

- In-app validation of edited XML is `io::import_xml` (`ImportError`) only; bpmn-moddle is JavaScript (needs `node`) and stays a test-time check (`tests/moddle_validate.rs`). User decision, 2026-10-21.
- A code sync renumbers every revision id in the dirty-state stack so none matches the saved one: the document stays dirty after undoing back past the sync.
- The editor uses `gpui-component`'s `html` language for highlighting; it has no dedicated XML grammar enabled (`tree-sitter-html` feature in `Cargo.toml`).

## Files

- `src/canvas/code_view.rs` — `init_editor` and `new_editor` (code editor state holding the XML).
- `src/canvas/xml_error.rs` — `XmlError` invalid-XML banner (message, Revert, Back to code).
- `src/canvas/view_tabs.rs` — `toggle_mode`, `leave_code_mode`, `replace_definitions`, `revert_code`, `back_to_code` and the code-mode `body`.
- `src/canvas/document.rs` — `Document::code_mode`, `editor`, `xml_error`.
- `src/canvas/doc_header.rs` — mode button `label`.
- `src/canvas/view_tests_code.rs` — headless code-mode tests (toggle, sync, invalid XML, revert, back to code, history).
- `src/canvas/view_tests_tabs.rs` — per-tab mode test.
