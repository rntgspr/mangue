use indexmap::IndexMap;

use super::id::ElementId;

/// Axis-aligned rectangle in diagram coordinates (BPMNDI `dc:Bounds`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Point of an edge path (BPMNDI `di:waypoint`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Waypoint {
    pub x: f64,
    pub y: f64,
}

/// Visual representation of a node element (`bpmndi:BPMNShape`).
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub element_id: ElementId,
    pub bounds: Bounds,
    pub is_expanded: Option<bool>,
    pub is_horizontal: Option<bool>,
    pub label_bounds: Option<Bounds>,
}

/// Visual representation of a connection element (`bpmndi:BPMNEdge`).
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub element_id: ElementId,
    pub waypoints: Vec<Waypoint>,
    pub label_bounds: Option<Bounds>,
}

/// Drawing surface bound to a process or collaboration (`bpmndi:BPMNPlane`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plane {
    pub id: String,
    pub element_id: Option<ElementId>,
    pub shapes: Vec<Shape>,
    pub edges: Vec<Edge>,
    /// Original DI ids by element; absent ones export as `<element>_di`.
    pub di_ids: IndexMap<ElementId, String>,
}

/// A BPMN diagram (`bpmndi:BPMNDiagram`) holding a single plane: the root
/// one, or the drill-down plane of a collapsed sub-process.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Diagram {
    pub id: String,
    pub plane: Plane,
}
