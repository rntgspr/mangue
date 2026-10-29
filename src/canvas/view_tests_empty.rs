//! Headless tests of a `Canvas` holding no document (empty state).

use super::*;
use crate::canvas::actions::{Redo, Save, ToggleMode, Undo, ZoomIn, ZoomOut, ZoomReset};

/// Opens a window whose root is a `Canvas` with zero documents.
fn open_empty(cx: &mut TestAppContext) -> (Entity<Canvas>, &mut VisualTestContext) {
    cx.update(actions::install);
    cx.update(crate::canvas::init_editor);

    cx.add_window_view(Canvas::empty)
}

/// An empty canvas has zero documents and the bare app title.
#[gpui::test]
fn startup_without_file_has_no_documents(cx: &mut TestAppContext) {
    let (view, cx) = open_empty(cx);
    cx.run_until_parked();

    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 0);
    assert_eq!(window_title(&view, cx), "mangue");
}

/// Closing the only tab leaves zero documents and the bare app title.
#[gpui::test]
fn closing_last_tab_leaves_no_documents(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.update(|window, app| view.update(app, |c, cx| c.close_tab(0, window, cx)));
    cx.run_until_parked();

    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 0);
    assert_eq!(window_title(&view, cx), "mangue");
}

/// Document actions are no-ops, not panics, without a document.
#[gpui::test]
fn document_actions_without_document_do_not_panic(cx: &mut TestAppContext) {
    let (view, cx) = open_empty(cx);

    cx.simulate_keystrokes("secondary-z secondary-shift-z secondary-s secondary-shift-s secondary-b secondary-= secondary-- secondary-0 secondary-w");
    cx.simulate_keystrokes("secondary-1 secondary-9 secondary-shift-] secondary-shift-[");
    cx.run_until_parked();

    view.update(cx, |c, cx| {
        c.toggle_mode(cx);
        c.fit_pending = true;
    });
    cx.dispatch_action(Undo);
    cx.dispatch_action(Redo);
    cx.dispatch_action(Save);
    cx.dispatch_action(ZoomIn);
    cx.dispatch_action(ZoomOut);
    cx.dispatch_action(ZoomReset);
    cx.dispatch_action(ToggleMode);
    cx.run_until_parked();

    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 0);
    assert!(!view.read_with(cx, |c, _| c.is_dirty()));
    assert!(!cx.has_pending_prompt());
}

/// New from the empty state adds one active Untitled tab.
#[gpui::test]
fn new_from_empty_adds_tab(cx: &mut TestAppContext) {
    let (view, cx) = open_empty(cx);

    cx.simulate_keystrokes("secondary-n");
    cx.run_until_parked();

    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 1);
    assert_eq!(view.read_with(cx, |c, _| c.active), 0);
    assert_eq!(window_title(&view, cx), "Untitled - mangue");
}

/// Opening a document from the empty state adds one active tab.
#[gpui::test]
fn open_tab_from_empty_adds_tab(cx: &mut TestAppContext) {
    let (view, cx) = open_empty(cx);

    view.update(cx, |c, cx| {
        c.open_tab(crate::io::new_diagram(), Some("/tmp/x.bpmn".into()), cx)
    });
    cx.run_until_parked();

    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 1);
    assert_eq!(view.read_with(cx, |c, _| c.active), 0);
    assert_eq!(window_title(&view, cx), "x.bpmn - mangue");
}

/// Opens a window whose root is a `Canvas` with zero documents.
fn open_no_document(cx: &mut TestAppContext) -> (Entity<Canvas>, &mut VisualTestContext) {
    cx.update(actions::install);
    cx.update(crate::canvas::init_editor);

    cx.add_window_view(Canvas::empty)
}

/// A canvas without documents renders the empty state; one with a document does not.
#[gpui::test]
fn empty_state_renders_only_without_documents(cx: &mut TestAppContext) {
    let (_view, cx) = open_no_document(cx);
    cx.run_until_parked();
    assert!(cx.debug_bounds("empty-state").is_some());
    assert!(cx.debug_bounds("empty-state-open").is_some());

    let (_view, cx) = open(cx);
    cx.run_until_parked();
    assert!(cx.debug_bounds("empty-state").is_none());
}

/// Clicking "Open File" runs the Open flow, the same as cmd-o.
#[gpui::test]
fn empty_state_button_dispatches_open(cx: &mut TestAppContext) {
    let (_view, cx) = open_no_document(cx);
    cx.run_until_parked();
    assert!(!cx.did_prompt_for_paths());

    let button = cx
        .debug_bounds("empty-state-open")
        .expect("open button rendered");
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();

    assert!(cx.did_prompt_for_paths());
}

/// The empty state is a gpui-component `Empty` whose header slots (title,
/// Cmd+O description) are tagged for lookup.
#[gpui::test]
fn empty_state_uses_kit_empty_header(cx: &mut TestAppContext) {
    let (_view, cx) = open_no_document(cx);
    cx.run_until_parked();

    assert!(cx.debug_bounds("empty-state-title").is_some());
    assert!(cx.debug_bounds("empty-state-hint").is_some());
}
