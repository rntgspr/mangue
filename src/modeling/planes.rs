//! Subprocess plane lifecycle and import compatibility.

use super::command::translate;
use super::rules::closure;
use super::undo::Undo;
use crate::model::{Bounds, Definitions, Diagram, ElementId, ElementKind, Plane, Waypoint};

/// Removes the plane at `index` of collapsed sub-process `id` with the
/// content drawn on it (bpmn-js `SubProcessPlaneBehavior` on replace to a
/// non-sub-process): a task cannot own flow elements.
pub(super) fn drop_plane(defs: &mut Definitions, id: &ElementId, index: usize, undo: &mut Undo) {
    undo.diagrams(defs);

    let children: Vec<ElementId> = defs.children(id).map(|e| e.id.clone()).collect();
    let inner = closure(defs, &children);
    for child in &inner {
        undo.all(defs, child);
    }

    for child in &inner {
        defs.remove_element(child);
    }

    defs.diagrams.remove(index);
}

/// Moves the DI of sub-process `id`'s content off the active plane onto a
/// new plane of its own, top-left at `PLANE_ORIGIN` (bpmn-js
/// `SubProcessPlaneBehavior` on collapse, `SubprocessCompatibility` on
/// import).
pub(crate) fn split_plane(defs: &mut Definitions, id: &ElementId) {
    let children: Vec<ElementId> = defs.children(id).map(|e| e.id.clone()).collect();
    let inner = closure(defs, &children);
    let plane = defs.plane_mut();
    let mut content = Plane::default();

    let (moved, kept) = plane
        .shapes
        .drain(..)
        .partition(|s| inner.contains(&s.element_id));
    (content.shapes, plane.shapes) = (moved, kept);

    let (moved, kept) = plane
        .edges
        .drain(..)
        .partition(|e| inner.contains(&e.element_id));
    (content.edges, plane.edges) = (moved, kept);

    let di_ids = std::mem::take(&mut plane.di_ids);
    for (element, di) in di_ids {
        let target = if inner.contains(&element) {
            &mut content.di_ids
        } else {
            &mut plane.di_ids
        };
        target.insert(element, di);
    }

    if let Some(b) = content_bounds(&content) {
        translate_plane(&mut content, PLANE_ORIGIN.x - b.x, PLANE_ORIGIN.y - b.y);
    }
    defs.diagrams.push(sub_process_diagram(id, content));
}

/// Gives every collapsed sub-process without a plane its own one, moving
/// its content's DI there from the plane it is drawn on (bpmn-js
/// `SubprocessCompatibility` on import); nested ones are split once their
/// parent's plane exists. Leaves the root plane active.
pub(crate) fn compat_planes(defs: &mut Definitions) {
    let mut d = 0;
    while d < defs.diagrams.len() {
        defs.active = d;
        let collapsed: Vec<ElementId> = defs
            .plane()
            .shapes
            .iter()
            .filter(|s| s.is_expanded != Some(true))
            .map(|s| s.element_id.clone())
            .filter(|id| {
                defs.get(id)
                    .is_some_and(|e| matches!(e.kind, ElementKind::SubProcess { .. }))
            })
            .collect();

        for id in collapsed {
            if defs.plane_index(&id).is_none() {
                split_plane(defs, &id);
            }
        }

        d += 1;
    }

    defs.active = 0;
}

/// Top-left of the content of a new sub-process plane (bpmn-js
/// `SubProcessPlaneBehavior` `DEFAULT_POSITION`).
const PLANE_ORIGIN: Waypoint = Waypoint { x: 180.0, y: 160.0 };

/// Diagram showing collapsed sub-process `id` with `plane`'s content, ids
/// `BPMNDiagram_<id>` / `<id>_plane` (bpmn-js names the plane root `<id>_plane`).
pub(super) fn sub_process_diagram(id: &ElementId, plane: Plane) -> Diagram {
    Diagram {
        id: format!("BPMNDiagram_{}", id.as_str()),
        plane: Plane {
            id: format!("{}_plane", id.as_str()),
            element_id: Some(id.clone()),
            ..plane
        },
    }
}

/// Union of the shape bounds of `plane`; `None` when it has no shapes.
pub(super) fn content_bounds(plane: &Plane) -> Option<Bounds> {
    plane.shapes.iter().map(|s| s.bounds).reduce(|a, b| {
        let (x, y) = (a.x.min(b.x), a.y.min(b.y));
        Bounds {
            x,
            y,
            w: (a.x + a.w).max(b.x + b.w) - x,
            h: (a.y + a.h).max(b.y + b.h) - y,
        }
    })
}

/// Shifts every shape, edge and label of `plane` by `(dx, dy)`.
pub(super) fn translate_plane(plane: &mut Plane, dx: f64, dy: f64) {
    for shape in &mut plane.shapes {
        translate(&mut shape.bounds, dx, dy);
        if let Some(label) = &mut shape.label_bounds {
            translate(label, dx, dy);
        }
    }

    for edge in &mut plane.edges {
        for wp in &mut edge.waypoints {
            wp.x += dx;
            wp.y += dy;
        }
        if let Some(label) = &mut edge.label_bounds {
            translate(label, dx, dy);
        }
    }
}
