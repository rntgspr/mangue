---
human_revised: false
plan: maintenance-homebrew-cask
task: T1
status: complete
date: 2026-10-29
summary: Hand-off for maintenance-homebrew-cask / T1 - SHA256SUMS step added to the release job.
---

# Hand-off — maintenance-homebrew-cask / T1

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) | modified — `Write SHA256SUMS` step in `release`, between download-artifact and action-gh-release |
<!-- /cumaru:touched -->

## Decisions made during implementation

- Explicit globs (`Mangue-*.dmg mangue-*.zip mangue-*.tar.gz`) instead of `*`, so `SHA256SUMS` never lists itself; it is then attached by the existing `files: dist/*`.
- README not touched in T1 (listed in `files:` but nothing to document; T3 owns README).

## Commands run / verification

- `ruby -ryaml` load of ci.yml — parses; release steps: `actions/download-artifact@v4`, `Write SHA256SUMS`, `softprops/action-gh-release@v2`.
- `gh release download v0.1.0` into the scratchpad, then the same command (`sha256sum Mangue-*.dmg mangue-*.zip mangue-*.tar.gz`):

```text
48d2bfd0707b405f93234c8bbbedc247153a1d94e8ef5cd987b1c4e795aeb068  Mangue-0.1.0.dmg
13c732cbddb646e380f02f5dff25b747f2ae4b98eb3d426a560cc4386501f207  mangue-0.1.0-windows-x86_64.zip
fe8da887f1d8e4019d41ac5409991e0c0be19299ca153938624f59967bea54e2  mangue-0.1.0-linux-x86_64.tar.gz
```

- Not run in CI yet; first proof is the next `v*` tag.

## Pending / follow-ups

- v0.1.0 has no SHA256SUMS (released before this step); not backfilled (no release changes allowed).

## Suggestions for the Lead

- None.
