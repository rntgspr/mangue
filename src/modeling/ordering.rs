//! Z-order of DI elements after bpmn-js `BpmnOrderingProvider` and
//! `BpmnDiOrdering`: every shape or edge is painted inside its graphical
//! parent (participant, expanded sub-process or the plane root), siblings
//! sorted by level (participants < lanes < flow nodes < sub-processes <
//! boundary events < flows), keeping document order within a level. The
//! exported DI follows the same order.

use crate::model::{Definitions, ElementId, ElementKind, Plane};

/// A DI element of a plane: index into `plane.shapes` or `plane.edges`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiItem {
    Shape(usize),
    Edge(usize),
}

/// Ordering level of `kind` (bpmn-js `BpmnOrderingProvider` `orders`).
pub fn level(kind: &ElementKind) -> i32 {
    use ElementKind::*;

    match kind {
        SubProcess { .. } => 6,
        SequenceFlow { .. }
        | DataInputAssociation { .. }
        | DataOutputAssociation { .. }
        | TextAnnotation { .. }
        | MessageFlow { .. } => 9,
        Association { .. } => 6,
        BoundaryEvent { .. } => 8,
        Group => 10,
        Participant { .. } => -2,
        Lane => -1,
        Process | Collaboration => 1,
        _ => 5,
    }
}

/// Graphical parent of `id` on `plane`: the participant or expanded
/// sub-process shape it is painted in, `None` for the plane root. Lanes
/// are skipped (diagram-js paints lane content in the participant) and a
/// boundary event sits beside its host.
pub fn gfx_parent(defs: &Definitions, plane: &Plane, id: &ElementId) -> Option<ElementId> {
    let element = defs.get(id)?;
    let on_plane = |p: &ElementId| plane.shapes.iter().any(|s| s.element_id == *p);

    let mut parent = match &element.kind {
        ElementKind::BoundaryEvent { attached_to, .. } => defs.get(attached_to)?.parent.clone(),
        ElementKind::MessageFlow { .. } | ElementKind::Participant { .. } => None,
        _ => element.parent.clone(),
    };

    for _ in 0..defs.elements.len() {
        let p = parent?;
        let e = defs.get(&p)?;

        match &e.kind {
            ElementKind::Lane => parent = e.parent.clone(),

            ElementKind::Process => {
                return defs
                    .elements
                    .values()
                    .find(|o| {
                        matches!(&o.kind, ElementKind::Participant { process_ref: Some(r) } if *r == p)
                    })
                    .map(|o| o.id.clone())
                    .filter(on_plane);
            }

            ElementKind::SubProcess { .. } if on_plane(&p) => return Some(p),

            ElementKind::SubProcess { .. } if plane.element_id.as_ref() == Some(&p) => {
                return None;
            }

            _ => parent = e.parent.clone(),
        }
    }

    None
}

/// Paint order of the DI elements of `plane`: a depth-first walk of the
/// graphical tree, siblings by `level`, then shapes before edges, then
/// document order.
pub fn paint_order(defs: &Definitions, plane: &Plane) -> Vec<DiItem> {
    let items: Vec<(DiItem, &ElementId)> = plane
        .shapes
        .iter()
        .enumerate()
        .map(|(i, s)| (DiItem::Shape(i), &s.element_id))
        .chain(
            plane
                .edges
                .iter()
                .enumerate()
                .map(|(i, e)| (DiItem::Edge(i), &e.element_id)),
        )
        .collect();

    let mut nodes: Vec<(DiItem, &ElementId, Option<ElementId>, i32)> = items
        .into_iter()
        .map(|(item, id)| {
            let parent = gfx_parent(defs, plane, id).filter(|p| p != id);
            let level = defs.get(id).map_or(1, |e| level(&e.kind));
            (item, id, parent, level)
        })
        .collect();
    nodes.sort_by_key(|(_, _, _, level)| *level);

    let mut out = Vec::with_capacity(nodes.len());
    let mut visited = vec![false; nodes.len()];
    visit(&nodes, None, &mut visited, &mut out);

    for (i, node) in nodes.iter().enumerate() {
        if !visited[i] {
            out.push(node.0);
        }
    }

    out
}

/// Appends, in sibling order, every unvisited node whose parent is `parent`
/// followed by its own subtree.
fn visit(
    nodes: &[(DiItem, &ElementId, Option<ElementId>, i32)],
    parent: Option<&ElementId>,
    visited: &mut [bool],
    out: &mut Vec<DiItem>,
) {
    for i in 0..nodes.len() {
        let (item, id, p, _) = &nodes[i];
        if visited[i] || p.as_ref() != parent {
            continue;
        }

        visited[i] = true;
        out.push(*item);
        if matches!(item, DiItem::Shape(_)) {
            visit(nodes, Some(id), visited, out);
        }
    }
}

/// Reorders the shapes and edges of every plane by `paint_order` (bpmn-js
/// `BpmnDiOrdering` on save).
pub fn order_planes(defs: &mut Definitions) {
    for index in 0..defs.diagrams.len() {
        let plane = &defs.diagrams[index].plane;
        let order = paint_order(defs, plane);

        let shapes = order
            .iter()
            .filter_map(|item| match item {
                DiItem::Shape(i) => Some(plane.shapes[*i].clone()),
                DiItem::Edge(_) => None,
            })
            .collect();
        let edges = order
            .iter()
            .filter_map(|item| match item {
                DiItem::Edge(i) => Some(plane.edges[*i].clone()),
                DiItem::Shape(_) => None,
            })
            .collect();

        let plane = &mut defs.diagrams[index].plane;
        plane.shapes = shapes;
        plane.edges = edges;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Bounds, Diagram, Edge, Element, Shape, Waypoint};

    /// A DI shape of `id` at the origin.
    fn shape(id: &str) -> Shape {
        Shape {
            element_id: id.into(),
            bounds: Bounds {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            is_expanded: Some(true),
            is_horizontal: None,
            label_bounds: None,
        }
    }

    /// Collaboration with a pool (one lane) holding a sub-process with a
    /// task, a boundary event on the sub-process and a flow; DI listed in
    /// the reverse of the bpmn-js order.
    fn sample() -> Definitions {
        let mut defs = Definitions::default();
        for element in [
            Element::new("C", ElementKind::Collaboration),
            Element::new("P", ElementKind::Process),
            Element::new(
                "Pool",
                ElementKind::Participant {
                    process_ref: Some("P".into()),
                },
            )
            .with_parent("C"),
            Element::new("Lane", ElementKind::Lane).with_parent("P"),
            Element::new(
                "Sub",
                ElementKind::SubProcess {
                    triggered_by_event: false,
                },
            )
            .with_parent("Lane"),
            Element::new("Inner", ElementKind::Task).with_parent("Sub"),
            Element::new("Outer", ElementKind::Task).with_parent("Lane"),
            Element::new(
                "Boundary",
                ElementKind::BoundaryEvent {
                    definition: crate::model::EventDefinition::None,
                    attached_to: "Sub".into(),
                    cancel_activity: true,
                },
            )
            .with_parent("P"),
            Element::new(
                "Flow",
                ElementKind::SequenceFlow {
                    source: "Outer".into(),
                    target: "Sub".into(),
                    is_default: false,
                    condition: None,
                },
            )
            .with_parent("P"),
        ] {
            defs.add_element(element);
        }

        defs.diagrams = vec![Diagram {
            id: "D".into(),
            plane: Plane {
                id: "Plane".into(),
                element_id: Some("C".into()),
                shapes: ["Boundary", "Inner", "Sub", "Outer", "Lane", "Pool"]
                    .map(shape)
                    .to_vec(),
                edges: vec![Edge {
                    element_id: "Flow".into(),
                    waypoints: vec![Waypoint { x: 0.0, y: 0.0 }, Waypoint { x: 5.0, y: 0.0 }],
                    label_bounds: None,
                }],
                ..Plane::default()
            },
        }];

        defs
    }

    /// Ids of `plane` in `paint_order`.
    fn ordered(defs: &Definitions) -> Vec<String> {
        let plane = defs.plane();

        paint_order(defs, plane)
            .into_iter()
            .map(|item| match item {
                DiItem::Shape(i) => plane.shapes[i].element_id.0.clone(),
                DiItem::Edge(i) => plane.edges[i].element_id.0.clone(),
            })
            .collect()
    }

    /// Pool, then lane, then flow nodes, the sub-process with its content,
    /// the boundary event above its host and the flow on top.
    #[test]
    fn bpmn_js_order() {
        let defs = sample();

        assert_eq!(
            ordered(&defs),
            ["Pool", "Lane", "Outer", "Sub", "Inner", "Boundary", "Flow"]
        );
    }

    /// Saving reorders the DI of the plane the same way.
    #[test]
    fn order_planes_sorts_di() {
        let mut defs = sample();
        order_planes(&mut defs);

        let ids: Vec<&str> = defs
            .plane()
            .shapes
            .iter()
            .map(|s| s.element_id.as_str())
            .collect();
        assert_eq!(ids, ["Pool", "Lane", "Outer", "Sub", "Inner", "Boundary"]);
        assert_eq!(ordered(&defs)[6], "Flow");
    }

    /// Graphical parents skip lanes and put boundary events beside hosts.
    #[test]
    fn gfx_parents() {
        let defs = sample();
        let plane = defs.plane();
        let parent = |id: &str| gfx_parent(&defs, plane, &id.into()).map(|p| p.0);

        assert_eq!(parent("Pool"), None);
        assert_eq!(parent("Lane").as_deref(), Some("Pool"));
        assert_eq!(parent("Inner").as_deref(), Some("Sub"));
        assert_eq!(parent("Boundary").as_deref(), Some("Pool"));
        assert_eq!(parent("Flow").as_deref(), Some("Pool"));
    }
}
