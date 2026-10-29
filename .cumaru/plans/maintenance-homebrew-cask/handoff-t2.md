---
human_revised: false
plan: maintenance-homebrew-cask
task: T2
status: partial
date: 2026-10-29
summary: Hand-off for maintenance-homebrew-cask / T2 - cask proposal verified on a throwaway tap; online audit blocked by the v0.1.0 pre-release flag.
---

# Hand-off — maintenance-homebrew-cask / T2

## Files touched

<!-- cumaru:touched -->
| Link | Description |
|------|-------------|
| [`handoff-t2.md`](handoff-t2.md) | created — holds the cask text (no `mangue.rb` file) |
<!-- /cumaru:touched -->

## Decisions made during implementation

- `cumaru fs plans/maintenance-homebrew-cask/mangue.rb create` refused (`directory names must not contain dots in <path>: mangue.rb`); the cask lives here and in `handoff-gengar.md`.
- `depends_on macos: ">= :ventura"` replaced by `depends_on macos: :ventura`: `brew style` flags the string form (`Homebrew/OSDependsOn: Use depends_on macos: :ventura`); the symbol already means "Ventura or later". Deviation from the acceptance criterion wording, same semantics.
- `zap trash:` is a single string (rubocop prefers it over a one-element array).
- livecheck kept on `:github_latest` (AC: GitHub releases). It currently fails because v0.1.0 is a pre-release (see below); `:git` strategy would work today (`mangue: 0.1.0 ==> 0.1.0`) but contradicts the AC.

## Final cask (`Casks/mangue.rb`)

```ruby
cask "mangue" do
  version "0.1.0"
  sha256 "48d2bfd0707b405f93234c8bbbedc247153a1d94e8ef5cd987b1c4e795aeb068"

  url "https://github.com/rntgspr/mangue/releases/download/v#{version}/Mangue-#{version}.dmg"
  name "Mangue"
  desc "Native BPMN 2.0 modeler"
  homepage "https://github.com/rntgspr/mangue"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: :ventura

  app "Mangue.app"

  zap trash: "~/.config/mangue"

  caveats <<~EOS
    Mangue is not notarized yet, so Gatekeeper blocks the first launch.
    Right-click Mangue.app > Open, or run:
      xattr -dr com.apple.quarantine /Applications/Mangue.app
  EOS
end
```

## Commands run / verification

Throwaway tap `rntgspr/mangue-test` (`brew tap-new --no-git`, then `brew trust` — required, newer brew refuses untrusted taps). `rntgspr/homebrew-tap` untouched.

- `brew style rntgspr/mangue-test/mangue` — `1 file inspected, no offenses detected`.
- `brew audit --cask --strict rntgspr/mangue-test/mangue` (offline) — no problems, rc 0.
- `brew audit --cask --online rntgspr/mangue-test/mangue` — **FAIL**: with `:github_latest`, `exception while auditing mangue: GitHub::API::HTTPNotFoundError` (`/repos/rntgspr/mangue/releases/latest` returns 404); with `:github_releases` / `:git`, `v0.1.0 is a GitHub pre-release`. Root cause: `gh release view v0.1.0` -> `isPrerelease: true`. Not a cask defect; clears once a non-pre-release exists (or v0.1.0 is un-flagged).
- `brew install --cask rntgspr/mangue-test/mangue` — moved `Mangue.app` to /Applications, printed the caveat. `brew uninstall --cask` — removed it (/Applications/Mangue.app absent before and after). Side effect: brew autoremove also uninstalled an orphaned `python@3.13`.
- `brew untap rntgspr/mangue-test` — `Untapped 1 cask`; no longer in `brew tap`.

## Pending / follow-ups

- Online audit and livecheck need a non-pre-release GitHub release (owner decision).

## Suggestions for the Lead

- Release flow marks tags as pre-release (draft then manual publish); decide whether stable tags should be published as "latest".
