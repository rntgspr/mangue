//! Tests for layout geometry and the bpmn-js `BpmnLayouter` preferred-direction table.

use super::layout::{
    End, Rule, Side, dock, is_orthogonal, loop_route_from, loop_side, manhattan, preferred,
    preferred_ends, routed, straight,
};
use crate::model::{Bounds, Waypoint};

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

/// Task at the origin, used as the source of most rules.
fn src() -> Bounds {
    b(0.0, 0.0, 100.0, 80.0)
}

/// Sequence flows between ordinary nodes are `h:h`: right mid to left mid.
#[test]
fn default_is_horizontal_to_horizontal() {
    let t = b(200.0, 100.0, 100.0, 80.0);
    let ends = preferred_ends(&src(), &t, Rule::Default);
    let route = preferred(&src(), &t, ends);

    assert_eq!(ends, (End::H, End::H));
    assert_eq!(route.first(), Some(&pt(100.0, 40.0)));
    assert_eq!(route.last(), Some(&pt(200.0, 140.0)));
    assert_ortho(&route);
}

/// Gateway outgoing flows are `v:h`: leave the bottom, enter the left side.
#[test]
fn from_gateway_leaves_vertically_enters_horizontally() {
    let gw = b(0.0, 0.0, 50.0, 50.0);
    let t = b(200.0, 100.0, 100.0, 80.0);
    let route = preferred(&gw, &t, preferred_ends(&gw, &t, Rule::FromGateway));

    assert_eq!(
        route,
        vec![pt(25.0, 50.0), pt(25.0, 140.0), pt(200.0, 140.0)]
    );
}

/// Incoming flows of a gateway are `h:v`: leave the right side, enter the
/// top of the gateway.
#[test]
fn to_gateway_leaves_horizontally_enters_vertically() {
    let gw = b(200.0, 100.0, 50.0, 50.0);
    let route = preferred(&src(), &gw, preferred_ends(&src(), &gw, Rule::ToGateway));

    assert_eq!(
        route,
        vec![pt(100.0, 40.0), pt(225.0, 40.0), pt(225.0, 100.0)]
    );
}

/// Aligned gateway flows stay straight.
#[test]
fn gateway_flow_aligned_is_straight() {
    let gw = b(0.0, 0.0, 50.0, 50.0);
    let t = b(200.0, 0.0, 100.0, 50.0);
    let route = preferred(&gw, &t, preferred_ends(&gw, &t, Rule::FromGateway));

    assert_eq!(route, vec![pt(50.0, 25.0), pt(200.0, 25.0)]);
}

/// Message flows are `v:v`: bottom of the upper shape to top of the lower.
#[test]
fn message_flow_is_vertical_to_vertical() {
    let t = b(300.0, 400.0, 100.0, 80.0);
    let route = preferred(&src(), &t, preferred_ends(&src(), &t, Rule::Message));

    assert_eq!(
        route,
        vec![
            pt(50.0, 80.0),
            pt(50.0, 240.0),
            pt(350.0, 240.0),
            pt(350.0, 400.0)
        ]
    );

    let up = preferred(&t, &src(), preferred_ends(&t, &src(), Rule::Message));
    assert_eq!(up.first(), Some(&pt(350.0, 400.0)));
    assert_eq!(up.last(), Some(&pt(50.0, 80.0)));
}

/// A rule that cannot be joined with facing sides falls back to the
/// default manhattan route and stays orthogonal.
#[test]
fn unjoinable_facing_falls_back_to_manhattan() {
    let t = b(50.0, 200.0, 100.0, 80.0);
    let route = preferred(&src(), &t, preferred_ends(&src(), &t, Rule::Default));

    assert_ortho(&route);
    assert_eq!(route.first(), Some(&pt(50.0, 80.0)));
}

/// Loops leave at the side the loop currently starts on (`t:r`, `r:b`,
/// `l:t`, else `b:l`) and re-enter clockwise.
#[test]
fn loops_follow_current_side() {
    let s = b(100.0, 100.0, 100.0, 80.0);

    assert_eq!(loop_side(&s, pt(150.0, 100.0)), Side::Top);
    assert_eq!(loop_side(&s, pt(200.0, 140.0)), Side::Right);
    assert_eq!(loop_side(&s, pt(100.0, 140.0)), Side::Left);
    assert_eq!(loop_side(&s, pt(150.0, 180.0)), Side::Bottom);
    assert_eq!(loop_side(&s, pt(150.0, 140.0)), Side::Bottom);

    let expected = [
        (Side::Top, pt(150.0, 100.0), pt(200.0, 140.0)),
        (Side::Right, pt(200.0, 140.0), pt(150.0, 180.0)),
        (Side::Bottom, pt(150.0, 180.0), pt(100.0, 140.0)),
        (Side::Left, pt(100.0, 140.0), pt(150.0, 100.0)),
    ];
    for (side, from, to) in expected {
        let route = loop_route_from(&s, side);

        assert_eq!(route.first(), Some(&from));
        assert_eq!(route.last(), Some(&to));
        assert_eq!(route.len(), 5);
        assert_ortho(&route);
    }
}

/// Boundary event on the bottom side with the target below: `b:v`, so the
/// flow leaves downward and enters the target from above.
#[test]
fn boundary_bottom_leaves_down() {
    let host = b(100.0, 100.0, 100.0, 80.0);
    let ev = b(132.0, 162.0, 36.0, 36.0);
    let t = b(100.0, 300.0, 100.0, 80.0);
    let rule = Rule::Boundary { host, loops: false };

    assert_eq!(
        preferred_ends(&ev, &t, rule),
        (End::At(Side::Bottom), End::V)
    );

    let route = preferred(&ev, &t, preferred_ends(&ev, &t, rule));
    assert_eq!(route.first(), Some(&pt(150.0, 198.0)));
    assert_eq!(route.last(), Some(&pt(150.0, 300.0)));
    assert_ortho(&route);
}

/// Boundary event on the right side with the target to the right: `r:h`.
#[test]
fn boundary_right_leaves_right() {
    let host = b(100.0, 100.0, 100.0, 80.0);
    let ev = b(182.0, 122.0, 36.0, 36.0);
    let t = b(350.0, 100.0, 100.0, 80.0);
    let rule = Rule::Boundary { host, loops: false };

    assert_eq!(
        preferred_ends(&ev, &t, rule),
        (End::At(Side::Right), End::H)
    );

    let below = b(350.0, 300.0, 100.0, 80.0);
    assert_eq!(
        preferred_ends(&ev, &below, rule),
        (End::At(Side::Right), End::V)
    );

    let route = preferred(&ev, &t, preferred_ends(&ev, &t, rule));
    assert_eq!(route.first(), Some(&pt(218.0, 140.0)));
    assert_eq!(route.last(), Some(&pt(350.0, 140.0)));
    assert_ortho(&route);
}

/// Boundary event on the bottom with the target to its right and level
/// with the host: `b:v` flips to `b:h` (target not below).
#[test]
fn boundary_bottom_with_side_target_enters_horizontally() {
    let host = b(100.0, 100.0, 100.0, 80.0);
    let ev = b(132.0, 162.0, 36.0, 36.0);
    let t = b(350.0, 100.0, 100.0, 80.0);
    let rule = Rule::Boundary { host, loops: false };

    assert_eq!(
        preferred_ends(&ev, &t, rule),
        (End::At(Side::Bottom), End::H)
    );
    assert_ortho(&preferred(&ev, &t, preferred_ends(&ev, &t, rule)));
}

/// Boundary event on a corner: leaves vertically when the target shares
/// its vertical side, else horizontally.
#[test]
fn boundary_corner_picks_leaving_side() {
    let host = b(100.0, 100.0, 100.0, 80.0);
    let ev = b(182.0, 162.0, 36.0, 36.0);
    let rule = Rule::Boundary { host, loops: false };

    let below = b(300.0, 400.0, 100.0, 80.0);
    assert_eq!(preferred_ends(&ev, &below, rule).0, End::At(Side::Bottom));

    let above = b(300.0, -100.0, 100.0, 80.0);
    assert_eq!(preferred_ends(&ev, &above, rule).0, End::At(Side::Right));

    assert_ortho(&preferred(&ev, &below, preferred_ends(&ev, &below, rule)));
    assert_ortho(&preferred(&ev, &above, preferred_ends(&ev, &above, rule)));
}

/// Boundary event looping back to its host: explicit source side, route
/// detours around the host and stays orthogonal.
#[test]
fn boundary_loop_returns_to_host() {
    let host = b(100.0, 100.0, 100.0, 80.0);
    let ev = b(132.0, 162.0, 36.0, 36.0);
    let rule = Rule::Boundary { host, loops: true };
    let (s, t) = preferred_ends(&ev, &host, rule);

    assert_eq!(s, End::At(Side::Bottom));
    assert!(matches!(t, End::V | End::At(Side::Left)));

    let route = preferred(&ev, &host, (s, t));
    assert_eq!(route.first(), Some(&pt(150.0, 198.0)));
    assert_ortho(&route);
}

/// A preferred route through an obstacle is replaced by an alternative.
#[test]
fn routed_avoids_obstacles() {
    let t = b(300.0, 0.0, 100.0, 80.0);
    let blocker = b(150.0, 20.0, 100.0, 40.0);
    let route = routed(&src(), &t, Rule::Default, &[blocker]);

    assert_ortho(&route);
    assert!(route.windows(2).all(|s| s[0].x.max(s[1].x) <= 150.0
        || s[0].x.min(s[1].x) >= 250.0
        || s[0].y.max(s[1].y) <= 20.0
        || s[0].y.min(s[1].y) >= 60.0));
}

/// Manhattan routes: aligned, H-V-H, V-H-V and overlapping fallback.
#[test]
fn manhattan_routes() {
    let a = b(0.0, 0.0, 100.0, 80.0);

    assert_eq!(
        manhattan(&a, &b(200.0, 0.0, 100.0, 80.0)),
        [pt(100.0, 40.0), pt(200.0, 40.0)]
    );
    assert_eq!(
        manhattan(&a, &b(200.0, 100.0, 100.0, 80.0)),
        [
            pt(100.0, 40.0),
            pt(150.0, 40.0),
            pt(150.0, 140.0),
            pt(200.0, 140.0)
        ]
    );
    assert_eq!(
        manhattan(&a, &b(50.0, 200.0, 100.0, 80.0)),
        [
            pt(50.0, 80.0),
            pt(50.0, 140.0),
            pt(100.0, 140.0),
            pt(100.0, 200.0)
        ]
    );
    assert_eq!(manhattan(&a, &b(10.0, 10.0, 10.0, 10.0)).len(), 2);
}

/// Docking clips the center ray at the bounds.
#[test]
fn dock_and_straight() {
    let a = b(0.0, 0.0, 100.0, 80.0);

    assert_eq!(dock(&a, pt(50.0, 40.0), pt(250.0, 40.0)), pt(100.0, 40.0));
    assert_eq!(
        straight(&a, &b(200.0, 0.0, 100.0, 80.0)),
        [pt(100.0, 40.0), pt(200.0, 40.0)]
    );
}
