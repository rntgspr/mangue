//! `SettingsView`: the Settings window (cmd-,) picking the theme per
//! appearance, grid visibility, snapping and grid size. Every change is
//! applied live through `settings::update` and persisted.

use gpui::{
    App, Bounds, Context, Entity, Global, SharedString, Subscription, Window, WindowBounds,
    WindowHandle, WindowOptions, div, prelude::*, px, size,
};
use gpui_component::setting::{
    SettingField, SettingGroup, SettingItem, SettingPage, Settings as KitSettings,
};
use gpui_component::{ActiveTheme, Root};

use super::{GRID_SIZES, Settings, update};
use crate::theme::{self, Theme};

/// The open settings window, reused by later cmd-, presses.
struct SettingsWindow(WindowHandle<Root>);

impl Global for SettingsWindow {}

/// Opens the settings window, or activates it when already open; returns
/// its `Root` handle.
pub fn open(cx: &mut App) -> WindowHandle<Root> {
    if let Some(handle) = cx.try_global::<SettingsWindow>().map(|w| w.0)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return handle;
    }

    let bounds = Bounds::centered(None, size(px(720.), px(560.)), cx);
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| SettingsView::new(window, cx));

                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("open settings window");

    cx.set_global(SettingsWindow(handle));

    handle
}

/// The `SettingsView` wrapped by the settings window's `Root`.
pub fn settings_view(handle: WindowHandle<Root>, cx: &App) -> Option<Entity<SettingsView>> {
    let root = handle.read(cx).ok()?;

    root.view().clone().downcast::<SettingsView>().ok()
}

/// Settings window root view.
pub struct SettingsView {
    /// Selectable theme names (built-ins and user themes).
    names: Vec<String>,
    /// Repaint on settings, theme and appearance changes.
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    /// Creates the view, listing the available theme names and following
    /// settings, theme and window appearance changes.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title("Settings");
        theme::apply(window.appearance(), cx);

        let subscriptions = vec![
            cx.observe_global_in::<Settings>(window, |_, window, cx| {
                theme::apply(window.appearance(), cx);
                cx.notify();
            }),
            cx.observe_global::<Theme>(|_, cx| cx.notify()),
            cx.observe_window_appearance(window, |_, window, cx| {
                theme::apply(window.appearance(), cx);
                cx.notify();
            }),
        ];

        SettingsView {
            names: theme::names(),
            _subscriptions: subscriptions,
        }
    }

    /// Toggles painting of the dot grid.
    pub fn toggle_grid(&mut self, cx: &mut Context<Self>) {
        change(|s| s.grid.visible = !s.grid.visible, cx);
    }

    /// Toggles snapping to the grid.
    pub fn toggle_snap(&mut self, cx: &mut Context<Self>) {
        change(|s| s.grid.snap = !s.grid.snap, cx);
    }

    /// Sets the grid size in diagram units.
    pub fn set_grid_size(&mut self, grid: f64, cx: &mut Context<Self>) {
        change(|s| s.grid.size = grid, cx);
    }

    /// Picks theme `name` for the light (`dark` false) or dark appearance.
    pub fn pick_theme(&mut self, dark: bool, name: String, cx: &mut Context<Self>) {
        change(|s| pick(s, dark, name), cx);
    }

    /// Theme dropdown for one appearance; an empty value means no explicit
    /// choice (the `theme.yaml` selection applies).
    fn theme_item(&self, dark: bool) -> SettingItem {
        let title = if dark {
            "Dark appearance"
        } else {
            "Light appearance"
        };
        let options = self
            .names
            .iter()
            .map(|name| {
                (
                    SharedString::from(name.clone()),
                    SharedString::from(name.clone()),
                )
            })
            .collect();

        let field = SettingField::scrollable_dropdown(
            options,
            move |cx| {
                let theme = Settings::current(cx).theme;
                let picked = if dark { theme.dark } else { theme.light };
                picked.unwrap_or_default().into()
            },
            move |name, cx| change(|s| pick(s, dark, name.to_string()), cx),
        );

        SettingItem::new(title, field)
    }
}

/// Applies `edit` to a copy of the current settings and updates them (live
/// apply and persistence happen in `settings::update`).
fn change(edit: impl FnOnce(&mut Settings), cx: &mut App) {
    let mut settings = Settings::current(cx);
    edit(&mut settings);

    update(settings, cx);
}

/// Stores theme `name` for the light (`dark` false) or dark appearance.
fn pick(settings: &mut Settings, dark: bool, name: String) {
    match dark {
        false => settings.theme.light = Some(name),
        true => settings.theme.dark = Some(name),
    }
}

impl Render for SettingsView {
    /// Renders the kit settings page: theme dropdowns, grid switches and the
    /// grid size dropdown.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = SettingGroup::new()
            .title("Theme")
            .item(self.theme_item(false))
            .item(self.theme_item(true));

        let visible = SettingField::switch(
            |cx| Settings::current(cx).grid.visible,
            |on, cx| change(|s| s.grid.visible = on, cx),
        );
        let snap = SettingField::switch(
            |cx| Settings::current(cx).grid.snap,
            |on, cx| change(|s| s.grid.snap = on, cx),
        );
        let sizes = GRID_SIZES
            .iter()
            .map(|grid| (grid.to_string().into(), grid.to_string().into()))
            .collect();
        let size = SettingField::dropdown(
            sizes,
            |cx| Settings::current(cx).grid.size.to_string().into(),
            |value: SharedString, cx| {
                if let Ok(grid) = value.parse::<f64>() {
                    change(|s| s.grid.size = grid, cx);
                }
            },
        );
        let grid = SettingGroup::new()
            .title("Grid")
            .item(SettingItem::new("Show grid", visible))
            .item(SettingItem::new("Snap to grid", snap))
            .item(SettingItem::new("Grid size", size));

        let page = SettingPage::new("General")
            .default_open(true)
            .group(theme)
            .group(grid);

        div()
            .id("settings")
            .debug_selector(|| "settings-kit".into())
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(KitSettings::new("settings-page").page(page))
    }
}
