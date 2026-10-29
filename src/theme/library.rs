//! User theme library: named themes loaded from `mangue/themes/*.yaml`
//! under the config dir, resolved with `extends` chains over built-ins.

use std::collections::BTreeMap;
use std::path::Path;

use serde_norway::{Mapping, Value};

use super::builtin::{LIGHT, builtin_value};

/// Named user themes (partial theme mappings, `name` key removed).
pub type Library = BTreeMap<String, Mapping>;

/// Removes `key` from `map`, requiring a string when present.
pub(super) fn take_name(map: &mut Mapping, key: &str) -> Result<Option<String>, String> {
    match map.remove(key) {
        None => Ok(None),
        Some(Value::String(name)) => Ok(Some(name)),
        Some(_) => Err(format!("{key} must be a theme name")),
    }
}

/// Deep-merges `over` into `base`: mappings merge key by key, any other value replaces the base value.
pub(super) fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Mapping(base), Value::Mapping(over)) => {
            for (key, value) in over {
                match base.get_mut(&key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, over) => *base = over,
    }
}

/// Loads every `*.yaml` in `dir` keyed by its `name:` key or file stem;
/// unreadable or non-mapping files are reported on stderr and skipped.
pub fn load_dir(dir: &Path) -> Library {
    let mut library = Library::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return library;
    };

    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();

    for path in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
    {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        match read_theme(path, &stem) {
            Ok((name, map)) => {
                library.insert(name, map);
            }
            Err(err) => eprintln!("mangue: ignoring theme {}: {err}", path.display()),
        }
    }

    library
}

/// Reads one user theme file as `(name, mapping)`.
fn read_theme(path: &Path, stem: &str) -> Result<(String, Mapping), String> {
    let yaml = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let Value::Mapping(mut map) = serde_norway::from_str(&yaml).map_err(|e| e.to_string())? else {
        return Err("theme file must be a mapping".into());
    };

    let name = take_name(&mut map, "name")?.unwrap_or_else(|| stem.to_string());

    Ok((name, map))
}

/// Complete mapping of theme `name`: a user theme (which overrides a
/// built-in of the same name) merged over what it `extends` (its own name
/// there means the built-in; none means that built-in, else Solarized
/// light), or a built-in. Cycles and unknown names are errors.
pub fn resolve(name: &str, library: &Library) -> Result<Value, String> {
    resolve_chain(name, library, &mut Vec::new())
}

/// `resolve` tracking the names being resolved in `chain` to detect cycles.
fn resolve_chain(name: &str, library: &Library, chain: &mut Vec<String>) -> Result<Value, String> {
    let Some(theme) = library
        .get(name)
        .filter(|_| !chain.iter().any(|c| c == name))
    else {
        if chain.last().is_some_and(|c| c == name) || !chain.iter().any(|c| c == name) {
            return builtin_value(name).ok_or(format!("unknown theme {name:?}"));
        }
        chain.push(name.to_string());
        return Err(format!("theme extends cycle: {}", chain.join(" -> ")));
    };

    let mut overrides = theme.clone();
    let parent = take_name(&mut overrides, "extends")?;
    chain.push(name.to_string());

    let mut base = match parent.as_deref() {
        Some(parent) => resolve_chain(parent, library, chain)?,
        None => builtin_value(name)
            .or_else(|| builtin_value(LIGHT))
            .ok_or("missing built-in light theme")?,
    };
    chain.pop();

    merge(&mut base, Value::Mapping(overrides));

    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Color, DARK, Theme, builtin};

    /// Library built from `(name, yaml)` pairs.
    fn library(themes: &[(&str, &str)]) -> Library {
        themes
            .iter()
            .map(|(name, yaml)| (name.to_string(), serde_norway::from_str(yaml).unwrap()))
            .collect()
    }

    /// Resolves `name` into a full theme.
    fn theme(name: &str, library: &Library) -> Result<Theme, String> {
        Theme::from_value(resolve(name, library)?)
    }

    /// Directory loading keys by `name:` or file stem and skips other files.
    #[test]
    fn loads_directory() {
        let dir = std::env::temp_dir().join(format!("mangue-themes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ocean.yaml"), "hover: \"#000000\"\n").unwrap();
        std::fs::write(dir.join("x.yaml"), "name: forest\nextends: ocean\n").unwrap();
        std::fs::write(dir.join("bad.yaml"), "- 1\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();

        let library = load_dir(&dir);
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(library.keys().collect::<Vec<_>>(), ["forest", "ocean"]);
        assert_eq!(
            theme("forest", &library).unwrap().hover,
            Color::parse("#000000").unwrap()
        );
    }

    /// Chains resolve over built-ins; no `extends` means Solarized light.
    #[test]
    fn resolves_names() {
        let library = library(&[
            ("base", "extends: solarized-dark\nhover: \"#111111\"\n"),
            ("child", "extends: base\nlasso: \"#222222\"\n"),
            ("plain", "lasso: \"#333333\"\n"),
        ]);
        let child = theme("child", &library).unwrap();

        assert_eq!(child.hover, Color::parse("#111111").unwrap());
        assert_eq!(child.lasso, Color::parse("#222222").unwrap());
        assert_eq!(child.canvas, builtin(DARK).unwrap().canvas);
        assert_eq!(
            theme("plain", &library).unwrap().canvas,
            Theme::default().canvas
        );

        assert!(resolve("nope", &library).is_err());
    }

    /// Cycles are reported with their chain.
    #[test]
    fn detects_cycles() {
        let library = library(&[("a", "extends: b\n"), ("b", "extends: a\n")]);
        let err = resolve("a", &library).unwrap_err();

        assert!(err.contains("a -> b -> a"), "{err}");
    }

    /// A user theme named like a built-in overrides it, extending the
    /// built-in when it names itself or nothing.
    #[test]
    fn user_overrides_builtin() {
        let red = Color::parse("#dc322f").unwrap();
        for yaml in [
            "selection: \"#dc322f\"\n",
            "extends: solarized-dark\nselection: \"#dc322f\"\n",
        ] {
            let library = library(&[("solarized-dark", yaml)]);
            let dark = theme(DARK, &library).unwrap();

            assert_eq!(dark.selection, red);
            assert_eq!(dark.canvas, builtin(DARK).unwrap().canvas);
        }
    }
}
