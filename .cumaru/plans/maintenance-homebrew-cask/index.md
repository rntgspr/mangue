---
human_revised: false
scope:
  - modeler
status: in-progress
summary: Distribute the macOS app through a Homebrew cask in rntgspr/homebrew-tap; mangue publishes checksums and the cask proposal, the tap owner (gengar) adds it.
targets: [platform]
aux: []
---

# Homebrew cask for mangue

## Overview

mangue ships a universal `Mangue-<version>.dmg` on GitHub Releases
(https://github.com/rntgspr/mangue/releases). A GUI app distributed as a DMG is
a Homebrew **cask** (`Casks/mangue.rb`), not a formula
(https://docs.brew.sh/Cask-Cookbook). The personal tap
`rntgspr/homebrew-tap` (local `~/agentic-workspace/gengar/homebrew-tap`) is
owned by the agent **gengar** and currently has only `Formula/`.

Ownership split:

- **mangue (this plan)**: publish SHA256 checksums with every release, author the cask proposal and the hand-off for gengar, document `brew install --cask` in the README.
- **gengar (tap project)**: add `Casks/mangue.rb` and the tap README entry. This plan never edits the tap repository; it delivers the change to gengar (Alter Bridge message when the bridge is available, otherwise the hand-off file in this plan).

Linux and Windows stay on the release tarball/zip; a Linux Homebrew formula is out of scope.

## Acceptance Criteria (EARS / RFC 2119)

- WHEN the release stage runs for a `v*` tag THE SYSTEM SHALL attach `SHA256SUMS` (one line per artifact) to the GitHub Release alongside the DMG, zip and tarball.
- The cask proposal MUST install `Mangue.app` from `https://github.com/rntgspr/mangue/releases/download/v#{version}/Mangue-#{version}.dmg`, with the real `sha256` of the `v0.1.0` DMG, `depends_on macos: ">= :ventura"`, a `livecheck` on GitHub releases and a `zap` stanza for `~/.config/mangue`.
- WHILE the DMG is not notarized THE cask MUST carry a caveat explaining the Gatekeeper workaround.
- The cask proposal MUST pass `brew style` and `brew audit --cask` run against a local temporary tap, without touching `rntgspr/homebrew-tap`.
- The README MUST document `brew install --cask rntgspr/tap/mangue`.
- The hand-off for gengar MUST contain the final `Casks/mangue.rb`, the tap README snippet and the per-release update steps (version + sha256 from `SHA256SUMS`).

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|-----------|
| [T1](t1.md) | Publish SHA256SUMS with every release | complete | — |
| [T2](t2.md) | Cask proposal, verified locally | partial (online audit blocked: v0.1.0 is a pre-release) | — |
| [T3](t3.md) | README install docs and hand-off for gengar | complete | T2 |

T1 and T2 may run in parallel (disjoint files).

## Out of scope

- Editing, committing or pushing `rntgspr/homebrew-tap` (gengar's project).
- A Linux Homebrew formula; submitting to the official `homebrew/cask`.
- Notarization (tracked in `specs/modeler/index.md` known gaps).
- Automating tap bumps from mangue CI (needs a cross-repo token; follow-up with gengar).

## Risks

- **Unnotarized DMG.** Homebrew applies quarantine, so Gatekeeper blocks the first launch; only notarization removes this. The caveat is a stopgap.
- **Bridge unavailable.** The Alter Bridge MCP tools are not connected in this session (ECONNREFUSED); delivery to gengar may stay as a hand-off file until the bridge is up.
- **Version drift.** Each release changes `version` and `sha256`; without automation the tap lags until gengar bumps it.
