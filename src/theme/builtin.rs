//! Built-in themes: YAML files under `assets/themes/`, embedded by `Assets`.

use gpui::AssetSource;
use serde_norway::Value;

use super::schema::Theme;
use crate::assets::Assets;

/// Name of the built-in theme used in light appearance.
pub const LIGHT: &str = "solarized-light";

/// Name of the built-in theme used in dark appearance.
pub const DARK: &str = "solarized-dark";

/// Names of every built-in theme, in display order.
pub const NAMES: [&str; 4] = [LIGHT, DARK, "tokyonight-day", "tokyonight-night"];

impl Default for Theme {
    /// The built-in Solarized light theme.
    fn default() -> Self {
        builtin(LIGHT).expect("built-in light theme parses")
    }
}

/// Parsed YAML mapping of the built-in theme `name`, `None` when unknown.
pub fn builtin_value(name: &str) -> Option<Value> {
    let bytes = Assets.load(&format!("themes/{name}.yaml")).ok()??;

    serde_norway::from_slice(&bytes).ok()
}

/// The built-in theme `name`, `None` when unknown or malformed.
pub fn builtin(name: &str) -> Option<Theme> {
    Theme::from_value(builtin_value(name)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Color;

    /// Both Solarized built-ins parse completely with their palette roles.
    #[test]
    fn solarized_parse() {
        let light = builtin(LIGHT).expect("light");
        let dark = builtin(DARK).expect("dark");

        assert_eq!(light.canvas.background, Color::parse("#fdf6e3").unwrap());
        assert_eq!(light.selection, Color::parse("#268bd2").unwrap());
        assert_eq!(dark.canvas.background, Color::parse("#002b36").unwrap());
        assert_eq!(dark.shape.label, Color::parse("#93a1a1").unwrap());

        assert!(builtin("missing").is_none());
    }

    /// Both Tokyo Night built-ins parse with their bg/fg/blue roles.
    #[test]
    fn tokyonight_parse() {
        let day = builtin("tokyonight-day").expect("day");
        let night = builtin("tokyonight-night").expect("night");

        assert_eq!(day.canvas.background, Color::parse("#e1e2e7").unwrap());
        assert_eq!(day.shape.label, Color::parse("#3760bf").unwrap());
        assert_eq!(night.canvas.background, Color::parse("#1a1b26").unwrap());
        assert_eq!(night.selection, Color::parse("#7aa2f7").unwrap());
    }
}
