# Themes

mangue paints the diagram with a YAML theme. By default it follows the macOS
appearance: Solarized light in light mode, Solarized dark in dark mode.

## Built-in themes

- `solarized-light`, `solarized-dark` (https://ethanschoonover.com/solarized/)
- `tokyonight-day`, `tokyonight-night` (https://github.com/folke/tokyonight.nvim)

## Choosing a theme

Create `~/.config/mangue/theme.yaml` (or `$XDG_CONFIG_HOME/mangue/theme.yaml`):

```yaml
# one theme for both appearances
extends: tokyonight-night
```

```yaml
# one theme per appearance
appearance:
  light: tokyonight-day
  dark: tokyonight-night
```

The theme picked in the Settings window (cmd-,), saved as `theme.light` /
`theme.dark` in `~/.config/mangue/settings.yaml`, wins over the selection in
`theme.yaml`; the key overrides below still apply.

Any other theme key in this file overrides the chosen themes, e.g.
`selection: "#dc322f"`.

## Your own themes

Put named themes in `~/.config/mangue/themes/*.yaml`. The name is the file
stem, or the `name:` key. A theme may `extends:` a built-in or another of
your themes; only the keys you write change. A theme named like a built-in
replaces it. Select your themes from `theme.yaml` exactly like built-ins.

A documented example with every common key:
[`assets/themes/example-custom.yaml`](../assets/themes/example-custom.yaml).
The `grid` key colors the dots of the snap grid; keep it close to
`canvas.background`. The full key reference is in
[`.cumaru/specs/modeler/theme.md`](../.cumaru/specs/modeler/theme.md).

The `overlay.error` colors style the tooltip and marker shown when a drag or
connect is refused (`overlay.badge` styles element badges).

Errors (bad YAML, unknown key, bad color, unknown theme, `extends` cycle) are
printed to stderr and mangue falls back to the built-in themes; a broken file
in `themes/` is skipped.
