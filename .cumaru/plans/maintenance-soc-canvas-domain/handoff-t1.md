---
human_revised: false
plan: maintenance-soc-canvas-domain
task: T1
status: complete
date: 2026-10-26
summary: Modeling owns attachment lookup, connection rules, move composition, and command introspection.
---

# Hand-off — maintenance-soc-canvas-domain / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`src/modeling/attach.rs`](../../../src/modeling/attach.rs) | Provides shared topmost attachment host lookup. |
| [`src/modeling/rules.rs`](../../../src/modeling/rules.rs) | Owns reconnect refusal, connection start rule, and attachment padding. |
| [`src/modeling/arrange.rs`](../../../src/modeling/arrange.rs) | Composes selection moves and participant membership refreshes. |
| [`src/modeling/command.rs`](../../../src/modeling/command.rs) | Owns command creation introspection. |
| [`src/modeling/mod.rs`](../../../src/modeling/mod.rs) | Exposes introspection and registers rule tests. |
| [`src/modeling/tests_refusal.rs`](../../../src/modeling/tests_refusal.rs) | Covers reconnect refusals. |
| [`src/modeling/tests_rules.rs`](../../../src/modeling/tests_rules.rs) | Covers connection start and topmost attachment host. |
| [`src/canvas/view.rs`](../../../src/canvas/view.rs) | Uses modeling move and attachment operations. |
| [`src/canvas/view_feedback.rs`](../../../src/canvas/view_feedback.rs) | Uses reconnect refusal. |
| [`src/canvas/view_aids.rs`](../../../src/canvas/view_aids.rs) | Uses modeling command introspection. |
| [`src/canvas/context_pad.rs`](../../../src/canvas/context_pad.rs) | Uses modeling connection start rule. |
<!-- /cumaru:touched -->

## Decisions and verification

- `mod.rs` was added to the task contract to expose the introspection functions. `tests_rules.rs`, previously planned for wave 5, was created here to house the new non-refusal tests.
- The attachment padding is shared across host lookup, boundary orientation and refusal; its definition replaces the literal-count check while retaining the single-owner criterion.
- `cargo test -q`: 326 unit, 17 roundtrip and 3 moddle tests passed; `cargo build -q` and `git diff --check` passed.
- The new rule tests initially failed to compile because the APIs did not exist; this was not a behavioral red. The host lookup test was added after extraction and was not observed failing before it.
- Plan close and specification absorption remain pending.
