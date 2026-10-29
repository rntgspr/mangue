---
human_revised: false
plan: maintenance-homebrew-cask
task: T3
status: complete
date: 2026-10-29
summary: Hand-off for maintenance-homebrew-cask / T3 - README brew install docs and the gengar hand-off.
---

# Hand-off — maintenance-homebrew-cask / T3

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`README.md`](../../../README.md) | modified — macOS section: `brew install --cask rntgspr/tap/mangue` above the manual DMG steps |
| [`handoff-gengar.md`](handoff-gengar.md) | created — final cask, tap README section, per-release bump steps |
<!-- /cumaru:touched -->

## Decisions made during implementation

- Tap README snippet follows the existing `### name` / one-liner / bash block / "Project & source" style, under a new `## Casks` heading.
- Not delivered via Alter Bridge: MCP server unreachable (ECONNREFUSED). Lead delivers the file.

## Commands run / verification

- README rendered by inspection; command matches the tap name `rntgspr/tap` (tap README: `brew tap rntgspr/tap`).
- The README command works only after gengar publishes the cask.

## Pending / follow-ups

- Delivery to gengar.

## Suggestions for the Lead

- None.
