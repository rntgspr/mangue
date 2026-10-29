//! Bridge from the YAML `Theme` to gpui-component's `Theme` global: every
//! kit color is derived from the active YAML theme so the app chrome matches
//! the canvas (mapping table in `specs/modeler/theme.md`).

use gpui::{App, WindowAppearance};
use gpui_component::{Theme as KitTheme, ThemeMode};

use super::schema::Theme;

/// Derives the gpui-component theme from `theme` for `appearance`; a no-op
/// until `gpui_component::init` registered the kit global.
pub fn sync(theme: &Theme, appearance: WindowAppearance, cx: &mut App) {
    if !cx.has_global::<KitTheme>() {
        return;
    }

    let mode = match appearance {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeMode::Light,
    };
    if KitTheme::global(cx).mode != mode {
        KitTheme::change(mode, None, cx);
    }

    let family = theme.font.family.clone();
    KitTheme::update(cx, |kit| {
        map_colors(theme, &mut kit.colors);
        kit.font_family = family.into();
    });
}

/// Writes the YAML-derived value of every mapped kit color token; tokens not
/// listed keep the kit default for the current mode.
fn map_colors(theme: &Theme, colors: &mut gpui_component::ThemeColor) {
    let surface = theme.canvas.background.hsla();
    let panel = theme.palette.background.hsla();
    let line = theme.palette.border.hsla();
    let text = theme.replace_menu.text.hsla();
    let muted_text = theme.palette.icon.hsla();
    let hover = theme.palette.hover.hsla();
    let primary = theme.selection.hsla();
    let danger = theme.overlay.error.border.hsla();

    colors.background = surface;
    colors.foreground = text;
    colors.border = theme.replace_menu.border.hsla();
    colors.input = theme.replace_menu.border.hsla();
    colors.ring = primary;
    colors.caret = text;
    colors.selection = theme.lasso.hsla();

    colors.accent = hover;
    colors.accent_foreground = text;
    colors.primary = primary;
    colors.primary_hover = primary.opacity(0.9);
    colors.primary_active = primary.opacity(0.8);
    colors.primary_foreground = surface;
    colors.secondary = panel;
    colors.secondary_hover = hover;
    colors.secondary_active = hover;
    colors.secondary_foreground = text;
    colors.muted = panel;
    colors.muted_foreground = muted_text;
    colors.danger = danger;
    colors.danger_foreground = theme.overlay.error.text.hsla();

    colors.popover = theme.replace_menu.background.hsla();
    colors.popover_foreground = text;
    colors.list = surface;
    colors.list_hover = hover;
    colors.list_active = primary.opacity(0.2);
    colors.list_active_border = primary;

    colors.sidebar = panel;
    colors.sidebar_foreground = text;
    colors.sidebar_border = line;
    colors.sidebar_accent = hover;
    colors.sidebar_accent_foreground = text;
    colors.sidebar_primary = primary;
    colors.sidebar_primary_foreground = surface;

    colors.tab_bar = panel;
    colors.tab = panel;
    colors.tab_active = surface;
    colors.tab_foreground = muted_text;
    colors.tab_active_foreground = text;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin;
    use gpui::TestAppContext;

    /// Solarized light/dark and Tokyo Night drive the kit's background,
    /// foreground, border, accent and primary from their YAML values.
    #[gpui::test]
    fn kit_colors_follow_yaml(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);

        let cases = [
            ("solarized-light", WindowAppearance::Light),
            ("solarized-dark", WindowAppearance::Dark),
            ("tokyonight-night", WindowAppearance::Dark),
            ("tokyonight-day", WindowAppearance::Light),
        ];

        for (name, appearance) in cases {
            let theme = builtin(name).expect(name);

            cx.update(|cx| sync(&theme, appearance, cx));

            cx.update(|cx| {
                let kit = KitTheme::global(cx);
                let colors = &kit.colors;

                assert_eq!(colors.background, theme.canvas.background.hsla(), "{name}");
                assert_eq!(colors.foreground, theme.replace_menu.text.hsla(), "{name}");
                assert_eq!(colors.border, theme.replace_menu.border.hsla(), "{name}");
                assert_eq!(colors.accent, theme.palette.hover.hsla(), "{name}");
                assert_eq!(colors.primary, theme.selection.hsla(), "{name}");
                assert_eq!(kit.mode.is_dark(), appearance == WindowAppearance::Dark);
            });
        }
    }
}
