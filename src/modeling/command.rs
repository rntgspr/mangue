//! `Command`: the editing operations of the modeler, after bpmn-js
//! `CommandStack` handlers (`shape.move`, `shape.create`, ...).

use super::label::{flow_label_bounds, has_external_label, shape_label_bounds};
use super::lanes::{close_lane_gap, lane_slot, update_flow_node_refs};
use super::layout::{orthogonalize, repair_end, route};
use super::planes::{
    content_bounds, drop_plane, split_plane, sub_process_diagram, translate_plane,
};
use super::rules::{can_connect, closure, container_at, is_connection, owner};
use super::undo::Undo;
use crate::model::{
    Bounds, Definitions, Edge, Element, ElementId, ElementKind, Plane, Shape, Waypoint,
};

/// Returns the id of the first element created by a command.
pub fn created_id(command: &Command) -> Option<ElementId> {
    match command {
        Command::CreateShape { element, .. } | Command::CreateConnection { element, .. } => {
            Some(element.id.clone())
        }
        Command::CreateParticipant { participant, .. } => Some(participant.id.clone()),
        Command::Compound(commands) => commands.first().and_then(created_id),
        _ => None,
    }
}

/// Returns the kind and shape created by a command.
pub fn created_shape(command: &Command) -> Option<(ElementKind, Shape)> {
    match command {
        Command::CreateShape { element, shape }
        | Command::CreateParticipant {
            participant: element,
            shape,
            ..
        } => Some((element.kind.clone(), shape.clone())),
        Command::Compound(commands) => commands.iter().find_map(created_shape),
        _ => None,
    }
}

/// One undoable model change. Executing it yields the `Undo` that reverts it.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Translates shapes (with their children, boundary events and edges).
    MoveShapes {
        ids: Vec<ElementId>,
        dx: f64,
        dy: f64,
    },
    /// Adds a node element with its DI shape.
    CreateShape { element: Element, shape: Shape },
    /// Adds a connection; empty `edge.waypoints` are routed automatically and
    /// a missing external label gets its default bounds.
    CreateConnection { element: Element, edge: Edge },
    /// Removes elements, their descendants and every attached connection.
    DeleteElements { ids: Vec<ElementId> },
    /// Sets the name (or text-annotation text); empty clears it.
    UpdateLabel { id: ElementId, name: String },
    /// Sets new bounds on a shape and re-routes its connections.
    ResizeShape { id: ElementId, bounds: Bounds },
    /// Changes the kind of an element keeping its id, name, connections and
    /// DI; connections the new kind no longer allows are deleted.
    ReplaceElement { id: ElementId, kind: ElementKind },
    /// Adds a participant (pool) with its shape painted below everything.
    /// With `collaboration`, the plane's process diagram becomes that
    /// collaboration and the participant references the existing process;
    /// with `process`, that new process is added for the participant.
    CreateParticipant {
        participant: Element,
        shape: Shape,
        process: Option<Element>,
        collaboration: Option<Element>,
    },
    /// Attaches event `id` to `host` as a boundary event (bpmn-js
    /// `AttachEventBehavior`), or detaches it with `host: None` (bpmn-js
    /// `DetachEventBehavior`: back to an intermediate catch event, or a throw
    /// event without definition), moving its center to `center`. Attaching
    /// parents it beside the host and deletes connections the rules reject
    /// for a boundary event (its incoming flows).
    AttachBoundary {
        id: ElementId,
        host: Option<ElementId>,
        center: Waypoint,
    },
    /// Replaces the waypoints of a connection (bpmn-js
    /// `connection.updateWaypoints`); its label returns to the default spot.
    UpdateWaypoints {
        id: ElementId,
        waypoints: Vec<Waypoint>,
    },
    /// Re-assigns lane membership (`flowNodeRef`) of every flow node of the
    /// participant's process to the innermost lane under its center, or the
    /// process when none (bpmn-js `UpdateFlowNodeRefsBehavior`).
    UpdateFlowNodeRefs { participant: ElementId },
    /// Moves the external label of a shape or connection by `(dx, dy)`,
    /// starting from its default spot when it has no DI label yet (bpmn-js
    /// moves the label element on its own).
    MoveLabel { id: ElementId, dx: f64, dy: f64 },
    /// Sets the source and target of connection `id` and re-routes it
    /// (bpmn-js `ReconnectConnectionHandler`).
    Reconnect {
        id: ElementId,
        source: ElementId,
        target: ElementId,
    },
    /// Collapses or expands sub-process `id` (bpmn-js `shape.toggleCollapse`
    /// with `SubProcessPlaneBehavior`): collapsing moves its content into a
    /// new `<id>_plane` diagram, top-left at (180, 160); expanding moves the
    /// plane content back, centered on the shape, and drops the plane. The
    /// shape takes the default size of its new state around its center.
    ToggleCollapse { id: ElementId },
    /// Adds `elements` with the shapes and edges of `plane` to the existing
    /// plane of collapsed sub-process `owner` (paste of a collapsed
    /// sub-process, bpmn-js `SubProcessPlaneBehavior` on `elements.create`);
    /// collapsed sub-processes among them get an empty plane of their own.
    CreatePlaneContent {
        owner: ElementId,
        elements: Vec<Element>,
        plane: Plane,
    },
    /// Executes several commands as one undoable step.
    Compound(Vec<Command>),
}

impl Command {
    /// Applies the command to `defs`; returns `None` when nothing changed.
    pub fn execute(&self, defs: &mut Definitions) -> Option<Undo> {
        let mut undo = Undo::default();

        match self {
            Command::MoveShapes { ids, dx, dy } => {
                if *dx == 0.0 && *dy == 0.0 {
                    return None;
                }

                move_shapes(defs, ids, *dx, *dy, &mut undo);
            }

            Command::CreateShape { element, shape } => {
                if defs.get(&element.id).is_some() {
                    return None;
                }

                let collapsed = matches!(element.kind, ElementKind::SubProcess { .. })
                    && shape.is_expanded != Some(true);
                if collapsed {
                    undo.diagrams(defs);
                }

                undo.all(defs, &element.id);
                defs.add_element(element.clone());

                let at = lane_slot(defs, &element.id);
                defs.plane_mut().shapes.insert(at, shape.clone());

                if collapsed {
                    defs.diagrams
                        .push(sub_process_diagram(&element.id, Plane::default()));
                }
            }

            Command::CreateConnection { element, edge } => {
                if defs.get(&element.id).is_some() {
                    return None;
                }

                undo.all(defs, &element.id);
                defs.add_element(element.clone());
                mark_for_compensation(defs, &element.kind, &mut undo);

                let mut edge = edge.clone();
                if edge.waypoints.is_empty() {
                    edge.waypoints = route(defs, &element.id, None).unwrap_or_default();
                }
                edge.waypoints = orthogonal(defs, &element.id, &edge.waypoints);

                if edge.label_bounds.is_none() && has_external_label(&element.kind) {
                    edge.label_bounds = flow_label_bounds(&edge.waypoints);
                }
                defs.add_edge(edge);
            }

            Command::DeleteElements { ids } => delete(defs, ids, &mut undo),

            Command::UpdateLabel { id, name } => {
                if defs.get(id).is_none() {
                    return None;
                }

                undo.element(defs, id);
                let value = (!name.is_empty()).then(|| name.clone());
                let element = defs.elements.get_mut(id).expect("checked above");
                match &mut element.kind {
                    ElementKind::TextAnnotation { text } => *text = value,
                    _ => element.name = value,
                }
            }

            Command::ReplaceElement { id, kind } => {
                if defs.get(id).is_none_or(|e| e.kind == *kind) {
                    return None;
                }

                replace(defs, id, kind, &mut undo);
            }

            Command::CreateParticipant {
                participant,
                shape,
                process,
                collaboration,
            } => {
                if defs.get(&participant.id).is_some() {
                    return None;
                }

                for element in [collaboration, process].into_iter().flatten() {
                    undo.all(defs, &element.id);
                    defs.add_element(element.clone());
                }

                if let Some(c) = collaboration {
                    undo.plane(defs);
                    defs.plane_mut().element_id = Some(c.id.clone());
                }

                undo.all(defs, &participant.id);
                defs.add_element(participant.clone());
                defs.plane_mut().shapes.insert(0, shape.clone());
            }

            Command::AttachBoundary { id, host, center } => {
                attach(defs, id, host.as_ref(), *center, &mut undo)?;
            }

            Command::UpdateWaypoints { id, waypoints } => {
                let waypoints = orthogonal(defs, id, waypoints);
                let edge = defs.edges().find(|e| &e.element_id == id)?;
                if edge.waypoints == waypoints || waypoints.len() < 2 {
                    return None;
                }

                undo.edge(defs, id);
                if let Some(edge) = edge_mut(defs, id) {
                    edge.waypoints = waypoints;
                    edge.label_bounds = None;
                }
            }

            Command::MoveLabel { id, dx, dy } => {
                if *dx == 0.0 && *dy == 0.0 {
                    return None;
                }

                move_label(defs, id, *dx, *dy, &mut undo)?;
            }

            Command::Reconnect { id, source, target } => {
                let element = defs.get(id)?;
                if element.kind.endpoints() == Some((source, target))
                    || defs.shape(source).is_none()
                    || defs.shape(target).is_none()
                {
                    return None;
                }

                let (old_source, old_target) = element
                    .kind
                    .endpoints()
                    .map(|(s, t)| (s.clone(), t.clone()))?;
                let keep_start = (old_source == *source && old_target != *target)
                    .then_some(true)
                    .or_else(|| (old_target == *target && old_source != *source).then_some(false));

                undo.element(defs, id);
                let element = defs.elements.get_mut(id).expect("checked above");
                match &mut element.kind {
                    ElementKind::SequenceFlow {
                        source: s,
                        target: t,
                        ..
                    }
                    | ElementKind::MessageFlow {
                        source: s,
                        target: t,
                    }
                    | ElementKind::Association {
                        source: s,
                        target: t,
                    }
                    | ElementKind::DataInputAssociation {
                        source: s,
                        target: t,
                    }
                    | ElementKind::DataOutputAssociation {
                        source: s,
                        target: t,
                    } => {
                        *s = source.clone();
                        *t = target.clone();
                    }
                    _ => return None,
                }

                reroute_keeping(defs, id, keep_start, &mut undo);
            }

            Command::ToggleCollapse { id } => toggle_collapse(defs, id, &mut undo)?,

            Command::UpdateFlowNodeRefs { participant } => {
                update_flow_node_refs(defs, participant, &mut undo);
            }

            Command::CreatePlaneContent {
                owner,
                elements,
                plane,
            } => {
                let index = defs.plane_index(owner)?;
                if elements.iter().any(|e| defs.get(&e.id).is_some()) {
                    return None;
                }

                undo.diagrams(defs);
                for element in elements {
                    undo.all(defs, &element.id);
                    defs.add_element(element.clone());
                }

                let target = &mut defs.diagrams[index].plane;
                target.shapes.extend(plane.shapes.iter().cloned());
                target.edges.extend(plane.edges.iter().cloned());

                for shape in &plane.shapes {
                    let collapsed = shape.is_expanded != Some(true)
                        && defs
                            .get(&shape.element_id)
                            .is_some_and(|e| matches!(e.kind, ElementKind::SubProcess { .. }));
                    if collapsed {
                        defs.diagrams
                            .push(sub_process_diagram(&shape.element_id, Plane::default()));
                    }
                }
            }

            Command::Compound(commands) => {
                for command in commands {
                    if let Some(step) = command.execute(defs) {
                        undo.step(step);
                    }
                }
            }

            Command::ResizeShape { id, bounds } => {
                if defs.shape(id).is_none() {
                    return None;
                }

                undo.shape(defs, id);
                if let Some(shape) = shape_mut(defs, id) {
                    shape.bounds = *bounds;
                }

                for cid in incident(defs, id) {
                    reroute(defs, &cid, &mut undo);
                }
            }
        }

        (!undo.is_empty()).then_some(undo)
    }
}

/// Translates the label of `id` (shape or edge), materializing its default
/// bounds first; `None` when the element has no external label.
fn move_label(
    defs: &mut Definitions,
    id: &ElementId,
    dx: f64,
    dy: f64,
    undo: &mut Undo,
) -> Option<()> {
    let kind = defs.get(id)?.kind.clone();
    if !has_external_label(&kind) {
        return None;
    }

    if let Some(shape) = defs.shape(id) {
        let label = shape
            .label_bounds
            .unwrap_or_else(|| shape_label_bounds(&kind, &shape.bounds));
        undo.shape(defs, id);
        let shape = shape_mut(defs, id)?;
        let mut label = label;
        translate(&mut label, dx, dy);
        shape.label_bounds = Some(label);
        return Some(());
    }

    let edge = defs.edges().find(|e| &e.element_id == id)?;
    let mut label = edge
        .label_bounds
        .or_else(|| flow_label_bounds(&edge.waypoints))?;
    undo.edge(defs, id);
    translate(&mut label, dx, dy);
    edge_mut(defs, id)?.label_bounds = Some(label);
    Some(())
}

/// Moves `ids` plus their move closure, updates edges and reparents roots.
fn move_shapes(defs: &mut Definitions, ids: &[ElementId], dx: f64, dy: f64, undo: &mut Undo) {
    let moved: Vec<ElementId> = closure(defs, ids)
        .into_iter()
        .filter(|id| defs.shape(id).is_some())
        .collect();

    for id in &moved {
        undo.shape(defs, id);
        if let Some(shape) = shape_mut(defs, id) {
            translate(&mut shape.bounds, dx, dy);
            if let Some(label) = &mut shape.label_bounds {
                translate(label, dx, dy);
            }
        }
    }

    let connections: Vec<(ElementId, bool, bool)> = defs
        .elements
        .values()
        .filter_map(|e| {
            let (s, t) = e.kind.endpoints()?;
            Some((e.id.clone(), moved.contains(s), moved.contains(t)))
        })
        .filter(|(_, s, t)| *s || *t)
        .collect();

    for (cid, source_moved, target_moved) in connections {
        if source_moved && target_moved {
            undo.edge(defs, &cid);
            if let Some(edge) = defs
                .plane_mut()
                .edges
                .iter_mut()
                .find(|e| e.element_id == cid)
            {
                for wp in &mut edge.waypoints {
                    wp.x += dx;
                    wp.y += dy;
                }
                if let Some(label) = &mut edge.label_bounds {
                    translate(label, dx, dy);
                }
            }
        } else {
            repair_or_reroute(defs, &cid, source_moved, dx, dy, undo);
        }
    }

    for id in ids {
        reparent(defs, id, &moved, undo);
    }
}

/// Re-assigns the parent of a moved node to the topmost drop container under
/// its center (participant process, lane or expanded sub-process), falling
/// back to the plane's process; then fixes the parent of its sequence flows.
fn reparent(defs: &mut Definitions, id: &ElementId, moved: &[ElementId], undo: &mut Undo) {
    let Some(element) = defs.get(id) else {
        return;
    };

    let reparentable = !matches!(
        element.kind,
        ElementKind::Participant { .. }
            | ElementKind::Lane
            | ElementKind::BoundaryEvent { .. }
            | ElementKind::Group
            | ElementKind::Process
            | ElementKind::Collaboration
    ) && element.kind.endpoints().is_none();
    let Some(shape) = defs.shape(id) else {
        return;
    };
    if !reparentable {
        return;
    }

    let c = super::layout::center(&shape.bounds);
    let Some(parent) = container_at(defs, c, moved) else {
        return;
    };
    if element.parent.as_ref() == Some(&parent) {
        return;
    }

    undo.element(defs, id);
    if let Some(e) = defs.elements.get_mut(id) {
        e.parent = Some(parent);
    }

    for cid in incident(defs, id) {
        let Some((s, t)) = defs.get(&cid).and_then(|e| e.kind.endpoints()) else {
            continue;
        };
        let is_sequence = matches!(
            defs.get(&cid).map(|e| &e.kind),
            Some(ElementKind::SequenceFlow { .. })
        );
        let (so, to) = (owner(defs, s), owner(defs, t));
        if !is_sequence || so.is_none() || so != to {
            continue;
        }

        undo.element(defs, &cid);
        if let Some(e) = defs.elements.get_mut(&cid) {
            e.parent = so;
        }
    }
}

/// Sets the kind of `id`, drops event payloads that no longer apply and
/// deletes the connections the rules reject for the new kind.
fn replace(defs: &mut Definitions, id: &ElementId, kind: &ElementKind, undo: &mut Undo) {
    if !matches!(kind, ElementKind::SubProcess { .. })
        && let Some(index) = defs.plane_index(id)
    {
        drop_plane(defs, id, index, undo);
    }

    undo.element(defs, id);
    if let Some(element) = defs.elements.get_mut(id) {
        if event_definition(&element.kind) != event_definition(kind) {
            element.details.event_ref = None;
            element.details.timer = None;
            element.details.condition = None;
        }

        element.kind = kind.clone();
    }

    let rejected: Vec<ElementId> = incident(defs, id)
        .into_iter()
        .filter(|cid| {
            let Some(connection) = defs.get(cid) else {
                return false;
            };
            let (s, t) = connection
                .kind
                .endpoints()
                .expect("incident is a connection");
            !can_connect(defs, s, t).is_some_and(|c| is_connection(&connection.kind, c))
        })
        .collect();

    if !rejected.is_empty() {
        delete(defs, &rejected, undo);
    }
}

/// Executes `ToggleCollapse`; `None` when `id` is not a sub-process with a
/// shape on the active plane (or, expanding, has no plane).
fn toggle_collapse(defs: &mut Definitions, id: &ElementId, undo: &mut Undo) -> Option<()> {
    let kind = defs.get(id)?.kind.clone();
    if !matches!(kind, ElementKind::SubProcess { .. }) {
        return None;
    }

    let old = defs.shape(id)?.bounds;
    let expanding = defs.shape(id)?.is_expanded != Some(true);
    let index = defs.plane_index(id);
    if expanding && index.is_none() {
        return None;
    }

    undo.diagrams(defs);

    let (w, h) = kind.default_size(expanding)?;
    let c = super::layout::center(&old);
    let bounds = Bounds {
        x: c.x - w / 2.0,
        y: c.y - h / 2.0,
        w,
        h,
    };
    let shape = shape_mut(defs, id)?;
    shape.bounds = bounds;
    shape.is_expanded = Some(expanding);

    if let Some(index) = index.filter(|_| expanding) {
        let owner = defs.plane_owner().cloned();
        let mut content = defs.diagrams.remove(index).plane;
        defs.open_plane(owner.as_ref());

        if let Some(b) = content_bounds(&content) {
            let (from, to) = (super::layout::center(&b), c);
            translate_plane(&mut content, to.x - from.x, to.y - from.y);
        }

        let plane = defs.plane_mut();
        let at = plane
            .shapes
            .iter()
            .position(|s| &s.element_id == id)
            .map_or(plane.shapes.len(), |i| i + 1);
        plane.shapes.splice(at..at, content.shapes);
        plane.edges.extend(content.edges);
        plane.di_ids.extend(content.di_ids);
    } else {
        split_plane(defs, id);
    }

    let boundaries: Vec<(ElementId, Waypoint)> = defs
        .elements
        .values()
        .filter(|e| matches!(&e.kind, ElementKind::BoundaryEvent { attached_to, .. } if attached_to == id))
        .filter_map(|e| Some((e.id.clone(), super::layout::center(&defs.shape(&e.id)?.bounds))))
        .collect();
    for (event, p) in boundaries {
        let x = bounds.x + (p.x - old.x) / old.w * bounds.w;
        let y = bounds.y + (p.y - old.y) / old.h * bounds.h;
        move_shapes(defs, std::slice::from_ref(&event), x - p.x, y - p.y, undo);
    }

    for cid in incident(defs, id) {
        reroute(defs, &cid, undo);
    }

    Some(())
}

/// Event definition of an event kind (`None` for non-events).
fn event_definition(kind: &ElementKind) -> Option<crate::model::EventDefinition> {
    match kind {
        ElementKind::StartEvent(d)
        | ElementKind::EndEvent(d)
        | ElementKind::IntermediateCatchEvent(d)
        | ElementKind::IntermediateThrowEvent(d)
        | ElementKind::BoundaryEvent { definition: d, .. } => Some(*d),
        _ => None,
    }
}

/// Removes `ids`, their descendants and every connection attached to them.
/// Deleting a lane (bpmn-js `DeleteLaneBehavior`) removes its child lanes
/// but keeps its flow nodes (back to the process, then lane membership is
/// recomputed) and lets the adjacent lanes absorb its space: half each when
/// lanes remain on both sides, all of it otherwise.
fn delete(defs: &mut Definitions, ids: &[ElementId], undo: &mut Undo) {
    let is_lane = |defs: &Definitions, id: &ElementId| {
        defs.get(id).is_some_and(|e| e.kind == ElementKind::Lane)
    };
    let (lanes, others): (Vec<ElementId>, Vec<ElementId>) =
        ids.iter().cloned().partition(|id| is_lane(defs, id));

    let mut doomed = closure(defs, &others);
    let mut queue = lanes.clone();
    while let Some(id) = queue.pop() {
        if doomed.contains(&id) {
            continue;
        }

        queue.extend(
            defs.children(&id)
                .filter(|e| e.kind == ElementKind::Lane)
                .map(|e| e.id.clone()),
        );
        doomed.push(id);
    }
    doomed.retain(|id| defs.get(id).is_some());

    let spared: Vec<(ElementId, Option<ElementId>)> = defs
        .elements
        .values()
        .filter(|e| {
            !doomed.contains(&e.id)
                && e.parent
                    .as_ref()
                    .is_some_and(|p| doomed.contains(p) && is_lane(defs, p))
        })
        .map(|e| (e.id.clone(), owner(defs, &e.id)))
        .collect();

    let removed_lanes: Vec<(Option<ElementId>, Bounds)> = doomed
        .iter()
        .filter(|id| is_lane(defs, id))
        .filter(|id| {
            !defs
                .get(id)
                .and_then(|e| e.parent.as_ref())
                .is_some_and(|p| doomed.contains(p))
        })
        .filter_map(|id| Some((owner(defs, id), defs.shape(id)?.bounds)))
        .collect();

    let attached: Vec<ElementId> = doomed.iter().flat_map(|id| incident(defs, id)).collect();
    for cid in attached {
        if !doomed.contains(&cid) {
            doomed.push(cid);
        }
    }

    for id in &doomed {
        undo.all(defs, id);
    }

    if doomed.iter().any(|id| defs.plane_index(id).is_some()) {
        undo.diagrams(defs);
    }

    // Snapshot what the lane compensation may touch while indexes are intact.
    for (process, _) in &removed_lanes {
        let touched: Vec<ElementId> = defs
            .elements
            .values()
            .filter(|e| owner(defs, &e.id) == *process)
            .map(|e| e.id.clone())
            .collect();
        for id in &touched {
            undo.element(defs, id);
            undo.shape(defs, id);
        }
    }

    for id in &doomed {
        defs.remove_element(id);
    }

    for (id, parent) in spared {
        undo.element(defs, &id);
        if let Some(e) = defs.elements.get_mut(&id) {
            e.parent = parent;
        }
    }

    for (process, bounds) in removed_lanes {
        let Some(process) = process else {
            continue;
        };

        close_lane_gap(defs, &process, &bounds, undo);

        let participant = defs
            .elements
            .values()
            .find(|e| matches!(&e.kind, ElementKind::Participant { process_ref: Some(p) } if *p == process))
            .map(|e| e.id.clone());
        if let Some(participant) = participant {
            update_flow_node_refs(defs, &participant, undo);
        }
    }
}

/// Ids of the connections whose source or target is `id`.
fn incident(defs: &Definitions, id: &ElementId) -> Vec<ElementId> {
    defs.incoming(id)
        .chain(defs.outgoing(id))
        .map(|e| e.id.clone())
        .collect()
}

/// Flags the target activity of a new compensation association
/// (`isForCompensation`, bpmn-js `CompensateBoundaryEventBehavior`), which
/// is what the compensation layout keys on. Nothing clears the flag later.
fn mark_for_compensation(defs: &mut Definitions, kind: &ElementKind, undo: &mut Undo) {
    let ElementKind::Association { source, target } = kind else {
        return;
    };
    let compensate = defs.get(source).is_some_and(|e| {
        matches!(
            e.kind,
            ElementKind::BoundaryEvent {
                definition: crate::model::EventDefinition::Compensation,
                ..
            }
        )
    });
    if !compensate || defs.get(target).is_none() {
        return;
    }

    undo.element(defs, target);
    if let Some(element) = defs.elements.get_mut(target) {
        element.details.is_for_compensation = Some(true);
    }
}

/// Recomputes the waypoints of connection `cid` from its endpoint shapes.
fn reroute(defs: &mut Definitions, cid: &ElementId, undo: &mut Undo) {
    reroute_keeping(defs, cid, None, undo);
}

/// `reroute` that keeps the current docking point of one end (diagram-js
/// `preserveDocking`): the start when `keep_start` is `Some(true)`, the end
/// when `Some(false)`, none when `None`.
fn reroute_keeping(
    defs: &mut Definitions,
    cid: &ElementId,
    keep_start: Option<bool>,
    undo: &mut Undo,
) {
    let Some(waypoints) = route(defs, cid, keep_start) else {
        return;
    };
    let waypoints = orthogonal(defs, cid, &waypoints);

    undo.edge(defs, cid);
    if let Some(edge) = defs
        .plane_mut()
        .edges
        .iter_mut()
        .find(|e| &e.element_id == cid)
    {
        edge.waypoints = waypoints;
        edge.label_bounds = None;
    }
}

/// Updates connection `cid` after one of its ends moved by `(dx, dy)`:
/// keeps user bendpoints (`repair_end`) when possible, else re-routes
/// keeping the docking point of the end that did not move.
fn repair_or_reroute(
    defs: &mut Definitions,
    cid: &ElementId,
    source_moved: bool,
    dx: f64,
    dy: f64,
    undo: &mut Undo,
) {
    let end = defs
        .get(cid)
        .and_then(|e| e.kind.endpoints())
        .map(|(s, t)| if source_moved { s.clone() } else { t.clone() });
    let moved = end.and_then(|id| defs.shape(&id)).map(|s| s.bounds);
    let current = defs
        .edges()
        .find(|e| &e.element_id == cid)
        .map(|e| e.waypoints.clone());

    let repaired = moved
        .zip(current)
        .and_then(|(b, wps)| repair_end(&wps, source_moved, dx, dy, &b));
    let Some(waypoints) = repaired else {
        reroute_keeping(defs, cid, Some(!source_moved), undo);
        return;
    };
    let waypoints = orthogonal(defs, cid, &waypoints);

    undo.edge(defs, cid);
    if let Some(edge) = edge_mut(defs, cid) {
        edge.waypoints = waypoints;
        edge.label_bounds = None;
    }
}

/// `waypoints` of connection `cid` made orthogonal against its endpoint
/// shapes (`layout::orthogonalize`): the single choke point enforcing that
/// every stored connection uses only horizontal and vertical segments.
fn orthogonal(defs: &Definitions, cid: &ElementId, waypoints: &[Waypoint]) -> Vec<Waypoint> {
    let ends = defs.get(cid).and_then(|e| e.kind.endpoints());
    let shape = |id| defs.shape(id).map(|s| s.bounds);
    let (s, t) = (
        ends.and_then(|(s, _)| shape(s)),
        ends.and_then(|(_, t)| shape(t)),
    );

    orthogonalize(waypoints, s.as_ref(), t.as_ref())
}

/// Mutable DI edge of `id`.
fn edge_mut<'a>(defs: &'a mut Definitions, id: &ElementId) -> Option<&'a mut Edge> {
    defs.plane_mut()
        .edges
        .iter_mut()
        .find(|e| &e.element_id == id)
}

/// Executes `AttachBoundary`; `None` when `id` has no shape or is not an
/// event that can be attached/detached.
fn attach(
    defs: &mut Definitions,
    id: &ElementId,
    host: Option<&ElementId>,
    center: Waypoint,
    undo: &mut Undo,
) -> Option<()> {
    use crate::model::EventDefinition;

    let kind = defs.get(id)?.kind.clone();
    let bounds = defs.shape(id)?.bounds;
    let c = super::layout::center(&bounds);
    let definition = event_definition(&kind)?;

    let new_kind = match host {
        Some(h) => ElementKind::BoundaryEvent {
            definition,
            attached_to: h.clone(),
            cancel_activity: match &kind {
                ElementKind::BoundaryEvent {
                    cancel_activity, ..
                } => *cancel_activity,
                _ => true,
            },
        },
        None if definition == EventDefinition::None => {
            ElementKind::IntermediateThrowEvent(definition)
        }
        None => ElementKind::IntermediateCatchEvent(definition),
    };

    match host {
        Some(h) => {
            let parent = defs.get(h)?.parent.clone();
            move_shapes(
                defs,
                std::slice::from_ref(id),
                center.x - c.x,
                center.y - c.y,
                undo,
            );

            undo.element(defs, id);
            if let Some(e) = defs.elements.get_mut(id) {
                e.parent = parent;
            }
            if new_kind != kind {
                replace(defs, id, &new_kind, undo);
            }
        }

        None => {
            if new_kind != kind {
                replace(defs, id, &new_kind, undo);
            }
            move_shapes(
                defs,
                std::slice::from_ref(id),
                center.x - c.x,
                center.y - c.y,
                undo,
            );
        }
    }

    Some(())
}

/// Mutable DI shape of `id`.
pub(super) fn shape_mut<'a>(defs: &'a mut Definitions, id: &ElementId) -> Option<&'a mut Shape> {
    defs.plane_mut()
        .shapes
        .iter_mut()
        .find(|s| &s.element_id == id)
}

/// Shifts bounds by a delta.
pub(super) fn translate(b: &mut Bounds, dx: f64, dy: f64) {
    b.x += dx;
    b.y += dy;
}
