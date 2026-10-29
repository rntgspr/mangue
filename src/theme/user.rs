//! User theme file (`theme.yaml` in `config_dir`): picks built-ins per
//! appearance and overrides any theme key; names resolve against user themes first.

use std::ffi::OsString;
use std::path::PathBuf;

use gpui::Global;
use serde_norway::{Mapping, Value};

use super::builtin::{DARK, LIGHT, NAMES};
use super::library::{Library, load_dir, merge, resolve, take_name};
use super::schema::Theme;

/// The resolved themes for light and dark appearance.
#[derive(Debug, Clone, PartialEq)]
pub struct Themes {
    pub light: Theme,
    pub dark: Theme,
}

impl Global for Themes {}

impl Default for Themes {
    /// The built-in Solarized light and dark themes.
    fn default() -> Self {
        Themes {
            light: Theme::default(),
            dark: super::builtin::builtin(DARK).expect("built-in dark theme parses"),
        }
    }
}

/// Path of the user theme file, `None` when no config dir can be found.
pub fn config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("theme.yaml"))
}

/// The `mangue` config directory: `%APPDATA%\mangue` on Windows, else
/// `$XDG_CONFIG_HOME/mangue` (default `~/.config/mangue`).
pub fn config_dir() -> Option<PathBuf> {
    config_dir_for(cfg!(windows), |key| std::env::var_os(key))
}

/// `config_dir` for the given platform (`windows`) and environment lookup.
fn config_dir_for(windows: bool, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |key: &str| env(key).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if windows {
        set("APPDATA")?
    } else {
        set("XDG_CONFIG_HOME").or_else(|| Some(set("HOME")?.join(".config")))?
    };

    Some(base.join("mangue"))
}

/// Themes from the user file and the user theme library (`themes/*.yaml`
/// next to it), or the built-ins when absent; an invalid file is reported
/// on stderr and ignored.
pub fn load() -> Themes {
    load_picked(None, None)
}

/// `load`, with `light` / `dark` (from settings) replacing the theme
/// file's selection for that appearance; its key overrides still apply.
pub fn load_picked(light: Option<&str>, dark: Option<&str>) -> Themes {
    let Some(dir) = config_dir() else {
        return parse_picked("", &Library::new(), light, dark).unwrap_or_default();
    };
    let path = dir.join("theme.yaml");
    let library = load_dir(&dir.join("themes"));
    let yaml = std::fs::read_to_string(&path).unwrap_or_default();

    parse_picked(&yaml, &library, light, dark).unwrap_or_else(|err| {
        eprintln!("mangue: ignoring theme {}: {err}", path.display());
        Themes::default()
    })
}

/// Resolves a user theme file: `extends` pins one built-in for both
/// appearances, `appearance.light` / `appearance.dark` pick one each
/// (default Solarized light/dark), and every other key overrides the base.
/// Names resolve against `library` first, then the built-ins.
pub fn parse(yaml: &str, library: &Library) -> Result<Themes, String> {
    parse_picked(yaml, library, None, None)
}

/// `parse`, with `light` / `dark` overriding the file's theme selection.
pub fn parse_picked(
    yaml: &str,
    library: &Library,
    light_pick: Option<&str>,
    dark_pick: Option<&str>,
) -> Result<Themes, String> {
    let value: Value = serde_norway::from_str(yaml).map_err(|err| err.to_string())?;
    let mut overrides = match value {
        Value::Mapping(map) => map,
        Value::Null => Mapping::new(),
        _ => return Err("theme file must be a mapping".into()),
    };

    let extends = take_name(&mut overrides, "extends")?;
    let (mut light, mut dark) = (LIGHT.to_string(), DARK.to_string());
    if let Some(appearance) = overrides.remove("appearance") {
        let Value::Mapping(mut appearance) = appearance else {
            return Err("appearance must be a mapping".into());
        };
        light = take_name(&mut appearance, "light")?.unwrap_or(light);
        dark = take_name(&mut appearance, "dark")?.unwrap_or(dark);

        if let Some(key) = appearance.keys().next() {
            return Err(format!("unknown appearance key {key:?}"));
        }
    }
    if let Some(name) = extends {
        (light, dark) = (name.clone(), name);
    }
    light = light_pick.map_or(light, str::to_string);
    dark = dark_pick.map_or(dark, str::to_string);

    let resolve = |name: &str| {
        let mut base = resolve(name, library)?;
        merge(&mut base, Value::Mapping(overrides.clone()));
        Theme::from_value(base)
    };

    Ok(Themes {
        light: resolve(&light)?,
        dark: resolve(&dark)?,
    })
}

/// Names of every selectable theme: the built-ins, then the user themes
/// of the config dir not named like a built-in.
pub fn names() -> Vec<String> {
    let library = config_dir()
        .map(|dir| load_dir(&dir.join("themes")))
        .unwrap_or_default();
    let mut names: Vec<String> = NAMES.iter().map(|n| n.to_string()).collect();

    names.extend(library.into_keys().filter(|n| !NAMES.contains(&n.as_str())));

    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Color;
    use crate::theme::builtin::builtin;

    /// Windows uses `%APPDATA%\mangue`; elsewhere `XDG_CONFIG_HOME`, then `~/.config`.
    #[test]
    fn config_dir_per_platform() {
        let env = |key: &str| match key {
            "APPDATA" => Some("C:/Users/u/AppData/Roaming".into()),
            "HOME" => Some("/home/u".into()),
            _ => None,
        };
        let xdg = |key: &str| match key {
            "XDG_CONFIG_HOME" => Some("/xdg".into()),
            _ => env(key),
        };

        assert_eq!(
            config_dir_for(true, env),
            Some(PathBuf::from("C:/Users/u/AppData/Roaming").join("mangue"))
        );
        assert_eq!(config_dir_for(false, env), Some(PathBuf::from("/home/u/.config/mangue")));
        assert_eq!(config_dir_for(false, xdg), Some(PathBuf::from("/xdg/mangue")));
        assert_eq!(config_dir_for(true, |_| None), None);
    }

    /// `parse` with an empty user theme library.
    fn parse_plain(yaml: &str) -> Result<Themes, String> {
        parse(yaml, &Library::new())
    }

    /// The user file selects user themes, which override same-named built-ins.
    #[test]
    fn selects_user_themes() {
        let mut library = Library::new();
        library.insert(
            "mine".into(),
            serde_norway::from_str("hover: \"#000000\"\n").unwrap(),
        );
        library.insert(
            DARK.into(),
            serde_norway::from_str("lasso: \"#111111\"\n").unwrap(),
        );
        let themes = parse("appearance:\n  light: mine\n", &library).unwrap();

        assert_eq!(themes.light.hover, Color::parse("#000000").unwrap());
        assert_eq!(themes.dark.lasso, Color::parse("#111111").unwrap());
        assert!(parse("extends: missing\n", &library).is_err());
    }

    /// Picked names (settings) win over the file's selection; overrides stay.
    #[test]
    fn picks_override_selection() {
        let yaml = "extends: solarized-dark\nselection: \"#dc322f\"\n";
        let themes = parse_picked(yaml, &Library::new(), Some("tokyonight-day"), None).unwrap();

        assert_eq!(
            themes.light.canvas,
            builtin("tokyonight-day").unwrap().canvas
        );
        assert_eq!(themes.light.selection, Color::parse("#dc322f").unwrap());
        assert_eq!(themes.dark.canvas, builtin(DARK).unwrap().canvas);
    }

    /// An empty file yields exactly the built-in Solarized pair.
    #[test]
    fn empty_file_is_builtin() {
        assert_eq!(parse_plain("").unwrap(), Themes::default());
    }

    /// Partial overrides change only the given keys, in both appearances.
    #[test]
    fn partial_override_merges() {
        let themes = parse_plain("selection: \"#dc322f\"\nfont:\n  size: 14\n").unwrap();
        let red = Color::parse("#dc322f").unwrap();

        assert_eq!(themes.light.selection, red);
        assert_eq!(themes.dark.selection, red);
        assert_eq!(themes.light.font.size, 14.0);
        assert_eq!(themes.light.font.family, ".SystemUIFont");
        assert_eq!(themes.dark.canvas, builtin(DARK).unwrap().canvas);
    }

    /// `extends` pins one built-in for both appearances.
    #[test]
    fn extends_pins_builtin() {
        let themes = parse_plain("extends: solarized-dark\n").unwrap();
        let dark = builtin(DARK).unwrap();

        assert_eq!(themes.light, dark);
        assert_eq!(themes.dark, dark);
    }

    /// `appearance` picks a built-in per appearance.
    #[test]
    fn appearance_picks_builtins() {
        let themes = parse_plain("appearance:\n  light: solarized-dark\n").unwrap();

        assert_eq!(themes.light, builtin(DARK).unwrap());
        assert_eq!(themes.dark, builtin(DARK).unwrap());
    }

    /// Tokyo Night built-ins are selectable per appearance.
    #[test]
    fn appearance_picks_tokyonight() {
        let themes =
            parse_plain("appearance:\n  light: tokyonight-day\n  dark: tokyonight-night\n")
                .unwrap();

        assert_eq!(themes.light, builtin("tokyonight-day").unwrap());
        assert_eq!(themes.dark, builtin("tokyonight-night").unwrap());
    }

    /// `extends` and `appearance` select user themes by name.
    #[test]
    fn selects_user_themes_by_name() {
        let user = Library::from([(
            "ocean".into(),
            serde_norway::from_str("extends: tokyonight-night\nhover: \"#123456\"\n").unwrap(),
        )]);
        let pinned = parse("extends: ocean\n", &user).unwrap();
        let picked = parse("appearance:\n  dark: ocean\n", &user).unwrap();

        assert_eq!(pinned.light.hover, Color::parse("#123456").unwrap());
        assert_eq!(picked.dark.hover, Color::parse("#123456").unwrap());
        assert_eq!(picked.light, builtin(LIGHT).unwrap());
    }

    /// Unknown keys, bad colors and unknown built-ins are errors.
    #[test]
    fn invalid_files_fail() {
        assert!(parse_plain("colour: \"#000000\"\n").is_err());
        assert!(parse_plain("selection: blue\n").is_err());
        assert!(parse_plain("extends: nope\n").is_err());
        assert!(parse_plain("appearance:\n  dim: solarized-dark\n").is_err());
        assert!(parse_plain("- a\n").is_err());
    }
}
