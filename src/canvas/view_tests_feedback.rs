//! Headless tests of the canvas overlay API and the modeling feedback of
//! refused gestures (bpmn-js `ModelingFeedback`): not-allowed cursor state
//! while dragging, error tooltip near the cursor, fade after about 2 s.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use super::*;
use crate::canvas::overlay::{Anchor, Click, Corner, ERROR_TTL, Overlay, OverlayId, Tone};
use crate::modeling::rules::Refusal;

/// Error overlays currently on the canvas, as `(id, text, anchor point)`.
fn errors(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> Vec<(OverlayId, String, Waypoint)> {
    view.read_with(cx, |c, _| {
        c.overlay_store
            .all()
            .iter()
            .filter(|o| o.tone == Tone::Error)
            .filter_map(|o| Some((o.id, o.text.clone(), o.position(c.definitions())?)))
            .collect()
    })
}

/// Advances the test clock by `ms` and lets the overlay clock run.
fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

/// Refusal of the current pointer state.
fn refusal(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> Option<Refusal> {
    view.read_with(cx, |c, _| c.current_refusal())
}

/// add / get / remove / overlays_on through the canvas, and an element
/// overlay follows the viewport and its element.
#[gpui::test]
fn overlay_api_follows_element_and_viewport(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let start = ElementId::from(START);

    let id = view.update(cx, |c, cx| {
        c.add_overlay(
            Anchor::Element(start.clone(), Corner::TopRight),
            "1",
            Tone::Badge,
            None,
            cx,
        )
    });
    view.read_with(cx, |c, _| {
        assert_eq!(c.overlay(id).map(|o| o.text.as_str()), Some("1"));
        assert_eq!(c.overlays_on(&start).len(), 1);
    });

    let corner = |view: &Entity<Canvas>, cx: &mut VisualTestContext| {
        view.read_with(cx, |c, _| {
            let p = c.overlay(id)?.position(c.definitions())?;
            Some(c.docs[c.active].viewport.to_screen(p))
        })
    };
    let before = corner(&view, cx).expect("position");
    assert_eq!((before.x, before.y), (209.0, 102.0));

    view.update(cx, |c, _| c.docs[c.active].viewport.pan(40.0, 10.0));
    let panned = corner(&view, cx).expect("position");
    assert_eq!((panned.x, panned.y), (249.0, 112.0));

    view.update(cx, |c, _| c.docs[c.active].viewport.pan(-40.0, -10.0));
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    view.read_with(cx, |c, _| assert!(c.overlay(id).is_none()));

    let again = view.update(cx, |c, cx| {
        c.add_overlay(
            Anchor::Point(Waypoint { x: 5.0, y: 5.0 }),
            "p",
            Tone::Badge,
            None,
            cx,
        )
    });
    assert!(view.update(cx, |c, cx| c.remove_overlay(again, cx)));
    assert!(!view.update(cx, |c, cx| c.remove_overlay(again, cx)));
}

/// Dragging a flow node out of its pool shows the not-allowed state while
/// dragging; on release nothing moves and the bpmn-js error tooltip appears
/// next to the cursor, then fades and disappears after about 2 s.
#[gpui::test]
fn drag_out_of_pool_is_refused_with_tooltip(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let pool = pool(&view, cx);
    let b = bounds_of(&view, cx, &pool);
    let start = ElementId::from(START);
    let before = bounds_of(&view, cx, &start);

    let from = at(START_CENTER.0, START_CENTER.1);
    let outside = at(START_CENTER.0, (b.y + b.h + 60.0) as f32);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    assert_eq!(refusal(&view, cx), Some(Refusal::OutsideParticipant));
    assert!(errors(&view, cx).is_empty());

    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    assert_eq!(bounds_of(&view, cx, &start), before);
    assert_eq!(refusal(&view, cx), None);

    let shown = errors(&view, cx);
    assert_eq!(shown.len(), 1);
    assert_eq!(
        shown[0].1,
        "flow elements must be children of pools/participants"
    );
    assert_eq!(
        (shown[0].2.x, shown[0].2.y),
        (f64::from(START_CENTER.0), f64::from(b.y + b.h + 60.0))
    );

    wait(cx, 1000);
    assert_eq!(errors(&view, cx).len(), 1);
    let opacity = view.read_with(cx, |c, _| c.overlay_store.all()[0].opacity());
    assert_eq!(opacity, 1.0);

    wait(cx, 750);
    let opacity = view.read_with(cx, |c, _| c.overlay_store.all()[0].opacity());
    assert!(opacity > 0.0 && opacity < 1.0, "{opacity}");

    wait(cx, ERROR_TTL.as_millis() as u64);
    assert!(errors(&view, cx).is_empty());
}

/// A drag that stays inside the pool is not refused.
#[gpui::test]
fn drag_inside_pool_is_allowed(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    pool(&view, cx);

    let from = at(START_CENTER.0, START_CENTER.1);
    let to = at(START_CENTER.0 + 80.0, START_CENTER.1);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    assert_eq!(refusal(&view, cx), None);
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());

    assert!(errors(&view, cx).is_empty());
}

/// Dropping an armed palette task on the collaboration outside every pool
/// is refused: not-allowed while hovering, tooltip on the click, no task.
#[gpui::test]
fn palette_drop_outside_pool_is_refused(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let pool = pool(&view, cx);
    let b = bounds_of(&view, cx, &pool);
    let task = Tool::Create(ElementKind::Task, false);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    let outside = at(START_CENTER.0, (b.y + b.h + 80.0) as f32);
    cx.simulate_mouse_move(outside, None, Modifiers::none());
    assert_eq!(refusal(&view, cx), Some(Refusal::OutsideParticipant));

    let tasks = count(&view, cx, is_task);
    cx.simulate_mouse_down(outside, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    assert_eq!(count(&view, cx, is_task), tasks);
    assert_eq!(errors(&view, cx).len(), 1);

    cx.simulate_click(palette_point(&task), Modifiers::none());
    let inside = at(START_CENTER.0 + 200.0, START_CENTER.1);
    cx.simulate_mouse_move(inside, MouseButton::Left, Modifiers::none());
    assert_eq!(refusal(&view, cx), None);
}

/// Connecting a start event to another start event is not allowed: the
/// pending connect shows not-allowed over the target and the click leaves
/// an error tooltip and no flow.
#[gpui::test]
fn refused_connect_shows_tooltip(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    create(
        &view,
        cx,
        ElementKind::StartEvent(EventDefinition::None),
        false,
        400.,
        300.,
    );

    cx.simulate_keystrokes("c");
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_mouse_move(at(400., 300.), None, Modifiers::none());
    assert_eq!(refusal(&view, cx), Some(Refusal::NotConnectable));

    cx.simulate_click(at(400., 300.), Modifiers::none());
    assert_eq!(count(&view, cx, is_flow), 0);

    let shown = errors(&view, cx);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].1, Refusal::NotConnectable.message());
}

/// An allowed connect leaves no tooltip.
#[gpui::test]
fn allowed_connect_has_no_tooltip(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    create(&view, cx, ElementKind::Task, false, 400., 300.);

    cx.simulate_keystrokes("c");
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.simulate_mouse_move(at(400., 300.), None, Modifiers::none());
    assert_eq!(refusal(&view, cx), None);

    cx.simulate_click(at(400., 300.), Modifiers::none());
    assert_eq!(count(&view, cx, is_flow), 1);
    assert!(errors(&view, cx).is_empty());
}

/// Dragging a boundary event onto the border of an event sub-process is
/// refused (it cannot attach there): not-allowed state, tooltip on release,
/// the event stays attached to its task.
#[gpui::test]
fn boundary_onto_event_subprocess_is_refused(cx: &mut TestAppContext) {
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
    let esp = create(
        &view,
        cx,
        ElementKind::SubProcess {
            triggered_by_event: true,
        },
        false,
        500.,
        700.,
    );
    let host = bounds_of(&view, cx, &task);
    let bottom = host.y + host.h;
    drag(cx, at(300., 400.), at(500., (bottom + 3.) as f32));
    assert!(matches!(
        kind_of(&view, cx, &event),
        ElementKind::BoundaryEvent { .. }
    ));
    let before = bounds_of(&view, cx, &event);

    let top = bounds_of(&view, cx, &esp).y;
    let from = at(500., bottom as f32);
    let onto = at(500., top as f32);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(onto, MouseButton::Left, Modifiers::none());
    assert_eq!(refusal(&view, cx), Some(Refusal::NotAttachable));

    cx.simulate_mouse_up(onto, MouseButton::Left, Modifiers::none());
    assert_eq!(bounds_of(&view, cx, &event), before);
    assert_eq!(errors(&view, cx)[0].1, Refusal::NotAttachable.message());
}

/// A new refusal replaces the previous tooltip instead of stacking.
#[gpui::test]
fn new_refusal_replaces_tooltip(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    for _ in 0..3 {
        view.update(cx, |c, cx| c.refuse(Refusal::NotConnectable, cx));
    }

    assert_eq!(errors(&view, cx).len(), 1);
}

/// A structured overlay runs its click handler when pressed and the press
/// does not reach the canvas (no selection change); a zoom range hides it
/// outside the range, so it no longer draws.
#[gpui::test]
fn structured_overlay_clicks_and_follows_zoom_range(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();

    let id = view.update(cx, |c, cx| {
        let mut overlay = Overlay::new(
            Anchor::Point(Waypoint { x: 700.0, y: 500.0 }),
            "go",
            Tone::Badge,
        );
        overlay.icon = Some("plus".to_string());
        overlay.color = Some("selection".to_string());
        overlay.show.max = Some(1.5);
        overlay.on_click = Some(Click::new(move |_, _| counter.set(counter.get() + 1)));

        c.insert_overlay(overlay, cx)
    });
    view.read_with(cx, |c, _| {
        let o = c.overlay(id).expect("inserted");
        assert_eq!(o.icon.as_deref(), Some("plus"));
        assert_eq!(c.overlay_boxes(false).len(), 1);
    });

    let press = view.read_with(cx, |c, _| {
        let p = c.docs[c.active]
            .viewport
            .to_screen(Waypoint { x: 700.0, y: 500.0 });
        at(p.x as f32 + 5.0 + 8.0, p.y as f32 + 5.0 + 8.0)
    });
    let selection = view.read_with(cx, |c, _| c.docs[c.active].selection.clone());
    cx.simulate_click(press, Modifiers::none());
    assert_eq!(clicks.get(), 1);
    assert_eq!(
        view.read_with(cx, |c, _| c.docs[c.active].selection.clone()),
        selection
    );

    view.update(cx, |c, _| c.docs[c.active].viewport.scale = 2.0);
    view.read_with(cx, |c, _| assert!(c.overlay_boxes(false).is_empty()));

    view.update(cx, |c, _| c.docs[c.active].viewport.scale = 1.0);
    view.read_with(cx, |c, _| assert_eq!(c.overlay_boxes(false).len(), 1));
}
