//! Tests for the bpmn-js parity features: connection rules (loops,
//! event-based gateways, compensation), boundary attach/detach, resize
//! rules, lane tools, waypoint edits, layout and label placement.

use super::attach::{
    attach, bendpoint_insert, bendpoint_move, bendpoint_remove, move_boundary, segment_move,
    snap_to_border,
};
use super::factory::connect;
use super::label::{flow_label_bounds, has_external_label, shape_label_bounds};
use super::lanes::{LaneSide, add_lane, child_lanes, resize, split_lane};
use super::layout::{loop_route, manhattan, manhattan_avoiding};
use super::rules::{Connection, can_attach, can_connect, min_size};
use super::waypoints::move_segment;
use super::*;
use crate::model::{
    Bounds, Definitions, Edge, Element, ElementId, ElementKind, EventDefinition, Shape, Waypoint,
};

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}

/// Shorthand point constructor.
fn pt(x: f64, y: f64) -> Waypoint {
    Waypoint { x, y }
}

/// Adds element `id` of `kind` under `parent` with a DI shape at `bounds`.
fn node(defs: &mut Definitions, id: &str, kind: ElementKind, parent: &str, bounds: Bounds) {
    defs.add_element(Element::new(id, kind).with_parent(parent));
    defs.add_shape(Shape {
        element_id: id.into(),
        bounds,
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });
}

/// Adds sequence flow `id` from `s` to `t` under `parent` with waypoints.
fn flow(defs: &mut Definitions, id: &str, s: &str, t: &str, parent: &str, wps: Vec<Waypoint>) {
    let kind = ElementKind::SequenceFlow {
        source: s.into(),
        target: t.into(),
        is_default: false,
        condition: None,
    };
    defs.add_element(Element::new(id, kind).with_parent(parent));
    defs.add_edge(Edge {
        element_id: id.into(),
        waypoints: wps,
        label_bounds: None,
    });
}

/// Pool `a` (process `pa`) at (0,0,600,250) with lanes `l1` (top) and `l2`
/// (bottom), `t1` in `l1`, `t2` in `l2`, and pool `b` (process `pb`) below.
fn lanes() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("c".into());
    defs.add_element(Element::new("c", ElementKind::Collaboration));
    defs.add_element(Element::new("pa", ElementKind::Process));
    defs.add_element(Element::new("pb", ElementKind::Process));

    let pool = |p: &str| ElementKind::Participant {
        process_ref: Some(p.into()),
    };
    node(&mut defs, "a", pool("pa"), "c", b(0.0, 0.0, 600.0, 250.0));
    node(
        &mut defs,
        "l1",
        ElementKind::Lane,
        "pa",
        b(30.0, 0.0, 570.0, 125.0),
    );
    node(
        &mut defs,
        "l2",
        ElementKind::Lane,
        "pa",
        b(30.0, 125.0, 570.0, 125.0),
    );
    node(
        &mut defs,
        "t1",
        ElementKind::Task,
        "l1",
        b(100.0, 20.0, 100.0, 80.0),
    );
    node(
        &mut defs,
        "t2",
        ElementKind::Task,
        "l2",
        b(100.0, 150.0, 100.0, 80.0),
    );
    node(&mut defs, "b", pool("pb"), "c", b(0.0, 400.0, 600.0, 250.0));

    defs
}

/// Process `p` with task `task` (100,100), timer catch `catch`, start
/// `start` -> `catch` flow, event sub-process `esp` and a gateway.
fn process() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));

    node(
        &mut defs,
        "task",
        ElementKind::Task,
        "p",
        b(100.0, 100.0, 100.0, 80.0),
    );
    node(
        &mut defs,
        "catch",
        ElementKind::IntermediateCatchEvent(EventDefinition::Timer),
        "p",
        b(300.0, 300.0, 36.0, 36.0),
    );
    node(
        &mut defs,
        "start",
        ElementKind::StartEvent(EventDefinition::None),
        "p",
        b(0.0, 300.0, 36.0, 36.0),
    );
    node(
        &mut defs,
        "esp",
        ElementKind::SubProcess {
            triggered_by_event: true,
        },
        "p",
        b(500.0, 100.0, 100.0, 80.0),
    );
    node(
        &mut defs,
        "gw",
        ElementKind::EventBasedGateway,
        "p",
        b(0.0, 500.0, 50.0, 50.0),
    );
    flow(
        &mut defs,
        "f1",
        "start",
        "catch",
        "p",
        vec![pt(36.0, 318.0), pt(300.0, 318.0)],
    );

    defs
}

/// Bounds of the shape of `id`.
fn bounds(defs: &Definitions, id: &str) -> Bounds {
    defs.shape(&id.into()).unwrap().bounds
}

/// Parent id of `id`.
fn parent(defs: &Definitions, id: &str) -> String {
    defs.get(&id.into()).unwrap().parent.clone().unwrap().0
}

/// Executes `command` on a fresh stack, asserts undo restores `defs`, then
/// redoes it so the caller sees the executed state.
fn run(defs: &mut Definitions, command: Command) {
    let before = defs.clone();
    let mut stack = CommandStack::default();

    assert!(stack.execute(command, defs));
    let after = defs.clone();

    assert!(stack.undo(defs));
    assert_eq!(*defs, before);

    assert!(stack.redo(defs));
    assert_eq!(*defs, after);
}

/// Sequence flow loops are allowed (bpmn-js has no self check for them);
/// other self-connections are not.
#[test]
fn rules_allow_sequence_loops_only() {
    let mut defs = process();
    let rule = |d: &Definitions, s: &str, t: &str| can_connect(d, &s.into(), &t.into());

    assert_eq!(rule(&defs, "task", "task"), Some(Connection::Sequence));
    assert_eq!(rule(&defs, "start", "start"), None);

    let lanes = lanes();
    assert_eq!(rule(&lanes, "a", "a"), None);

    let command = connect(&defs, &"task".into(), &"task".into()).unwrap();
    command.execute(&mut defs).unwrap();
    let edge = defs.edges().last().unwrap();
    assert_eq!(edge.waypoints, loop_route(&b(100.0, 100.0, 100.0, 80.0)));
    assert_eq!(edge.waypoints.len(), 5);
}

/// Event-based gateways only connect to receive tasks and message, timer,
/// signal or conditional catch events.
#[test]
fn rules_event_based_gateway_targets() {
    let mut defs = process();
    node(
        &mut defs,
        "rt",
        ElementKind::ReceiveTask,
        "p",
        b(200.0, 500.0, 100.0, 80.0),
    );
    node(
        &mut defs,
        "none",
        ElementKind::IntermediateCatchEvent(EventDefinition::Link),
        "p",
        b(400.0, 500.0, 36.0, 36.0),
    );
    let rule = |s: &str, t: &str| can_connect(&defs, &s.into(), &t.into());

    assert_eq!(rule("gw", "catch"), Some(Connection::Sequence));
    assert_eq!(rule("gw", "rt"), Some(Connection::Sequence));
    assert_eq!(rule("gw", "task"), None);
    assert_eq!(rule("gw", "none"), None);
    assert_eq!(rule("task", "none"), None);
    assert_eq!(rule("task", "esp"), None);
    assert_eq!(rule("esp", "task"), None);
}

/// Compensation boundaries link to activities by a directed association,
/// never by sequence flow; data associations accept events.
#[test]
fn rules_compensation_and_data_events() {
    let mut defs = process();
    node(
        &mut defs,
        "comp",
        ElementKind::BoundaryEvent {
            definition: EventDefinition::Compensation,
            attached_to: "task".into(),
            cancel_activity: true,
        },
        "p",
        b(132.0, 162.0, 36.0, 36.0),
    );
    node(
        &mut defs,
        "undo",
        ElementKind::Task,
        "p",
        b(100.0, 300.0, 100.0, 80.0),
    );
    node(
        &mut defs,
        "data",
        ElementKind::DataObjectReference,
        "p",
        b(700.0, 0.0, 36.0, 50.0),
    );
    let rule = |d: &Definitions, s: &str, t: &str| can_connect(d, &s.into(), &t.into());

    assert_eq!(rule(&defs, "comp", "undo"), Some(Connection::Compensation));
    assert_eq!(rule(&defs, "comp", "task"), None);
    assert_eq!(rule(&defs, "comp", "catch"), None);
    assert_eq!(rule(&defs, "start", "data"), Some(Connection::DataOutput));
    assert_eq!(rule(&defs, "data", "start"), None);

    connect(&defs, &"comp".into(), &"undo".into())
        .unwrap()
        .execute(&mut defs)
        .unwrap();
    let assoc = defs.elements.values().last().unwrap();
    assert!(matches!(assoc.kind, ElementKind::Association { .. }));
    assert_eq!(
        assoc.details.association_direction,
        Some(crate::model::AssociationDirection::One)
    );
}

/// Attach rules: candidate events on an activity border only.
#[test]
fn can_attach_rules() {
    let mut defs = process();
    let ok =
        |d: &Definitions, e: &str, h: &str, p: Waypoint| can_attach(d, &e.into(), &h.into(), p);

    assert!(ok(&defs, "catch", "task", pt(150.0, 180.0)));
    assert!(ok(&defs, "catch", "task", pt(105.0, 140.0)));
    assert!(!ok(&defs, "catch", "task", pt(150.0, 140.0)));
    assert!(!ok(&defs, "start", "task", pt(150.0, 180.0)));
    assert!(!ok(&defs, "catch", "esp", pt(550.0, 180.0)));
    assert!(!ok(&defs, "catch", "gw", pt(25.0, 550.0)));

    node(
        &mut defs,
        "rt",
        ElementKind::ReceiveTask,
        "p",
        b(200.0, 500.0, 100.0, 80.0),
    );
    flow(
        &mut defs,
        "f9",
        "gw",
        "rt",
        "p",
        vec![pt(50.0, 525.0), pt(200.0, 525.0)],
    );
    assert!(!ok(&defs, "catch", "rt", pt(250.0, 580.0)));
}

/// Snapping puts the center on the nearest border(s).
#[test]
fn snap_to_border_sides() {
    let host = b(100.0, 100.0, 100.0, 80.0);

    assert_eq!(
        snap_to_border(pt(150.0, 175.0), &host),
        Some(pt(150.0, 180.0))
    );
    assert_eq!(
        snap_to_border(pt(95.0, 140.0), &host),
        Some(pt(100.0, 140.0))
    );
    assert_eq!(
        snap_to_border(pt(198.0, 102.0), &host),
        Some(pt(200.0, 100.0))
    );
    assert_eq!(snap_to_border(pt(150.0, 140.0), &host), None);
}

/// Attaching turns a catch event into a boundary event on the host border,
/// drops its incoming flow; the host then drags and deletes it.
#[test]
fn attach_move_and_delete_with_host() {
    let mut defs = process();

    let command = attach(&defs, &"catch".into(), &"task".into(), pt(150.0, 178.0)).unwrap();
    run(&mut defs, command);

    let event = defs.get(&"catch".into()).unwrap();
    assert_eq!(
        event.kind,
        ElementKind::BoundaryEvent {
            definition: EventDefinition::Timer,
            attached_to: "task".into(),
            cancel_activity: true,
        }
    );
    assert_eq!(parent(&defs, "catch"), "p");
    assert_eq!(bounds(&defs, "catch"), b(132.0, 162.0, 36.0, 36.0));
    assert!(defs.get(&"f1".into()).is_none());

    let mv = Command::MoveShapes {
        ids: vec!["task".into()],
        dx: 10.0,
        dy: 0.0,
    };
    mv.execute(&mut defs).unwrap();
    assert_eq!(bounds(&defs, "catch").x, 142.0);

    let del = Command::DeleteElements {
        ids: vec!["task".into()],
    };
    del.execute(&mut defs).unwrap();
    assert!(defs.get(&"catch".into()).is_none());
}

/// Dragging a boundary event off its host detaches it; onto another
/// activity's border re-attaches it there.
#[test]
fn move_boundary_detaches_and_reattaches() {
    let mut defs = process();
    attach(&defs, &"catch".into(), &"task".into(), pt(150.0, 180.0))
        .unwrap()
        .execute(&mut defs)
        .unwrap();
    node(
        &mut defs,
        "other",
        ElementKind::Task,
        "p",
        b(100.0, 300.0, 100.0, 80.0),
    );

    let reattach = move_boundary(&defs, &"catch".into(), 0.0, 200.0).unwrap();
    run(&mut defs, reattach);
    assert!(matches!(
        &defs.get(&"catch".into()).unwrap().kind,
        ElementKind::BoundaryEvent { attached_to, .. } if attached_to.as_str() == "other"
    ));
    assert_eq!(bounds(&defs, "catch"), b(132.0, 362.0, 36.0, 36.0));

    let detach = move_boundary(&defs, &"catch".into(), 400.0, 0.0).unwrap();
    run(&mut defs, detach);
    assert_eq!(
        defs.get(&"catch".into()).unwrap().kind,
        ElementKind::IntermediateCatchEvent(EventDefinition::Timer)
    );
    assert_eq!(bounds(&defs, "catch"), b(532.0, 362.0, 36.0, 36.0));
}

/// bpmn-js resize rules: tasks are not resizable; minimum sizes per kind.
#[test]
fn resize_rules_and_min_sizes() {
    let mut defs = process();

    assert!(resize(&defs, &"task".into(), b(100.0, 100.0, 200.0, 80.0)).is_none());
    assert_eq!(min_size(&ElementKind::Lane, false), Some((300.0, 60.0)));
    assert_eq!(
        min_size(&ElementKind::Participant { process_ref: None }, false),
        Some((300.0, 150.0))
    );
    assert_eq!(
        min_size(&ElementKind::TextAnnotation { text: None }, false),
        Some((50.0, 30.0))
    );

    let sub = ElementKind::SubProcess {
        triggered_by_event: false,
    };
    node(&mut defs, "sub", sub, "p", b(0.0, 700.0, 350.0, 200.0));
    defs.plane_mut().shapes.last_mut().unwrap().is_expanded = Some(true);

    let left_drag = resize(&defs, &"sub".into(), b(300.0, 700.0, 50.0, 200.0)).unwrap();
    run(&mut defs, left_drag);
    assert_eq!(bounds(&defs, "sub"), b(210.0, 700.0, 140.0, 200.0));
}

/// Resizing keeps children inside (padded) on the moved edges.
#[test]
fn resize_keeps_children_inside() {
    let mut defs = process();
    let sub = ElementKind::SubProcess {
        triggered_by_event: false,
    };
    node(&mut defs, "sub", sub, "p", b(0.0, 700.0, 350.0, 200.0));
    defs.plane_mut().shapes.last_mut().unwrap().is_expanded = Some(true);
    node(
        &mut defs,
        "inner",
        ElementKind::Task,
        "sub",
        b(150.0, 750.0, 100.0, 80.0),
    );

    let command = resize(&defs, &"sub".into(), b(0.0, 700.0, 150.0, 150.0)).unwrap();
    command.execute(&mut defs).unwrap();
    assert_eq!(bounds(&defs, "sub"), b(0.0, 700.0, 270.0, 150.0));
}

/// Resizing a pool or a lane drags the lanes sharing the moved edge.
#[test]
fn resize_redistributes_lanes() {
    let mut defs = lanes();

    let pool = resize(&defs, &"a".into(), b(0.0, 0.0, 600.0, 300.0)).unwrap();
    run(&mut defs, pool);
    assert_eq!(bounds(&defs, "l1"), b(30.0, 0.0, 570.0, 125.0));
    assert_eq!(bounds(&defs, "l2"), b(30.0, 125.0, 570.0, 175.0));

    let lane = resize(&defs, &"l1".into(), b(30.0, 0.0, 570.0, 145.0)).unwrap();
    run(&mut defs, lane);
    assert_eq!(bounds(&defs, "l2"), b(30.0, 145.0, 570.0, 155.0));
    assert_eq!(bounds(&defs, "a"), b(0.0, 0.0, 600.0, 300.0));

    let wide = resize(&defs, &"l1".into(), b(30.0, 0.0, 670.0, 145.0)).unwrap();
    wide.execute(&mut defs).unwrap();
    assert_eq!(bounds(&defs, "a").w, 700.0);
    assert_eq!(bounds(&defs, "l2").w, 670.0);
}

/// Adding a lane below grows the pool, moves nothing above, and the new
/// lane fills the opened space; above grows the pool upward.
#[test]
fn add_lane_above_and_below() {
    let mut defs = lanes();

    let below = add_lane(&defs, &"l2".into(), LaneSide::Below).unwrap();
    run(&mut defs, below);
    assert_eq!(bounds(&defs, "a"), b(0.0, 0.0, 600.0, 370.0));
    assert_eq!(bounds(&defs, "Lane_1"), b(30.0, 250.0, 570.0, 120.0));
    assert_eq!(bounds(&defs, "t2"), b(100.0, 150.0, 100.0, 80.0));
    assert_eq!(bounds(&defs, "b"), b(0.0, 400.0, 600.0, 250.0));
    assert_eq!(parent(&defs, "Lane_1"), "pa");

    let above = add_lane(&defs, &"l1".into(), LaneSide::Above).unwrap();
    run(&mut defs, above);
    assert_eq!(bounds(&defs, "a"), b(0.0, -120.0, 600.0, 490.0));
    assert_eq!(bounds(&defs, "Lane_2"), b(30.0, -120.0, 570.0, 120.0));
    assert_eq!(bounds(&defs, "l1"), b(30.0, 0.0, 570.0, 125.0));
    assert_eq!(parent(&defs, "t1"), "l1");
    assert_eq!(child_lanes(&defs, &"pa".into()).len(), 4);

    let order: Vec<&str> = defs.shapes().map(|s| s.element_id.as_str()).collect();
    assert_eq!(&order[..5], ["a", "l1", "l2", "Lane_1", "Lane_2"]);
}

/// Adding a lane to a pool without lanes first wraps its content in one.
#[test]
fn add_lane_to_plain_pool() {
    let mut defs = lanes();
    node(
        &mut defs,
        "tb",
        ElementKind::Task,
        "pb",
        b(100.0, 450.0, 100.0, 80.0),
    );

    let command = add_lane(&defs, &"b".into(), LaneSide::Below).unwrap();
    run(&mut defs, command);
    assert_eq!(bounds(&defs, "Lane_1"), b(30.0, 400.0, 570.0, 250.0));
    assert_eq!(bounds(&defs, "Lane_2"), b(30.0, 650.0, 570.0, 120.0));
    assert_eq!(bounds(&defs, "b"), b(0.0, 400.0, 600.0, 370.0));
    assert_eq!(parent(&defs, "tb"), "Lane_1");
}

/// Split divides the pool into equal lanes; the last absorbs rounding.
#[test]
fn split_pool_into_three() {
    let mut defs = lanes();
    node(
        &mut defs,
        "tb",
        ElementKind::Task,
        "pb",
        b(100.0, 600.0, 40.0, 40.0),
    );

    let command = split_lane(&defs, &"b".into(), 3).unwrap();
    run(&mut defs, command);
    let lanes = child_lanes(&defs, &"pb".into());
    let heights: Vec<f64> = lanes
        .iter()
        .map(|l| defs.shape(l).unwrap().bounds.h)
        .collect();
    assert_eq!(heights, [83.0, 83.0, 84.0]);
    assert_eq!(parent(&defs, "tb"), lanes[2].as_str());

    assert!(split_lane(&defs, &"b".into(), 2).is_none());

    let resplit = split_lane(&defs, &"a".into(), 3).unwrap();
    resplit.execute(&mut defs).unwrap();
    assert_eq!(bounds(&defs, "l1").h, 83.0);
    assert_eq!(bounds(&defs, "l2"), b(30.0, 83.0, 570.0, 83.0));
}

/// Deleting a lane keeps its flow nodes and lets neighbors absorb its space.
#[test]
fn delete_lane_collapses_space() {
    let mut defs = lanes();

    let del = Command::DeleteElements {
        ids: vec!["l1".into()],
    };
    run(&mut defs, del);
    assert!(defs.get(&"l1".into()).is_none());
    assert!(defs.get(&"t1".into()).is_some());
    assert_eq!(bounds(&defs, "l2"), b(30.0, 0.0, 570.0, 250.0));
    assert_eq!(parent(&defs, "t1"), "l2");

    let mut three = lanes();
    split_lane(&three, &"a".into(), 3)
        .unwrap()
        .execute(&mut three)
        .unwrap();
    Command::DeleteElements {
        ids: vec!["l2".into()],
    }
    .execute(&mut three)
    .unwrap();
    assert_eq!(bounds(&three, "l1"), b(30.0, 0.0, 570.0, 124.5));
    assert_eq!(bounds(&three, "Lane_1"), b(30.0, 124.5, 570.0, 125.5));
}

/// Bendpoint edits on a connection, each one undoable and orthogonal: a
/// collinear inserted bendpoint is dropped, a moved bendpoint drags its
/// inner neighbor along and a removed one collapses the path.
#[test]
fn bendpoint_commands() {
    let mut defs = process();
    let id: ElementId = "f1".into();

    let command = bendpoint_insert(&defs, &id, 0, pt(150.0, 318.0)).unwrap();
    assert!(command.execute(&mut defs.clone()).is_none());

    let command = segment_move(&defs, &id, 0, 20.0).unwrap();
    run(&mut defs, command);
    assert_eq!(
        defs.edges().next().unwrap().waypoints,
        [
            pt(36.0, 318.0),
            pt(36.0, 338.0),
            pt(300.0, 338.0),
            pt(300.0, 318.0)
        ]
    );

    let command = bendpoint_move(&defs, &id, 2, pt(300.0, 360.0)).unwrap();
    run(&mut defs, command);
    assert_eq!(
        defs.edges().next().unwrap().waypoints,
        [
            pt(36.0, 318.0),
            pt(36.0, 360.0),
            pt(300.0, 360.0),
            pt(300.0, 318.0)
        ]
    );

    assert!(bendpoint_move(&defs, &id, 0, pt(0.0, 0.0)).is_none());
    let command = bendpoint_remove(&defs, &id, 1).unwrap();
    run(&mut defs, command);
    assert_eq!(
        defs.edges().next().unwrap().waypoints,
        [pt(36.0, 318.0), pt(300.0, 318.0)]
    );
}

/// Moving an inner orthogonal segment shifts both of its points.
#[test]
fn move_inner_segment() {
    let wps = [
        pt(0.0, 0.0),
        pt(50.0, 0.0),
        pt(50.0, 100.0),
        pt(100.0, 100.0),
    ];

    assert_eq!(
        move_segment(&wps, 1, 10.0).unwrap(),
        [
            pt(0.0, 0.0),
            pt(60.0, 0.0),
            pt(60.0, 100.0),
            pt(100.0, 100.0)
        ]
    );
    assert!(move_segment(&[pt(0.0, 0.0), pt(10.0, 10.0)], 0, 5.0).is_none());
}

/// Moving one end keeps user bendpoints and fixes the adjacent one.
#[test]
fn move_keeps_user_bendpoints() {
    let mut defs = process();
    let custom = vec![
        pt(36.0, 318.0),
        pt(150.0, 318.0),
        pt(150.0, 250.0),
        pt(318.0, 250.0),
        pt(318.0, 300.0),
    ];
    defs.plane_mut().edges[0].waypoints = custom;

    Command::MoveShapes {
        ids: vec!["start".into()],
        dx: 0.0,
        dy: 10.0,
    }
    .execute(&mut defs)
    .unwrap();
    assert_eq!(
        defs.edges().next().unwrap().waypoints,
        [
            pt(36.0, 328.0),
            pt(150.0, 328.0),
            pt(150.0, 250.0),
            pt(318.0, 250.0),
            pt(318.0, 300.0)
        ]
    );
}

/// Routes go around an obstacle when an alternative is clear.
#[test]
fn manhattan_avoids_obstacles() {
    let (s, t) = (b(0.0, 0.0, 100.0, 80.0), b(300.0, 200.0, 100.0, 80.0));
    let blocker = b(140.0, 0.0, 100.0, 120.0);

    assert_eq!(manhattan_avoiding(&s, &t, &[]), manhattan(&s, &t));

    let routed = manhattan_avoiding(&s, &t, &[blocker]);
    assert_eq!(
        routed,
        [
            pt(50.0, 80.0),
            pt(50.0, 140.0),
            pt(350.0, 140.0),
            pt(350.0, 200.0)
        ]
    );
}

/// External labels: 90x20 below shapes, inside the top of groups, and 15px
/// off the middle segment of flows.
#[test]
fn label_positions() {
    assert!(has_external_label(&ElementKind::ExclusiveGateway));
    assert!(!has_external_label(&ElementKind::Task));

    let event = ElementKind::StartEvent(EventDefinition::None);
    assert_eq!(
        shape_label_bounds(&event, &b(100.0, 100.0, 36.0, 36.0)),
        b(73.0, 136.0, 90.0, 20.0)
    );
    assert_eq!(
        shape_label_bounds(&ElementKind::Group, &b(0.0, 0.0, 300.0, 300.0)),
        b(105.0, 0.0, 90.0, 20.0)
    );
    assert_eq!(
        flow_label_bounds(&[pt(0.0, 0.0), pt(100.0, 0.0)]),
        Some(b(5.0, -25.0, 90.0, 20.0))
    );
    assert_eq!(
        flow_label_bounds(&[pt(0.0, 0.0), pt(0.0, 100.0)]),
        Some(b(-30.0, 40.0, 90.0, 20.0))
    );
}

/// Moving a shape moves its external label with it.
#[test]
fn move_moves_external_label() {
    let mut defs = process();
    let label = shape_label_bounds(
        &ElementKind::StartEvent(EventDefinition::None),
        &bounds(&defs, "start"),
    );
    defs.plane_mut().shapes[2].label_bounds = Some(label);

    Command::MoveShapes {
        ids: vec!["start".into()],
        dx: 5.0,
        dy: 7.0,
    }
    .execute(&mut defs)
    .unwrap();
    assert_eq!(
        defs.shape(&"start".into()).unwrap().label_bounds,
        Some(b(label.x + 5.0, label.y + 7.0, 90.0, 20.0))
    );
}
