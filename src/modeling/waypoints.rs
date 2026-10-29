//! Pure waypoint edits behind bendpoint and segment dragging, after
//! diagram-js `BendpointMove` and `ConnectionSegmentMove`. Each returns the
//! new waypoint list for `Command::UpdateWaypoints`, or `None` when the edit
//! does not apply.

use super::layout::orthogonalize;
use crate::model::Waypoint;

/// Moves the inner bendpoint `index` to `to`; endpoints are not bendpoints.
/// Inner neighbors follow on the axis of their shared segment so adjacent
/// orthogonal segments stay orthogonal (diagram-js manhattan bendpoint
/// move); a docked endpoint neighbor stays put and the command's
/// orthogonalization adds the elbow.
pub fn move_bendpoint(waypoints: &[Waypoint], index: usize, to: Waypoint) -> Option<Vec<Waypoint>> {
    if index == 0 || index + 1 >= waypoints.len() {
        return None;
    }

    let mut out = waypoints.to_vec();
    let from = out[index];
    let last = out.len() - 1;
    for n in [index - 1, index + 1] {
        if n == 0 || n == last {
            continue;
        }

        if (out[n].y - from.y).abs() < 1.0 {
            out[n].y = to.y;
        } else if (out[n].x - from.x).abs() < 1.0 {
            out[n].x = to.x;
        }
    }

    out[index] = to;
    Some(out)
}

/// Inserts a bendpoint at `at` on segment `segment` (between waypoints
/// `segment` and `segment + 1`).
pub fn insert_bendpoint(
    waypoints: &[Waypoint],
    segment: usize,
    at: Waypoint,
) -> Option<Vec<Waypoint>> {
    if segment + 1 >= waypoints.len() {
        return None;
    }

    let mut out = waypoints.to_vec();
    out.insert(segment + 1, at);
    Some(out)
}

/// Removes the inner bendpoint `index`. When the corner joined two
/// orthogonal segments, the opposite corner of the same box replaces it so
/// the path stays orthogonal and collapses with its collinear neighbors.
pub fn remove_bendpoint(waypoints: &[Waypoint], index: usize) -> Option<Vec<Waypoint>> {
    if index == 0 || index + 1 >= waypoints.len() {
        return None;
    }

    let (prev, corner, next) = (waypoints[index - 1], waypoints[index], waypoints[index + 1]);
    let mut out = waypoints.to_vec();
    out.remove(index);

    let opposite = if (corner.x - next.x).abs() < 1.0 && (corner.y - prev.y).abs() < 1.0 {
        Some(Waypoint {
            x: prev.x,
            y: next.y,
        })
    } else if (corner.x - prev.x).abs() < 1.0 && (corner.y - next.y).abs() < 1.0 {
        Some(Waypoint {
            x: next.x,
            y: prev.y,
        })
    } else {
        None
    };
    if let Some(p) = opposite {
        out.insert(index, p);
    }

    Some(orthogonalize(&out, None, None))
}

/// Moves segment `segment` perpendicular to itself by `delta` (horizontal
/// segments move in y, vertical ones in x). When the segment touches a
/// connection end, a bendpoint is inserted there so the end stays docked.
/// Diagonal segments cannot be moved.
pub fn move_segment(waypoints: &[Waypoint], segment: usize, delta: f64) -> Option<Vec<Waypoint>> {
    if segment + 1 >= waypoints.len() || delta == 0.0 {
        return None;
    }

    let (a, b) = (waypoints[segment], waypoints[segment + 1]);
    let horizontal = (a.y - b.y).abs() < 1.0;
    let vertical = (a.x - b.x).abs() < 1.0;
    if !horizontal && !vertical {
        return None;
    }

    let shift = |p: Waypoint| {
        if horizontal {
            Waypoint {
                x: p.x,
                y: p.y + delta,
            }
        } else {
            Waypoint {
                x: p.x + delta,
                y: p.y,
            }
        }
    };

    let mut out = waypoints[..segment].to_vec();
    if segment == 0 {
        out.push(a);
    }
    out.push(shift(a));
    out.push(shift(b));
    if segment + 2 == waypoints.len() {
        out.push(b);
    }
    out.extend_from_slice(&waypoints[segment + 2..]);

    Some(out)
}
