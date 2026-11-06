//! Tests for docking slots: connections sharing a shape side dock at the
//! middles of equal slices of that side.

use super::docking::{distribute, slot};
use super::factory::connect;
use super::layout::{Side, is_orthogonal};
use super::*;
use crate::model::{Bounds, Definitions, Edge, Element, ElementId, ElementKind, Shape, Waypoint};

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}

/// Shorthand point constructor.
fn pt(x: f64, y: f64) -> Waypoint {
    Waypoint { x, y }
}

/// Adds task `id` to process `p` with a DI shape at `bounds`.
fn task(defs: &mut Definitions, id: &str, bounds: Bounds) {
    defs.add_element(Element::new(id, ElementKind::Task).with_parent("p"));
    defs.add_shape(Shape {
        element_id: id.into(),
        bounds,
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });
}

/// Adds sequence flow `id` from `s` to `t` with explicit waypoints.
fn flow(defs: &mut Definitions, id: &str, s: &str, t: &str, waypoints: Vec<Waypoint>) {
    let kind = ElementKind::SequenceFlow {
        source: s.into(),
        target: t.into(),
        is_default: false,
        condition: None,
    };
    defs.add_element(Element::new(id, kind).with_parent("p"));
    defs.add_edge(Edge {
        element_id: id.into(),
        waypoints,
        label_bounds: None,
    });
}

/// Process `p` with target task `t` at (300, 100) 100x80 and sources `a`
/// (above-left) and `b` (below-left).
fn diagram() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));

    task(&mut defs, "t", b(300.0, 100.0, 100.0, 80.0));
    task(&mut defs, "a", b(0.0, 0.0, 100.0, 80.0));
    task(&mut defs, "b", b(0.0, 200.0, 100.0, 80.0));

    defs
}

/// Waypoints of connection `id`.
fn wps(defs: &Definitions, id: &str) -> Vec<Waypoint> {
    defs.edges()
        .find(|e| e.element_id == ElementId::from(id))
        .expect("edge exists")
        .waypoints
        .clone()
}

/// Asserts that every segment of `route` is horizontal or vertical.
fn assert_ortho(route: &[Waypoint]) {
    for seg in route.windows(2) {
        assert!(is_orthogonal(seg[0], seg[1]), "diagonal in {route:?}");
    }
}

/// Slot i of n sits at `(i + 0.5) / n` of the side.
#[test]
fn slot_splits_the_side_in_equal_parts() {
    let t = b(0.0, 0.0, 100.0, 60.0);

    assert_eq!(slot(&t, Side::Left, 0, 1), pt(0.0, 30.0));
    assert_eq!(slot(&t, Side::Left, 0, 2), pt(0.0, 15.0));
    assert_eq!(slot(&t, Side::Left, 1, 2), pt(0.0, 45.0));
    assert_eq!(slot(&t, Side::Right, 2, 3), pt(100.0, 50.0));
    assert_eq!(slot(&t, Side::Top, 0, 2), pt(25.0, 0.0));
    assert_eq!(slot(&t, Side::Bottom, 1, 2), pt(75.0, 60.0));
}

/// Two flows entering the same side split it in halves, ordered by where
/// their sources are (upper source takes the upper slot).
#[test]
fn two_incoming_flows_share_the_left_side() {
    let mut defs = diagram();
    flow(
        &mut defs,
        "fb",
        "b",
        "t",
        vec![
            pt(100.0, 240.0),
            pt(200.0, 240.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0),
        ],
    );
    flow(
        &mut defs,
        "fa",
        "a",
        "t",
        vec![
            pt(100.0, 40.0),
            pt(200.0, 40.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0),
        ],
    );

    let changes = distribute(&defs);
    for (id, waypoints) in changes {
        defs.plane_mut()
            .edges
            .iter_mut()
            .find(|e| e.element_id == id)
            .unwrap()
            .waypoints = waypoints;
    }

    assert_eq!(wps(&defs, "fa").last(), Some(&pt(300.0, 120.0)));
    assert_eq!(wps(&defs, "fb").last(), Some(&pt(300.0, 160.0)));
    assert_ortho(&wps(&defs, "fa"));
    assert_ortho(&wps(&defs, "fb"));
    assert_eq!(wps(&defs, "fa").first(), Some(&pt(100.0, 40.0)));
}

/// Three connections on one side take its sixths 1, 3 and 5; incoming
/// and outgoing share the slots.
#[test]
fn three_connections_take_thirds() {
    let mut defs = diagram();
    task(&mut defs, "c", b(0.0, 400.0, 100.0, 80.0));
    flow(
        &mut defs,
        "fa",
        "a",
        "t",
        vec![
            pt(100.0, 40.0),
            pt(200.0, 40.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0),
        ],
    );
    flow(
        &mut defs,
        "fb",
        "b",
        "t",
        vec![
            pt(100.0, 240.0),
            pt(200.0, 240.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0),
        ],
    );
    flow(
        &mut defs,
        "fc",
        "t",
        "c",
        vec![
            pt(300.0, 140.0),
            pt(250.0, 140.0),
            pt(250.0, 440.0),
            pt(100.0, 440.0),
        ],
    );

    let changes: Vec<(ElementId, Vec<Waypoint>)> = distribute(&defs);
    let end = |id: &str, last: bool| {
        let w = changes
            .iter()
            .find(|(c, _)| *c == ElementId::from(id))
            .map(|(_, w)| w.clone())
            .unwrap_or_else(|| wps(&defs, id));
        if last { *w.last().unwrap() } else { w[0] }
    };

    let ys = [end("fa", true).y, end("fb", true).y, end("fc", false).y];
    assert_eq!(ys, [100.0 + 80.0 / 6.0, 140.0, 100.0 + 5.0 * 80.0 / 6.0]);
}

/// A single connection stays at the side middle: nothing changes.
#[test]
fn single_connection_is_untouched() {
    let mut defs = diagram();
    flow(
        &mut defs,
        "fa",
        "a",
        "t",
        vec![
            pt(100.0, 40.0),
            pt(200.0, 40.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0),
        ],
    );

    assert!(distribute(&defs).is_empty());
}

/// A user order (docking positions along the side) wins over the order of
/// the other ends.
#[test]
fn current_order_is_kept() {
    let mut defs = diagram();
    flow(
        &mut defs,
        "fa",
        "a",
        "t",
        vec![
            pt(100.0, 40.0),
            pt(200.0, 40.0),
            pt(200.0, 170.0),
            pt(300.0, 170.0),
        ],
    );
    flow(
        &mut defs,
        "fb",
        "b",
        "t",
        vec![
            pt(100.0, 240.0),
            pt(200.0, 240.0),
            pt(200.0, 110.0),
            pt(300.0, 110.0),
        ],
    );

    let changes = distribute(&defs);
    let last = |id: &str| {
        *changes
            .iter()
            .find(|(c, _)| *c == ElementId::from(id))
            .unwrap()
            .1
            .last()
            .unwrap()
    };

    assert_eq!(last("fb"), pt(300.0, 120.0));
    assert_eq!(last("fa"), pt(300.0, 160.0));
}

/// Connecting a second flow through the command redistributes the side in
/// the same undoable step; undo restores the single middle docking.
#[test]
fn connect_redistributes_and_undo_restores() {
    let mut defs = diagram();
    let first = connect(&defs, &"a".into(), &"t".into()).unwrap();
    first.execute(&mut defs).unwrap();
    let before = defs.clone();

    let second = connect(&defs, &"b".into(), &"t".into()).unwrap();
    let undo = second.execute(&mut defs).unwrap();

    let ends: Vec<Waypoint> = defs.edges().map(|e| *e.waypoints.last().unwrap()).collect();
    assert!(ends.contains(&pt(300.0, 120.0)), "{ends:?}");
    assert!(ends.contains(&pt(300.0, 160.0)), "{ends:?}");
    for edge in defs.edges() {
        assert_ortho(&edge.waypoints);
    }

    undo.restore(&mut defs);
    assert_eq!(defs, before);
}

/// Deleting one of two flows brings the other back to the side middle.
#[test]
fn delete_redistributes() {
    let mut defs = diagram();
    connect(&defs, &"a".into(), &"t".into())
        .unwrap()
        .execute(&mut defs)
        .unwrap();
    let second = connect(&defs, &"b".into(), &"t".into()).unwrap();
    let id = created_id(&second).unwrap();
    second.execute(&mut defs).unwrap();

    Command::DeleteElements { ids: vec![id] }
        .execute(&mut defs)
        .unwrap();

    let edge = defs.edges().next().unwrap();
    assert_eq!(edge.waypoints.last(), Some(&pt(300.0, 140.0)));
}

/// Loading distributes every plane but keeps imported label positions.
#[test]
fn distribute_all_on_load_keeps_labels() {
    let mut defs = diagram();
    flow(
        &mut defs,
        "fa",
        "a",
        "t",
        vec![
            pt(100.0, 40.0),
            pt(200.0, 40.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0),
        ],
    );
    flow(
        &mut defs,
        "fb",
        "b",
        "t",
        vec![
            pt(100.0, 240.0),
            pt(200.0, 240.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0),
        ],
    );
    let label = Some(b(150.0, 20.0, 40.0, 14.0));
    defs.plane_mut().edges[0].label_bounds = label;

    super::docking::distribute_all(&mut defs);

    assert_eq!(wps(&defs, "fa").last(), Some(&pt(300.0, 120.0)));
    assert_eq!(wps(&defs, "fb").last(), Some(&pt(300.0, 160.0)));
    assert_eq!(defs.plane().edges[0].label_bounds, label);
}

/// Dragging an end along its own shape swaps the slot order without
/// disconnecting.
#[test]
fn redock_reorders_slots_on_the_same_side() {
    let mut defs = diagram();
    connect(&defs, &"a".into(), &"t".into())
        .unwrap()
        .execute(&mut defs)
        .unwrap();
    let second = connect(&defs, &"b".into(), &"t".into()).unwrap();
    let fb = created_id(&second).unwrap();
    second.execute(&mut defs).unwrap();
    assert_eq!(wps(&defs, fb.as_str()).last(), Some(&pt(300.0, 160.0)));

    super::docking::redock(&defs, &fb, false, pt(290.0, 101.0))
        .expect("redock")
        .execute(&mut defs)
        .expect("changed");

    assert_eq!(wps(&defs, fb.as_str()).last(), Some(&pt(300.0, 120.0)));
    let kind = &defs.get(&fb).unwrap().kind;
    assert_eq!(kind.endpoints().map(|(_, t)| t.as_str()), Some("t"));
    for edge in defs.edges() {
        assert_ortho(&edge.waypoints);
    }
}

/// Dragging an end onto another side of its shape moves it there; the
/// connection stays connected, orthogonal and outside the shape.
#[test]
fn redock_moves_an_end_to_another_side() {
    let mut defs = diagram();
    connect(&defs, &"a".into(), &"t".into())
        .unwrap()
        .execute(&mut defs)
        .unwrap();
    let fa = defs.edges().next().unwrap().element_id.clone();

    super::docking::redock(&defs, &fa, false, pt(330.0, 95.0))
        .expect("redock")
        .execute(&mut defs)
        .expect("changed");

    let route = wps(&defs, fa.as_str());
    assert_eq!(route.last(), Some(&pt(350.0, 100.0)), "{route:?}");
    assert_ortho(&route);
    let t = b(300.0, 100.0, 100.0, 80.0);
    for seg in route.windows(2) {
        let (x0, x1) = (seg[0].x.min(seg[1].x), seg[0].x.max(seg[1].x));
        let (y0, y1) = (seg[0].y.min(seg[1].y), seg[0].y.max(seg[1].y));
        let crosses = x1 > t.x && x0 < t.x + t.w && y1 > t.y && y0 < t.y + t.h;
        assert!(!crosses, "segment through the shape in {route:?}");
    }
}
