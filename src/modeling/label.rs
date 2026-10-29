//! External label placement, after bpmn-js `LabelUtil`
//! (https://github.com/bpmn-io/bpmn-js/blob/develop/lib/util/LabelUtil.js).

use super::layout::center;
use crate::model::{Bounds, ElementKind, Waypoint};

/// Default external label size (bpmn-js `DEFAULT_LABEL_SIZE`).
pub const LABEL_SIZE: (f64, f64) = (90.0, 20.0);

/// Offset of a flow label from its path (bpmn-js `FLOW_LABEL_INDENT`).
const FLOW_LABEL_INDENT: f64 = 15.0;

/// Whether elements of `kind` carry an external label (bpmn-js
/// `isLabelExternal`): events, gateways, data objects/stores, sequence and
/// message flows, groups.
pub fn has_external_label(kind: &ElementKind) -> bool {
    use ElementKind::*;

    matches!(
        kind,
        StartEvent(_)
            | EndEvent(_)
            | IntermediateCatchEvent(_)
            | IntermediateThrowEvent(_)
            | BoundaryEvent { .. }
            | ExclusiveGateway
            | ParallelGateway
            | InclusiveGateway
            | EventBasedGateway
            | ComplexGateway
            | DataObjectReference
            | DataStoreReference
            | SequenceFlow { .. }
            | MessageFlow { .. }
            | Group
    )
}

/// Default label bounds of a shape (bpmn-js `getExternalLabelMid`): centered
/// below the shape, or inside its top edge for a group.
pub fn shape_label_bounds(kind: &ElementKind, shape: &Bounds) -> Bounds {
    let h = LABEL_SIZE.1;
    let x = center(shape).x;
    let y = if *kind == ElementKind::Group {
        shape.y + h / 2.0
    } else {
        shape.y + shape.h + h / 2.0
    };

    around(Waypoint { x, y })
}

/// Default label bounds of a connection (bpmn-js `getFlowLabelPosition`):
/// at the middle segment's mid, 15px above it when it is not vertical, else
/// 15px to its right. `None` for fewer than two waypoints.
pub fn flow_label_bounds(waypoints: &[Waypoint]) -> Option<Bounds> {
    if waypoints.len() < 2 {
        return None;
    }

    let mid = waypoints.len() as f64 / 2.0 - 1.0;
    let first = waypoints[mid.floor() as usize];
    let second = waypoints[(mid + 0.01).ceil() as usize];
    let mut p = Waypoint {
        x: first.x + (second.x - first.x) / 2.0,
        y: first.y + (second.y - first.y) / 2.0,
    };

    let vertical = (second.x - first.x).abs() < f64::EPSILON;
    if vertical {
        p.x += FLOW_LABEL_INDENT;
    } else {
        p.y -= FLOW_LABEL_INDENT;
    }

    Some(around(p))
}

/// Label-size bounds centered at `mid`.
fn around(mid: Waypoint) -> Bounds {
    let (w, h) = LABEL_SIZE;

    Bounds {
        x: mid.x - w / 2.0,
        y: mid.y - h / 2.0,
        w,
        h,
    }
}
