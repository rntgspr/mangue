//! Docking slots: the connections docked on one side of a shape split that
//! side in equal parts and dock at the middle of their part, ordered by
//! their current docking position, then by where their other end is.

use std::collections::HashMap;

use super::command::Command;
use super::layout::{Side, border_side, orthogonalize, side_route};
use super::undo::Undo;
use crate::model::{Bounds, Definitions, ElementId, Waypoint};

/// Middle of slot `i` of `n` equal slots along `side` of `b`, i.e. the
/// point at `(i + 0.5) / n` of the side, measured from its top or left end.
pub fn slot(b: &Bounds, side: Side, i: usize, n: usize) -> Waypoint {
    let f = (i as f64 + 0.5) / n as f64;

    match side {
        Side::Left => Waypoint {
            x: b.x,
            y: b.y + b.h * f,
        },
        Side::Right => Waypoint {
            x: b.x + b.w,
            y: b.y + b.h * f,
        },
        Side::Top => Waypoint {
            x: b.x + b.w * f,
            y: b.y,
        },
        Side::Bottom => Waypoint {
            x: b.x + b.w * f,
            y: b.y + b.h,
        },
    }
}

/// Nearest point on the border of `b` to `p`, with the side holding it.
fn project(b: &Bounds, p: Waypoint) -> (Waypoint, Side) {
    let x = p.x.clamp(b.x, b.x + b.w);
    let y = p.y.clamp(b.y, b.y + b.h);
    let candidates = [
        (Side::Top, Waypoint { x, y: b.y }),
        (Side::Right, Waypoint { x: b.x + b.w, y }),
        (Side::Bottom, Waypoint { x, y: b.y + b.h }),
        (Side::Left, Waypoint { x: b.x, y }),
    ];
    let distance = |q: Waypoint| (q.x - p.x).hypot(q.y - p.y);

    candidates
        .into_iter()
        .min_by(|a, c| distance(a.1).total_cmp(&distance(c.1)))
        .map(|(side, q)| (q, side))
        .expect("four candidates")
}

/// Command moving one end of connection `id` (its start when `start`) to the
/// border point of its own shape nearest to `at`, on any side, without
/// changing source or target; the slot pass then orders it by that point.
/// Staying on the same side only moves the end; another side re-routes from
/// the new side to the other end. `None` when the end has no shape or
/// nothing would change.
pub fn redock(defs: &Definitions, id: &ElementId, start: bool, at: Waypoint) -> Option<Command> {
    let (s, t) = defs.get(id)?.kind.endpoints()?;
    let (own, other) = if start { (s, t) } else { (t, s) };
    let own = defs.shape(own)?.bounds;
    let other = defs.shape(other)?.bounds;
    let current = defs
        .edges()
        .find(|e| &e.element_id == id)?
        .waypoints
        .clone();
    if current.len() < 2 {
        return None;
    }

    let (point, side) = project(&own, at);
    let (end, far) = if start {
        (current[0], current[current.len() - 1])
    } else {
        (current[current.len() - 1], current[0])
    };
    if (end.x - point.x).abs() < 1e-6 && (end.y - point.y).abs() < 1e-6 {
        return None;
    }

    let waypoints = match (border_side(&own, end), border_side(&other, far)) {
        (Some(old), _) if old == side => {
            let mut waypoints = current;
            let index = if start { 0 } else { waypoints.len() - 1 };
            waypoints[index] = point;
            waypoints
        }
        (_, Some(far_side)) if start => side_route(point, side, far, far_side),
        (_, Some(far_side)) => side_route(far, far_side, point, side),
        _ if start => vec![point, far],
        _ => vec![far, point],
    };

    Some(Command::UpdateWaypoints {
        id: id.clone(),
        waypoints,
    })
}

/// One connection end docked on a shape side.
struct Dock {
    edge: usize,
    at_start: bool,
    along: f64,
    other: f64,
}

/// Coordinate of `p` along `side` (y for left/right sides, x for top/bottom).
fn along(side: Side, p: Waypoint) -> f64 {
    match side {
        Side::Left | Side::Right => p.y,
        Side::Top | Side::Bottom => p.x,
    }
}

/// New waypoints of every edge of the active plane whose docking points are
/// not at their slot: ends are grouped by shape and side, sorted by current
/// position along the side (rounded to whole units, so freshly routed ends
/// docked at the same middle tie) then by the other end's docking position,
/// and moved to their slot; the path is made orthogonal again. Ends that do
/// not lie on their shape's border are left alone and not counted.
pub fn distribute(defs: &Definitions) -> Vec<(ElementId, Vec<Waypoint>)> {
    let edges = &defs.plane().edges;
    let mut sides: HashMap<(ElementId, u8), (Bounds, Side, Vec<Dock>)> = HashMap::new();

    for (index, edge) in edges.iter().enumerate() {
        let Some((s, t)) = defs.get(&edge.element_id).and_then(|e| e.kind.endpoints()) else {
            continue;
        };
        let (Some(first), Some(last)) = (edge.waypoints.first(), edge.waypoints.last()) else {
            continue;
        };
        if edge.waypoints.len() < 2 {
            continue;
        }

        for (id, at_start, point, other) in [(s, true, *first, *last), (t, false, *last, *first)] {
            let Some(bounds) = defs.shape(id).map(|sh| sh.bounds) else {
                continue;
            };
            let Some(side) = border_side(&bounds, point) else {
                continue;
            };

            sides
                .entry((id.clone(), side as u8))
                .or_insert_with(|| (bounds, side, Vec::new()))
                .2
                .push(Dock {
                    edge: index,
                    at_start,
                    along: along(side, point),
                    other: along(side, other),
                });
        }
    }

    let mut moved: HashMap<usize, (Option<Waypoint>, Option<Waypoint>)> = HashMap::new();
    for (bounds, side, mut docks) in sides.into_values() {
        docks.sort_by(|a, b| {
            a.along
                .round()
                .total_cmp(&b.along.round())
                .then(a.other.total_cmp(&b.other))
        });

        let n = docks.len();
        for (i, dock) in docks.iter().enumerate() {
            let target = slot(&bounds, side, i, n);
            let waypoints = &edges[dock.edge].waypoints;
            let current = if dock.at_start {
                waypoints[0]
            } else {
                waypoints[waypoints.len() - 1]
            };
            if (current.x - target.x).abs() < 1e-6 && (current.y - target.y).abs() < 1e-6 {
                continue;
            }

            let entry = moved.entry(dock.edge).or_default();
            if dock.at_start {
                entry.0 = Some(target);
            } else {
                entry.1 = Some(target);
            }
        }
    }

    let mut changes: Vec<(usize, Vec<Waypoint>)> = moved
        .into_iter()
        .map(|(index, (start, end))| {
            let edge = &edges[index];
            let mut waypoints = edge.waypoints.clone();
            let last = waypoints.len() - 1;
            if let Some(p) = start {
                waypoints[0] = p;
            }
            if let Some(p) = end {
                waypoints[last] = p;
            }

            let ends = defs.get(&edge.element_id).and_then(|e| e.kind.endpoints());
            let shape = |id| defs.shape(id).map(|sh| sh.bounds);
            let (sb, tb) = (
                ends.and_then(|(s, _)| shape(s)),
                ends.and_then(|(_, t)| shape(t)),
            );

            (index, orthogonalize(&waypoints, sb.as_ref(), tb.as_ref()))
        })
        .collect();
    changes.sort_by_key(|(index, _)| *index);

    changes
        .into_iter()
        .map(|(index, waypoints)| (edges[index].element_id.clone(), waypoints))
        .collect()
}

/// Applies `distribute` to the active plane, recording before-images in
/// `undo`; moved edges drop their label position back to the default.
pub(super) fn apply(defs: &mut Definitions, undo: &mut Undo) {
    for (id, waypoints) in distribute(defs) {
        undo.edge(defs, &id);
        if let Some(edge) = defs
            .plane_mut()
            .edges
            .iter_mut()
            .find(|e| e.element_id == id)
        {
            edge.waypoints = waypoints;
            edge.label_bounds = None;
        }
    }
}

/// Distributes the docking points of every plane; used after import so
/// loaded files follow the slot layout; imported label positions are kept.
pub fn distribute_all(defs: &mut Definitions) {
    let active = defs.active;

    for index in 0..defs.diagrams.len() {
        defs.active = index;
        for (id, waypoints) in distribute(defs) {
            if let Some(edge) = defs
                .plane_mut()
                .edges
                .iter_mut()
                .find(|e| e.element_id == id)
            {
                edge.waypoints = waypoints;
            }
        }
    }

    defs.active = active;
}
