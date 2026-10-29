//! Appearance-driven theme selection: applies the light or dark theme of
//! the `Themes` global (built-ins when unset) as the `Theme` global.

use gpui::{App, WindowAppearance};

use super::schema::Theme;
use super::user::Themes;

/// The theme of `themes` for the window `appearance`.
pub fn for_appearance(themes: &Themes, appearance: WindowAppearance) -> &Theme {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => &themes.light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => &themes.dark,
    }
}

/// Sets the `Theme` global for `appearance` and derives the gpui-component
/// theme from it.
pub fn apply(appearance: WindowAppearance, cx: &mut App) {
    let theme = match cx.try_global::<Themes>() {
        Some(themes) => for_appearance(themes, appearance).clone(),
        None => for_appearance(&Themes::default(), appearance).clone(),
    };

    super::component::sync(&theme, appearance, cx);
    cx.set_global(theme);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Light and vibrant light pick the light theme; dark ones the dark theme.
    #[test]
    fn picks_by_appearance() {
        let themes = Themes::default();

        assert_eq!(
            for_appearance(&themes, WindowAppearance::Light),
            &themes.light
        );
        assert_eq!(
            for_appearance(&themes, WindowAppearance::VibrantLight),
            &themes.light
        );
        assert_eq!(
            for_appearance(&themes, WindowAppearance::Dark),
            &themes.dark
        );
        assert_eq!(
            for_appearance(&themes, WindowAppearance::VibrantDark),
            &themes.dark
        );
    }
}
