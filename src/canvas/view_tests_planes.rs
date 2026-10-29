//! Headless tests of drill-down: the open button of a collapsed
//! sub-process, the breadcrumbs back to the root with per-plane viewports,
//! and collapse/expand from the replace menu with undo.

use super::*;
use crate::canvas::Viewport;
use crate::canvas::drilldown::{button_rect, crumbs};

/// bpmn-js collapsed sub-process fixture (`SubProcess_1` at 270,80 100x80).
const COLLAPSED: &str = include_str!("../../tests/fixtures/collapsed-subprocess.bpmn");

/// Opens a canvas window on the collapsed sub-process fixture.
fn open_collapsed(cx: &mut TestAppContext) -> (Entity<Canvas>, &mut VisualTestContext) {
    cx.update(actions::install);
    cx.update(crate::canvas::init_editor);
    let defs = crate::io::import_xml(COLLAPSED).expect("fixture imports");

    cx.add_window_view(|window, cx| Canvas::new(defs, None, window, cx))
}

/// Sub-process whose plane is shown.
fn owner(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> Option<ElementId> {
    view.read_with(cx, |c, _| c.definitions().plane_owner().cloned())
}

/// Window point at the center of the drill-down button of `SubProcess_1`.
fn drill_point(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> Point<Pixels> {
    view.read_with(cx, |c, _| {
        let b = c
            .definitions()
            .shape(&"SubProcess_1".into())
            .unwrap()
            .bounds;
        let r = button_rect(&c.docs[c.active].viewport.bounds_to_screen(b));
        at((r.x + r.w / 2.0) as f32, (r.y + r.h / 2.0) as f32)
    })
}

/// Clicking the drill-down button opens the sub-process plane; the root
/// crumb leads back, and each plane keeps its own viewport.
#[gpui::test]
fn drill_down_and_breadcrumb_back(cx: &mut TestAppContext) {
    let (view, cx) = open_collapsed(cx);

    let button = drill_point(&view, cx);
    cx.simulate_click(button, Modifiers::none());
    assert_eq!(owner(&view, cx), Some("SubProcess_1".into()));
    assert!(view.read_with(cx, |c, _| {
        c.definitions().shape(&"Task_1".into()).is_some()
    }));

    let labels: Vec<String> = view.read_with(cx, |c, _| {
        crumbs(c.definitions())
            .into_iter()
            .map(|c| c.label)
            .collect()
    });
    assert_eq!(labels, vec!["Process_1".to_string(), "Pack order".into()]);

    view.update(cx, |c, _| c.docs[c.active].viewport.pan(-40.0, -30.0));
    let sub_viewport = view.read_with(cx, |c, _| c.docs[c.active].viewport);

    let bar = cx
        .debug_bounds("breadcrumbs")
        .expect("breadcrumbs rendered");
    let root_crumb = point(bar.origin.x + px(16.), bar.center().y);
    cx.simulate_click(root_crumb, Modifiers::none());
    assert_eq!(owner(&view, cx), None);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].viewport),
        Viewport::default()
    );

    cx.simulate_click(button, Modifiers::none());
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].viewport),
        sub_viewport
    );
}

/// The replace menu's first row expands the selected collapsed
/// sub-process onto the root plane; cmd-z collapses it back into its plane.
#[gpui::test]
fn replace_menu_toggles_collapse_with_undo(cx: &mut TestAppContext) {
    let (view, cx) = open_collapsed(cx);
    let sub = ElementId::from("SubProcess_1");

    cx.simulate_click(at(320., 120.), Modifiers::none());
    assert_eq!(selection(&view, cx), vec![sub.clone()]);
    cx.simulate_keystrokes("r");
    assert!(view.read_with(cx, |c, _| c.replacing));

    let row = view.read_with(cx, |c, _| {
        let kind = &c.definitions().get(&sub).unwrap().kind;
        let groups = context_pad::entries(kind, true);
        let screen = c.docs[c.active]
            .viewport
            .bounds_to_screen(c.definitions().shape(&sub).unwrap().bounds);
        let origin = context_pad::origin_for(&screen);
        at(
            (origin.x + 30.0) as f32,
            (origin.y + context_pad::pad_size(&groups).1 + 4.0 + 15.0) as f32,
        )
    });
    cx.simulate_click(row, Modifiers::none());

    let expanded = view.read_with(cx, |c, _| {
        let defs = c.definitions();
        (
            defs.shape(&sub).unwrap().is_expanded,
            defs.diagrams.len(),
            defs.shape(&"Task_1".into()).is_some(),
        )
    });
    assert_eq!(expanded, (Some(true), 1, true));

    cx.simulate_keystrokes("secondary-z");
    let collapsed = view.read_with(cx, |c, _| {
        let defs = c.definitions();
        (defs.shape(&sub).unwrap().is_expanded, defs.diagrams.len())
    });
    assert_eq!(collapsed, (None, 2));
}
