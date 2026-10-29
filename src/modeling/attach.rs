//! Builders for boundary attachment and connection waypoint edits, after
//! bpmn-js `AttachSupport`/`BpmnCreateMoveSnapping` and diagram-js
//! `BendpointMove`/`ConnectionSegmentMove`.

use super::command::Command;
use super::layout::center;
use super::rules::{ATTACH_PADDING, boundary_side, can_attach};
use super::waypoints::{insert_bendpoint, move_bendpoint, move_segment, remove_bendpoint};
use crate::model::{Bounds, Definitions, ElementId, ElementKind, Waypoint};

/// Snaps `point` onto the border of `host` on the side(s) it attaches to
/// (bpmn-js `snapBoundaryEvent`); `None` when it does not attach.
pub fn snap_to_border(point: Waypoint, host: &Bounds) -> Option<Waypoint> {
    let (dx, dy) = boundary_side(point, host)?;
    let mut out = point;

    match dx {
        -1 => out.x = host.x,
        1 => out.x = host.x + host.w,
        _ => {}
    }

    match dy {
        -1 => out.y = host.y,
        1 => out.y = host.y + host.h,
        _ => {}
    }

    Some(out)
}

/// Command attaching `event` to `host` with its center snapped to the
/// host border near `point`; `None` when `rules::can_attach` refuses.
pub fn attach(
    defs: &Definitions,
    event: &ElementId,
    host: &ElementId,
    point: Waypoint,
) -> Option<Command> {
    if !can_attach(defs, event, host, point) {
        return None;
    }

    let center = snap_to_border(point, &defs.shape(host)?.bounds)?;

    Some(Command::AttachBoundary {
        id: event.clone(),
        host: Some(host.clone()),
        center,
    })
}

/// Command for a boundary event dragged by `(dx, dy)`: re-attached (snapped)
/// to the activity under its new center when allowed, else detached into an
/// intermediate event. Plain moves of non-boundary shapes return `None`.
pub fn move_boundary(defs: &Definitions, event: &ElementId, dx: f64, dy: f64) -> Option<Command> {
    if !matches!(defs.get(event)?.kind, ElementKind::BoundaryEvent { .. }) {
        return None;
    }

    let c = center(&defs.shape(event)?.bounds);
    let point = Waypoint {
        x: c.x + dx,
        y: c.y + dy,
    };

    let host = host_at(defs, event, point);

    match host {
        Some(host) => attach(defs, event, &host, point),
        None => Some(Command::AttachBoundary {
            id: event.clone(),
            host: None,
            center: point,
        }),
    }
}

/// Finds the topmost attachable host near a point.
pub fn host_at(defs: &Definitions, event: &ElementId, point: Waypoint) -> Option<ElementId> {
    defs.shapes()
        .filter(|s| s.element_id != *event)
        .filter(|s| {
            let b = &s.bounds;
            let pad = ATTACH_PADDING;
            point.x >= b.x - pad
                && point.x <= b.x + b.w + pad
                && point.y >= b.y - pad
                && point.y <= b.y + b.h + pad
        })
        .filter(|s| can_attach(defs, event, &s.element_id, point))
        .last()
        .map(|s| s.element_id.clone())
}

/// Current waypoints of connection `id`.
fn waypoints(defs: &Definitions, id: &ElementId) -> Option<Vec<Waypoint>> {
    defs.edges()
        .find(|e| &e.element_id == id)
        .map(|e| e.waypoints.clone())
}

/// Command moving bendpoint `index` of connection `id` to `to`.
pub fn bendpoint_move(
    defs: &Definitions,
    id: &ElementId,
    index: usize,
    to: Waypoint,
) -> Option<Command> {
    let waypoints = move_bendpoint(&waypoints(defs, id)?, index, to)?;

    Some(Command::UpdateWaypoints {
        id: id.clone(),
        waypoints,
    })
}

/// Command inserting a bendpoint at `at` on segment `segment` of `id`.
pub fn bendpoint_insert(
    defs: &Definitions,
    id: &ElementId,
    segment: usize,
    at: Waypoint,
) -> Option<Command> {
    let waypoints = insert_bendpoint(&waypoints(defs, id)?, segment, at)?;

    Some(Command::UpdateWaypoints {
        id: id.clone(),
        waypoints,
    })
}

/// Command removing bendpoint `index` of connection `id`.
pub fn bendpoint_remove(defs: &Definitions, id: &ElementId, index: usize) -> Option<Command> {
    let waypoints = remove_bendpoint(&waypoints(defs, id)?, index)?;

    Some(Command::UpdateWaypoints {
        id: id.clone(),
        waypoints,
    })
}

/// Command moving segment `segment` of connection `id` by `delta`
/// perpendicular to itself.
pub fn segment_move(
    defs: &Definitions,
    id: &ElementId,
    segment: usize,
    delta: f64,
) -> Option<Command> {
    let waypoints = move_segment(&waypoints(defs, id)?, segment, delta)?;

    Some(Command::UpdateWaypoints {
        id: id.clone(),
        waypoints,
    })
}
