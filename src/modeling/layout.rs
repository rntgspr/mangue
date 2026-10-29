//! Connection routing: docking at shape bounds and a manhattan-lite layout
//! (horizontal-vertical-horizontal), after bpmn-js `BpmnLayouter`.

use super::rules::{is_direction_horizontal, is_flow_node};
use crate::model::{Bounds, Definitions, ElementId, ElementKind, Waypoint};

/// Returns the center of `b`.
pub fn center(b: &Bounds) -> Waypoint {
    Waypoint {
        x: b.x + b.w / 2.0,
        y: b.y + b.h / 2.0,
    }
}

/// Orthogonal route from `source` to `target` docked at their bounds.
///
/// Horizontally separated shapes connect right/left mids (two points when
/// aligned, else H-V-H through the gap middle); vertically separated shapes
/// connect bottom/top mids (V-H-V); overlapping shapes get a straight line.
pub fn manhattan(source: &Bounds, target: &Bounds) -> Vec<Waypoint> {
    let (s, t) = (center(source), center(target));
    let wp = |x, y| Waypoint { x, y };

    let (sx, tx) = if source.x + source.w <= target.x {
        (source.x + source.w, target.x)
    } else if target.x + target.w <= source.x {
        (source.x, target.x + target.w)
    } else {
        (f64::NAN, f64::NAN)
    };

    if !sx.is_nan() {
        if (s.y - t.y).abs() < 1.0 {
            return vec![wp(sx, s.y), wp(tx, s.y)];
        }

        let mx = (sx + tx) / 2.0;
        return vec![wp(sx, s.y), wp(mx, s.y), wp(mx, t.y), wp(tx, t.y)];
    }

    let (sy, ty) = if source.y + source.h <= target.y {
        (source.y + source.h, target.y)
    } else if target.y + target.h <= source.y {
        (source.y, target.y + target.h)
    } else {
        return straight(source, target);
    };

    if (s.x - t.x).abs() < 1.0 {
        return vec![wp(s.x, sy), wp(s.x, ty)];
    }

    let my = (sy + ty) / 2.0;
    vec![wp(s.x, sy), wp(s.x, my), wp(t.x, my), wp(t.x, ty)]
}

/// Straight route between the centers, clipped to both bounds.
pub fn straight(source: &Bounds, target: &Bounds) -> Vec<Waypoint> {
    let (s, t) = (center(source), center(target));

    vec![dock(source, s, t), dock(target, t, s)]
}

/// Point where the ray from `inside` (center of `b`) toward `toward` leaves `b`.
pub fn dock(b: &Bounds, inside: Waypoint, toward: Waypoint) -> Waypoint {
    let (dx, dy) = (toward.x - inside.x, toward.y - inside.y);
    if dx == 0.0 && dy == 0.0 {
        return inside;
    }

    let tx = if dx != 0.0 {
        (b.w / 2.0) / dx.abs()
    } else {
        f64::INFINITY
    };
    let ty = if dy != 0.0 {
        (b.h / 2.0) / dy.abs()
    } else {
        f64::INFINITY
    };
    let k = tx.min(ty).min(1.0);

    Waypoint {
        x: inside.x + dx * k,
        y: inside.y + dy * k,
    }
}

/// Minimum segment length of a loop leg (diagram-js `ManhattanLayout`).
const LOOP_GAP: f64 = 20.0;

/// Side of a shape's bounds a connection docks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    /// Outward unit normal of the side.
    fn normal(self) -> (f64, f64) {
        match self {
            Side::Top => (0.0, -1.0),
            Side::Right => (1.0, 0.0),
            Side::Bottom => (0.0, 1.0),
            Side::Left => (-1.0, 0.0),
        }
    }

    /// Whether the side is a left or right one (its normal is horizontal).
    fn is_horizontal(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }

    /// Opposite side.
    fn opposite(self) -> Side {
        match self {
            Side::Top => Side::Bottom,
            Side::Right => Side::Left,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
        }
    }

    /// Middle point of the side of `b`.
    fn mid(self, b: &Bounds) -> Waypoint {
        let c = center(b);

        match self {
            Side::Top => Waypoint { x: c.x, y: b.y },
            Side::Right => Waypoint {
                x: b.x + b.w,
                y: c.y,
            },
            Side::Bottom => Waypoint {
                x: c.x,
                y: b.y + b.h,
            },
            Side::Left => Waypoint { x: b.x, y: c.y },
        }
    }
}

/// One end of a preferred layout (`h`, `v` or `t`/`r`/`b`/`l` in bpmn-js
/// `BpmnLayouter`): a docking side facing the other end on an axis, or an
/// explicit side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// Left or right side, whichever faces the other shape.
    H,
    /// Top or bottom side, whichever faces the other shape.
    V,
    /// This exact side.
    At(Side),
}

/// Which preferred layout (bpmn-js `PREFERRED_LAYOUTS_HORIZONTAL`) a
/// connection follows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rule {
    /// `h:h`: sequence flows between ordinary nodes and expanded sub-processes.
    Default,
    /// `v:h`: sequence flows leaving a gateway.
    FromGateway,
    /// `h:v`: sequence flows entering a gateway.
    ToGateway,
    /// `v:v`: message flows.
    Message,
    /// Sequence flows leaving a boundary event attached to `host`;
    /// `loops` when the target is the host itself.
    Boundary { host: Bounds, loops: bool },
}

/// Self-loop route of a shape, clockwise from its right mid to its bottom
/// mid (bpmn-js `BpmnLayouter` loop layout `r:b`).
pub fn loop_route(b: &Bounds) -> Vec<Waypoint> {
    loop_route_from(b, Side::Right)
}

/// Self-loop route leaving `b` at `from` and re-entering clockwise at the
/// next side: `t:r`, `r:b`, `b:l` or `l:t` (bpmn-js `PREFERRED_LAYOUTS`
/// `loop`).
pub fn loop_route_from(b: &Bounds, from: Side) -> Vec<Waypoint> {
    let c = center(b);
    let (l, t, r, bt) = (b.x, b.y, b.x + b.w, b.y + b.h);
    let g = LOOP_GAP;
    let wp = |x, y| Waypoint { x, y };

    match from {
        Side::Top => vec![
            wp(c.x, t),
            wp(c.x, t - g),
            wp(r + g, t - g),
            wp(r + g, c.y),
            wp(r, c.y),
        ],
        Side::Right => vec![
            wp(r, c.y),
            wp(r + g, c.y),
            wp(r + g, bt + g),
            wp(c.x, bt + g),
            wp(c.x, bt),
        ],
        Side::Bottom => vec![
            wp(c.x, bt),
            wp(c.x, bt + g),
            wp(l - g, bt + g),
            wp(l - g, c.y),
            wp(l, c.y),
        ],
        Side::Left => vec![
            wp(l, c.y),
            wp(l - g, c.y),
            wp(l - g, t - g),
            wp(c.x, t - g),
            wp(c.x, t),
        ],
    }
}

/// Self-loop route leaving `b` at `from` for a pool of the given
/// direction: clockwise in horizontal ones (`loop_route_from`), counter-
/// clockwise in vertical ones (`t:l`, `r:t`, `l:b`, `b:r`).
pub fn loop_route_in(b: &Bounds, from: Side, horizontal: bool) -> Vec<Waypoint> {
    if horizontal {
        return loop_route_from(b, from);
    }

    let into = match from {
        Side::Top => Side::Left,
        Side::Right => Side::Top,
        Side::Left => Side::Bottom,
        Side::Bottom => Side::Right,
    };

    detour(from.mid(b), from, into.mid(b), into)
}

/// Loop side chosen from where the loop currently leaves the shape
/// (bpmn-js `getLoopPreferredLayout`): top, right and left keep their side,
/// anything else (bottom, a corner, inside) leaves at the bottom.
pub fn loop_side(b: &Bounds, first: Waypoint) -> Side {
    match orientation(first, b, 0.0, 0.0) {
        (Some(Vertical::Top), None) => Side::Top,
        (None, Some(Horizontal::Right)) => Side::Right,
        (None, Some(Horizontal::Left)) => Side::Left,
        _ => Side::Bottom,
    }
}

/// Vertical part of an orientation (diagram-js `getOrientation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vertical {
    Top,
    Bottom,
}

/// Horizontal part of an orientation (diagram-js `getOrientation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Horizontal {
    Left,
    Right,
}

/// Where the point `p` lies relative to `reference`, with a padding
/// (diagram-js `LayoutUtil.getOrientation` for a point; `(None, None)` is
/// `intersect`).
fn orientation(
    p: Waypoint,
    reference: &Bounds,
    pad_x: f64,
    pad_y: f64,
) -> (Option<Vertical>, Option<Horizontal>) {
    let vertical = if p.y + pad_y <= reference.y {
        Some(Vertical::Top)
    } else if p.y - pad_y >= reference.y + reference.h {
        Some(Vertical::Bottom)
    } else {
        None
    };
    let horizontal = if p.x + pad_x <= reference.x {
        Some(Horizontal::Left)
    } else if p.x - pad_x >= reference.x + reference.w {
        Some(Horizontal::Right)
    } else {
        None
    };

    (vertical, horizontal)
}

/// Side named by the vertical part of an orientation.
fn vertical_side(v: Vertical) -> Side {
    match v {
        Vertical::Top => Side::Top,
        Vertical::Bottom => Side::Bottom,
    }
}

/// Side named by the horizontal part of an orientation.
fn horizontal_side(h: Horizontal) -> Side {
    match h {
        Horizontal::Left => Side::Left,
        Horizontal::Right => Side::Right,
    }
}

/// Preferred `(source, target)` ends of `rule` for a connection between
/// `source` and `target` in a horizontal pool or process (bpmn-js
/// `BpmnLayouter.layoutConnection`, `PREFERRED_LAYOUTS_HORIZONTAL`).
pub fn preferred_ends(source: &Bounds, target: &Bounds, rule: Rule) -> (End, End) {
    preferred_ends_in(source, target, rule, true)
}

/// Preferred `(source, target)` ends of `rule`; `horizontal` false picks the
/// table of vertical pools and lanes (`PREFERRED_LAYOUTS_VERTICAL`): `h` and
/// `v` swap, so sequence flows are `v:v`, out of a gateway `h:v`, into one
/// `v:h`, message flows `h:h`.
pub fn preferred_ends_in(
    source: &Bounds,
    target: &Bounds,
    rule: Rule,
    horizontal: bool,
) -> (End, End) {
    let (along, across) = if horizontal {
        (End::H, End::V)
    } else {
        (End::V, End::H)
    };

    match rule {
        Rule::Default => (along, along),
        Rule::FromGateway => (across, along),
        Rule::ToGateway => (along, across),
        Rule::Message => (across, across),
        Rule::Boundary { host, loops: true } => {
            boundary_loop_ends(source, target, &host, horizontal)
        }
        Rule::Boundary { host, .. } => boundary_ends(source, target, &host, horizontal),
    }
}

/// Ends of a boundary event flow to another node (bpmn-js
/// `getBoundaryEventSourceLayout` / `getBoundaryEventTargetLayout`); in a
/// vertical pool the corner cases swap their axes.
fn boundary_ends(source: &Bounds, target: &Bounds, host: &Bounds, horizontal: bool) -> (End, End) {
    let (sm, tm) = (center(source), center(target));
    let attach = orientation(sm, host, -10.0, -10.0);
    let toward = orientation(
        tm,
        &Bounds {
            x: sm.x,
            y: sm.y,
            w: 0.0,
            h: 0.0,
        },
        source.w / 2.0 + target.w / 2.0,
        source.h / 2.0 + target.h / 2.0,
    );

    match attach {
        (Some(v), None) => {
            let same = toward == (Some(v), None);
            let opposite = toward.0 == Some(opposite_vertical(v));

            (
                End::At(vertical_side(v)),
                if same || opposite { End::V } else { End::H },
            )
        }
        (None, Some(h)) => {
            let same = toward == (None, Some(h));
            let opposite = toward.1 == Some(opposite_horizontal(h));

            (
                End::At(horizontal_side(h)),
                if same || opposite { End::H } else { End::V },
            )
        }
        (Some(v), Some(h)) if horizontal => {
            let leaves_vertically = toward.0 == Some(v) || toward.1 == Some(opposite_horizontal(h));
            let side = if leaves_vertically {
                vertical_side(v)
            } else {
                horizontal_side(h)
            };

            (
                End::At(side),
                if toward.0 == Some(v) { End::H } else { End::V },
            )
        }
        (Some(v), Some(h)) => {
            let leaves_horizontally = toward.1 == Some(h) || toward.0 == Some(opposite_vertical(v));
            let side = if leaves_horizontally {
                horizontal_side(h)
            } else {
                vertical_side(v)
            };

            (
                End::At(side),
                if toward.1 == Some(h) { End::V } else { End::H },
            )
        }
        (None, None) if horizontal => (End::H, End::H),
        (None, None) => (End::V, End::V),
    }
}

/// Ends of a boundary event flow back to its own host (bpmn-js
/// `getBoundaryEventLoopLayout`); a vertical pool alternates to the top or
/// right side and leaves corners horizontally.
fn boundary_loop_ends(
    source: &Bounds,
    target: &Bounds,
    host: &Bounds,
    horizontal: bool,
) -> (End, End) {
    let sm = center(source);
    let end = center(target);
    let attach = orientation(sm, host, -10.0, -10.0);
    let (alt_horizontal, alt_vertical) = if horizontal {
        (Side::Bottom, Side::Left)
    } else {
        (Side::Top, Side::Right)
    };

    match attach {
        (Some(v), None) => {
            let same = same_side('x', source, target, end);

            (
                End::At(vertical_side(v)),
                if same { End::V } else { End::At(alt_vertical) },
            )
        }
        (None, Some(h)) => {
            let same = same_side('y', source, target, end);

            (
                End::At(horizontal_side(h)),
                if same {
                    End::H
                } else {
                    End::At(alt_horizontal)
                },
            )
        }
        (Some(v), Some(_)) if horizontal => (End::At(vertical_side(v)), End::V),
        (Some(_), Some(h)) => (End::At(horizontal_side(h)), End::H),
        (None, None) if horizontal => (End::H, End::H),
        (None, None) => (End::V, End::V),
    }
}

/// Opposite of a vertical orientation.
fn opposite_vertical(v: Vertical) -> Vertical {
    match v {
        Vertical::Top => Vertical::Bottom,
        Vertical::Bottom => Vertical::Top,
    }
}

/// Opposite of a horizontal orientation.
fn opposite_horizontal(h: Horizontal) -> Horizontal {
    match h {
        Horizontal::Left => Horizontal::Right,
        Horizontal::Right => Horizontal::Left,
    }
}

/// Whether a loop back to the host may enter its near side
/// (bpmn-js `shouldConnectToSameSide`, threshold 40): false when the
/// docking `end` is within 40 of the target's start or end on `axis`, or of
/// the source's middle.
fn same_side(axis: char, source: &Bounds, target: &Bounds, end: Waypoint) -> bool {
    let pick = |p: Waypoint| if axis == 'x' { p.x } else { p.y };
    let near = |p: Waypoint| (pick(end) - pick(p)).abs() < 40.0;
    let start = Waypoint {
        x: target.x,
        y: target.y,
    };
    let finish = Waypoint {
        x: target.x + target.w,
        y: target.y + target.h,
    };

    !(near(start) || near(finish) || near(center(source)))
}

/// A docking point to keep when one end of a connection changes
/// (diagram-js `preserveDocking` hint).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keep {
    /// Whether the kept end is the connection's start (else its end).
    pub at_start: bool,
    /// The docking point to keep.
    pub point: Waypoint,
}

/// Orthogonal route between `source` and `target` docked as `ends` say
/// (the `a:b` layouts of bpmn-js `ManhattanLayout`). Facing ends that are
/// aligned or cannot be joined with one or two bends fall back to the
/// default `manhattan` route; explicit sides always get a route, detouring
/// around the shapes when needed.
pub fn preferred(source: &Bounds, target: &Bounds, ends: (End, End)) -> Vec<Waypoint> {
    preferred_keeping(source, target, ends, None)
}

/// Side of `b` whose border holds `p`, or `None` when `p` is not on the
/// border (within `ALIGN_EPS`).
fn border_side(b: &Bounds, p: Waypoint) -> Option<Side> {
    let inside = p.x >= b.x - ALIGN_EPS
        && p.x <= b.x + b.w + ALIGN_EPS
        && p.y >= b.y - ALIGN_EPS
        && p.y <= b.y + b.h + ALIGN_EPS;
    let gaps = [
        (Side::Top, (p.y - b.y).abs()),
        (Side::Right, (p.x - b.x - b.w).abs()),
        (Side::Bottom, (p.y - b.y - b.h).abs()),
        (Side::Left, (p.x - b.x).abs()),
    ];

    gaps.into_iter()
        .filter(|_| inside)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, gap)| *gap < ALIGN_EPS)
        .map(|(side, _)| side)
}

/// `preferred` that keeps the docking point of `keep` when it still lies on
/// its shape's border and the layout can join it with one or two bends
/// (diagram-js `preserveDocking`); otherwise the plain layout.
pub fn preferred_keeping(
    source: &Bounds,
    target: &Bounds,
    ends: (End, End),
    keep: Option<Keep>,
) -> Vec<Waypoint> {
    let (s, t) = (center(source), center(target));
    let keep = keep.and_then(|k| {
        let shape = if k.at_start { source } else { target };

        border_side(shape, k.point).map(|side| (k, side))
    });
    let explicit = matches!(ends.0, End::At(_)) || matches!(ends.1, End::At(_));
    let aligned = (s.x - t.x).abs() < 1.0 || (s.y - t.y).abs() < 1.0;
    if aligned && !explicit && keep.is_none() {
        return manhattan(source, target);
    }

    let toward = |b: &Bounds, other: Waypoint, end: End| match end {
        End::At(side) => side,
        End::H if other.x >= center(b).x => Side::Right,
        End::H => Side::Left,
        End::V if other.y >= center(b).y => Side::Bottom,
        End::V => Side::Top,
    };
    let (mut sa, mut a) = (toward(source, t, ends.0), None);
    let (mut sb, mut b) = (toward(target, s, ends.1), None);
    if let Some((k, side)) = keep {
        if k.at_start {
            (sa, a) = (side, Some(k.point));
            sb = toward(target, k.point, ends.1);
        } else {
            (sb, b) = (side, Some(k.point));
            sa = toward(source, k.point, ends.0);
        }
    }
    let a = a.unwrap_or_else(|| sa.mid(source));
    let b = b.unwrap_or_else(|| sb.mid(target));

    if let Some(route) = join(a, sa, b, sb) {
        return simplify(&route);
    }
    if !explicit {
        return manhattan(source, target);
    }

    simplify(&detour(a, sa, b, sb))
}

/// One- or two-bend route from `a` (leaving along side `sa`) to `b`
/// (entered along side `sb`), when the sides allow it: an L for
/// perpendicular sides, a Z through the middle for facing parallel ones.
fn join(a: Waypoint, sa: Side, b: Waypoint, sb: Side) -> Option<Vec<Waypoint>> {
    let wp = |x, y| Waypoint { x, y };
    let (na, nb) = (sa.normal(), sb.normal());

    if sa.is_horizontal() != sb.is_horizontal() {
        let corner = if sa.is_horizontal() {
            wp(b.x, a.y)
        } else {
            wp(a.x, b.y)
        };
        let out = (corner.x - a.x) * na.0 + (corner.y - a.y) * na.1 > 0.0;
        let into = (corner.x - b.x) * nb.0 + (corner.y - b.y) * nb.1 > 0.0;

        return (out && into).then(|| vec![a, corner, b]);
    }

    if sa != sb.opposite() {
        return None;
    }

    if sa.is_horizontal() {
        let facing = (b.x - a.x) * na.0 > 0.0;
        let mx = (a.x + b.x) / 2.0;

        return facing.then(|| vec![a, wp(mx, a.y), wp(mx, b.y), b]);
    }

    let facing = (b.y - a.y) * na.1 > 0.0;
    let my = (a.y + b.y) / 2.0;

    facing.then(|| vec![a, wp(a.x, my), wp(b.x, my), b])
}

/// Route that leaves `a` and enters `b` by `LOOP_GAP` along their side
/// normals and joins the two stubs with a single elbow.
fn detour(a: Waypoint, sa: Side, b: Waypoint, sb: Side) -> Vec<Waypoint> {
    let wp = |x, y| Waypoint { x, y };
    let (na, nb) = (sa.normal(), sb.normal());
    let a1 = wp(a.x + na.0 * LOOP_GAP, a.y + na.1 * LOOP_GAP);
    let b1 = wp(b.x + nb.0 * LOOP_GAP, b.y + nb.1 * LOOP_GAP);
    let elbow = if sa.is_horizontal() {
        wp(a1.x, b1.y)
    } else {
        wp(b1.x, a1.y)
    };

    vec![a, a1, elbow, b1, b]
}

/// Route for `rule` in a horizontal pool: the preferred layout when it
/// crosses none of `obstacles`, else the obstacle-avoiding manhattan route.
pub fn routed(source: &Bounds, target: &Bounds, rule: Rule, obstacles: &[Bounds]) -> Vec<Waypoint> {
    routed_in(source, target, rule, true, obstacles, None)
}

/// `routed` for a pool of the given direction (`horizontal` false = vertical
/// table), keeping the docking point `keep` says when the layout allows it
/// (the obstacle-avoiding fallback re-docks).
pub fn routed_in(
    source: &Bounds,
    target: &Bounds,
    rule: Rule,
    horizontal: bool,
    obstacles: &[Bounds],
    keep: Option<Keep>,
) -> Vec<Waypoint> {
    let ends = preferred_ends_in(source, target, rule, horizontal);
    let route = preferred_keeping(source, target, ends, keep);
    let clear = route.windows(2).all(|seg| {
        obstacles
            .iter()
            .all(|o| !segment_crosses(seg[0], seg[1], o))
    });

    if clear {
        route
    } else {
        manhattan_avoiding(source, target, obstacles)
    }
}

/// Manhattan route avoiding `obstacles` when possible: the default
/// `manhattan` route when it crosses none of them, else the first of the
/// alternatives (V-H-V, then the two L-shapes, docked at bounds mids) that
/// crosses none, else the default route.
pub fn manhattan_avoiding(source: &Bounds, target: &Bounds, obstacles: &[Bounds]) -> Vec<Waypoint> {
    let default = manhattan(source, target);
    let clear = |route: &[Waypoint]| {
        route.windows(2).all(|seg| {
            obstacles
                .iter()
                .all(|o| !segment_crosses(seg[0], seg[1], o))
        })
    };
    if clear(&default) {
        return default;
    }

    let (s, t) = (center(source), center(target));
    let wp = |x, y| Waypoint { x, y };
    let side_x = |b: &Bounds, toward: f64| {
        if toward >= center(b).x {
            b.x + b.w
        } else {
            b.x
        }
    };
    let side_y = |b: &Bounds, toward: f64| {
        if toward >= center(b).y {
            b.y + b.h
        } else {
            b.y
        }
    };

    let (sy, ty) = (side_y(source, t.y), side_y(target, s.y));
    let my = (sy + ty) / 2.0;
    let vhv = vec![wp(s.x, sy), wp(s.x, my), wp(t.x, my), wp(t.x, ty)];
    let hv = vec![
        wp(side_x(source, t.x), s.y),
        wp(t.x, s.y),
        wp(t.x, side_y(target, s.y)),
    ];
    let vh = vec![
        wp(s.x, side_y(source, t.y)),
        wp(s.x, t.y),
        wp(side_x(target, s.x), t.y),
    ];

    [vhv, hv, vh]
        .into_iter()
        .find(|route| clear(route))
        .unwrap_or(default)
}

/// Whether the axis-aligned segment `a`-`b` passes through the interior of `o`.
fn segment_crosses(a: Waypoint, b: Waypoint, o: &Bounds) -> bool {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));

    x1 > o.x && x0 < o.x + o.w && y1 > o.y && y0 < o.y + o.h
}

/// Keeps user bendpoints when one end of a connection moved by `(dx, dy)`
/// (diagram-js `ManhattanLayout.repairConnection`, simplified): the end
/// waypoint is translated and its neighbor follows on the axis that keeps
/// the first segment orthogonal. `at_start` tells which end moved.
/// Returns `None` when the path has no inner bendpoints or the neighbor
/// would land inside `moved`, so the caller re-routes instead.
pub fn repair_end(
    waypoints: &[Waypoint],
    at_start: bool,
    dx: f64,
    dy: f64,
    moved: &Bounds,
) -> Option<Vec<Waypoint>> {
    if waypoints.len() < 3 {
        return None;
    }

    let mut out = waypoints.to_vec();
    if !at_start {
        out.reverse();
    }

    let horizontal = (out[0].y - out[1].y).abs() < 1.0;
    out[0].x += dx;
    out[0].y += dy;
    if horizontal {
        out[1].y = out[0].y;
    } else {
        out[1].x = out[0].x;
    }

    let n = out[1];
    let inside =
        n.x > moved.x && n.x < moved.x + moved.w && n.y > moved.y && n.y < moved.y + moved.h;
    if inside {
        return None;
    }

    if !at_start {
        out.reverse();
    }
    Some(out)
}

/// Tolerance below which two coordinates count as aligned.
const ALIGN_EPS: f64 = 1.0;

/// Whether segment `a`-`b` is horizontal or vertical (within `ALIGN_EPS`).
pub fn is_orthogonal(a: Waypoint, b: Waypoint) -> bool {
    (a.x - b.x).abs() < ALIGN_EPS || (a.y - b.y).abs() < ALIGN_EPS
}

/// Whether point `p` docks on a left/right side of `b` (closer to a
/// vertical border than to a horizontal one), so its segment leaves
/// horizontally.
fn docks_horizontal(b: &Bounds, p: Waypoint) -> bool {
    let dx = (p.x - b.x).abs().min((p.x - b.x - b.w).abs());
    let dy = (p.y - b.y).abs().min((p.y - b.y - b.h).abs());

    dx <= dy
}

/// Turns every diagonal segment of `waypoints` into an L elbow (or a Z
/// through the middle when both docked ends leave on the same axis), then
/// drops duplicate and collinear inner points. Endpoints never move. The
/// first leg of an elbow follows the docking side on `source` for the first
/// segment, else continues the previous segment's axis; the last segment
/// enters `target` perpendicular to its docking side.
pub fn orthogonalize(
    waypoints: &[Waypoint],
    source: Option<&Bounds>,
    target: Option<&Bounds>,
) -> Vec<Waypoint> {
    let mut pts: Vec<Waypoint> = Vec::with_capacity(waypoints.len());
    for &p in waypoints {
        if pts.last().is_none_or(|q| q.x != p.x || q.y != p.y) {
            pts.push(p);
        }
    }
    if pts.len() < 2 {
        return pts;
    }

    let n = pts.len();
    let wp = |x, y| Waypoint { x, y };
    let mut out = vec![pts[0]];
    for i in 0..n - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        if !is_orthogonal(a, b) {
            let previous_h = if out.len() >= 2 {
                (out[out.len() - 2].y - a.y).abs() < ALIGN_EPS
            } else {
                true
            };
            let lead_h = match source {
                Some(s) if i == 0 => docks_horizontal(s, a),
                _ => previous_h,
            };
            let end_h = target
                .filter(|_| i + 2 == n)
                .map(|t| docks_horizontal(t, b));

            match end_h {
                Some(true) if lead_h => {
                    let mx = (a.x + b.x) / 2.0;
                    out.extend([wp(mx, a.y), wp(mx, b.y)]);
                }
                Some(false) if !lead_h => {
                    let my = (a.y + b.y) / 2.0;
                    out.extend([wp(a.x, my), wp(b.x, my)]);
                }
                _ if lead_h => out.push(wp(b.x, a.y)),
                _ => out.push(wp(a.x, b.y)),
            }
        }

        out.push(b);
    }

    simplify(&out)
}

/// Drops consecutive duplicates and inner points collinear with both
/// neighbors (same x or same y) whose removal keeps the path orthogonal,
/// keeping both endpoints.
fn simplify(points: &[Waypoint]) -> Vec<Waypoint> {
    let same = |a: Waypoint, b: Waypoint| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6;
    let mut out: Vec<Waypoint> = Vec::with_capacity(points.len());
    for (i, &p) in points.iter().enumerate() {
        let last = i + 1 == points.len();
        if out.last().is_some_and(|&q| same(q, p)) {
            if last && out.len() > 1 {
                out.pop();
            } else {
                continue;
            }
        }

        out.push(p);
    }

    let mut i = 1;
    while i + 1 < out.len() {
        let (a, b, c) = (out[i - 1], out[i], out[i + 1]);
        let vertical = (a.x - b.x).abs() < ALIGN_EPS && (b.x - c.x).abs() < ALIGN_EPS;
        let horizontal = (a.y - b.y).abs() < ALIGN_EPS && (b.y - c.y).abs() < ALIGN_EPS;
        if (vertical || horizontal) && is_orthogonal(a, c) {
            out.remove(i);
        } else {
            i += 1;
        }
    }

    out
}

/// Orthogonalizes every edge of `defs`, on every plane, against its
/// endpoint shapes; used after import so that diagonal edges of foreign
/// files are rewritten (and saved) as horizontal/vertical segments.
pub fn orthogonalize_all(defs: &mut Definitions) {
    let active = defs.active;

    for index in 0..defs.diagrams.len() {
        defs.active = index;
        let bounds: Vec<(Option<Bounds>, Option<Bounds>)> = defs
            .edges()
            .map(|edge| {
                let ends = defs.get(&edge.element_id).and_then(|e| e.kind.endpoints());
                let shape = |id| defs.shape(id).map(|s| s.bounds);

                (
                    ends.and_then(|(s, _)| shape(s)),
                    ends.and_then(|(_, t)| shape(t)),
                )
            })
            .collect();

        for (edge, (s, t)) in defs.plane_mut().edges.iter_mut().zip(bounds) {
            edge.waypoints = orthogonalize(&edge.waypoints, s.as_ref(), t.as_ref());
        }
    }

    defs.active = active;
}

/// Routes connection `cid` manhattan with obstacle avoidance, in the table
/// of its pool direction; associations too (a deliberate deviation from
/// bpmn-js, which routes them straight). `keep_start` names the end whose
/// current docking point is preserved (see `reroute_keeping`).
pub(super) fn route(
    defs: &Definitions,
    cid: &ElementId,
    keep_start: Option<bool>,
) -> Option<Vec<Waypoint>> {
    let kind = &defs.get(cid)?.kind;
    let (s, t) = kind.endpoints()?;
    let (sb, tb) = (defs.shape(s)?.bounds, defs.shape(t)?.bounds);
    let horizontal = is_direction_horizontal(defs, s);
    let waypoints = defs
        .edges()
        .find(|e| &e.element_id == cid)
        .map(|e| e.waypoints.as_slice())
        .unwrap_or_default();

    if s == t {
        let side = waypoints
            .first()
            .map_or(Side::Right, |p| loop_side(&sb, *p));

        return Some(loop_route_in(&sb, side, horizontal));
    }

    let keep = keep_start.and_then(|at_start| {
        let point = if at_start {
            waypoints.first()
        } else {
            waypoints.last()
        };

        point.map(|point| Keep {
            at_start,
            point: *point,
        })
    });

    Some(routed_in(
        &sb,
        &tb,
        layout_rule(defs, kind, s, t),
        horizontal,
        &obstacles(defs, s, t),
        keep,
    ))
}

/// Preferred layout of a connection of `kind` from `s` to `t` (bpmn-js
/// `BpmnLayouter.layoutConnection`): message flows `v:v`; sequence flows
/// follow the boundary event, expanded sub-process, gateway source and
/// gateway target rules in that order; a compensation association (boundary
/// event to an activity `isForCompensation`) follows the boundary event
/// rule too; everything else, associations included, is `h:h`.
fn layout_rule(defs: &Definitions, kind: &ElementKind, s: &ElementId, t: &ElementId) -> Rule {
    let kind_of = |id: &ElementId| defs.get(id).map(|e| &e.kind);
    let expanded = |id: &ElementId| {
        matches!(kind_of(id), Some(ElementKind::SubProcess { .. }))
            && defs
                .shape(id)
                .is_some_and(|sh| sh.is_expanded == Some(true))
    };
    let gateway = |id: &ElementId| {
        matches!(
            kind_of(id),
            Some(
                ElementKind::ExclusiveGateway
                    | ElementKind::ParallelGateway
                    | ElementKind::InclusiveGateway
                    | ElementKind::EventBasedGateway
                    | ElementKind::ComplexGateway
            )
        )
    };

    let compensation = matches!(kind, ElementKind::Association { .. })
        && matches!(kind_of(s), Some(ElementKind::BoundaryEvent { .. }))
        && defs
            .get(t)
            .is_some_and(|e| e.details.is_for_compensation == Some(true));

    match kind {
        ElementKind::MessageFlow { .. } => Rule::Message,
        ElementKind::SequenceFlow { .. } | ElementKind::Association { .. }
            if compensation || matches!(kind, ElementKind::SequenceFlow { .. }) =>
        {
            if let Some(ElementKind::BoundaryEvent { attached_to, .. }) = kind_of(s) {
                if let Some(host) = defs.shape(attached_to) {
                    return Rule::Boundary {
                        host: host.bounds,
                        loops: attached_to == t,
                    };
                }
            }

            if expanded(s) || expanded(t) {
                Rule::Default
            } else if gateway(s) {
                Rule::FromGateway
            } else if gateway(t) {
                Rule::ToGateway
            } else {
                Rule::Default
            }
        }
        _ => Rule::Default,
    }
}

/// Bounds of the shapes a flow from `s` to `t` should route around: flow
/// nodes and data, minus both ends, their boundary events and containers
/// (expanded sub-processes).
fn obstacles(defs: &Definitions, s: &ElementId, t: &ElementId) -> Vec<Bounds> {
    defs.shapes()
        .filter(|shape| &shape.element_id != s && &shape.element_id != t)
        .filter(|shape| shape.is_expanded != Some(true))
        .filter(|shape| {
            defs.get(&shape.element_id).is_some_and(|e| match &e.kind {
                ElementKind::BoundaryEvent { attached_to, .. } => {
                    attached_to != s && attached_to != t
                }
                k => {
                    is_flow_node(k)
                        || matches!(
                            k,
                            ElementKind::DataObjectReference | ElementKind::DataStoreReference
                        )
                }
            })
        })
        .map(|shape| shape.bounds)
        .collect()
}
