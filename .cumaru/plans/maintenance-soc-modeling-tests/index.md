---
human_revised: false
scope:
  - modeler/modeling.md
status: in-progress
summary: Move misplaced modeling tests into the test file that owns their concern (layout, factory, rules, orthogonal routing).
targets: [platform]
aux: []
---

# Separation of concerns: modeling test files by concern

## Overview

The separation-of-concerns audit of `src/modeling` (2026-10-26) found tests
filed under a module whose header declares a different concern (all low
severity):

- `tests.rs:371-411` (`manhattan_routes`, `dock_and_straight`): layout geometry in the command-stack file -> `tests_layout.rs`.
- `tests_layout.rs:252-305` (`node`, `command_applies_rule_by_kind`): `factory::connect` integration in the layout-table file -> `tests_factory.rs`.
- `tests_arrange.rs:227-281` (`annotation`, `orthogonal`, `associations_route_orthogonally`): association routing in the arrange file -> `tests_orthogonal.rs`.
- `tests_vertical.rs:243-298` (`compensation_association_follows_boundary_layout`): factory compensation bookkeeping in the vertical-layout file -> `tests_factory.rs`.
- `tests_factory.rs:113-191` (`rule`, `rules_sequence_flow`, `rules_message_flow`, `rules_associations`, `container_hit_test`): rules tests in the factory file -> new `tests_rules.rs` (`can_connect`, `container_at`); `tests_refusal.rs` stays scoped to refusal rules.

Tests move verbatim; no assertion changes.

## Acceptance Criteria (EARS / RFC 2119)

- Every moved test MUST keep its name and assertions unchanged.
- Each test file's header comment MUST match the concern of the tests it contains.
- The total number of tests reported by `cargo test` MUST be unchanged before and after.
- `cargo test` MUST pass.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Move misplaced tests to their concern's file | done | — |

## Out of scope

- Splitting the deliberate `tests_parity.rs` bucket.
- Sharing `pt` / `b` / `node` helpers across test files.
- `auto_resize.rs` test of private `optimal` (kept; cheap geometry check).

## Risks

- Runs last among modeling plans: after `maintenance-soc-modeling-command` (`tests_vertical.rs` imports), `maintenance-soc-io` (`tests_orthogonal.rs`) and `maintenance-soc-canvas-domain` (adds a `reconnect_refusal` test to `tests_refusal.rs`). Global order: `.cumaru/notepad.md`.
