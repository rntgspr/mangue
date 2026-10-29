---
human_revised: false
plan: maintenance-soc-theme
task: T1
status: complete
date: 2026-10-26
summary: Theme module cycles removed and verified.
---

# Hand-off — maintenance-soc-theme / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/theme/schema.rs`](../../../src/theme/schema.rs) | Removed the built-in dependency and default implementation. |
| [`src/theme/builtin.rs`](../../../src/theme/builtin.rs) | Owns the default implementation. |
| [`src/theme/library.rs`](../../../src/theme/library.rs) | Owns the mapping helpers. |
| [`src/theme/user.rs`](../../../src/theme/user.rs) | Imports the mapping helpers from the library. |
<!-- /cumaru:touched -->

## Decisions made during implementation

- None beyond the task contract.

## Commands run / verification

- `cargo test theme::` — 18 passed after both moves.
- `cargo test` — the IO agent reports 318 unit, 17 roundtrip and 3 moddle tests passed after concurrent edits stabilized.
- `rg` checks for `super::builtin` in schema, `super::user` in library, and `crate::canvas` or `crate::modeling` in theme returned no matches; `merge` and `take_name` each have one definition in library.
- The first full test run during concurrent IO edits failed in `modeling::tests_planes::import_moves_legacy_content_to_plane`; a subsequent theme test attempt encountered temporary IO compilation errors. Both were resolved by the IO agent before the reported consolidated run.

## Pending / follow-ups

- Absorb the theme ownership update into the living spec during plan close.

## Suggestions for the Lead

- None.
