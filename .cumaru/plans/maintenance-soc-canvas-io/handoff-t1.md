---
human_revised: false
plan: maintenance-soc-canvas-io
task: T1
status: complete
date: 2026-10-26
summary: File system helpers moved from canvas and main into files.
---

# Hand-off — maintenance-soc-canvas-io / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/files/mod.rs`](../../../src/files/mod.rs) | Owns dialog directory, openable extension, and file URL helpers and tests. |
| [`src/files/tree.rs`](../../../src/files/tree.rs) | Owns mutable tree lookup. |
| [`src/canvas/view.rs`](../../../src/canvas/view.rs) | Calls the filesystem helpers. |
| [`src/canvas/sidebar.rs`](../../../src/canvas/sidebar.rs) | Removes the tree lookup helper. |
| [`src/canvas/view_tests.rs`](../../../src/canvas/view_tests.rs) | Moves filesystem helper tests to their owner. |
| [`src/main.rs`](../../../src/main.rs) | Calls the shared file URL decoder. |
<!-- /cumaru:touched -->

## Verification

- `cargo test -q files::`: 7 tests passed, including percent-decoding and non-file URLs.
- The new URL test was added after moving the existing implementation and was not observed failing first.
- Full `cargo test -q` during concurrent modeling work failed only in the other task's allocator test. After stabilization, 323 unit, 17 roundtrip and 3 moddle tests passed.
- The structural search for moved helpers in canvas and main returned no matches; `git diff --check` and `cumaru doctor` passed.

## Pending

- Absorb the filesystem ownership update into the canvas spec when the plan closes.
