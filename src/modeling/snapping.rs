//! Shape snapping after diagram-js `Snapping` / `CreateMoveSnapping` /
//! `ResizeSnapping` and bpmn-js `BpmnCreateMoveSnapping`: a moved shape
//! snaps its center, top-left and bottom-right to the same points of its
//! siblings, a resized corner snaps to their corners, each axis on its own
//! within a screen tolerance; axes left unsnapped fall back to the grid
//! (diagram-js `GridSnapping` runs after and skips snapped axes).

use super::ordering::gfx_parent;
use super::rules::closure;
use crate::model::{Bounds, Definitions, ElementId, ElementKind, Waypoint};

/// Snap distance in screen px (diagram-js `SNAP_TOLERANCE`).
pub const TOLERANCE: f64 = 7.0;

/// Extra length (diagram units) of a snap line past the shapes it joins.
const LINE_OVERHANG: f64 = 20.0;

/// A snap line to draw, in diagram coordinates.
pub type Line = (Waypoint, Waypoint);

/// Result of snapping a drag: the corrected delta and the lines to show.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapped {
    pub dx: f64,
    pub dy: f64,
    pub lines: Vec<Line>,
}

/// Snaps a move of `moving` by `(dx, dy)`: the first moving shape's
/// center/top-left/bottom-right to its siblings' (`tolerance` in diagram
/// units), then each unsnapped axis puts that center on the grid of size
/// `grid` (`None` = grid snapping off).
pub fn snap_move(
    defs: &Definitions,
    moving: &[ElementId],
    dx: f64,
    dy: f64,
    tolerance: f64,
    grid: Option<f64>,
) -> Snapped {
    let Some(b) = moving
        .first()
        .and_then(|id| defs.shape(id))
        .map(|s| s.bounds)
    else {
        return Snapped {
            dx,
            dy,
            lines: Vec::new(),
        };
    };
    let moved = Bounds {
        x: b.x + dx,
        y: b.y + dy,
        ..b
    };
    let targets = targets(defs, moving);

    let points = |b: &Bounds, horizontal: bool| {
        if horizontal {
            [b.x, b.x + b.w / 2.0, b.x + b.w]
        } else {
            [b.y, b.y + b.h / 2.0, b.y + b.h]
        }
    };
    let best = |horizontal: bool| {
        let own = points(&moved, horizontal);
        targets
            .iter()
            .flat_map(|t| {
                let theirs = points(t, horizontal);
                (0..3).map(move |i| (theirs[i] - own[i], *t))
            })
            .filter(|(d, _)| d.abs() <= tolerance)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
    };

    let mut out = Snapped {
        dx,
        dy,
        lines: Vec::new(),
    };
    let c = Waypoint {
        x: b.x + b.w / 2.0,
        y: b.y + b.h / 2.0,
    };

    match best(true) {
        Some((d, t)) => {
            out.dx += d;
            let x = points(&moved, true)
                .into_iter()
                .map(|v| v + d)
                .find(|v| points(&t, true).iter().any(|p| (p - v).abs() < 1e-6))
                .unwrap_or(c.x + out.dx);
            out.lines.push(vertical(x, &moved, &t));
        }
        None => out.dx = quantize(c.x + dx, grid) - c.x,
    }

    match best(false) {
        Some((d, t)) => {
            out.dy += d;
            let y = points(&moved, false)
                .into_iter()
                .map(|v| v + d)
                .find(|v| points(&t, false).iter().any(|p| (p - v).abs() < 1e-6))
                .unwrap_or(c.y + out.dy);
            out.lines.push(horizontal(y, &moved, &t));
        }
        None => out.dy = quantize(c.y + dy, grid) - c.y,
    }

    out
}

/// Snaps the dragged corner `p` of resized shape `id`: each axis to the
/// nearest sibling left/right (top/bottom) edge within `tolerance`, else to
/// the grid of size `grid` (`None` = off). Returns the snapped point and its lines.
pub fn snap_corner(
    defs: &Definitions,
    id: &ElementId,
    p: Waypoint,
    tolerance: f64,
    grid: Option<f64>,
) -> (Waypoint, Vec<Line>) {
    let targets = targets(defs, std::slice::from_ref(id));
    let mut lines = Vec::new();
    let dot = Bounds {
        x: p.x,
        y: p.y,
        w: 0.0,
        h: 0.0,
    };

    let nearest = |horizontal: bool| {
        targets
            .iter()
            .flat_map(|t| {
                let edges = if horizontal {
                    [t.x, t.x + t.w]
                } else {
                    [t.y, t.y + t.h]
                };
                edges.map(|e| (e, *t))
            })
            .filter(|(e, _)| (e - if horizontal { p.x } else { p.y }).abs() <= tolerance)
            .min_by(|a, b| {
                let v = if horizontal { p.x } else { p.y };
                (a.0 - v).abs().total_cmp(&(b.0 - v).abs())
            })
    };

    let x = match nearest(true) {
        Some((x, t)) => {
            lines.push(vertical(x, &dot, &t));
            x
        }
        None => quantize(p.x, grid),
    };
    let y = match nearest(false) {
        Some((y, t)) => {
            lines.push(horizontal(y, &dot, &t));
            y
        }
        None => quantize(p.y, grid),
    };

    (Waypoint { x, y }, lines)
}

/// `p` on the grid of size `grid`, unchanged for `None` (bendpoint and
/// segment drags).
pub fn snap_point(p: Waypoint, grid: Option<f64>) -> Waypoint {
    Waypoint {
        x: quantize(p.x, grid),
        y: quantize(p.y, grid),
    }
}

/// `v` rounded to the nearest line of a grid of size `grid` (diagram-js
/// `quantize`); unchanged for `None`.
pub fn quantize(v: f64, grid: Option<f64>) -> f64 {
    match grid {
        Some(size) if size > 0.0 => (v / size).round() * size,
        _ => v,
    }
}

/// Bounds of the snap targets of `moving`: shapes sharing the graphical
/// parent of the first one, outside the moved closure, never lanes (and
/// participants only for a moving participant); none for a boundary event,
/// which follows its host border instead.
fn targets(defs: &Definitions, moving: &[ElementId]) -> Vec<Bounds> {
    let Some(primary) = moving.first() else {
        return Vec::new();
    };
    let plane = defs.plane();
    let parent = gfx_parent(defs, plane, primary);
    if matches!(
        defs.get(primary).map(|e| &e.kind),
        Some(ElementKind::BoundaryEvent { .. })
    ) {
        return Vec::new();
    }
    let pool = matches!(
        defs.get(primary).map(|e| &e.kind),
        Some(ElementKind::Participant { .. })
    );
    let excluded = closure(defs, moving);

    defs.shapes()
        .filter(|s| !excluded.contains(&s.element_id))
        .filter(|s| {
            defs.get(&s.element_id).is_some_and(|e| match e.kind {
                ElementKind::Lane => false,
                ElementKind::Participant { .. } => pool,
                _ => !pool,
            })
        })
        .filter(|s| gfx_parent(defs, plane, &s.element_id) == parent)
        .map(|s| s.bounds)
        .collect()
}

/// Vertical snap line at `x` spanning `a` and `b`.
fn vertical(x: f64, a: &Bounds, b: &Bounds) -> Line {
    let top = a.y.min(b.y) - LINE_OVERHANG;
    let bottom = (a.y + a.h).max(b.y + b.h) + LINE_OVERHANG;

    (Waypoint { x, y: top }, Waypoint { x, y: bottom })
}

/// Horizontal snap line at `y` spanning `a` and `b`.
fn horizontal(y: f64, a: &Bounds, b: &Bounds) -> Line {
    let left = a.x.min(b.x) - LINE_OVERHANG;
    let right = (a.x + a.w).max(b.x + b.w) + LINE_OVERHANG;

    (Waypoint { x: left, y }, Waypoint { x: right, y })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeling::factory;

    /// The bpmn-js initial diagram plus a task centered at (400, 300).
    fn with_task() -> (Definitions, ElementId) {
        let mut defs = crate::io::new_diagram();
        let command = factory::create_shape(
            &defs,
            ElementKind::Task,
            false,
            Waypoint { x: 400.0, y: 300.0 },
        )
        .expect("create");
        command.execute(&mut defs).expect("created");
        let task = defs
            .elements
            .values()
            .find(|e| e.kind == ElementKind::Task)
            .expect("task")
            .id
            .clone();

        (defs, task)
    }

    /// A start event moved near the task's center line snaps onto it and
    /// reports a horizontal snap line; the other axis goes to the grid.
    #[test]
    fn move_snaps_to_sibling_center() {
        let (defs, _) = with_task();
        let start = ElementId::from("StartEvent_1");

        let s = snap_move(
            &defs,
            std::slice::from_ref(&start),
            100.0,
            176.0,
            TOLERANCE,
            Some(10.0),
        );
        let b = defs.shape(&start).unwrap().bounds;

        assert_eq!(b.y + b.h / 2.0 + s.dy, 300.0);
        assert_eq!((b.x + b.w / 2.0 + s.dx) % 10.0, 0.0);
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.lines[0].0.y, 300.0);
    }

    /// Far from any sibling and with the grid off, the delta is unchanged.
    #[test]
    fn move_without_target_or_grid_is_free() {
        let (defs, _) = with_task();
        let start = ElementId::from("StartEvent_1");

        let s = snap_move(&defs, &[start], 3.0, 4.0, TOLERANCE, None);
        assert_eq!((s.dx, s.dy, s.lines.len()), (3.0, 4.0, 0));
    }

    /// A resize corner snaps to a sibling edge, else to the grid.
    #[test]
    fn corner_snaps_to_edge_then_grid() {
        let (defs, task) = with_task();
        let start = ElementId::from("StartEvent_1");
        let edge_x = defs.shape(&task).unwrap().bounds.x;

        let (p, lines) = snap_corner(
            &defs,
            &start,
            Waypoint {
                x: edge_x - 4.0,
                y: 503.0,
            },
            7.0,
            Some(10.0),
        );
        assert_eq!(
            p,
            Waypoint {
                x: edge_x,
                y: 500.0
            }
        );
        assert_eq!(lines.len(), 1);

        assert_eq!(
            snap_point(Waypoint { x: 14.0, y: 16.0 }, Some(10.0)),
            Waypoint { x: 10.0, y: 20.0 }
        );
    }
}
