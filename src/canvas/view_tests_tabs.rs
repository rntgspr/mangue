//! Headless tests of the document tab list: switching keeps each document's
//! own state and the window title follows the active tab.

use super::*;

/// Opens a second tab holding the empty diagram, loaded from `path`.
fn open_second(view: &Entity<Canvas>, cx: &mut VisualTestContext, path: Option<&str>) {
    let path = path.map(std::path::PathBuf::from);

    view.update(cx, |c, cx| {
        c.open_tab(crate::io::new_diagram(), path, cx);
    });
}

/// Switching tabs preserves each document's definitions, history, selection
/// and viewport.
#[gpui::test]
fn switching_tabs_preserves_document_state(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    cx.simulate_click(at(400., 300.), Modifiers::none());
    assert_eq!(count(&view, cx, is_task), 1);

    let first_sel = selection(&view, cx);
    view.update(cx, |c, _| c.docs[0].viewport.pan(30.0, 20.0));
    let first_viewport = view.read_with(cx, |c, _| c.docs[0].viewport);

    open_second(&view, cx, None);
    assert_eq!(view.read_with(cx, |c, _| c.active), 1);
    assert_eq!(count(&view, cx, is_task), 0);
    assert!(selection(&view, cx).is_empty());
    assert_eq!(view.read_with(cx, |c, _| c.docs[1].history_len()), 0);

    view.update(cx, |c, cx| c.activate_tab(0, cx));
    assert_eq!(count(&view, cx, is_task), 1);
    assert_eq!(selection(&view, cx), first_sel);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[0].viewport),
        first_viewport
    );
    assert_eq!(view.read_with(cx, |c, _| c.docs[0].history_len()), 1);
    assert!(view.read_with(cx, |c, _| c.is_dirty()));

    view.update(cx, |c, cx| c.activate_tab(1, cx));
    assert!(!view.read_with(cx, |c, _| c.is_dirty()));
}

/// The window title follows the active tab (name and dirty marker).
#[gpui::test]
fn title_follows_active_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    cx.simulate_click(at(400., 300.), Modifiers::none());
    assert_eq!(window_title(&view, cx), "Untitled * - mangue");

    open_second(&view, cx, Some("/tmp/other.bpmn"));
    cx.run_until_parked();
    assert_eq!(window_title(&view, cx), "other.bpmn - mangue");

    view.update(cx, |c, cx| c.activate_tab(0, cx));
    cx.run_until_parked();
    assert_eq!(window_title(&view, cx), "Untitled * - mangue");
}

/// Switching tabs cancels an armed tool and an in-progress label edit.
#[gpui::test]
fn switching_tabs_resets_transient_state(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = Tool::Create(ElementKind::Task, false);

    open_second(&view, cx, None);
    view.update(cx, |c, cx| c.activate_tab(0, cx));

    cx.simulate_click(palette_point(&task), Modifiers::none());
    assert!(view.read_with(cx, |c, _| c.tool.is_some()));

    view.update(cx, |c, cx| c.activate_tab(1, cx));
    assert!(view.read_with(cx, |c, _| c.tool.is_none()));
    assert!(view.read_with(cx, |c, _| c.editing.is_none()));
}

/// Opens `path` through the shared open flow.
fn load_path(view: &Entity<Canvas>, cx: &mut VisualTestContext, path: &std::path::Path) {
    cx.update(|window, app| view.update(app, |c, cx| c.load(path.to_path_buf(), window, cx)));
    cx.run_until_parked();
}

/// Opening a file adds a tab and activates it without any discard prompt,
/// even when the current document is dirty.
#[gpui::test]
fn open_adds_tab_without_prompt(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let dir = sidebar_fixture("tabs-open");
    let file = dir.join("a.bpmn");

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    load_path(&view, cx, &file);

    assert!(!cx.has_pending_prompt());
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 2);
    assert_eq!(view.read_with(cx, |c, _| c.active), 1);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[1].path.clone()),
        Some(file)
    );
    assert!(view.read_with(cx, |c, _| c.docs[0].is_dirty()));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Opening an already open path (also through a different spelling of it)
/// activates its tab instead of adding a duplicate.
#[gpui::test]
fn open_same_path_activates_existing_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let dir = sidebar_fixture("tabs-dedup");
    let file = dir.join("a.bpmn");
    let alias = dir.join(".").join("a.bpmn");

    load_path(&view, cx, &file);
    open_second(&view, cx, None);
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 3);

    load_path(&view, cx, &alias);
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 3);
    assert_eq!(view.read_with(cx, |c, _| c.active), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A failed open adds no tab and keeps the active one.
#[gpui::test]
fn failed_open_adds_no_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    load_path(&view, cx, std::path::Path::new("/tmp/missing-tab.bpmn"));

    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("OK");
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 1);
    assert_eq!(view.read_with(cx, |c, _| c.active), 0);
}

/// cmd-n opens a new Untitled tab and keeps the dirty one untouched.
#[gpui::test]
fn new_opens_untitled_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    cx.simulate_keystrokes("secondary-n");
    cx.run_until_parked();

    assert!(!cx.has_pending_prompt());
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 2);
    assert_eq!(view.read_with(cx, |c, _| c.active), 1);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[1].title_name()),
        "Untitled"
    );
    assert!(view.read_with(cx, |c, _| c.docs[0].is_dirty()));
    assert!(!view.read_with(cx, |c, _| c.docs[1].is_dirty()));
}

/// Makes the active document dirty by deleting the start event.
fn dirty_active(cx: &mut VisualTestContext) {
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
}

/// cmd-w on a clean tab closes it at once and keeps `active` valid.
#[gpui::test]
fn close_clean_tab_closes_immediately(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    open_second(&view, cx, Some("/tmp/other.bpmn"));
    cx.simulate_keystrokes("secondary-w");
    cx.run_until_parked();

    assert!(!cx.has_pending_prompt());
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 1);
    assert_eq!(view.read_with(cx, |c, _| c.active), 0);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[0].title_name()),
        "Untitled"
    );
}

/// A dirty tab prompts; Cancel keeps it, Don't Save closes only that tab.
#[gpui::test]
fn close_dirty_tab_prompts(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    dirty_active(cx);
    open_second(&view, cx, Some("/tmp/other.bpmn"));
    cx.update(|window, app| view.update(app, |c, cx| c.close_tab(0, window, cx)));
    assert!(cx.has_pending_prompt());

    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 2);

    cx.update(|window, app| view.update(app, |c, cx| c.close_tab(0, window, cx)));
    cx.simulate_prompt_answer("Don't Save");
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 1);
    assert_eq!(view.read_with(cx, |c, _| c.active), 0);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[0].title_name()),
        "other.bpmn"
    );
}

/// The window close path prompts for every dirty tab in turn, activating
/// each, and aborts on Cancel.
#[gpui::test]
fn window_close_prompts_each_dirty_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    dirty_active(cx);
    open_second(&view, cx, None);
    dirty_active(cx);
    view.update(cx, |c, cx| c.activate_tab(0, cx));

    let closed = cx.update(|window, app| view.update(app, |c, cx| c.should_close(window, cx)));
    assert!(!closed);
    assert!(cx.has_pending_prompt());

    cx.simulate_prompt_answer("Don't Save");
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |c, _| c.active), 1);
    assert!(cx.has_pending_prompt());

    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 2);
}

/// With every tab clean the window closes without a prompt.
#[gpui::test]
fn window_close_with_clean_tabs_does_not_prompt(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    open_second(&view, cx, None);
    let closed = cx.update(|window, app| view.update(app, |c, cx| c.should_close(window, cx)));

    assert!(closed);
    assert!(!cx.has_pending_prompt());
}

/// Opens `extra` more tabs (path-less) and activates tab `start`.
fn tabs_at(view: &Entity<Canvas>, cx: &mut VisualTestContext, extra: usize, start: usize) {
    for _ in 0..extra {
        open_second(view, cx, None);
    }

    view.update(cx, |c, cx| c.activate_tab(start, cx));
}

/// Index of the active tab.
fn active_tab(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> usize {
    view.read_with(cx, |c, _| c.active)
}

/// Next tab advances and wraps from the last to the first.
#[gpui::test]
fn next_tab_wraps_around(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    tabs_at(&view, cx, 2, 1);

    cx.simulate_keystrokes("secondary-shift-]");
    assert_eq!(active_tab(&view, cx), 2);

    cx.simulate_keystrokes("secondary-shift-]");
    assert_eq!(active_tab(&view, cx), 0);

    cx.simulate_keystrokes("ctrl-tab");
    assert_eq!(active_tab(&view, cx), 1);
}

/// Previous tab retreats and wraps from the first to the last.
#[gpui::test]
fn prev_tab_wraps_around(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    tabs_at(&view, cx, 2, 1);

    cx.simulate_keystrokes("secondary-shift-[");
    assert_eq!(active_tab(&view, cx), 0);

    cx.simulate_keystrokes("secondary-shift-[");
    assert_eq!(active_tab(&view, cx), 2);

    cx.simulate_keystrokes("ctrl-shift-tab");
    assert_eq!(active_tab(&view, cx), 1);
}

/// cmd-N activates the N-th tab; an out-of-range N does nothing.
#[gpui::test]
fn cmd_number_activates_nth_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    tabs_at(&view, cx, 2, 0);

    cx.simulate_keystrokes("secondary-3");
    assert_eq!(active_tab(&view, cx), 2);

    cx.simulate_keystrokes("secondary-2");
    assert_eq!(active_tab(&view, cx), 1);

    cx.simulate_keystrokes("secondary-8");
    assert_eq!(active_tab(&view, cx), 1);
}

/// cmd-9 activates the last tab whatever the count.
#[gpui::test]
fn cmd_nine_activates_last_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    tabs_at(&view, cx, 3, 0);

    cx.simulate_keystrokes("secondary-9");
    assert_eq!(active_tab(&view, cx), 3);
}

/// Whether the active document is in code mode.
fn code_mode(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |c, _| c.docs[c.active].code_mode)
}

/// Each tab keeps its own mode across tab switches.
#[gpui::test]
fn mode_is_kept_per_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.dispatch_action(actions::ToggleMode);
    open_second(&view, cx, None);
    assert!(!code_mode(&view, cx));

    view.update(cx, |c, cx| c.activate_tab(0, cx));
    assert!(code_mode(&view, cx));

    view.update(cx, |c, cx| c.activate_tab(1, cx));
    assert!(!code_mode(&view, cx));
}

/// Clicking a rendered tab activates it; its close button closes that tab,
/// and the dirty tab shows the ` *` marker.
#[gpui::test]
fn tab_strip_clicks_activate_and_close(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    open_second(&view, cx, Some("/tmp/other.bpmn"));
    cx.run_until_parked();
    assert_eq!(active_tab(&view, cx), 1);

    let first = cx.debug_bounds("tab-0").expect("first tab rendered");
    cx.simulate_click(first.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(active_tab(&view, cx), 0);

    dirty_active(cx);
    cx.run_until_parked();
    assert!(cx.debug_bounds("tab-dirty-0").is_some());
    assert!(cx.debug_bounds("tab-dirty-1").is_none());

    let close = cx
        .debug_bounds("tab-close-1")
        .expect("close button rendered");
    cx.simulate_click(close.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 1);
}
