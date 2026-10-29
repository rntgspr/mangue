---
human_revised: false
plan: maintenance-soc-io
task: T1
status: complete
date: 2026-10-26
summary: Implementation and verification evidence for moving IO normalization to canvas persistence.
targets: [platform]
---

# Hand-off — maintenance-soc-io / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/io/import.rs`](../../../src/io/import.rs) | Returns parsed definitions without modeling normalization. |
| [`src/io/load.rs`](../../../src/io/load.rs) | Loads raw definitions and formats import failures with Display. |
| [`src/canvas/persist.rs`](../../../src/canvas/persist.rs) | Composes normalized open, code import, and ordered save. |
| [`src/canvas/view.rs`](../../../src/canvas/view.rs) | Routes open and save through persistence composition. |
| [`src/canvas/view_tabs.rs`](../../../src/canvas/view_tabs.rs) | Routes code import through persistence composition. |
| [`src/main.rs`](../../../src/main.rs) | Uses persistence composition for startup loading. |
| [`src/modeling/tests_planes.rs`](../../../src/modeling/tests_planes.rs) | Normalizes imported fixtures explicitly in modeling tests. |
| [`tests/moddle_validate.rs`](../../../tests/moddle_validate.rs) | Uses app composition for legacy subprocess normalization. |
<!-- /cumaru:touched -->

- `canvas::persist` composes opening, code import, and ordered save; `io` returns raw imported definitions and formats import errors with Display.
- Modeling plane tests explicitly normalize imports. The legacy integration caller uses the application composition boundary; its file was added to the task contract.
- The legacy import test was observed failing before implementation (two planes instead of one) and passed in the subsequent suite.
- Consolidated `cargo test`: 320 unit tests, 17 roundtrip tests, and 3 moddle validation tests passed; doc tests passed.
- Structural searches found no `modeling` in `src/io`, no persistence dependency in `src/modeling`, and no canvas filesystem writes outside `persist.rs` and the existing test fixture in `view_tests.rs`.
- `canvas::persist::tests::open_normalizes_legacy_subprocess_and_edges` verifies a legacy single-plane fixture with a diagonal edge becomes two planes with orthogonal edges.
- `canvas::persist::tests::save_writes_ordered_export_bytes` verifies saved bytes equal the previous ordered export composition for an opened fixture.
- Both focused tests passed; they were added after the implementation and were not observed failing before it.
- TDD limitation: the two focused persistence tests were added after implementation and were not observed failing first; the earlier legacy integration test did fail before routing through the composition boundary.
