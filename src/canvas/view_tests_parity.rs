//! Headless interaction tests of the T4 parity gestures: bpmn-js keyboard
//! bindings, copy/paste, zoom and fit, align, grid snapping, connection
//! reconnect and independent label moves.

use super::*;
use crate::canvas::actions::AlignTop;
use crate::modeling::label::shape_label_bounds;

/// Clicks the start event so it is the single selection.
fn select_start(view: &Entity<Canvas>, cx: &mut VisualTestContext) {
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    assert_eq!(selection(view, cx), vec![ElementId::from(START)]);
}

/// `h`, `l`, `s` and `c` arm the hand, lasso, space and global connect
/// tools (bpmn-js `BpmnKeyboardBindings`); Escape disarms.
#[gpui::test]
fn letter_keys_arm_tools(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let tool = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| c.tool.clone())
    };

    for (key, expected) in [
        ("h", Tool::Hand),
        ("l", Tool::Lasso),
        ("s", Tool::Space),
        ("c", Tool::Connect),
    ] {
        cx.simulate_keystrokes(key);
        assert_eq!(tool(&view, cx), Some(expected));
    }

    cx.simulate_keystrokes("escape");
    assert_eq!(tool(&view, cx), None);
}

/// `e` opens the label editor on the selection; `r` opens the replace menu.
#[gpui::test]
fn letter_keys_edit_and_replace(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("r");
    assert!(view.read_with(cx, |c, _| c.replacing));

    cx.simulate_keystrokes("e");
    assert!(view.read_with(cx, |c, _| c.editing.is_some()));
}

/// cmd-a selects every shape and connection; Backspace then deletes them.
#[gpui::test]
fn select_all_then_delete(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    create(&view, cx, ElementKind::Task, false, 400., 300.);

    cx.simulate_keystrokes("secondary-a");
    assert_eq!(selection(&view, cx).len(), 2);

    cx.simulate_keystrokes("backspace");
    assert_eq!(count(&view, cx, is_task), 0);
}

/// Arrows move the selection 1px, shift + arrows 10px (diagram-js
/// `KeyboardMoveSelection`); cmd + arrows scroll the canvas instead.
#[gpui::test]
fn arrows_move_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let start = ElementId::from(START);
    select_start(&view, cx);

    cx.simulate_keystrokes("right");
    cx.simulate_keystrokes("shift-down");
    let b = bounds_of(&view, cx, &start);
    assert_eq!((b.x, b.y), (174.0, 112.0));

    cx.simulate_keystrokes("secondary-left");
    assert_eq!(bounds_of(&view, cx, &start).x, 174.0);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].viewport.offset.x),
        50.0
    );

    cx.simulate_keystrokes("secondary-z");
    assert_eq!(bounds_of(&view, cx, &start).y, 102.0);
}

/// cmd-c / cmd-v pastes a copy with a fresh id centered at the cursor and
/// selects it.
#[gpui::test]
fn copy_paste_at_cursor(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    cx.simulate_keystrokes("secondary-c");
    cx.simulate_mouse_move(at(400., 300.), None, Modifiers::none());
    cx.simulate_keystrokes("secondary-v");

    let pasted = selection(&view, cx);
    assert_eq!(pasted.len(), 1);
    assert_ne!(pasted[0], ElementId::from(START));
    assert_eq!(
        center(&bounds_of(&view, cx, &pasted[0])),
        Waypoint { x: 400.0, y: 300.0 }
    );
    assert!(matches!(
        kind_of(&view, cx, &pasted[0]),
        ElementKind::StartEvent(_)
    ));
}

/// cmd-= / cmd-- step the zoom on the log scale; cmd-0 resets it to 1.
#[gpui::test]
fn zoom_keys(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let scale = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| c.docs[c.active].viewport.scale)
    };

    cx.simulate_keystrokes("secondary-=");
    assert!(scale(&view, cx) > 1.0);

    cx.simulate_keystrokes("secondary-0");
    assert!((scale(&view, cx) - 1.0).abs() < 1e-9);

    cx.simulate_keystrokes("secondary--");
    assert!(scale(&view, cx) < 1.0);
}

/// Opening a file fits the diagram into the canvas once its size is known
/// (diagram-js `zoom('fit-viewport', 'auto')`): the content is centered.
#[gpui::test]
fn open_fits_viewport(cx: &mut TestAppContext) {
    cx.update(actions::install);
    cx.update(crate::canvas::init_editor);
    let path = std::path::PathBuf::from("fit.bpmn");
    let (view, cx) = cx.add_window_view(move |window, cx| {
        Canvas::new(crate::io::new_diagram(), Some(path), window, cx)
    });
    cx.run_until_parked();
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    view.read_with(cx, |c, _| {
        assert!(!c.fit_pending);

        let (w, h) = c.size.get();
        let mid = c.docs[c.active]
            .viewport
            .to_screen(Waypoint { x: 191.0, y: 120.0 });
        assert!((mid.x - w / 2.0).abs() < 1e-6 && (mid.y - h / 2.0).abs() < 1e-6);
    });
}

/// Arrange > Align Top lines two selected tasks up on the topmost edge.
#[gpui::test]
fn align_top_action(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let a = create(&view, cx, ElementKind::Task, false, 400., 300.);
    let b = create(&view, cx, ElementKind::Task, false, 600., 360.);
    view.update(cx, |c, _| {
        c.docs[c.active].selection = vec![a.clone(), b.clone()]
    });

    cx.dispatch_action(AlignTop);
    assert_eq!(bounds_of(&view, cx, &b).y, bounds_of(&view, cx, &a).y);
}

/// Creating from the palette snaps the new shape's center to the 10px grid.
#[gpui::test]
fn create_snaps_to_grid(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    cx.simulate_click(at(403., 297.), Modifiers::none());

    let id = selection(&view, cx)[0].clone();
    assert_eq!(
        center(&bounds_of(&view, cx, &id)),
        Waypoint { x: 400.0, y: 300.0 }
    );
}

/// Dragging the end handle of the selected flow onto another task
/// reconnects it there (diagram-js `BendpointMove` reconnect); undo restores.
#[gpui::test]
fn drag_end_reconnects(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let flow = bent_flow(&view, cx);
    let other = create(&view, cx, ElementKind::Task, false, 450., 550.);
    select_edge(&view, cx, &flow);

    let end = *waypoints_of(&view, cx, &flow).last().unwrap();
    drag(cx, win(end), at(450., 550.));

    let endpoints =
        |view: &Entity<Canvas>, cx: &mut VisualTestContext| match kind_of(view, cx, &flow) {
            ElementKind::SequenceFlow { target, .. } => target,
            _ => panic!("not a flow"),
        };
    assert_eq!(endpoints(&view, cx), other);

    cx.simulate_keystrokes("secondary-z");
    assert_ne!(endpoints(&view, cx), other);
}

/// Dragging a named event's external label moves only the label.
#[gpui::test]
fn drag_label_moves_label_only(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let start = ElementId::from(START);
    view.update(cx, |c, cx| {
        c.execute(
            Command::UpdateLabel {
                id: start.clone(),
                name: "Begin".into(),
            },
            cx,
        );
    });

    let shape = bounds_of(&view, cx, &start);
    let label = shape_label_bounds(&ElementKind::StartEvent(EventDefinition::None), &shape);
    let from = center(&label);
    drag(
        cx,
        win(from),
        win(Waypoint {
            x: from.x + 30.,
            y: from.y + 20.,
        }),
    );

    assert_eq!(bounds_of(&view, cx, &start), shape);
    let moved = view.read_with(cx, |c, _| {
        c.definitions().shape(&start).unwrap().label_bounds.unwrap()
    });
    assert_eq!((moved.x, moved.y), (label.x + 30., label.y + 20.));
}

/// cmd-z undoes, cmd-shift-z and cmd-y redo (diagram-js `undo` / `redo`).
#[gpui::test]
fn undo_redo_keys(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    create(&view, cx, ElementKind::Task, false, 400., 300.);
    assert_eq!(count(&view, cx, is_task), 1);

    cx.simulate_keystrokes("secondary-z");
    assert_eq!(count(&view, cx, is_task), 0);

    cx.simulate_keystrokes("secondary-shift-z");
    assert_eq!(count(&view, cx, is_task), 1);

    cx.simulate_keystrokes("secondary-z");
    cx.simulate_keystrokes("secondary-y");
    assert_eq!(count(&view, cx, is_task), 1);
}

/// `delete` removes the selection like backspace (`removeSelection`).
#[gpui::test]
fn delete_key_removes_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    create(&view, cx, ElementKind::Task, false, 400., 300.);
    cx.simulate_click(at(400., 300.), Modifiers::none());
    assert_eq!(selection(&view, cx).len(), 1);

    cx.simulate_keystrokes("delete");
    assert_eq!(count(&view, cx, is_task), 0);
}

/// cmd-+ and cmd-= zoom in, cmd-- zooms out, cmd-0 resets (`stepZoom`,
/// `zoom`), one step each.
#[gpui::test]
fn zoom_plus_minus_keys(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let scale = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| c.docs[c.active].viewport.scale)
    };

    cx.simulate_keystrokes("secondary-+");
    let in_plus = scale(&view, cx);
    assert!(in_plus > 1.0);

    cx.simulate_keystrokes("secondary-0");
    cx.simulate_keystrokes("secondary-=");
    assert!((scale(&view, cx) - in_plus).abs() < 1e-9);

    cx.simulate_keystrokes("secondary-0");
    cx.simulate_keystrokes("secondary--");
    assert!(scale(&view, cx) < 1.0);
}

/// Tool, edit and replace letters only act without modifiers
/// (`hasModifier` guard of `BpmnKeyboardBindings`).
#[gpui::test]
fn modified_letters_do_nothing(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    select_start(&view, cx);

    for key in ["shift-h", "alt-l", "ctrl-s", "shift-c", "alt-e", "shift-r"] {
        cx.simulate_keystrokes(key);
        assert_eq!(view.read_with(cx, |c, _| c.tool.clone()), None, "{key}");
        assert!(view.read_with(cx, |c, _| c.editing.is_none()), "{key}");
        assert!(!view.read_with(cx, |c, _| c.replacing), "{key}");
    }
}

/// `e` and `r` do nothing without a single selected element they apply to.
#[gpui::test]
fn edit_and_replace_need_a_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_keystrokes("e");
    assert!(view.read_with(cx, |c, _| c.editing.is_none()));

    cx.simulate_keystrokes("r");
    assert!(!view.read_with(cx, |c, _| c.replacing));
}

/// `a` opens the append menu on a selected shape and nothing without one.
#[gpui::test]
fn append_key_needs_a_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_keystrokes("a");
    assert!(view.read_with(cx, |c, _| c.append_menu.is_none()));

    select_start(&view, cx);
    cx.simulate_keystrokes("a");
    assert!(view.read_with(cx, |c, _| c.append_menu.is_some()));
}
