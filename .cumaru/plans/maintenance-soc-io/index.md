---
human_revised: false
scope:
  - modeler/io.md
  - modeler/modeling.md
  - modeler/canvas.md
status: in-progress
summary: Remove the io -> modeling dependency (compat_planes on import, orthogonalize_all on load) and compose load and save in one application-layer home, out of canvas/view.rs.
targets: [platform]
aux: []
---

# Separation of concerns: io must not depend on modeling

## Overview

The separation-of-concerns audit of `src/io` (2026-10-26) found the XML layer
reaching up into `modeling`, against the declared direction (`io.md`
`depends-on: [modeler/model.md]`):

- `src/io/import.rs:57` — `import_xml` calls `crate::modeling::compat_planes`
  (bpmn-js `SubprocessCompatibility`), defined in `src/modeling/command.rs:595`
  and sharing `split_plane` with `ToggleCollapse`.
- `src/io/load.rs:11` — `load` calls `crate::modeling::layout::orthogonalize_all`.

Both are post-import model normalization policies, not XML mapping. They move
to the application layer that already depends on both `io` and `modeling`, so
`io` parses and `modeling` normalizes, with no cycle. Observable behavior (what a
user sees after opening a file or syncing code mode) stays the same.

Saving is the mirror image: `src/canvas/view.rs:1636-1641` (`write`) runs
`modeling::ordering::order_planes` + `io::export_xml` + `std::fs::write` inside
the view. The same composition home owns it, so load and save are composed in
one place (moved here from `maintenance-soc-canvas-io` by the 2026-10-26
overlap review).

The same audit noted `load.rs:9` formatting `ImportError` with `Debug` while it
implements `Display`; the doc comment promises a displayable error.

## Acceptance Criteria (EARS / RFC 2119)

- `src/io` MUST NOT reference `crate::modeling` (`rg -n "modeling" src/io` returns nothing).
- `src/modeling` MUST NOT reference `crate::io` outside tests.
- WHEN a BPMN file is opened (startup argument, Open, Open Recent) THE SYSTEM SHALL produce the same `Definitions` as today: sub-process planes split (`compat_planes`) and edges orthogonalized (`orthogonalize_all`).
- WHEN code mode syncs accepted XML THE SYSTEM SHALL apply `compat_planes` to the imported definitions, as today, and SHALL NOT orthogonalize them (current behavior).
- Writing a document (plane ordering, XML export, file write) MUST live in the same composition home as loading; `canvas/view.rs` MUST NOT call `std::fs::write` and keeps only the dialog/alert flow.
- WHEN a document is saved or saved as THE SYSTEM SHALL write the same bytes as today.
- WHEN a file fails to import THE SYSTEM SHALL report the `ImportError` `Display` text, not its `Debug` form.
- `cargo test` MUST pass with no test weakened or deleted.
- `io.md` and `modeling.md` MUST state where post-import normalization lives (absorbed from the delta draft).

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Move post-import normalization out of io | done | — |

## Out of scope

- Moving `compat_planes` / `split_plane` into `src/model` (they share undo-aware plane logic with `ToggleCollapse`).
- Splitting `export.rs`'s private `Xml` writer, renaming `ns.rs`, moving `new_diagram` (audit: compliant, no change).
- Audits of the other `src/` directories (separate plans per directory).

## Risks

- Missing a caller of `import_xml` that relies on `compat_planes` (18 call sites outside `src/io`, mostly tests) silently changes plane structure; mitigated by routing every non-io caller through the new helper and the plane tests (`src/modeling/tests_planes.rs`, `src/canvas/view_tests_planes.rs`).
- The `Display` change alters the user-facing error text; no test asserts the `Debug` form today (verify).
- Overlaps `maintenance-soc-canvas-document` in `src/canvas/view_tabs.rs`: this plan only re-points the `import_xml` call inside `leave_code_mode`; `replace_definitions` belongs to `canvas-document`, which runs after this plan.
- Overlaps `maintenance-soc-modeling-tests` in `src/modeling/tests_orthogonal.rs` (merge only); this plan runs first.
- Runs before `maintenance-soc-modeling-command` (`compat_planes`). Global order: `.cumaru/notepad.md`.
