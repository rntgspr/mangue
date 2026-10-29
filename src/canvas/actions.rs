//! App-wide GPUI actions, their key bindings and the macOS menu bar.
//! Canvas bindings follow diagram-js `KeyboardBindings` and bpmn-js
//! `BpmnKeyboardBindings` (single-letter tools are handled in the view).

use gpui::{Action, App, KeyBinding, Menu, MenuItem, actions};

actions!(
    mangue,
    [
        Open,
        OpenFolder,
        ToggleSidebar,
        ToggleMode,
        Save,
        SaveAs,
        New,
        CloseTab,
        Undo,
        Redo,
        Delete,
        Quit,
        SelectAll,
        Copy,
        Paste,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        ZoomFit,
        AlignLeft,
        AlignCenter,
        AlignRight,
        AlignTop,
        AlignMiddle,
        AlignBottom,
        DistributeHorizontally,
        DistributeVertically,
        Find,
        OpenSettings,
        ClearRecent,
        NextTab,
        PrevTab,
        ActivateLastTab,
    ]
);

/// Opens the `index`-th entry of the recent files list.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = mangue, no_json)]
pub struct OpenRecent {
    pub index: usize,
}

/// Activates the tab at zero-based `index`; out of range is a no-op.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = mangue, no_json)]
pub struct ActivateTab {
    pub index: usize,
}

/// Registers key bindings (`cmd` resolved per platform, see `bind`) and the
/// File/Edit/Arrange/View menu bar; `Quit` and `OpenSettings` are handled here.
pub fn install(cx: &mut App) {
    cx.bind_keys([
        bind("cmd-o", Open, None),
        bind("cmd-shift-o", OpenFolder, None),
        bind("cmd-b", ToggleMode, None),
        bind("cmd-s", Save, None),
        bind("cmd-shift-s", SaveAs, None),
        bind("cmd-n", New, None),
        bind("cmd-w", CloseTab, None),
        bind("cmd-z", Undo, None),
        bind("cmd-shift-z", Redo, None),
        bind("cmd-y", Redo, None),
        bind("backspace", Delete, Some("Canvas")),
        bind("delete", Delete, Some("Canvas")),
        bind("cmd-a", SelectAll, Some("Canvas")),
        bind("cmd-c", Copy, Some("Canvas")),
        bind("cmd-v", Paste, Some("Canvas")),
        bind("cmd-=", ZoomIn, Some("Canvas")),
        bind("cmd-+", ZoomIn, Some("Canvas")),
        bind("cmd--", ZoomOut, Some("Canvas")),
        bind("cmd-0", ZoomReset, Some("Canvas")),
        bind("cmd-shift-0", ZoomFit, Some("Canvas")),
        bind("cmd-f", Find, Some("Canvas")),
        bind("cmd-shift-]", NextTab, None),
        bind("ctrl-tab", NextTab, None),
        bind("cmd-shift-[", PrevTab, None),
        bind("ctrl-shift-tab", PrevTab, None),
        bind("cmd-1", ActivateTab { index: 0 }, None),
        bind("cmd-2", ActivateTab { index: 1 }, None),
        bind("cmd-3", ActivateTab { index: 2 }, None),
        bind("cmd-4", ActivateTab { index: 3 }, None),
        bind("cmd-5", ActivateTab { index: 4 }, None),
        bind("cmd-6", ActivateTab { index: 5 }, None),
        bind("cmd-7", ActivateTab { index: 6 }, None),
        bind("cmd-8", ActivateTab { index: 7 }, None),
        bind("cmd-9", ActivateLastTab, None),
        bind("cmd-q", Quit, None),
        bind("cmd-,", OpenSettings, None),
    ]);
    cx.bind_keys(super::text_input::bindings());

    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &OpenSettings, cx| {
        crate::settings::view::open(cx);
    });

    cx.on_action(|_: &ClearRecent, cx| crate::settings::recent::clear(cx));

    cx.observe_global::<crate::settings::recent::RecentFiles>(set_menus)
        .detach();
    set_menus(cx);
}

/// `spec` resolved for the platform: `cmd-` becomes `ctrl-` off macOS.
pub fn keystroke(spec: &str, macos: bool) -> String {
    match spec.strip_prefix("cmd-") {
        Some(rest) if !macos => format!("ctrl-{rest}"),
        _ => spec.to_string(),
    }
}

/// A key binding whose `spec` is resolved for the current platform.
pub fn bind<A: Action>(spec: &str, action: A, context: Option<&str>) -> KeyBinding {
    KeyBinding::new(&keystroke(spec, cfg!(target_os = "macos")), action, context)
}

/// Installs the menu bar from the current recent files list; re-run whenever
/// that list changes. Off macOS the same menus feed the in-window
/// `AppMenuBar` (see `menu_bar`).
pub fn set_menus(cx: &mut App) {
    if !cfg!(target_os = "macos") {
        let owned = menus(cx).into_iter().map(Menu::owned).collect();
        gpui_component::GlobalState::init(cx);
        gpui_component::GlobalState::global_mut(cx).set_app_menus(owned);
    }

    let menus = menus(cx);
    cx.set_menus(menus);
}

/// The File/Edit/Arrange/View/Window menu definition, with the current
/// recent files under File > Open Recent.
fn menus(cx: &App) -> Vec<Menu> {
    let mut recent: Vec<MenuItem> = crate::settings::recent::current(cx)
        .files()
        .iter()
        .enumerate()
        .map(|(index, path)| MenuItem::action(path.display().to_string(), OpenRecent { index }))
        .collect();
    recent.push(MenuItem::separator());
    recent.push(MenuItem::action("Clear Menu", ClearRecent));

    vec![
        Menu {
            name: "mangue".into(),
            items: vec![
                MenuItem::action("Settings...", OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Quit mangue", Quit),
            ],
            disabled: false,
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New", New),
                MenuItem::action("Open...", Open),
                MenuItem::action("Open Folder...", OpenFolder),
                MenuItem::submenu(Menu {
                    name: "Open Recent".into(),
                    items: recent,
                    disabled: false,
                }),
                MenuItem::action("Close Tab", CloseTab),
                MenuItem::separator(),
                MenuItem::action("Save", Save),
                MenuItem::action("Save As...", SaveAs),
            ],
            disabled: false,
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::action("Delete", Delete),
                MenuItem::separator(),
                MenuItem::action("Select All", SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find...", Find),
            ],
            disabled: false,
        },
        Menu {
            name: "Arrange".into(),
            items: vec![
                MenuItem::action("Align Left", AlignLeft),
                MenuItem::action("Align Center", AlignCenter),
                MenuItem::action("Align Right", AlignRight),
                MenuItem::separator(),
                MenuItem::action("Align Top", AlignTop),
                MenuItem::action("Align Middle", AlignMiddle),
                MenuItem::action("Align Bottom", AlignBottom),
                MenuItem::separator(),
                MenuItem::action("Distribute Horizontally", DistributeHorizontally),
                MenuItem::action("Distribute Vertically", DistributeVertically),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Actual Size", ZoomReset),
                MenuItem::action("Zoom to Fit", ZoomFit),
            ],
            disabled: false,
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Next Tab", NextTab),
                MenuItem::action("Previous Tab", PrevTab),
            ],
            disabled: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::keystroke;

    /// macOS keeps `cmd`; elsewhere `cmd` becomes `ctrl` and other keys pass through.
    #[test]
    fn resolves_cmd_per_platform() {
        assert_eq!(keystroke("cmd-shift-s", true), "cmd-shift-s");
        assert_eq!(keystroke("cmd-shift-s", false), "ctrl-shift-s");
        assert_eq!(keystroke("cmd--", false), "ctrl--");
        assert_eq!(keystroke("ctrl-tab", false), "ctrl-tab");
        assert_eq!(keystroke("ctrl-tab", true), "ctrl-tab");
        assert_eq!(keystroke("backspace", false), "backspace");
    }
}
