//! Headless interaction tests of the drag aids and the remaining parity
//! gestures: auto-scroll, auto-resize, shape snapping with snap lines,
//! replace preview and pool/lane copy-paste.

use std::time::Duration;

use super::*;

/// Canvas size in screen px once painted.
fn canvas_size(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> (f64, f64) {
    cx.run_until_parked();
    view.read_with(cx, |c, _| c.size.get())
}

/// Dragging the start event to the right canvas edge scrolls the canvas one
/// step right away, keeps scrolling on the repeat timer and stops on
/// release; the dragged shape follows the pointer into the revealed area.
#[gpui::test]
fn drag_near_edge_auto_scrolls(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let (w, _) = canvas_size(&view, cx);
    assert!(w > 100.0);

    let from = at(START_CENTER.0, START_CENTER.1);
    let edge = at((w - 5.0) as f32, START_CENTER.1);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(edge, MouseButton::Left, Modifiers::none());
    let first = view.read_with(cx, |c, _| c.docs[c.active].viewport.offset.x);
    assert_eq!(first, -auto_scroll_step());

    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    let later = view.read_with(cx, |c, _| c.docs[c.active].viewport.offset.x);
    assert!(later < first);

    cx.simulate_mouse_up(edge, MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].viewport.offset.x),
        later
    );

    let b = bounds_of(&view, cx, &ElementId::from(START));
    assert!(b.x + b.w / 2.0 > w - 5.0 - 10.0);
}

/// Scroll step of `auto_scroll`, as a pan offset.
fn auto_scroll_step() -> f64 {
    crate::canvas::auto_scroll::STEP
}

/// Dragging the start event across the pool's right border (its center
/// still inside) grows the pool in the same undoable step.
#[gpui::test]
fn move_beyond_pool_auto_resizes(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let pool = pool(&view, cx);
    let before = bounds_of(&view, cx, &pool);
    let right = before.x + before.w;

    drag(
        cx,
        at(START_CENTER.0, START_CENTER.1),
        at((right - 4.0) as f32, START_CENTER.1),
    );
    let after = bounds_of(&view, cx, &pool);
    let start = bounds_of(&view, cx, &ElementId::from(START));
    assert!(after.x + after.w >= start.x + start.w + 100.0);
    assert_eq!((after.x, after.y, after.h), (before.x, before.y, before.h));

    cx.simulate_keystrokes("secondary-z");
    assert_eq!(bounds_of(&view, cx, &pool), before);
}

/// Moving the start event close to a task's center line snaps it onto the
/// line and shows a snap line while dragging; release clears the lines.
#[gpui::test]
fn move_snaps_to_sibling(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = create(&view, cx, ElementKind::Task, false, 500., 300.);
    let task_mid = center(&bounds_of(&view, cx, &task)).y;

    let to = at(300., (task_mid - 4.0) as f32);
    cx.simulate_mouse_down(
        at(START_CENTER.0, START_CENTER.1),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(at(250., 200.), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    let lines = view.read_with(cx, |c, _| c.aids.lines.clone());
    assert!(
        lines
            .iter()
            .any(|(a, b)| a.y == task_mid && b.y == task_mid)
    );

    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert!(view.read_with(cx, |c, _| c.aids.lines.is_empty()));
    let start = center(&bounds_of(&view, cx, &ElementId::from(START)));
    assert_eq!(start.y, task_mid);
}

/// Hovering a replace-menu option previews the replaced shape without
/// touching the document; leaving the menu ends the preview.
#[gpui::test]
fn replace_menu_hover_previews(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("r");

    let first = view.read_with(cx, |c, _| {
        let kind = &c.definitions().get(&ElementId::from(START)).unwrap().kind;
        factory::replace_options(kind)[0].clone()
    });
    view.update(cx, |c, cx| c.preview_replace(Some(&first), cx));
    let previewed = view.read_with(cx, |c, _| {
        c.replace_preview()
            .and_then(|d| d.get(&ElementId::from(START)).map(|e| e.kind.clone()))
    });
    assert_eq!(previewed, Some(first));
    assert!(matches!(
        kind_of(&view, cx, &ElementId::from(START)),
        ElementKind::StartEvent(_)
    ));

    cx.simulate_keystrokes("escape");
    assert!(view.read_with(cx, |c, _| c.replace_preview().is_none()));
}

/// Moving the pointer over the first replace-menu row reports it through
/// the row's hover handler.
#[gpui::test]
fn replace_menu_row_hover(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("r");
    cx.run_until_parked();

    let row = view.read_with(cx, |c, _| {
        let defs = c.definitions();
        let id = ElementId::from(START);
        let kind = &defs.get(&id).unwrap().kind;
        let groups = context_pad::entries(kind, true);
        let screen = c.docs[c.active]
            .viewport
            .bounds_to_screen(defs.shape(&id).unwrap().bounds);
        let origin = context_pad::origin_for(&screen);
        let (_, h) = context_pad::pad_size(&groups);
        at(
            (origin.x + 60.0) as f32,
            (origin.y + h + 4.0 + 1.0 + 4.0 + 12.0) as f32,
        )
    });
    cx.simulate_mouse_move(row, None, Modifiers::none());
    cx.run_until_parked();

    assert!(view.read_with(cx, |c, _| c.aids.replace_preview.is_some()));
}

/// Down / Up move the replace-menu highlight, previewing the highlighted
/// option without touching the document; Enter applies it.
#[gpui::test]
fn replace_menu_keyboard_previews_and_applies(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("r");

    let options = view.read_with(cx, |c, _| {
        let kind = &c.definitions().get(&ElementId::from(START)).unwrap().kind;
        factory::replace_options(kind)
    });
    let start_bounds = bounds_of(&view, cx, &ElementId::from(START));
    let previewed = |cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| {
            c.replace_preview()
                .and_then(|d| d.get(&ElementId::from(START)).map(|e| e.kind.clone()))
        })
    };

    cx.simulate_keystrokes("down down");
    assert_eq!(previewed(cx), Some(options[1].clone()));
    cx.simulate_keystrokes("up");
    assert_eq!(previewed(cx), Some(options[0].clone()));
    assert!(matches!(
        kind_of(&view, cx, &ElementId::from(START)),
        ElementKind::StartEvent(_)
    ));
    assert_eq!(bounds_of(&view, cx, &ElementId::from(START)), start_bounds);

    cx.simulate_keystrokes("down enter");
    assert_eq!(kind_of(&view, cx, &ElementId::from(START)), options[1]);
    assert!(view.read_with(cx, |c, _| !c.replacing));
    assert!(view.read_with(cx, |c, _| c.replace_preview().is_none()));
}

/// Copying a pool and pasting it below creates a second pool with its own
/// process and a copy of its content.
#[gpui::test]
fn copy_paste_pool(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let pool = pool(&view, cx);
    let b = bounds_of(&view, cx, &pool);
    let pools = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        count(view, cx, |k| matches!(k, ElementKind::Participant { .. }))
    };

    view.update(cx, |c, _| c.docs[c.active].selection = vec![pool.clone()]);
    cx.simulate_keystrokes("secondary-c");
    cx.simulate_mouse_move(
        at((b.x + b.w / 2.0) as f32, (b.y + b.h + 200.0) as f32),
        None,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("secondary-v");

    assert_eq!(pools(&view, cx), 2);
    assert_eq!(count(&view, cx, |k| *k == ElementKind::Process), 2);
    assert_eq!(
        count(&view, cx, |k| matches!(k, ElementKind::StartEvent(_))),
        2
    );

    let pasted = selection(&view, cx);
    assert_eq!(pasted.len(), 1);
    view.read_with(cx, |c, _| {
        let process = process_of(c, &pasted[0]);
        assert_ne!(process, process_of(c, &pool));
    });

    cx.simulate_keystrokes("secondary-z");
    assert_eq!(pools(&view, cx), 1);
}

/// Copying a lane and pasting it on a pool adds a lane to that pool.
#[gpui::test]
fn copy_paste_lane(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let pool = pool(&view, cx);
    exec_any(&view, cx, |d| lanes::add_lane(d, &pool, LaneSide::Below));
    assert_eq!(lanes_count(&view, cx), 2);

    let lane = view.read_with(cx, |c, _| {
        c.definitions()
            .elements
            .values()
            .find(|e| e.kind == ElementKind::Lane)
            .unwrap()
            .id
            .clone()
    });
    view.update(cx, |c, _| c.docs[c.active].selection = vec![lane]);
    cx.simulate_keystrokes("secondary-c");

    let b = bounds_of(&view, cx, &pool);
    cx.simulate_mouse_move(
        at((b.x + 100.0) as f32, (b.y + 20.0) as f32),
        None,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("secondary-v");

    assert_eq!(lanes_count(&view, cx), 3);
    assert!(bounds_of(&view, cx, &pool).h > b.h);
}

/// Executes the command `build` returns on the canvas.
fn exec_any(
    view: &Entity<Canvas>,
    cx: &mut VisualTestContext,
    build: impl FnOnce(&Definitions) -> Option<Command>,
) {
    view.update(cx, |canvas, cx| {
        let command = build(canvas.definitions()).expect("command");
        assert!(canvas.execute(command, cx));
    });
}

/// With the grid off, a palette create near a task snaps its center to the
/// task's center line (diagram-js `CreateMoveSnapping`); the free axis
/// stays where the cursor is.
#[gpui::test]
fn create_snaps_to_sibling(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, |c, _| c.grid.snap = false);
    let a = create(&view, cx, ElementKind::Task, false, 400., 300.);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    cx.simulate_mouse_move(at(603., 304.), None, Modifiers::none());
    cx.simulate_click(at(603., 304.), Modifiers::none());

    let id = selection(&view, cx)[0].clone();
    assert_ne!(id, a);
    assert_eq!(
        center(&bounds_of(&view, cx, &id)),
        Waypoint { x: 603.0, y: 300.0 }
    );
}

/// The space tool moves shapes by a grid multiple (diagram-js
/// `GridSnapping` on `spaceTool.move`).
#[gpui::test]
fn space_tool_snaps_to_grid(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let start = ElementId::from(START);
    let before = bounds_of(&view, cx, &start);

    cx.simulate_keystrokes("s");
    drag(cx, at(150., 120.), at(183., 122.));

    let after = bounds_of(&view, cx, &start);
    assert_eq!(after.x - before.x, 30.0);
}
