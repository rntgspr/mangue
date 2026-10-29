//! Headless interaction tests of the `Canvas` view: real GPUI mouse and
//! keyboard events dispatched through `gpui::VisualTestContext` (gpui
//! `test-support`, test platform; nothing reaches the live desktop).

use std::path::Path;

use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, WindowAppearance, point, px,
};

use super::Canvas;
use crate::canvas::actions;
use crate::canvas::context_pad::{self, PadEntry};
use crate::canvas::palette::{self, Tool};
use crate::model::{Bounds, Definitions, ElementId, ElementKind, EventDefinition, Waypoint};
use crate::modeling::lanes::{self, LaneSide};
use crate::modeling::layout::center;
use crate::modeling::{Command, factory};
use crate::theme::{self, Theme};

/// Id of the start event of `io::new_diagram` (bounds 173,102 36x36).
const START: &str = "StartEvent_1";

/// Center of the start event in window coordinates (identity viewport).
const START_CENTER: (f32, f32) = (191., 120.);

/// Opens a window whose root is a `Canvas` on the bpmn-js initial diagram,
/// with the app key bindings installed.
fn open(cx: &mut TestAppContext) -> (Entity<Canvas>, &mut VisualTestContext) {
    cx.update(actions::install);
    cx.update(crate::canvas::init_editor);

    cx.add_window_view(|window, cx| Canvas::new(crate::io::new_diagram(), None, window, cx))
}

/// Window point from canvas-surface coordinates (the surface sits below the
/// tab strip).
fn at(x: f32, y: f32) -> Point<Pixels> {
    point(
        px(x),
        px(y + crate::canvas::tab_bar::HEIGHT + crate::canvas::doc_header::HEIGHT),
    )
}

/// Window point at the center of the palette cell of `tool` (the palette
/// frame has a 1px border around its content).
fn palette_point(tool: &Tool) -> Point<Pixels> {
    let (_, b) = palette::layout()
        .into_iter()
        .find(|(t, _)| t == tool)
        .expect("palette entry");

    at(
        (palette::ORIGIN.x + 1.0 + b.x + b.w / 2.0) as f32,
        (palette::ORIGIN.y + 1.0 + b.y + b.h / 2.0) as f32,
    )
}

/// Window point at the center of the context-pad cell of `entry` for the
/// single selected shape `id`.
fn pad_point(
    view: &Entity<Canvas>,
    cx: &mut VisualTestContext,
    id: &str,
    entry: &PadEntry,
) -> Point<Pixels> {
    view.read_with(cx, |canvas, _| {
        let defs = canvas.definitions();
        let id = ElementId::from(id);
        let kind = &defs.get(&id).expect("element").kind;
        let groups = context_pad::entries(kind, !factory::replace_options(kind).is_empty());
        let screen = canvas.docs[canvas.active]
            .viewport
            .bounds_to_screen(defs.shape(&id).expect("shape").bounds);
        let origin = context_pad::origin_for(&screen);
        let (_, b) = context_pad::layout(&groups)
            .into_iter()
            .find(|(e, _)| e == entry)
            .expect("pad entry");

        at(
            (origin.x + b.x + b.w / 2.0) as f32,
            (origin.y + b.y + b.h / 2.0) as f32,
        )
    })
}

/// Presses, drags (through one intermediate point) and releases the left button.
fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    let mid = point((from.x + to.x) / 2., (from.y + to.y) / 2.);

    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(mid, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
}

/// Number of elements whose kind satisfies `pred`.
fn count(
    view: &Entity<Canvas>,
    cx: &mut VisualTestContext,
    pred: fn(&ElementKind) -> bool,
) -> usize {
    view.read_with(cx, |canvas, _| {
        canvas
            .definitions()
            .elements
            .values()
            .filter(|e| pred(&e.kind))
            .count()
    })
}

/// Title last applied to the window (the test platform does not report
/// titles back through `Window::window_title`).
fn window_title(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |c, _| c.shown_title.clone())
}

/// Selected element ids.
fn selection(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> Vec<ElementId> {
    view.read_with(cx, |canvas, _| canvas.docs[canvas.active].selection.clone())
}

/// Whether `kind` is a task.
fn is_task(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::Task)
}

/// Whether `kind` is a sequence flow.
fn is_flow(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::SequenceFlow { .. })
}

/// Clicking the palette task entry arms it; clicking the canvas creates and
/// selects a task there, marks the document dirty and the title edited.
#[gpui::test]
fn palette_click_creates(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    assert_eq!(view.read_with(cx, |c, _| c.tool.clone()), Some(task));

    cx.simulate_mouse_move(at(400., 300.), None, Modifiers::none());
    assert!(view.read_with(cx, |c, _| c.cursor) == Waypoint { x: 400.0, y: 300.0 });

    cx.simulate_click(at(400., 300.), Modifiers::none());
    assert_eq!(count(&view, cx, is_task), 1);

    let selected = selection(&view, cx);
    let bounds = view.read_with(cx, |c, _| {
        c.definitions().shape(&selected[0]).unwrap().bounds
    });
    assert_eq!(
        (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0),
        (400.0, 300.0)
    );

    assert!(view.read_with(cx, |c, _| c.is_dirty()));
    assert_eq!(window_title(&view, cx), "Untitled * - mangue");
}

/// Dragging a palette entry onto the canvas creates the shape at the drop point.
#[gpui::test]
fn palette_drag_creates(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let end = Tool::Create(ElementKind::EndEvent(EventDefinition::None), false);

    drag(cx, palette_point(&end), at(500., 300.));

    assert_eq!(
        count(&view, cx, |k| matches!(k, ElementKind::EndEvent(_))),
        1
    );
    assert_eq!(view.read_with(cx, |c, _| c.tool.clone()), None);
}

/// Context pad append task adds a task connected by a sequence flow.
#[gpui::test]
fn pad_append_task(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    assert_eq!(selection(&view, cx), vec![ElementId::from(START)]);

    let append = pad_point(&view, cx, START, &PadEntry::Append(ElementKind::Task));
    cx.simulate_click(append, Modifiers::none());

    assert_eq!(count(&view, cx, is_task), 1);
    assert_eq!(count(&view, cx, is_flow), 1);
}

/// Context pad connect, then clicking another shape, adds a sequence flow.
#[gpui::test]
fn pad_connect(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    cx.simulate_click(at(400., 300.), Modifiers::none());
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());

    let connect = pad_point(&view, cx, START, &PadEntry::Connect);
    cx.simulate_click(connect, Modifiers::none());
    cx.simulate_click(at(400., 300.), Modifiers::none());

    assert_eq!(count(&view, cx, is_flow), 1);
}

/// Context pad change type opens the replace menu; its first row replaces
/// the start event with another kind.
#[gpui::test]
fn pad_replace(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    let replace = pad_point(&view, cx, START, &PadEntry::Replace);
    cx.simulate_click(replace, Modifiers::none());
    assert!(view.read_with(cx, |c, _| c.replacing));

    let (first, pad_bottom) = view.read_with(cx, |c, _| {
        let id = ElementId::from(START);
        let kind = &c.definitions().get(&id).unwrap().kind;
        let options = factory::replace_options(kind);
        let groups = context_pad::entries(kind, true);
        let screen = c.docs[c.active]
            .viewport
            .bounds_to_screen(c.definitions().shape(&id).unwrap().bounds);
        let origin = context_pad::origin_for(&screen);
        (
            options[0].clone(),
            origin.y + context_pad::pad_size(&groups).1,
        )
    });
    let x = replace.x;
    cx.simulate_click(
        at(f32::from(x), pad_bottom as f32 + 4. + 15.),
        Modifiers::none(),
    );

    let kind = view.read_with(cx, |c, _| {
        c.definitions()
            .get(&ElementId::from(START))
            .unwrap()
            .kind
            .clone()
    });
    assert_eq!(kind, first);
}

/// Context pad delete removes the selected shape.
#[gpui::test]
fn pad_delete(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    let delete = pad_point(&view, cx, START, &PadEntry::Delete);
    cx.simulate_click(delete, Modifiers::none());

    assert!(view.read_with(cx, |c, _| {
        c.definitions().get(&ElementId::from(START)).is_none()
    }));
}

/// Dragging a shape moves it by the drag delta with its center snapped to
/// the 10px grid (191 + 100 -> 290); cmd-z restores it and
/// cmd-shift-z moves it again, with the dirty flag following the history.
#[gpui::test]
fn move_undo_redo(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let x_of = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| {
            c.definitions()
                .shape(&ElementId::from(START))
                .unwrap()
                .bounds
                .x
        })
    };

    drag(
        cx,
        at(START_CENTER.0, START_CENTER.1),
        at(START_CENTER.0 + 100., START_CENTER.1 + 50.),
    );
    assert_eq!(x_of(&view, cx), 272.0);
    assert!(view.read_with(cx, |c, _| c.is_dirty()));

    cx.simulate_keystrokes("secondary-z");
    assert_eq!(x_of(&view, cx), 173.0);
    assert!(!view.read_with(cx, |c, _| c.is_dirty()));
    assert_eq!(window_title(&view, cx), "Untitled - mangue");

    cx.simulate_keystrokes("secondary-shift-z");
    assert_eq!(x_of(&view, cx), 272.0);
}

/// A hover fills the paint-order cache used by hit testing, a command
/// drops it, and a burst of drag moves leaves one stale preview that the
/// next frame rebuilds.
#[gpui::test]
fn paint_order_cache_and_preview_coalescing(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let cached = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| c.docs[c.active].order.borrow().is_some())
    };

    cx.simulate_mouse_move(at(START_CENTER.0, START_CENTER.1), None, Modifiers::none());
    assert!(cached(&view, cx));

    let from = at(START_CENTER.0, START_CENTER.1);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=4 {
        let to = at(START_CENTER.0 + 20. * step as f32, START_CENTER.1 + 10.);
        cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    }
    cx.simulate_mouse_up(
        at(START_CENTER.0 + 80., START_CENTER.1 + 10.),
        MouseButton::Left,
        Modifiers::none(),
    );

    assert!(!view.read_with(cx, |c, _| c.preview_stale));
    assert!(view.read_with(cx, |c, _| c.is_dirty()));
}

/// Dragging on empty canvas lasso-selects the enclosed start event.
#[gpui::test]
fn lasso_selects(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    drag(cx, at(150., 80.), at(250., 170.));

    assert_eq!(selection(&view, cx), vec![ElementId::from(START)]);
}

/// Double-click opens the label editor; typed text and Enter commit the name.
#[gpui::test]
fn label_edit(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let center = at(START_CENTER.0, START_CENTER.1);

    cx.simulate_click(center, Modifiers::none());
    cx.simulate_event(MouseDownEvent {
        position: center,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: center,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 2,
    });
    assert!(view.read_with(cx, |c, _| c.editing.is_some()));

    cx.simulate_input("Go");
    cx.simulate_keystrokes("enter");

    let name = view.read_with(cx, |c, _| {
        c.definitions()
            .get(&ElementId::from(START))
            .unwrap()
            .name
            .clone()
    });
    assert_eq!(name.as_deref(), Some("Go"));
}

/// cmd-n on a clean document replaces it without prompting.
#[gpui::test]
fn new_when_clean(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);

    cx.simulate_keystrokes("secondary-n");

    assert!(!cx.has_pending_prompt());
}

/// cmd-s writes the file at the document path and clears the dirty flag.
#[gpui::test]
fn save_marks_clean(cx: &mut TestAppContext) {
    let path = std::env::temp_dir().join(format!("mangue-save-{}.bpmn", std::process::id()));
    cx.update(actions::install);
    cx.update(crate::canvas::init_editor);
    let target = path.clone();
    let (view, cx) = cx.add_window_view(move |window, cx| {
        Canvas::new(crate::io::new_diagram(), Some(target), window, cx)
    });
    view.update(cx, |c, _| {
        c.fit_pending = false;
        c.docs[c.active].viewport = Default::default();
    });

    drag(
        cx,
        at(START_CENTER.0, START_CENTER.1),
        at(START_CENTER.0 + 50., START_CENTER.1),
    );
    assert!(view.read_with(cx, |c, _| c.is_dirty()));

    cx.simulate_keystrokes("secondary-s");
    assert!(!view.read_with(cx, |c, _| c.is_dirty()));
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("StartEvent_1")
    );
    assert!(window_title(&view, cx).starts_with("mangue-save-"));

    let _ = std::fs::remove_file(&path);
}

/// While a create entry is armed, the ghost the view renders is the shape
/// the create command would add, centered at the cursor.
#[gpui::test]
fn ghost_follows_cursor(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    cx.simulate_mouse_move(at(420., 260.), None, Modifiers::none());

    let ghost = view.read_with(cx, |c, _| {
        factory::create_shape(c.definitions(), ElementKind::Task, false, c.cursor)
            .as_ref()
            .and_then(super::created_shape)
    });
    let (kind, shape) = ghost.expect("ghost");
    let b = shape.bounds;

    assert_eq!(kind, ElementKind::Task);
    assert_eq!((b.x + b.w / 2.0, b.y + b.h / 2.0), (420.0, 260.0));
}

/// cmd-q on a dirty document prompts instead of quitting; Cancel keeps it.
#[gpui::test]
fn quit_confirms_discard(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace secondary-q");
    assert!(cx.has_pending_prompt());

    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(view.read_with(cx, |c, _| c.is_dirty()));
}

/// The canvas applies the light theme for the (light) test window, then
/// the dark theme when the appearance turns dark. The test platform always
/// reports light, so the dark switch drives the appearance observer's
/// handler directly.
#[gpui::test]
fn theme_follows_appearance(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let light = theme::builtin(theme::LIGHT).unwrap();
    let dark = theme::builtin(theme::DARK).unwrap();

    assert_eq!(cx.read(|cx| cx.global::<Theme>().clone()), light);

    view.update(cx, |canvas, cx| {
        canvas.on_appearance(WindowAppearance::Dark, cx)
    });
    assert_eq!(cx.read(|cx| cx.global::<Theme>().clone()), dark);
}

/// Executes the command `build` makes from the current document through
/// the view (as a gesture would) and returns the id it creates.
fn exec(
    view: &Entity<Canvas>,
    cx: &mut VisualTestContext,
    build: impl FnOnce(&Definitions) -> Option<Command>,
) -> ElementId {
    view.update(cx, |canvas, cx| {
        let command = build(canvas.definitions()).expect("command");
        let id = super::created_id(&command).expect("created id");

        assert!(canvas.execute(command, cx));
        id
    })
}

/// Bounds of the DI shape of `id`.
fn bounds_of(view: &Entity<Canvas>, cx: &mut VisualTestContext, id: &ElementId) -> Bounds {
    view.read_with(cx, |c, _| c.definitions().shape(id).expect("shape").bounds)
}

/// Kind of element `id`.
fn kind_of(view: &Entity<Canvas>, cx: &mut VisualTestContext, id: &ElementId) -> ElementKind {
    view.read_with(cx, |c, _| {
        c.definitions().get(id).expect("element").kind.clone()
    })
}

/// Waypoints of connection `id`.
fn waypoints_of(
    view: &Entity<Canvas>,
    cx: &mut VisualTestContext,
    id: &ElementId,
) -> Vec<Waypoint> {
    view.read_with(cx, |c, _| {
        c.definitions()
            .edges()
            .find(|e| e.element_id == *id)
            .expect("edge")
            .waypoints
            .clone()
    })
}

/// Window point of a diagram point (identity viewport).
fn win(p: Waypoint) -> Point<Pixels> {
    at(p.x as f32, p.y as f32)
}

/// Creates a shape of `kind` centered at `(x, y)`; returns its id.
fn create(
    view: &Entity<Canvas>,
    cx: &mut VisualTestContext,
    kind: ElementKind,
    expanded: bool,
    x: f64,
    y: f64,
) -> ElementId {
    exec(view, cx, |d| {
        factory::create_shape(d, kind, expanded, Waypoint { x, y })
    })
}

/// Dragging the bottom-right handle of a selected expanded sub-process
/// previews the new size while dragging and commits a resize on release,
/// the corner snapped to the grid.
#[gpui::test]
fn resize_handle_drag(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let sub = ElementKind::SubProcess {
        triggered_by_event: false,
    };
    let id = create(&view, cx, sub, true, 500., 400.);
    let b = bounds_of(&view, cx, &id);

    cx.simulate_click(at(500., 400.), Modifiers::none());
    assert_eq!(selection(&view, cx), vec![id.clone()]);

    let corner = at((b.x + b.w) as f32, (b.y + b.h) as f32);
    let to = at((b.x + b.w + 62.) as f32, (b.y + b.h + 38.) as f32);
    cx.simulate_mouse_down(corner, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());

    let preview = view.read_with(cx, |c, _| match &c.gesture {
        super::Gesture::Resize {
            preview: Some(p), ..
        } => p.shape(&id).map(|s| s.bounds),
        _ => None,
    });
    assert_eq!(preview.map(|p| (p.w, p.h)), Some((b.w + 65., b.h + 40.)));

    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    let after = bounds_of(&view, cx, &id);
    assert_eq!(
        (after.x, after.y, after.w, after.h),
        (b.x, b.y, b.w + 65., b.h + 40.)
    );

    cx.simulate_keystrokes("secondary-z");
    assert_eq!(bounds_of(&view, cx, &id), b);
}

/// Resizing below the bpmn-js minimum clamps to it (sub-process 140x120).
#[gpui::test]
fn resize_respects_min_size(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let sub = ElementKind::SubProcess {
        triggered_by_event: false,
    };
    let id = create(&view, cx, sub, true, 500., 400.);
    let b = bounds_of(&view, cx, &id);

    cx.simulate_click(at(500., 400.), Modifiers::none());
    drag(
        cx,
        at((b.x + b.w) as f32, (b.y + b.h) as f32),
        at((b.x + 20.) as f32, (b.y + 20.) as f32),
    );

    let after = bounds_of(&view, cx, &id);
    assert_eq!((after.w, after.h), (140., 120.));
}

/// Tasks have no resize handles (bpmn-js `canResize`): a corner drag moves
/// the task instead of resizing it.
#[gpui::test]
fn task_corner_drag_moves(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let id = create(&view, cx, ElementKind::Task, false, 500., 400.);
    let b = bounds_of(&view, cx, &id);

    cx.simulate_click(at(500., 400.), Modifiers::none());
    drag(
        cx,
        at((b.x + b.w - 1.) as f32, (b.y + b.h - 1.) as f32),
        at((b.x + b.w + 49.) as f32, (b.y + b.h - 1.) as f32),
    );

    let after = bounds_of(&view, cx, &id);
    assert_eq!((after.x, after.w, after.h), (b.x + 50., b.w, b.h));
}

/// Dropping an intermediate event on a task border attaches it as a
/// boundary event snapped to the border; dragging it along the border keeps
/// it attached; dragging it away detaches it.
#[gpui::test]
fn boundary_attach_slide_detach(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = create(&view, cx, ElementKind::Task, false, 500., 400.);
    let event = create(
        &view,
        cx,
        ElementKind::IntermediateThrowEvent(EventDefinition::None),
        false,
        300.,
        400.,
    );
    let host = bounds_of(&view, cx, &task);
    let bottom = host.y + host.h;

    drag(cx, at(300., 400.), at(500., (bottom + 3.) as f32));
    assert!(matches!(
        kind_of(&view, cx, &event),
        ElementKind::BoundaryEvent { attached_to, .. } if attached_to == task
    ));
    let c = center(&bounds_of(&view, cx, &event));
    assert_eq!((c.x, c.y), (500., bottom));

    drag(cx, at(500., bottom as f32), at(530., (bottom + 5.) as f32));
    assert!(matches!(
        kind_of(&view, cx, &event),
        ElementKind::BoundaryEvent { .. }
    ));
    let c = center(&bounds_of(&view, cx, &event));
    assert_eq!((c.x, c.y), (530., bottom));

    drag(cx, at(530., bottom as f32), at(530., 600.));
    assert!(matches!(
        kind_of(&view, cx, &event),
        ElementKind::IntermediateThrowEvent(_)
    ));
}

/// Moving a host task carries its boundary event along.
#[gpui::test]
fn boundary_follows_host(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let task = create(&view, cx, ElementKind::Task, false, 500., 400.);
    let event = create(
        &view,
        cx,
        ElementKind::IntermediateThrowEvent(EventDefinition::None),
        false,
        300.,
        400.,
    );
    let host = bounds_of(&view, cx, &task);
    drag(cx, at(300., 400.), at(500., (host.y + host.h) as f32));
    let before = bounds_of(&view, cx, &event);

    drag(cx, at(480., 380.), at(480., 330.));

    let after = bounds_of(&view, cx, &event);
    assert_eq!((after.x, after.y), (before.x, before.y - 50.));
}

/// Wraps the initial diagram in a pool and returns its id.
fn pool(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> ElementId {
    exec(view, cx, |d| {
        factory::create_participant(d, Waypoint { x: 400., y: 200. })
    })
}

/// Lanes of the document.
fn lanes_count(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> usize {
    count(view, cx, |k| *k == ElementKind::Lane)
}

/// Context-pad lane tools on a pool: add lane below gives two lanes (the
/// pool's first lane plus the new one); split into three on a fresh pool
/// gives three.
#[gpui::test]
fn pad_lane_tools(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let id = pool(&view, cx);
    let b = bounds_of(&view, cx, &id);
    let band = at((b.x + 10.) as f32, (b.y + b.h / 2.) as f32);

    cx.simulate_click(band, Modifiers::none());
    assert_eq!(selection(&view, cx), vec![id.clone()]);
    let below = pad_point(&view, cx, id.as_str(), &PadEntry::AddLane(LaneSide::Below));
    cx.simulate_click(below, Modifiers::none());
    assert_eq!(lanes_count(&view, cx), 2);
    assert_eq!(bounds_of(&view, cx, &id).h, b.h + 120.);

    cx.simulate_keystrokes("secondary-z");
    assert_eq!(lanes_count(&view, cx), 0);

    cx.simulate_click(band, Modifiers::none());
    let split = pad_point(&view, cx, id.as_str(), &PadEntry::SplitLane(3));
    cx.simulate_click(split, Modifiers::none());
    assert_eq!(lanes_count(&view, cx), 3);
}

/// Moving a flow node into another lane updates its lane membership.
#[gpui::test]
fn move_updates_lane_membership(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let id = pool(&view, cx);
    exec(&view, cx, |d| lanes::split_lane(d, &id, 2));

    let start = ElementId::from(START);
    let parent_of = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| {
            c.definitions().get(&start).unwrap().parent.clone()
        })
    };
    let lower = view.read_with(cx, |c, _| {
        let lanes = lanes::child_lanes(c.definitions(), &process_of(c, &id));
        lanes
            .into_iter()
            .max_by(|a, b| {
                let (a, b) = (
                    c.definitions().shape(a).unwrap(),
                    c.definitions().shape(b).unwrap(),
                );
                a.bounds.y.total_cmp(&b.bounds.y)
            })
            .unwrap()
    });
    let lower_b = bounds_of(&view, cx, &lower);
    assert_ne!(parent_of(&view, cx), Some(lower.clone()));

    let target = (lower_b.y + lower_b.h / 2.) as f32;
    drag(
        cx,
        at(START_CENTER.0, START_CENTER.1),
        at(START_CENTER.0, target),
    );

    assert_eq!(parent_of(&view, cx), Some(lower));
}

/// Process referenced by participant `id`.
fn process_of(c: &Canvas, id: &ElementId) -> ElementId {
    match &c.definitions().get(id).unwrap().kind {
        ElementKind::Participant {
            process_ref: Some(p),
        } => p.clone(),
        _ => panic!("participant without process"),
    }
}

/// Connects the start event to a task placed down-right of it, so the
/// manhattan route has bendpoints; returns the flow id.
fn bent_flow(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> ElementId {
    let task = create(view, cx, ElementKind::Task, false, 450., 350.);
    let start = ElementId::from(START);

    exec(view, cx, |d| factory::connect(d, &start, &task))
}

/// Selects connection `id` by clicking the middle of its first segment.
fn select_edge(view: &Entity<Canvas>, cx: &mut VisualTestContext, id: &ElementId) {
    let w = waypoints_of(view, cx, id);
    let p = Waypoint {
        x: (w[0].x + w[1].x) / 2.,
        y: (w[0].y + w[1].y) / 2.,
    };

    cx.simulate_click(win(p), Modifiers::none());
    assert_eq!(selection(view, cx), vec![id.clone()]);
}

/// Dragging a bendpoint of the selected flow moves it (the path stays
/// orthogonal); double-clicking a bendpoint removes it.
#[gpui::test]
fn bendpoint_move_and_remove(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let flow = bent_flow(&view, cx);
    let w = waypoints_of(&view, cx, &flow);
    assert!(w.len() >= 3, "route has a bendpoint: {w:?}");
    select_edge(&view, cx, &flow);

    let to = Waypoint {
        x: w[1].x + 20.,
        y: w[1].y + 30.,
    };
    drag(cx, win(w[1]), win(to));
    let moved = waypoints_of(&view, cx, &flow);
    assert_ne!(moved, w);
    assert!(
        moved
            .windows(2)
            .all(|s| crate::modeling::layout::is_orthogonal(s[0], s[1])),
        "{moved:?}"
    );

    cx.simulate_event(MouseDownEvent {
        position: win(moved[1]),
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: win(moved[1]),
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 2,
    });
    assert_ne!(waypoints_of(&view, cx, &flow), moved);
}

/// Dragging a point of a segment away from its middle inserts a bendpoint
/// there and moves it to the drop point.
#[gpui::test]
fn segment_drag_inserts_bendpoint(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let flow = bent_flow(&view, cx);
    let w = waypoints_of(&view, cx, &flow);
    select_edge(&view, cx, &flow);

    let last = w.len() - 2;
    let (a, b) = (w[last], w[last + 1]);
    let grab = Waypoint {
        x: a.x + (b.x - a.x) * 0.8,
        y: a.y + (b.y - a.y) * 0.8,
    };
    let to = Waypoint {
        x: grab.x.round() + 15.,
        y: grab.y.round() + 40.,
    };
    drag(cx, win(grab), win(to));

    let after = waypoints_of(&view, cx, &flow);
    assert_ne!(after, w);
    assert!(
        after
            .windows(2)
            .all(|s| crate::modeling::layout::is_orthogonal(s[0], s[1])),
        "{after:?}"
    );
}

/// Dragging the middle of a segment moves it perpendicular to itself.
#[gpui::test]
fn segment_mid_drag_moves_segment(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let flow = bent_flow(&view, cx);
    let w = waypoints_of(&view, cx, &flow);
    select_edge(&view, cx, &flow);

    let i = (0..w.len() - 1)
        .find(|i| (w[*i].x - w[i + 1].x).abs() < 1.)
        .expect("vertical segment");
    let mid = Waypoint {
        x: w[i].x,
        y: (w[i].y + w[i + 1].y) / 2.,
    };
    drag(
        cx,
        win(mid),
        win(Waypoint {
            x: mid.x + 25.,
            y: mid.y + 7.,
        }),
    );

    let after = waypoints_of(&view, cx, &flow);
    let x = crate::modeling::arrange::snap(w[i].x + 25.);
    assert!(after.iter().any(|p| p.x == x && p.y == w[i].y));
    assert!(after.iter().any(|p| p.x == x && p.y == w[i + 1].y));
}

/// New events and flows get external label bounds (bpmn-js `LabelUtil`);
/// moving the shape moves its label by the same delta.
#[gpui::test]
fn external_labels_follow(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let id = create(
        &view,
        cx,
        ElementKind::EndEvent(EventDefinition::None),
        false,
        500.,
        400.,
    );
    let label = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| c.definitions().shape(&id).unwrap().label_bounds)
    };
    let before = label(&view, cx).expect("label bounds");

    drag(cx, at(500., 400.), at(540., 430.));
    let after = label(&view, cx).unwrap();
    assert_eq!((after.x, after.y), (before.x + 40., before.y + 30.));

    let flow = bent_flow(&view, cx);
    let flow_label = view.read_with(cx, |c, _| {
        c.definitions()
            .edges()
            .find(|e| e.element_id == flow)
            .unwrap()
            .label_bounds
    });
    assert!(flow_label.is_some());
}

/// The window close button on a dirty document is refused and prompts;
/// on a clean document it closes.
#[gpui::test]
fn close_button_confirms_discard(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    assert!(!cx.simulate_close());
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(view.read_with(cx, |c, _| c.is_dirty()));

    cx.simulate_keystrokes("secondary-z");
    assert!(cx.simulate_close());
}

#[path = "view_tests_parity.rs"]
mod parity;

#[path = "view_tests_text.rs"]
mod text;

#[path = "view_tests_settings.rs"]
mod settings_window;

#[path = "view_tests_planes.rs"]
mod planes;

#[path = "view_tests_code.rs"]
mod code;

#[path = "view_tests_tabs.rs"]
mod tabs;

#[path = "view_tests_empty.rs"]
mod empty;

#[path = "view_tests_aids.rs"]
mod aids;

#[path = "view_tests_feedback.rs"]
mod feedback;

/// A failed open shows a critical alert and leaves a usable, unbound document.
#[gpui::test]
fn failed_open_alerts_and_keeps_document(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    view.update_in(cx, |canvas, window, cx| {
        canvas.alert_open_failure(Path::new("/tmp/missing.bpmn"), "no such file", window, cx)
    });
    assert!(cx.has_pending_prompt());

    cx.simulate_prompt_answer("OK");
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    assert!(view.read_with(cx, |c, _| c.is_dirty()));
}

/// After a save-as write the path is rebound and the document is clean.
#[gpui::test]
fn save_as_rebinds_path_and_marks_clean(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let target = std::env::temp_dir().join(format!("mangue-save-as-{}.bpmn", std::process::id()));

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    assert!(view.read_with(cx, |c, _| c.is_dirty()));

    let saved = view.update_in(cx, |canvas, window, cx| {
        canvas.finish_save_as(target.clone(), window, cx)
    });

    assert!(saved);
    assert!(target.exists());
    assert!(!view.read_with(cx, |c, _| c.is_dirty()));
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].path.clone()),
        Some(target.clone())
    );

    std::fs::remove_file(&target).unwrap();
}

/// A chosen folder is stored on the view as the browser root.
#[gpui::test]
fn open_folder_stores_root(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert_eq!(view.read_with(cx, |c, _| c.folder().cloned()), None);

    view.update(cx, |c, cx| {
        c.set_folder(std::path::PathBuf::from("/a/b"), cx)
    });

    assert_eq!(
        view.read_with(cx, |c, _| c.folder().cloned()),
        Some(std::path::PathBuf::from("/a/b"))
    );
}

/// Creates a fresh folder holding one valid `a.bpmn`.
fn sidebar_fixture(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mangue-sidebar-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a.bpmn"),
        crate::io::export_xml(&crate::io::new_diagram()),
    )
    .unwrap();

    dir
}

/// Opening a folder shows the sidebar; `ToggleSidebar` toggles it and cmd-b does not.
#[gpui::test]
fn toggle_sidebar(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert!(!view.read_with(cx, |c, _| c.sidebar_visible()));

    let dir = sidebar_fixture("toggle");
    view.update(cx, |c, cx| c.set_folder(dir.clone(), cx));
    assert!(view.read_with(cx, |c, _| c.sidebar_visible()));

    cx.simulate_keystrokes("secondary-b");
    assert!(view.read_with(cx, |c, _| c.sidebar_visible()));

    cx.dispatch_action(crate::canvas::actions::ToggleSidebar);
    assert!(!view.read_with(cx, |c, _| c.sidebar_visible()));

    cx.dispatch_action(crate::canvas::actions::ToggleSidebar);
    assert!(view.read_with(cx, |c, _| c.sidebar_visible()));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Picking a file in the sidebar loads it and marks it as the active file.
#[gpui::test]
fn sidebar_pick_opens_file(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let dir = sidebar_fixture("pick");
    let file = dir.join("a.bpmn");
    view.update(cx, |c, cx| c.set_folder(dir.clone(), cx));

    cx.update(|window, app| view.update(app, |c, cx| c.on_sidebar_pick(file.clone(), window, cx)));
    cx.run_until_parked();

    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].path.clone()),
        Some(file)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Picking a file while the document is dirty opens a tab without asking.
#[gpui::test]
fn sidebar_pick_keeps_dirty_document_in_its_tab(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let dir = sidebar_fixture("dirty");
    let file = dir.join("a.bpmn");
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    view.update(cx, |c, cx| c.set_folder(dir.clone(), cx));

    cx.update(|window, app| view.update(app, |c, cx| c.on_sidebar_pick(file.clone(), window, cx)));
    cx.run_until_parked();

    assert!(!cx.has_pending_prompt());
    assert_eq!(view.read_with(cx, |c, _| c.docs.len()), 2);
    assert!(view.read_with(cx, |c, _| c.docs[0].is_dirty()));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A fixture with diagonal edges loaded through the startup path
/// (with normalization) opens clean: no `*` in the title and
/// nothing to undo.
#[gpui::test]
fn freshly_loaded_document_is_clean(cx: &mut TestAppContext) {
    cx.update(actions::install);
    cx.update(crate::canvas::init_editor);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/message-delivery.bpmn");
    let defs = crate::canvas::persist::open(&path).expect("fixture loads");

    let (canvas, cx) =
        cx.add_window_view(|window, cx| Canvas::new(defs, Some(path.clone()), window, cx));
    cx.run_until_parked();

    canvas.read_with(cx, |c, _| {
        assert!(!c.is_dirty());
        assert!(!c.title().contains('*'), "title: {}", c.title());
        assert!(!c.docs[c.active].stack.can_undo());
    });
}

/// Clicking a sidebar directory row expands it (loading its children) and
/// clicking a file row below it opens that file; refresh keeps it open.
#[gpui::test]
fn sidebar_rows_expand_and_open_by_click(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let dir = sidebar_fixture("rows");
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::copy(dir.join("a.bpmn"), sub.join("b.bpmn")).unwrap();
    view.update(cx, |c, cx| c.set_folder(dir.clone(), cx));
    cx.run_until_parked();

    let row = cx
        .debug_bounds("sidebar-row-sub")
        .expect("sub row rendered");
    assert!(cx.debug_bounds("sidebar-row-b.bpmn").is_none());
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(view.read_with(cx, |c, _| c.expanded.contains(&sub)));

    let file = cx
        .debug_bounds("sidebar-row-b.bpmn")
        .expect("child row rendered");
    cx.simulate_click(file.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].path.clone()),
        Some(sub.join("b.bpmn"))
    );

    view.update(cx, |c, cx| c.on_sidebar_refresh(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-row-b.bpmn").is_some());
    std::fs::remove_dir_all(&dir).unwrap();
}
