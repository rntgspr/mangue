---
human_revised: false
plan: maintenance-homebrew-cask
task: T3
status: complete
date: 2026-10-29
summary: Hand-off for gengar - add Casks/mangue.rb and a README entry to rntgspr/homebrew-tap, plus per-release bump steps.
---

# Hand-off for gengar — Mangue cask in rntgspr/homebrew-tap

Mangue (https://github.com/rntgspr/mangue) is a GUI app shipped as a DMG, so it is a cask. Please add it to the tap; mangue never edits the tap.

## 1. Add `Casks/mangue.rb`

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

Verified on a throwaway tap: `brew style` clean, `brew audit --cask --strict` clean, install/uninstall OK. `brew audit --cask --online` currently fails only because v0.1.0 is a GitHub **pre-release** (`releases/latest` 404); it passes once a non-pre-release exists.

## 2. Tap README section

The README heading `## Formulae` could become `## Formulae and casks`, or add a `## Casks` section:

~~~markdown
## Casks

### mangue

Native BPMN 2.0 modeler for macOS (universal, macOS 13+).

```bash
brew install --cask rntgspr/tap/mangue
```

The build is not notarized yet: right-click Mangue.app > Open on first launch, or run
`xattr -dr com.apple.quarantine /Applications/Mangue.app`.

Project & source: https://github.com/rntgspr/mangue
~~~

## 3. Per-release bump

Every mangue `v*` release attaches `SHA256SUMS`.

1. `gh release download vX.Y.Z -R rntgspr/mangue -p SHA256SUMS -O -` and take the `Mangue-X.Y.Z.dmg` line.
2. In `Casks/mangue.rb` set `version "X.Y.Z"` and `sha256 "<hash>"`.
3. `brew style rntgspr/tap/mangue && brew audit --cask --online rntgspr/tap/mangue`.
4. Commit and push the tap.
