---
human_revised: false
plan: maintenance-soc-modeling-dry
task: T1
status: complete
date: 2026-10-26
summary: Fresh id allocation is shared by modeling builders, paste, and lanes.
---

# Hand-off — maintenance-soc-modeling-dry / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/modeling/factory.rs`](../../../src/modeling/factory.rs) | Owns the allocator and its exclusion test. |
| [`src/modeling/clipboard.rs`](../../../src/modeling/clipboard.rs) | Allocates paste ids through the shared function. |
| [`src/modeling/lanes.rs`](../../../src/modeling/lanes.rs) | Allocates distinct lane ids through the shared function. |
<!-- /cumaru:touched -->

## Decisions made during implementation

- Used a slice of reserved ids to preserve the existing allocation order and formats.

## Commands run / verification

- `cargo test --lib skips_existing_and_reserved_ids` failed with `Activity_2` instead of `Activity_3` using a provisional test-only allocator that ignored reservations, then passed after the implementation.
- `cargo test -q` passed: 323 unit, 17 roundtrip and 3 moddle tests.
- The task's `rg` check found the format only in `factory.rs`; `git diff --check` passed for the three source files.
- An initial general test invocation encountered a transient concurrent `canvas-io` compile error before that agent finished moving `url_to_path`; the later full suite passed.

## Pending / follow-ups

- Absorb the allocator ownership into the modeling specification when the plan closes.
