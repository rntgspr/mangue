---
human_revised: false
name: modeler
summary: Native BPMN 2.0 modeler in Rust/GPUI aiming at bpmn-js modeler parity; currently an app shell plus semantic model.
depends-on: []
relates: []
targets: [platform]
---

# Modeler

## Overview

`mangue` is a native desktop BPMN 2.0 modeler built in Rust on GPUI, the
GPU-accelerated UI framework from Zed (https://github.com/zed-industries/zed/tree/main/crates/gpui).
The functional reference is the bpmn-js modeler (https://github.com/bpmn-io/bpmn-js):
the same modeling, import and export scope, with no vendor extensions.

Current state: a single window boots on macOS, opens/saves BPMN files and edits
the diagram (select, lasso, move, delete, rename, undo/redo) on a pan/zoom
canvas (see `canvas.md`, `modeling.md`).
The crate is lib+bin; the library holds a GPUI-independent semantic + DI model
(see `model.md`) and BPMN 2.0 XML import/export (see `io.md`).

## Requirements (EARS / RFC 2119)

- WHEN the binary is launched on macOS THE SYSTEM SHALL open a 1280x800 window centered on screen.
- The app MUST build and pass `cargo test` on macOS, Windows (x86_64) and Linux (x86_64, X11 and Wayland).
- The modeler MUST NOT depend on Camunda/Zeebe extensions to model or save diagrams.

## Packaging

- WHEN built with `scripts/bundle-macos.sh` THE SYSTEM SHALL produce a universal (`arm64` + `x86_64`) `target/release/bundle/Mangue.app` (`io.mangue.app`, min macOS 13.0, `.bpmn` Editor document type) and `Mangue-<version>.dmg` with an `/Applications` link.
- The macOS icon MUST be `Mangue.icns` plus, when `actool` compiles the Icon Composer document (Xcode 26+), `Assets.car` with `CFBundleIconName`.
- WHERE `APPLE_SIGNING_IDENTITY` is set THE SYSTEM SHALL sign with hardened runtime, and with `APPLE_TEAM_ID`, `APPLE_ID` and `APPLE_APP_PASSWORD` also notarize and staple the DMG; otherwise it MUST sign ad-hoc and print a warning.
- WHEN built with `scripts/package-windows.ps1` THE SYSTEM SHALL produce `mangue-<version>-windows-x86_64.zip` with `mangue.exe` (icon embedded by `build.rs`, no console window in release).
- WHEN built with `scripts/package-linux.sh` THE SYSTEM SHALL produce `mangue-<version>-linux-x86_64.tar.gz` with the binary, `mangue.desktop`, the `.bpmn` MIME glob and hicolor icons (48/128/256/512).
- Every platform icon MUST be generated from `assets/app-icon/` into `target/`; derived icons are never committed.
- `.github/workflows/ci.yml` MUST run in three stages: `test` (test + clippy on macos-14, windows-latest, ubuntu-latest, also on pull requests), then `build-macos` / `build-windows` / `build-linux` on pushes once every test passed, then `release` attaching the three packages to a draft GitHub Release on `v*` tags.
- Checkouts MUST use LF line endings (`.gitattributes`), so fixture-based tests behave the same on Windows.
- WHEN a window opens THE SYSTEM SHALL activate the window and the app ignoring other apps (GPUI sets the Regular activation policy), also for unbundled runs.
- WHEN the OS delivers Open Documents URLs (Finder, `open file.bpmn`) THE SYSTEM SHALL open each `file://` path in a new window.

## Decisions

- GPUI pinned to `gpui-pre` `=0.3.9` (with `gpui-pre-platform`), plus `gpui-component` `=0.8.0` for the code editor and the app chrome (see [canvas](canvas.md)); no Zed crates; pre-1.0 API, upgrades are deliberate.
- Single crate (`mangue`) until a second consumer justifies a workspace split.
- Rust edition 2024.
- Off macOS, tests use `secondary-` keystrokes (gpui maps it to cmd on macOS, ctrl elsewhere); platform-dependent logic takes the platform as a parameter so its tests run on every OS.
- Known gaps at absorb (accepted): the Windows and Linux artifacts and the in-window menu bar have no manual smoke run; notarization has never run (no Apple Developer credentials); CI runners lack Xcode 26, so CI DMGs ship only `Mangue.icns`.

## Files

- `Cargo.toml` — crate manifest, GPUI dependency (dev: GPUI `test-support` for headless interaction tests).
- `src/assets.rs` — embedded icons and generated vertical label SVGs; see [canvas](canvas.md).
- `src/main.rs` — app entry; boots GPUI and opens the `Canvas` root view (demo diagram via `load_definitions`), installs `Settings` and the `Themes` they select.
- `src/lib.rs` — library root; see [model](model.md) for `src/model/`.
- `src/io/` — BPMN XML import/export; see [io](io.md).
- `src/canvas/` — diagram canvas; see [canvas](canvas.md).
- Code mode (BPMN XML editor); see [code-editor](code-editor.md).
- `src/theme/`, `assets/themes/` — YAML themes; see [theme](theme.md).
- `src/modeling/` — commands and undo/redo; see [modeling](modeling.md).
- `src/settings/` — `settings.yaml` and the Settings window; see [settings](settings.md).
- `scripts/bundle-macos.sh` — universal release build, icons, `Mangue.app` (Info.plist), signing / notarization, DMG; `main.rs` also drains `on_open_urls` paths into new windows.
- `scripts/entitlements.plist` — hardened-runtime entitlements for Developer ID signing.
- `scripts/package-windows.ps1`, `build.rs` — Windows zip and embedded icon.
- `scripts/package-linux.sh`, `assets/app/` — Linux tarball, desktop entry and MIME glob.
- `assets/app-icon/` — icon artwork (Icon Composer `mangue.icon`, 1024px export, flat PNG).
- `.github/workflows/ci.yml` — staged CI / release pipeline.
- `.gitattributes` — LF checkouts.
- `README.md` — end-user install per OS from GitHub Releases, first launch and usage.
- `CONTRIBUTING.md` — source setup, platform build deps, architecture map, workflow, verification, packaging and CI/release.
- `LICENSE` — MIT (`Cargo.toml` `license = "MIT"`).
- `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md` — conduct (Contributor Covenant 2.1), private vulnerability reporting, Keep a Changelog.
- `.github/ISSUE_TEMPLATE/`, `.github/pull_request_template.md` — bug / feature issue forms and PR checklist.
