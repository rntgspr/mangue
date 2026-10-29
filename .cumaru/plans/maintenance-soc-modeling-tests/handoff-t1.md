---
human_revised: false
plan: maintenance-soc-modeling-tests
task: T1
status: complete
date: 2026-10-26
summary: Modeling tests were grouped by their owning concern without changing their coverage.
---

# Hand-off — maintenance-soc-modeling-tests / T1

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/modeling/tests.rs`](../../../src/modeling/tests.rs) | Moved layout geometry tests. |
| [`src/modeling/tests_layout.rs`](../../../src/modeling/tests_layout.rs) | Owns layout geometry tests. |
| [`src/modeling/tests_arrange.rs`](../../../src/modeling/tests_arrange.rs) | Moved association routing test. |
| [`src/modeling/tests_vertical.rs`](../../../src/modeling/tests_vertical.rs) | Moved compensation factory test. |
| [`src/modeling/tests_factory.rs`](../../../src/modeling/tests_factory.rs) | Owns factory integration tests and moved rules tests. |
| [`src/modeling/tests_orthogonal.rs`](../../../src/modeling/tests_orthogonal.rs) | Owns association routing test. |
| [`src/modeling/tests_rules.rs`](../../../src/modeling/tests_rules.rs) | Owns connection and container rules tests. |
<!-- /cumaru:touched -->

## Verification

- Baseline `cargo test`: 326 unit, 17 roundtrip, 3 moddle passed.
- After all moves `cargo test -q`: 326 unit, 17 roundtrip, 3 moddle passed; no unused import warnings.
- `git diff --check` passed.
- Tests retained names and assertions; fixture helpers were copied or renamed as required by their new modules. Existing rules tests from wave 4 were preserved.

## Pending

- Update test ownership in the modeling spec during absorption.
