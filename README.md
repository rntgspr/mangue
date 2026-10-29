# Mangue

A native BPMN 2.0 modeler for macOS, Windows and Linux, built in Rust on
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).

Mangue aims at the modeling, import and export scope of the
[bpmn-js](https://github.com/bpmn-io/bpmn-js) modeler as a fast desktop app:
plain BPMN 2.0 XML in and out, no vendor extensions required.

## Install

Download the package for your platform from
[GitHub Releases](https://github.com/rntgspr/mangue/releases).

### macOS (13 or later)

1. Open `Mangue-<version>.dmg` and drag `Mangue.app` to `Applications`.
2. If the build is not notarized, right-click `Mangue.app` > **Open** the first
   time to get past Gatekeeper.

The app is universal (Apple silicon and Intel) and registers as an editor for
`.bpmn` files, so double-clicking a diagram in Finder or running
`open file.bpmn -a Mangue` opens it.

### Windows (x86_64)

Extract `mangue-<version>-windows-x86_64.zip` and run `mangue.exe`. Settings
are stored in `%APPDATA%\mangue`.

### Linux (x86_64, X11 and Wayland)

The tarball is laid out as a `/usr/local`-style prefix. To install it there:

```sh
tar -xzf mangue-<version>-linux-x86_64.tar.gz
cd mangue-<version>-linux-x86_64
sudo cp -r bin share /usr/local/
sudo update-mime-database /usr/local/share/mime
sudo update-desktop-database /usr/local/share/applications
```

For a per-user install use `~/.local` as the prefix, without `sudo`.

## Use

Open a `.bpmn` file from the app, your file manager, or a terminal:

```sh
mangue path/to/file.bpmn   # the file is optional; a demo diagram opens otherwise
```

Open, edit and save BPMN 2.0 diagrams on a pan/zoom canvas: select, lasso,
move, connect, rename, delete, undo/redo, multiple tabs, a BPMN XML code mode
and YAML themes.

## Contributing

Want to build from source, run the tests or help with development? See
[CONTRIBUTING.md](CONTRIBUTING.md). Please follow the
[Code of Conduct](CODE_OF_CONDUCT.md) and report vulnerabilities as described in
[SECURITY.md](SECURITY.md). Notable changes are listed in
[CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE)
