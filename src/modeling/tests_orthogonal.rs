//! Tests for the orthogonal-connection invariant: `layout::orthogonalize`
//! unit cases and a property-style run of many editing operations on
//! imported fixtures, asserting after each step that every connection uses
//! only horizontal and vertical segments.

use super::arrange::{Alignment, align};
use super::attach::{bendpoint_move, bendpoint_remove, segment_move};
use super::clipboard::{copy, paste};
use super::factory::{connect, reconnect};
use super::layout::{is_orthogonal, orthogonalize, orthogonalize_all};
use super::rules::is_flow_node;
use super::*;
use crate::model::{Bounds, Definitions, Edge, Element, ElementId, ElementKind, Shape, Waypoint};

/// Shorthand point constructor.
fn pt(x: f64, y: f64) -> Waypoint {
    Waypoint { x, y }
}

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}

/// Asserts that every edge of `defs` has at least two waypoints and only
/// horizontal or vertical segments.
pub(super) fn assert_orthogonal(defs: &Definitions, context: &str) {
    for edge in defs.edges() {
        let wps = &edge.waypoints;
        assert!(wps.len() >= 2, "{context}: {} too short", edge.element_id.0);

        for seg in wps.windows(2) {
            assert!(
                is_orthogonal(seg[0], seg[1]),
                "{context}: {} diagonal {:?} -> {:?} in {wps:?}",
                edge.element_id.0,
                seg[0],
                seg[1]
            );
        }
    }
}

/// A diagonal between side-docked ends becomes a Z through the middle.
#[test]
fn diagonal_between_side_docks_becomes_z() {
    let (s, t) = (b(0.0, 0.0, 100.0, 80.0), b(300.0, 100.0, 100.0, 80.0));
    let out = orthogonalize(&[pt(100.0, 40.0), pt(300.0, 140.0)], Some(&s), Some(&t));

    assert_eq!(
        out,
        [
            pt(100.0, 40.0),
            pt(200.0, 40.0),
            pt(200.0, 140.0),
            pt(300.0, 140.0)
        ]
    );
}

/// A side dock leaving to a top dock gets a single L elbow.
#[test]
fn side_to_top_becomes_l() {
    let (s, t) = (b(0.0, 0.0, 100.0, 80.0), b(300.0, 200.0, 100.0, 80.0));
    let out = orthogonalize(&[pt(100.0, 40.0), pt(350.0, 200.0)], Some(&s), Some(&t));

    assert_eq!(out, [pt(100.0, 40.0), pt(350.0, 40.0), pt(350.0, 200.0)]);
}

/// Bottom-to-top docks get a vertical Z.
#[test]
fn bottom_to_top_becomes_vertical_z() {
    let (s, t) = (b(0.0, 0.0, 100.0, 80.0), b(200.0, 200.0, 100.0, 80.0));
    let out = orthogonalize(&[pt(50.0, 80.0), pt(250.0, 200.0)], Some(&s), Some(&t));

    assert_eq!(
        out,
        [
            pt(50.0, 80.0),
            pt(50.0, 140.0),
            pt(250.0, 140.0),
            pt(250.0, 200.0)
        ]
    );
}

/// Inner diagonals continue the previous axis; duplicates and collinear
/// points are dropped; endpoints stay.
#[test]
fn inner_diagonal_and_cleanup() {
    let wps = [
        pt(0.0, 0.0),
        pt(50.0, 0.0),
        pt(50.0, 0.0),
        pt(100.0, 0.0),
        pt(200.0, 100.0),
        pt(200.0, 200.0),
    ];
    let out = orthogonalize(&wps, None, None);

    assert_eq!(out, [pt(0.0, 0.0), pt(200.0, 0.0), pt(200.0, 200.0)]);
}

/// Already orthogonal routes are left unchanged.
#[test]
fn orthogonal_route_unchanged() {
    let wps = [pt(0.0, 0.0), pt(50.0, 0.0), pt(50.0, 90.0), pt(120.0, 90.0)];

    assert_eq!(orthogonalize(&wps, None, None), wps);
}

/// Imports fixture `name`, normalizes it like the load path and checks the
/// invariant.
fn fixture(name: &str) -> Definitions {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let xml = std::fs::read_to_string(&path).unwrap();
    let mut defs = crate::io::import_xml(&xml).unwrap();

    orthogonalize_all(&mut defs);
    assert_orthogonal(&defs, &format!("import {name}"));
    defs
}

/// Ids of the flow-node shapes of `defs`, in paint order.
fn nodes(defs: &Definitions) -> Vec<ElementId> {
    defs.shapes()
        .map(|s| s.element_id.clone())
        .filter(|id| defs.get(id).is_some_and(|e| is_flow_node(&e.kind)))
        .collect()
}

/// Ids of the connections of `defs`, in paint order.
fn connections(defs: &Definitions) -> Vec<ElementId> {
    defs.edges().map(|e| e.element_id.clone()).collect()
}

/// Many pseudo-random operations (move, connect, bendpoint move/remove,
/// segment move, reconnect, paste, align, undo) on every fixture keep every
/// connection orthogonal.
#[test]
fn operations_keep_connections_orthogonal() {
    for name in [
        "message-delivery.bpmn",
        "collaboration.bpmn",
        "events.bpmn",
        "simple.bpmn",
    ] {
        let mut defs = fixture(name);
        let mut stack = CommandStack::default();
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move |n: usize| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) as usize) % n.max(1)
        };

        for step in 0..200 {
            let (ns, cs) = (nodes(&defs), connections(&defs));
            if ns.is_empty() {
                break;
            }

            let node = ns[next(ns.len())].clone();
            let other = ns[next(ns.len())].clone();
            let conn = cs.get(next(cs.len())).cloned();
            let len = conn
                .as_ref()
                .and_then(|c| defs.edges().find(|e| &e.element_id == c))
                .map_or(0, |e| e.waypoints.len());
            let offset = (next(9) as f64 - 4.0) * 17.0;

            let command = match next(9) {
                0 => Some(Command::MoveShapes {
                    ids: vec![node],
                    dx: offset,
                    dy: -offset * 1.5,
                }),
                1 => connect(&defs, &node, &other),
                2 if len > 2 => {
                    let wps = &defs.edges().find(|e| Some(&e.element_id) == conn.as_ref());
                    let p = wps.unwrap().waypoints[1];
                    bendpoint_move(
                        &defs,
                        conn.as_ref().unwrap(),
                        1,
                        pt(p.x + offset, p.y - offset),
                    )
                }
                3 if len > 2 => bendpoint_remove(&defs, conn.as_ref().unwrap(), len - 2),
                4 if len > 1 => segment_move(&defs, conn.as_ref().unwrap(), 0, offset),
                5 => conn.and_then(|c| reconnect(&defs, &c, next(2) == 0, &node)),
                6 => copy(&defs, &[node]).and_then(|clip| {
                    paste(&defs, &clip, pt(400.0 + offset, 300.0 - offset)).map(|(c, _)| c)
                }),
                7 => align(&defs, &[node, other], Alignment::Middle),
                _ => {
                    stack.undo(&mut defs);
                    None
                }
            };

            if let Some(command) = command {
                stack.execute(command, &mut defs);
            }
            assert_orthogonal(&defs, &format!("{name} step {step}"));
        }
    }
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

/// Process `p` (plane root) with tasks `a`, `b`, `c` and flow `f` from `a`
/// to `b`.
fn diagram() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));

    task(&mut defs, "a", b(0.0, 0.0, 100.0, 80.0));
    task(&mut defs, "b", b(200.0, 40.0, 100.0, 80.0));
    task(&mut defs, "c", b(500.0, 100.0, 100.0, 60.0));

    let kind = ElementKind::SequenceFlow {
        source: "a".into(),
        target: "b".into(),
        is_default: false,
        condition: None,
    };
    defs.add_element(Element::new("f", kind).with_parent("p"));
    defs.add_edge(Edge {
        element_id: "f".into(),
        waypoints: vec![
            Waypoint { x: 100.0, y: 40.0 },
            Waypoint { x: 200.0, y: 80.0 },
        ],
        label_bounds: None,
    });

    defs
}

/// Ids from string slices.
fn ids(list: &[&str]) -> Vec<ElementId> {
    list.iter().map(|s| ElementId::from(*s)).collect()
}

/// Executes `command` on `defs`, asserting it changed something.
fn run(command: Command, defs: &mut Definitions) -> Undo {
    command.execute(defs).expect("command changes the diagram")
}

/// Adds text annotation `note` at (400, 300) to `defs`.
fn annotation(defs: &mut Definitions) {
    defs.add_element(
        Element::new("note", ElementKind::TextAnnotation { text: None }).with_parent("p"),
    );
    defs.add_shape(Shape {
        element_id: "note".into(),
        bounds: b(400.0, 300.0, 100.0, 30.0),
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });
}

/// Whether every segment of `w` is horizontal or vertical.
fn orthogonal(w: &[Waypoint]) -> bool {
    w.windows(2).all(|s| s[0].x == s[1].x || s[0].y == s[1].y)
}

/// A new association routes orthogonally (deliberate deviation from
/// bpmn-js straight associations), and re-routes orthogonally when its
/// host moves.
#[test]
fn associations_route_orthogonally() {
    let mut defs = diagram();
    annotation(&mut defs);

    let command = super::factory::connect(&defs, &"a".into(), &"note".into()).unwrap();
    run(command, &mut defs);
    let id = defs
        .elements
        .values()
        .find(|e| matches!(e.kind, ElementKind::Association { .. }))
        .unwrap()
        .id
        .clone();
    let wps = |defs: &Definitions| {
        defs.edges()
            .find(|e| e.element_id == id)
            .unwrap()
            .waypoints
            .clone()
    };
    assert!(wps(&defs).len() > 2 && orthogonal(&wps(&defs)));

    run(
        Command::MoveShapes {
            ids: ids(&["a"]),
            dx: 0.0,
            dy: 37.0,
        },
        &mut defs,
    );
    assert!(orthogonal(&wps(&defs)));
}
