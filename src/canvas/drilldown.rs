//! Drill-down helpers (bpmn-js `DrilldownOverlayBehavior` and
//! `DrilldownBreadcrumbs`): which collapsed sub-processes get an "open"
//! button, where it sits, and the breadcrumb path of the active plane.

use crate::model::{Bounds, Definitions, ElementId, ElementKind};

/// Side of the drill-down button (px, screen space).
pub const BUTTON: f64 = 20.0;

/// One breadcrumb: its label and the plane it opens (`None` = root).
#[derive(Debug, Clone, PartialEq)]
pub struct Crumb {
    pub label: String,
    pub plane: Option<ElementId>,
}

/// Collapsed sub-processes on the active plane that own a plane, with
/// their shape bounds. Like bpmn-js, an empty plane's button shows only
/// while its sub-process is selected.
pub fn targets(defs: &Definitions, selection: &[ElementId]) -> Vec<(ElementId, Bounds)> {
    defs.shapes()
        .filter(|s| s.is_expanded != Some(true))
        .filter(|s| {
            defs.get(&s.element_id)
                .is_some_and(|e| matches!(e.kind, ElementKind::SubProcess { .. }))
        })
        .filter_map(|s| {
            let index = defs.plane_index(&s.element_id)?;
            let empty = defs.diagrams[index].plane.shapes.is_empty();
            (!empty || selection.contains(&s.element_id)).then(|| (s.element_id.clone(), s.bounds))
        })
        .collect()
}

/// Screen rect of the button for a shape at screen bounds `b`: bottom-right
/// corner, overhanging by 8px right and 7px down (bpmn-js overlay position).
pub fn button_rect(b: &Bounds) -> Bounds {
    Bounds {
        x: b.x + b.w + 8.0 - BUTTON,
        y: b.y + b.h + 7.0 - BUTTON,
        w: BUTTON,
        h: BUTTON,
    }
}

/// Breadcrumbs of the active plane, root first: every process or
/// sub-process ancestor of the active plane's sub-process (itself included)
/// that has a plane, labeled by name or id. Empty on the root plane
/// (bpmn-js shows the bar only for paths longer than one).
pub fn crumbs(defs: &Definitions) -> Vec<Crumb> {
    let Some(owner) = defs.plane_owner() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut cursor = Some(owner.clone());
    while let Some(id) = cursor {
        let Some(element) = defs.get(&id) else {
            break;
        };

        let plane = match element.kind {
            ElementKind::SubProcess { .. } => defs.plane_index(&id).map(|_| Some(id.clone())),
            ElementKind::Process => Some(None),
            _ => None,
        };
        if let Some(plane) = plane {
            let label = element.name.clone().unwrap_or_else(|| id.0.clone());
            out.push(Crumb { label, plane });
        }

        cursor = element.parent.clone();
    }

    out.reverse();
    out
}
