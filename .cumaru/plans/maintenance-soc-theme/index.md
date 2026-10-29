---
human_revised: false
scope:
  - modeler/theme.md
status: in-progress
summary: Remove the two module cycles inside src/theme (schema <-> builtin via Theme::default, library <-> user via merge/take_name).
targets: [platform]
aux: []
---

# Separation of concerns: acyclic theme modules

## Overview

The separation-of-concerns audit of `src/theme` (2026-10-26) found two
two-way module dependencies, both verified with `rg`:

- `schema.rs:8` imports `builtin::{LIGHT, builtin}` for `impl Default for Theme`
  (`schema.rs:121-125`), while `builtin.rs:6` imports `schema::Theme`. The data
  schema reaches the embedded-asset loader.
- `library.rs:10` imports `user::{merge, take_name}` (YAML mapping helpers
  defined at `user.rs:140,150`), while `user.rs:11` imports
  `library::{Library, load_dir, resolve}`. The lower-level resolver depends on
  the higher-level user loader.

Both are small moves with no behavior change.

## Acceptance Criteria (EARS / RFC 2119)

- `schema.rs` MUST NOT import `builtin` (`rg -n "super::builtin" src/theme/schema.rs` empty); `Theme::default()` MUST still return the built-in light theme.
- `library.rs` MUST NOT import `user` (`rg -n "super::user" src/theme/library.rs` empty); `merge` and `take_name` MUST have one definition, owned by the lower-level module that both files use.
- `src/theme` MUST still not reference `canvas` or `modeling`.
- `cargo test` MUST pass with no test weakened or deleted.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Break schema/builtin and library/user cycles | done | — |

## Out of scope

- `Theme::current` / `impl Global` on the schema type (consistent GPUI global convention, ~30 callers).
- `NAMES` hand-kept list vs `assets/themes/` (no drift today; KISS).
- `schema::fill_for` depending on `model::ElementKind` (theme-owned kind-to-slot policy, one-way).

## Risks

- None material; pure moves inside one private module tree.
