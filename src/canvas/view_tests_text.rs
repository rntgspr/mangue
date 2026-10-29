//! Headless interaction tests of the `TextInput` overlays: multi-line label
//! editing with caret motion, cmd-f search and the append-anything menu.

use super::*;

/// Clicks the start event so it is the single selection.
fn select_start(view: &Entity<Canvas>, cx: &mut VisualTestContext) {
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    assert_eq!(selection(view, cx), vec![ElementId::from(START)]);
}

/// Name of the start event.
fn start_name(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |c, _| {
        c.definitions()
            .get(&ElementId::from(START))
            .unwrap()
            .name
            .clone()
    })
}

/// Shift+Enter inserts a newline, arrows move the caret anywhere, shift
/// selects and typing replaces the selection; Enter commits.
#[gpui::test]
fn label_edit_multiline_caret(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("e");
    cx.simulate_input("ab");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("cd");
    cx.simulate_keystrokes("up left");
    cx.simulate_input("X");
    cx.simulate_keystrokes("secondary-right shift-left");
    cx.simulate_input("Y");
    cx.simulate_keystrokes("enter");

    assert!(view.read_with(cx, |c, _| c.editing.is_none()));
    assert_eq!(start_name(&view, cx).as_deref(), Some("aXY\ncd"));
}

/// Escape discards the edit; a click outside the editor commits it.
#[gpui::test]
fn label_edit_cancel_and_click_out(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("e");
    cx.simulate_input("Nope");
    cx.simulate_keystrokes("escape");
    assert_eq!(start_name(&view, cx), None);

    cx.simulate_keystrokes("e");
    cx.simulate_input("Kept");
    cx.simulate_click(at(600., 500.), Modifiers::none());
    assert!(view.read_with(cx, |c, _| c.editing.is_none()));
    assert_eq!(start_name(&view, cx).as_deref(), Some("Kept"));
}

/// Single-letter shortcuts type into the editor instead of arming tools,
/// and cmd-a selects the text, not the diagram.
#[gpui::test]
fn label_edit_captures_keys(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("e");
    cx.simulate_input("hlsc");
    assert_eq!(view.read_with(cx, |c, _| c.tool.clone()), None);

    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("Z");
    cx.simulate_keystrokes("enter");
    assert_eq!(start_name(&view, cx).as_deref(), Some("Z"));
}

/// cmd-f opens search; typing filters by name or id, arrows move the
/// highlight and Enter selects the hit and centers the view on it.
#[gpui::test]
fn search_selects_and_centers(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = create(&view, cx, ElementKind::Task, false, 500., 400.);
    cx.simulate_click(at(800., 600.), Modifiers::none());

    cx.simulate_keystrokes("secondary-f");
    assert!(view.read_with(cx, |c, _| c.search.is_some()));

    cx.simulate_input("_");
    let hits = view.update(cx, |c, cx| c.search_hits(cx).len());
    assert!(hits >= 2);

    cx.simulate_keystrokes("down up down");
    assert_eq!(
        view.read_with(cx, |c, _| c.search.as_ref().unwrap().active),
        1
    );

    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("activity");
    cx.simulate_keystrokes("enter");

    assert!(view.read_with(cx, |c, _| c.search.is_none()));
    assert_eq!(selection(&view, cx), vec![task.clone()]);

    let (screen, size) = view.read_with(cx, |c, _| {
        let b = c.definitions().shape(&task).unwrap().bounds;
        (
            c.docs[c.active].viewport.to_screen(center(&b)),
            c.size.get(),
        )
    });
    assert!((screen.x - size.0 / 2.0).abs() < 0.5);
    assert!((screen.y - size.1 / 2.0).abs() < 0.5);
}

/// Escape closes search without changing the selection.
#[gpui::test]
fn search_escape_closes(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("secondary-f");
    cx.simulate_input("start");
    cx.simulate_keystrokes("escape");

    assert!(view.read_with(cx, |c, _| c.search.is_none()));
    assert_eq!(selection(&view, cx), vec![ElementId::from(START)]);
}

/// `a` opens the append menu; typing filters it and Enter appends the
/// highlighted kind, connected to the selected shape.
#[gpui::test]
fn append_menu_key(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("a");
    assert!(view.read_with(cx, |c, _| c.append_menu.is_some()));

    cx.simulate_input("user");
    cx.simulate_keystrokes("enter");

    assert!(view.read_with(cx, |c, _| c.append_menu.is_none()));
    assert_eq!(count(&view, cx, |k| matches!(k, ElementKind::UserTask)), 1);
    assert_eq!(count(&view, cx, is_flow), 1);
}

/// The context-pad append-anything entry opens the same menu.
#[gpui::test]
fn append_menu_pad_entry(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    let entry = pad_point(&view, cx, START, &PadEntry::AppendAnything);
    cx.simulate_click(entry, Modifiers::none());
    assert!(view.read_with(cx, |c, _| c.append_menu.is_some()));

    cx.simulate_keystrokes("escape");
    assert!(view.read_with(cx, |c, _| c.append_menu.is_none()));
}

/// The search overlay renders a kit input and one row per hit; clicking a
/// row selects that hit and closes search.
#[gpui::test]
fn search_hit_row_click_picks(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = create(&view, cx, ElementKind::Task, false, 500., 400.);
    cx.simulate_click(at(800., 600.), Modifiers::none());

    cx.simulate_keystrokes("secondary-f");
    cx.run_until_parked();
    assert!(cx.debug_bounds("search-input").is_some());

    cx.simulate_input("activity");
    cx.run_until_parked();
    let row = cx.debug_bounds("search-hit-0").expect("hit row rendered");
    cx.simulate_click(row.center(), Modifiers::none());

    assert!(view.read_with(cx, |c, _| c.search.is_none()));
    assert_eq!(selection(&view, cx), vec![task]);
}

/// The append menu renders a kit input and one kit row per option;
/// clicking the filtered row appends that kind.
#[gpui::test]
fn append_menu_row_click_appends(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert!(cx.debug_bounds("append-input").is_some());

    cx.simulate_input("user");
    cx.run_until_parked();
    let row = cx
        .debug_bounds("append-option-0")
        .expect("option row rendered");
    assert!(cx.debug_bounds("append-option-1").is_none());
    cx.simulate_click(row.center(), Modifiers::none());

    assert!(view.read_with(cx, |c, _| c.append_menu.is_none()));
    assert_eq!(count(&view, cx, |k| matches!(k, ElementKind::UserTask)), 1);
}
