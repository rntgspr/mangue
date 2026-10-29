---
human_revised: false
scope:
  - modeler/canvas.md
status: in-progress
summary: Give TextInput one explicit operation for the paint-time layout cache that text_element.rs writes into its fields.
targets: [platform]
aux: []
---

# Separation of concerns: TextInput owns its layout cache

## Overview

The separation-of-concerns audit of `src/canvas` (2026-10-26) found
`src/canvas/text_element.rs:233-237` writing `input.layout`, `input.bounds`,
`input.line_height` straight into `TextInput` fields (`text_input.rs:105-111`,
`pub(super)`) during paint; hit-testing (`index_at`, `range_bounds`) reads them
back. Writing the cache during paint is the normal GPUI element pattern; the
issue is that the state transition has no owner.

## Acceptance Criteria (EARS / RFC 2119)

- `TextInput` MUST expose one method (e.g. `record_layout(lines, bounds, line_height)`) that `text_element.rs` calls; `layout`, `bounds`, `line_height` MUST be private to `text_input.rs`.
- WHEN the user clicks, drags or uses IME in the label editor, search box or append menu THE SYSTEM SHALL behave as today (`view_tests_text.rs` green).
- `cargo test` MUST pass.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Add TextInput::record_layout | done | — |

## Out of scope

- `text_input <-> text_element` module cycle (standard GPUI element/entity pairing).
- `TextEdit` `pub marked` write at `text_input.rs:459` (low).

## Risks

- None material.
