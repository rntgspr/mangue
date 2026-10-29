//! Tests for vertical pools (the `PREFERRED_LAYOUTS_VERTICAL` table of
//! bpmn-js `BpmnLayouter`) and `preserveDocking`.

use super::Command;
use super::factory::{connect, reconnect};
use super::layout::{
    End, Keep, Rule, Side, is_orthogonal, loop_route_in, preferred, preferred_ends,
    preferred_ends_in, preferred_keeping,
};
use super::rules::is_direction_horizontal;
use crate::model::{Bounds, Definitions, Edge, Element, ElementKind, Shape, Waypoint};

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}

/// Shorthand point constructor.
fn pt(x: f64, y: f64) -> Waypoint {
    Waypoint { x, y }
}

/// Asserts that every segment of `route` is horizontal or vertical.
fn assert_ortho(route: &[Waypoint]) {
    assert!(route.len() >= 2, "{route:?}");
    for seg in route.windows(2) {
        assert!(is_orthogonal(seg[0], seg[1]), "diagonal in {route:?}");
    }
}

/// Adds `element` with a DI shape at `bounds`.
fn put(defs: &mut Definitions, element: Element, bounds: Bounds, horizontal: Option<bool>) {
    let id = element.id.clone();
    defs.add_element(element);
    defs.add_shape(Shape {
        element_id: id,
        bounds,
        is_expanded: None,
        is_horizontal: horizontal,
        label_bounds: None,
    });
}

/// Collaboration with a vertical pool `pool` (process `p`) holding two
/// tasks, `a` at the top-left and `c` below and to the right of it.
fn vertical_pool() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("collab".into());
    defs.add_element(Element::new("collab", ElementKind::Collaboration));
    defs.add_element(Element::new("p", ElementKind::Process));
    put(
        &mut defs,
        Element::new(
            "pool",
            ElementKind::Participant {
                process_ref: Some("p".into()),
            },
        ),
        b(-50.0, -80.0, 500.0, 600.0),
        Some(false),
    );
    put(
        &mut defs,
        Element::new("a", ElementKind::Task).with_parent("p"),
        b(0.0, 0.0, 100.0, 80.0),
        None,
    );
    put(
        &mut defs,
        Element::new("c", ElementKind::Task).with_parent("p"),
        b(200.0, 150.0, 100.0, 80.0),
        None,
    );

    defs
}

/// The vertical table swaps `h` and `v`: `v:v` flows, `h:v` out of a gateway,
/// `v:h` into one, `h:h` message flows; the horizontal table is unchanged.
#[test]
fn vertical_table_swaps_axes() {
    let (s, t) = (b(0.0, 0.0, 100.0, 80.0), b(200.0, 150.0, 100.0, 80.0));
    let table = [
        (Rule::Default, (End::V, End::V), (End::H, End::H)),
        (Rule::FromGateway, (End::H, End::V), (End::V, End::H)),
        (Rule::ToGateway, (End::V, End::H), (End::H, End::V)),
        (Rule::Message, (End::H, End::H), (End::V, End::V)),
    ];

    for (rule, vertical, horizontal) in table {
        assert_eq!(preferred_ends_in(&s, &t, rule, false), vertical);
        assert_eq!(preferred_ends_in(&s, &t, rule, true), horizontal);
        assert_eq!(preferred_ends(&s, &t, rule), horizontal);
    }

    let route = preferred(&s, &t, preferred_ends_in(&s, &t, Rule::Default, false));
    assert_eq!(
        route,
        vec![
            pt(50.0, 80.0),
            pt(50.0, 115.0),
            pt(250.0, 115.0),
            pt(250.0, 150.0)
        ]
    );
}

/// Boundary events in a vertical pool: side-attached ends match the
/// horizontal table, corner-attached ones leave horizontally when the
/// target shares the horizontal side.
#[test]
fn vertical_boundary_variants() {
    let host = b(100.0, 100.0, 100.0, 80.0);
    let rule = Rule::Boundary { host, loops: false };

    let side = b(182.0, 122.0, 36.0, 36.0);
    let right = b(350.0, 100.0, 100.0, 80.0);
    assert_eq!(
        preferred_ends_in(&side, &right, rule, false),
        (End::At(Side::Right), End::H)
    );

    let corner = b(182.0, 162.0, 36.0, 36.0);
    let below_right = b(300.0, 400.0, 100.0, 80.0);
    assert_eq!(
        preferred_ends_in(&corner, &below_right, rule, false),
        (End::At(Side::Right), End::V)
    );
    assert_eq!(
        preferred_ends_in(&corner, &below_right, rule, true).0,
        End::At(Side::Bottom)
    );

    let below_left = b(-100.0, 400.0, 100.0, 80.0);
    assert_eq!(
        preferred_ends_in(&corner, &below_left, rule, false),
        (End::At(Side::Bottom), End::H)
    );

    for target in [below_right, below_left, right] {
        let ends = preferred_ends_in(&corner, &target, rule, false);
        assert_ortho(&preferred(&corner, &target, ends));
    }
}

/// A boundary loop alternates to the top (right) side in a vertical pool,
/// the bottom (left) one in a horizontal pool.
#[test]
fn vertical_boundary_loop_alternates_sides() {
    let host = b(100.0, 100.0, 100.0, 80.0);
    let ev = b(182.0, 122.0, 36.0, 36.0);
    let rule = Rule::Boundary { host, loops: true };

    assert_eq!(
        preferred_ends_in(&ev, &host, rule, false),
        (End::At(Side::Right), End::At(Side::Top))
    );
    assert_eq!(
        preferred_ends_in(&ev, &host, rule, true),
        (End::At(Side::Right), End::At(Side::Bottom))
    );

    let corner = b(182.0, 162.0, 36.0, 36.0);
    assert_eq!(
        preferred_ends_in(&corner, &host, rule, false),
        (End::At(Side::Right), End::H)
    );
}

/// Vertical self-loops run counter-clockwise: `t:l`, `r:t`, `l:b`, `b:r`.
#[test]
fn vertical_self_loops_run_counter_clockwise() {
    let s = b(100.0, 100.0, 100.0, 80.0);
    let expected = [
        (Side::Top, pt(150.0, 100.0), pt(100.0, 140.0)),
        (Side::Right, pt(200.0, 140.0), pt(150.0, 100.0)),
        (Side::Left, pt(100.0, 140.0), pt(150.0, 180.0)),
        (Side::Bottom, pt(150.0, 180.0), pt(200.0, 140.0)),
    ];

    for (side, from, to) in expected {
        let route = loop_route_in(&s, side, false);

        assert_eq!(route.first(), Some(&from));
        assert_eq!(route.last(), Some(&to));
        assert_eq!(route.len(), 5);
        assert_ortho(&route);
    }
}

/// The direction comes from the pool DI of the element, its lane or the
/// pool whose process holds it.
#[test]
fn direction_follows_the_pool() {
    let mut defs = vertical_pool();
    put(
        &mut defs,
        Element::new("lane", ElementKind::Lane).with_parent("pool"),
        b(-20.0, -50.0, 300.0, 400.0),
        Some(false),
    );
    put(
        &mut defs,
        Element::new("free", ElementKind::Task).with_parent("other"),
        b(0.0, 700.0, 100.0, 80.0),
        None,
    );

    for id in ["pool", "lane", "a", "c"] {
        assert!(!is_direction_horizontal(&defs, &id.into()), "{id}");
    }
    assert!(is_direction_horizontal(&defs, &"free".into()));
    assert!(is_direction_horizontal(&defs, &"missing".into()));

    defs.plane_mut()
        .shapes
        .iter_mut()
        .find(|s| s.element_id == "pool".into())
        .expect("pool shape")
        .is_horizontal = Some(true);
    assert!(is_direction_horizontal(&defs, &"a".into()));
}

/// Connecting two tasks inside a vertical pool leaves the bottom and enters
/// the top (`v:v`).
#[test]
fn command_lays_out_vertical_pool_flows() {
    let mut defs = vertical_pool();
    connect(&defs, &"a".into(), &"c".into())
        .expect("sequence flow")
        .execute(&mut defs);

    let route = defs.edges().next().expect("edge").waypoints.clone();
    assert_eq!(route.first(), Some(&pt(50.0, 80.0)));
    assert_eq!(route.last(), Some(&pt(250.0, 150.0)));
    assert_ortho(&route);
}

/// `preferred_keeping` keeps a docking point that lies on its border and
/// ignores one that does not.
#[test]
fn keep_preserves_docking_on_the_border() {
    let (s, t) = (b(0.0, 0.0, 100.0, 80.0), b(300.0, 200.0, 100.0, 80.0));
    let ends = (End::H, End::H);

    let at_start = Keep {
        at_start: true,
        point: pt(100.0, 20.0),
    };
    let route = preferred_keeping(&s, &t, ends, Some(at_start));
    assert_eq!(route.first(), Some(&pt(100.0, 20.0)));
    assert_eq!(route.last(), Some(&pt(300.0, 240.0)));
    assert_ortho(&route);

    let at_end = Keep {
        at_start: false,
        point: pt(300.0, 260.0),
    };
    let route = preferred_keeping(&s, &t, ends, Some(at_end));
    assert_eq!(route.first(), Some(&pt(100.0, 40.0)));
    assert_eq!(route.last(), Some(&pt(300.0, 260.0)));
    assert_ortho(&route);

    let floating = Keep {
        at_start: true,
        point: pt(50.0, 40.0),
    };
    assert_eq!(
        preferred_keeping(&s, &t, ends, Some(floating)),
        preferred(&s, &t, ends)
    );
}

/// Flow `f` from `a` to `c` whose start docks off-center at (100, 20).
fn docked_flow(defs: &mut Definitions) {
    defs.add_element(
        Element::new(
            "f",
            ElementKind::SequenceFlow {
                source: "a".into(),
                target: "c".into(),
                is_default: false,
                condition: None,
            },
        )
        .with_parent("p"),
    );
    defs.add_edge(Edge {
        element_id: "f".into(),
        waypoints: vec![pt(100.0, 20.0), pt(200.0, 20.0), pt(200.0, 190.0)],
        label_bounds: None,
    });
}

/// Reconnecting the target keeps the source docking point.
#[test]
fn reconnect_keeps_the_other_docking() {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));
    for (id, bounds) in [
        ("a", b(0.0, 0.0, 100.0, 80.0)),
        ("c", b(200.0, 150.0, 100.0, 80.0)),
        ("d", b(300.0, 300.0, 100.0, 80.0)),
    ] {
        put(
            &mut defs,
            Element::new(id, ElementKind::Task).with_parent("p"),
            bounds,
            None,
        );
    }
    docked_flow(&mut defs);

    reconnect(&defs, &"f".into(), false, &"d".into())
        .expect("reconnect")
        .execute(&mut defs)
        .expect("changed");

    let route = defs.edges().next().expect("edge").waypoints.clone();
    assert_eq!(route.first(), Some(&pt(100.0, 20.0)), "{route:?}");
    assert_eq!(route.last(), Some(&pt(300.0, 340.0)), "{route:?}");
    assert_ortho(&route);
}

/// Moving the target of a bendless flow keeps the source docking point.
#[test]
fn move_keeps_the_other_docking() {
    let mut defs = vertical_pool();
    defs.remove_element(&"pool".into());
    defs.add_element(Element::new(
        "f",
        ElementKind::SequenceFlow {
            source: "a".into(),
            target: "c".into(),
            is_default: false,
            condition: None,
        },
    ));
    defs.add_edge(Edge {
        element_id: "f".into(),
        waypoints: vec![pt(100.0, 20.0), pt(200.0, 190.0)],
        label_bounds: None,
    });

    Command::MoveShapes {
        ids: vec!["c".into()],
        dx: 0.0,
        dy: 200.0,
    }
    .execute(&mut defs)
    .expect("moved");

    let route = defs.edges().next().expect("edge").waypoints.clone();
    assert_eq!(route.first(), Some(&pt(100.0, 20.0)), "{route:?}");
    assert_ortho(&route);
}
