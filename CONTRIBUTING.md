# Contributing to Mangue

This guide covers the source checkout, local build, verification, packaging and
release. For installing and using the app, start with the [README](README.md).
By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to help

- **Report bugs.** Open an issue with your OS, Mangue version, steps to
  reproduce and, whenever possible, the `.bpmn` file that triggers it.
- **Close parity gaps.** Mangue targets the
  [bpmn-js](https://github.com/bpmn-io/bpmn-js) modeler. Known gaps are tracked in
  [`.cumaru/specs/modeler/parity.md`](.cumaru/specs/modeler/parity.md).
- **Contribute fixtures.** Real-world BPMN 2.0 files that fail to import, render
  or round-trip are valuable test inputs (`tests/fixtures/`).
- **Send pull requests.** Small, focused changes with tests are easiest to review.
- **Report vulnerabilities privately.** See [SECURITY.md](SECURITY.md).

## Get the source

```sh
git clone https://github.com/rntgspr/mangue.git
cd mangue
```

Use a stable [Rust toolchain](https://rustup.rs/) with Cargo and clippy.
Platform requirements:

- **macOS:** Xcode Command Line Tools (`xcode-select --install`). Xcode 26+ is
  needed only to compile the layered `mangue.icon` when bundling.
- **Windows:** the MSVC toolchain (`x86_64-pc-windows-msvc`).
- **Linux** (Debian/Ubuntu names):

  ```sh
  sudo apt-get install build-essential pkg-config libxcb1-dev libxkbcommon-dev \
    libxkbcommon-x11-dev libwayland-dev libfontconfig1-dev libfreetype-dev \
    libvulkan1 libssl-dev imagemagick
  ```

  `imagemagick` is only needed by `scripts/package-linux.sh`.

## Run locally

```sh
cargo run --release -- path/to/file.bpmn   # file is optional; demo diagram otherwise
```

## Architecture

The crate is lib + bin. The library holds a GPUI-independent semantic and DI
model plus BPMN 2.0 XML import/export; the binary is the GPUI desktop app.

| Path | Responsibility |
|---|---|
| `src/model/` | BPMN semantic + DI model. |
| `src/io/` | BPMN 2.0 XML import / export. |
| `src/modeling/` | Commands, rules, layout, undo / redo. |
| `src/canvas/` | Windows, tabs, canvas rendering and interaction. |
| `src/theme/`, `assets/themes/` | YAML themes. |
| `src/settings/` | `settings.yaml`, recent files, Settings window. |
| `src/files/` | Filesystem helpers. |
| `tests/` | Integration tests and BPMN fixtures. |

Canonical contracts live in the living spec at
[`.cumaru/specs/modeler/`](.cumaru/specs/modeler/index.md); read the relevant
area before changing behavior.

## Development workflow

The repository uses [Cumaru](https://github.com/rntgspr/cumaru) (`sdlc-light`
domain) for project knowledge:

- `.cumaru/specs/` describes what is true now.
- `.cumaru/plans/` holds active work; a plan is absorbed into the specs and
  removed when it closes.
- `.cumaru/exploring/` holds ideas that are not plans yet.

Contributors without Cumaru can ignore the tooling, but behavior changes should
update the matching spec area. Commits follow
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
(`feat(canvas): ...`, `fix(io): ...`). Add an entry under `Unreleased` in
[CHANGELOG.md](CHANGELOG.md) for user-visible changes.

## Verify changes

Run what CI runs before opening a pull request:

```sh
cargo test
cargo clippy --all-targets
git diff --check
```

Checkouts use LF line endings (`.gitattributes`), so fixture-based tests behave
the same on Windows.

## Packaging

### macOS

An unbundled binary may not show a Dock icon or appear in Cmd+Tab when started
from a fullscreen terminal Space. Build and open the bundle instead:

```sh
scripts/bundle-macos.sh
open target/release/bundle/Mangue.app --args /path/to/file.bpmn
```

The script builds a universal (`arm64` + `x86_64`) `Mangue.app` with its icon
(generated from `assets/app-icon/`; Xcode 26+ also compiles the layered
`mangue.icon` for macOS 26) and `target/release/bundle/Mangue-<version>.dmg`
with an `/Applications` link. The bundle (`io.mangue.app`) registers as an
editor for `.bpmn`. It is ad-hoc signed unless these are set:

- `APPLE_SIGNING_IDENTITY` (a Developer ID Application identity in the
  keychain) signs with hardened runtime (`scripts/entitlements.plist`).
- `APPLE_TEAM_ID`, `APPLE_ID` and `APPLE_APP_PASSWORD` (app-specific password)
  additionally notarize the DMG (`xcrun notarytool`) and staple it.

### Windows

```powershell
scripts/package-windows.ps1
```

Builds `mangue.exe` (icon embedded by `build.rs` from `assets/app-icon/`, no
console window in release) and
`target/release/bundle/mangue-<version>-windows-x86_64.zip`.

### Linux

```sh
scripts/package-linux.sh
```

Builds `mangue` with X11 and Wayland support and
`target/release/bundle/mangue-<version>-linux-x86_64.tar.gz`, laid out as a
`/usr/local`-style prefix: `bin/mangue`, `share/applications/mangue.desktop`
(opens `.bpmn`), `share/mime/packages/mangue-bpmn.xml` and hicolor icons
generated from `assets/app-icon/`.

## CI and releases

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) is one pipeline in three stages:

1. `test`: `cargo test` and `cargo clippy --all-targets` on macOS, Windows and Linux (also on pull requests).
2. `build-macos`, `build-windows`, `build-linux`: on pushes, once every test job passed, each OS builds its package (DMG, zip, tarball) as a workflow artifact.
3. `release`: on `v*` tags, attaches the three packages to a draft GitHub Release.

Optional secrets `APPLE_CERTIFICATE_P12` (base64), `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_APP_PASSWORD` and `APPLE_TEAM_ID`
enable Developer ID signing and notarization of the DMG.

To release: bump `version` in `Cargo.toml`, move `Unreleased` entries in
`CHANGELOG.md` under the new version, tag `vX.Y.Z`, push the tag, then review
and publish the draft release.
