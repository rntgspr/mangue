//! Headless tests of the Settings window driving open canvases: grid
//! toggles and theme picks apply live (no settings file is installed, so
//! nothing is saved).

use super::*;
use crate::settings::{Settings, view};

/// Recent file changes rebuild the installed menu before and after windows exist.
#[gpui::test]
fn recent_files_rebuild_installed_menu(cx: &mut TestAppContext) {
    use gpui::OwnedMenuItem;
    use std::path::Path;

    let path = Path::new("/tmp/mangue-recent-test.bpmn");
    cx.update(|cx| crate::settings::recent::record(path, cx));
    cx.update(actions::install);
    cx.run_until_parked();

    let contains_path = |cx: &mut gpui::App| {
        let menus = cx.get_menus().expect("menus installed");
        let file = menus.iter().find(|menu| menu.name == "File").unwrap();
        let recent = file
            .items
            .iter()
            .find_map(|item| match item {
                OwnedMenuItem::Submenu(menu) if menu.name == "Open Recent" => Some(menu),
                _ => None,
            })
            .unwrap();

        recent.items.iter().any(|item| matches!(item, OwnedMenuItem::Action { name, .. } if name == path.to_str().unwrap()))
    };

    assert!(cx.update(contains_path));
    cx.update(|cx| crate::settings::recent::forget(path, cx));
    cx.run_until_parked();
    assert!(!cx.update(contains_path));

    cx.update(|cx| crate::settings::recent::record(path, cx));
    cx.run_until_parked();
    assert!(cx.update(contains_path));

    cx.update(crate::settings::recent::clear);
    cx.run_until_parked();
    assert!(!cx.update(contains_path));
}

/// cmd-, opens the settings window; toggling grid and snap there updates
/// the canvas grid state, and a grid size pick reaches it too.
#[gpui::test]
fn settings_window_toggles_canvas_grid(cx: &mut TestAppContext) {
    let (canvas, cx) = open(cx);
    cx.update(|_, cx| cx.set_global(Settings::default()));
    assert!(canvas.read_with(cx, |c, _| c.grid.visible && c.grid.snap));

    cx.simulate_keystrokes("secondary-,");
    let settings = cx.update(|_, cx| {
        let handle = view::open(cx);
        view::settings_view(handle, cx).expect("settings view inside Root")
    });

    settings.update(cx, |v, cx| {
        v.toggle_grid(cx);
        v.toggle_snap(cx);
        v.set_grid_size(20.0, cx);
    });
    cx.run_until_parked();

    let grid = canvas.read_with(cx, |c, _| c.grid.clone());
    assert!(!grid.visible);
    assert!(!grid.snap);
    assert_eq!(grid.size, 20.0);
    assert_eq!(grid.snap(31.0), 31.0);
}

/// Picking a light theme in the settings window repaints the canvas with it.
#[gpui::test]
fn settings_window_picks_theme(cx: &mut TestAppContext) {
    let (_canvas, cx) = open(cx);
    cx.update(|_, cx| cx.set_global(Settings::default()));
    let settings = cx.update(|_, cx| {
        let handle = view::open(cx);
        view::settings_view(handle, cx).expect("settings view inside Root")
    });

    settings.update(cx, |v, cx| v.pick_theme(false, "tokyonight-day".into(), cx));
    cx.run_until_parked();

    let day = theme::builtin("tokyonight-day").unwrap();
    cx.update(|window, cx| {
        assert_eq!(window.appearance(), WindowAppearance::Light);
        assert_eq!(Theme::current(cx).canvas, day.canvas);
        assert_eq!(Theme::current(cx).grid, day.grid);
    });
}

/// The settings window renders a gpui-component `Settings` page.
#[gpui::test]
fn settings_window_renders_kit_settings(cx: &mut TestAppContext) {
    let (_canvas, cx) = open(cx);
    cx.update(|_, cx| cx.set_global(Settings::default()));
    let handle = cx.update(|_, cx| view::open(cx));
    cx.run_until_parked();

    let mut settings_cx = VisualTestContext::from_window(handle.into(), cx);
    settings_cx.run_until_parked();
    assert!(settings_cx.debug_bounds("settings-kit").is_some());
}
