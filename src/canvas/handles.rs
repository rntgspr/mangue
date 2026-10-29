//! Editing handles of the single selection, after diagram-js `Resize` and
//! `Bendpoints`: corner resize handles of a shape and the bendpoint /
//! segment grabs of a connection. GPUI-independent geometry.

use crate::model::{Bounds, Waypoint};

/// Side (screen px) of a resize handle square (diagram-js `HANDLE_SIZE`).
pub const HANDLE_SIZE: f64 = 8.0;

/// Screen distance (px) within which a press grabs a handle or segment.
pub const GRAB_TOLERANCE: f64 = 6.0;

/// Corner directions: `(-1, -1)` top-left ... `(1, 1)` bottom-right.
pub const CORNERS: [(i8, i8); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];

/// What a press on a selected connection grabs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EdgeGrab {
    /// First (`true`) or last waypoint: reconnects that end.
    Endpoint(bool),
    /// Inner bendpoint at this waypoint index.
    Bendpoint(usize),
    /// Middle of this segment (moves it perpendicular to itself).
    SegmentMid(usize),
    /// Elsewhere on this segment (inserts a bendpoint there).
    Segment(usize),
}

/// Diagram point of `corner` of `b`.
pub fn corner_point(b: &Bounds, corner: (i8, i8)) -> Waypoint {
    Waypoint {
        x: if corner.0 < 0 { b.x } else { b.x + b.w },
        y: if corner.1 < 0 { b.y } else { b.y + b.h },
    }
}

/// Corner of `b` whose handle is within `tolerance` (diagram units) of `p`.
pub fn corner_at(b: &Bounds, p: Waypoint, tolerance: f64) -> Option<(i8, i8)> {
    CORNERS.into_iter().find(|c| {
        let q = corner_point(b, *c);
        (q.x - p.x).abs() <= tolerance && (q.y - p.y).abs() <= tolerance
    })
}

/// `b` with `corner` dragged by `(dx, dy)`; the opposite corner stays put.
pub fn resized(b: &Bounds, corner: (i8, i8), dx: f64, dy: f64) -> Bounds {
    let mut out = *b;

    if corner.0 < 0 {
        out.x += dx;
        out.w -= dx;
    } else {
        out.w += dx;
    }

    if corner.1 < 0 {
        out.y += dy;
        out.h -= dy;
    } else {
        out.h += dy;
    }

    out
}

/// Part of the connection `waypoints` under `p` within `tolerance`
/// (diagram units): endpoints first (diagram-js `BendpointMove`
/// reconnect handles), then inner bendpoints, segment middles and any point
/// of a segment.
pub fn edge_grab(waypoints: &[Waypoint], p: Waypoint, tolerance: f64) -> Option<EdgeGrab> {
    let near = |q: Waypoint| (q.x - p.x).hypot(q.y - p.y) <= tolerance;

    if waypoints.first().is_some_and(|q| near(*q)) {
        return Some(EdgeGrab::Endpoint(true));
    }
    if waypoints.last().is_some_and(|q| near(*q)) {
        return Some(EdgeGrab::Endpoint(false));
    }
    let inner = 1..waypoints.len().saturating_sub(1);

    if let Some(i) = inner.into_iter().find(|i| near(waypoints[*i])) {
        return Some(EdgeGrab::Bendpoint(i));
    }

    let segments = || waypoints.windows(2).enumerate();
    let mid = |a: Waypoint, b: Waypoint| Waypoint {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    };

    if let Some((i, _)) = segments().find(|(_, s)| near(mid(s[0], s[1]))) {
        return Some(EdgeGrab::SegmentMid(i));
    }

    segments()
        .find(|(_, s)| distance_to_segment(p, s[0], s[1]) <= tolerance)
        .map(|(i, _)| EdgeGrab::Segment(i))
}

/// Distance from `p` to the segment `a`-`b`.
fn distance_to_segment(p: Waypoint, a: Waypoint, b: Waypoint) -> f64 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len = vx * vx + vy * vy;
    let t = if len == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * vx + (p.y - a.y) * vy) / len).clamp(0.0, 1.0)
    };

    (a.x + t * vx - p.x).hypot(a.y + t * vy - p.y)
}
