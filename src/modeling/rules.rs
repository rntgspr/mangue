//! Modeling rules: which connection may join two elements and which container
//! receives a dropped shape, after bpmn-js `BpmnRules`
//! (https://github.com/bpmn-io/bpmn-js/blob/develop/lib/features/rules/BpmnRules.js).

use crate::model::{Bounds, Definitions, ElementId, ElementKind, EventDefinition, Waypoint};

/// Kind of connection the rules allow between a source and a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Sequence,
    Message,
    Association,
    DataInput,
    DataOutput,
    /// Association from a compensation boundary event to its compensation activity.
    Compensation,
}

/// Returns the connection allowed from `source` to `target`, if any
/// (bpmn-js `BpmnRules.canConnect`).
///
/// - one end a text annotation: association;
/// - compensation boundary event to another activity in its scope:
///   compensation association (`associationDirection="One"`);
/// - data object/store to activity or throw event: data input; activity or
///   catch event to data: data output;
/// - ends in different participants: message flow, from a participant,
///   activity, end or intermediate throw event to a participant, activity,
///   start, intermediate catch or boundary event;
/// - flow nodes in the same scope: sequence flow, never out of an end event,
///   a link throw, a compensation boundary or an event sub-process, nor into
///   a start or boundary event, a link catch or an event sub-process; out of
///   an event-based gateway only into receive tasks and message, timer,
///   signal or conditional intermediate catch events.
///
/// Self-connections are only allowed as sequence flows (loops).
pub fn can_connect(
    defs: &Definitions,
    source: &ElementId,
    target: &ElementId,
) -> Option<Connection> {
    let (s, t) = (&defs.get(source)?.kind, &defs.get(target)?.kind);

    if source == target {
        return (is_sequence_source(s) && is_sequence_target(t)).then_some(Connection::Sequence);
    }

    if is_annotation(s) || is_annotation(t) {
        let connectable = |k: &ElementKind| k.endpoints().is_none() && !is_root(k);
        return (connectable(s) && connectable(t)).then_some(Connection::Association);
    }

    if is_data(s) || is_data(t) {
        return match (is_data(s), is_data(t)) {
            (true, false) if is_activity(t) || is_throw_event(t) => Some(Connection::DataInput),
            (false, true) if is_activity(s) || is_catch_event(s) => Some(Connection::DataOutput),
            _ => None,
        };
    }

    if is_compensation_boundary(s) {
        let host =
            matches!(s, ElementKind::BoundaryEvent { attached_to, .. } if attached_to == target);
        let ok = is_activity(t)
            && !is_event_sub_process(t)
            && !host
            && owner(defs, source).is_some()
            && owner(defs, source) == owner(defs, target);
        return ok.then_some(Connection::Compensation);
    }

    let (sp, tp) = (participant_of(defs, source), participant_of(defs, target));
    if sp.is_some() && tp.is_some() && sp != tp {
        return (is_message_source(s) && is_message_target(t)).then_some(Connection::Message);
    }

    let event_based = *s != ElementKind::EventBasedGateway || is_event_based_target(t);
    let sequence = is_sequence_source(s)
        && is_sequence_target(t)
        && event_based
        && owner(defs, source).is_some()
        && owner(defs, source) == owner(defs, target);

    sequence.then_some(Connection::Sequence)
}

/// Whether a sequence flow may start at `kind` (bpmn-js `isSequenceFlowSource`).
fn is_sequence_source(kind: &ElementKind) -> bool {
    is_flow_node(kind)
        && !matches!(
            kind,
            ElementKind::EndEvent(_) | ElementKind::IntermediateThrowEvent(EventDefinition::Link)
        )
        && !is_event_sub_process(kind)
        && !is_compensation_boundary(kind)
}

/// Whether a sequence flow may end at `kind` (bpmn-js `isSequenceFlowTarget`).
fn is_sequence_target(kind: &ElementKind) -> bool {
    is_flow_node(kind)
        && !matches!(
            kind,
            ElementKind::StartEvent(_)
                | ElementKind::BoundaryEvent { .. }
                | ElementKind::IntermediateCatchEvent(EventDefinition::Link)
        )
        && !is_event_sub_process(kind)
}

/// Whether `kind` may follow an event-based gateway: a receive task or a
/// message, timer, signal or conditional intermediate catch event.
pub fn is_event_based_target(kind: &ElementKind) -> bool {
    use EventDefinition::*;

    matches!(
        kind,
        ElementKind::ReceiveTask
            | ElementKind::IntermediateCatchEvent(Message | Timer | Signal | Conditional)
    )
}

/// Whether `kind` is an event sub-process (`triggeredByEvent`).
fn is_event_sub_process(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::SubProcess {
            triggered_by_event: true
        }
    )
}

/// Whether `kind` is a compensation boundary event.
fn is_compensation_boundary(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::BoundaryEvent {
            definition: EventDefinition::Compensation,
            ..
        }
    )
}

/// Whether `kind` is a BPMN throw event (end or intermediate throw).
fn is_throw_event(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::EndEvent(_) | ElementKind::IntermediateThrowEvent(_)
    )
}

/// Whether `kind` is a BPMN catch event (start, intermediate catch, boundary).
fn is_catch_event(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::StartEvent(_)
            | ElementKind::IntermediateCatchEvent(_)
            | ElementKind::BoundaryEvent { .. }
    )
}

/// Whether `event` may be attached to `host` as a boundary event when dropped
/// at `point` (bpmn-js `BpmnRules.canAttach`): boundary events, plain
/// intermediate throw events and message/timer/signal/conditional catch
/// events attach to the border of an activity that is not an event
/// sub-process nor a receive task following an event-based gateway.
pub fn can_attach(
    defs: &Definitions,
    event: &ElementId,
    host: &ElementId,
    point: Waypoint,
) -> bool {
    use EventDefinition::*;

    let (Some(e), Some(h)) = (defs.get(event), defs.get(host)) else {
        return false;
    };

    let candidate = matches!(
        e.kind,
        ElementKind::BoundaryEvent { .. }
            | ElementKind::IntermediateThrowEvent(None)
            | ElementKind::IntermediateCatchEvent(Message | Timer | Signal | Conditional)
    );
    if !candidate || event == host || !is_activity(&h.kind) || is_event_sub_process(&h.kind) {
        return false;
    }

    let after_event_based = h.kind == ElementKind::ReceiveTask
        && defs.incoming(host).any(|c| {
            c.kind
                .endpoints()
                .and_then(|(s, _)| defs.get(s))
                .is_some_and(|s| s.kind == ElementKind::EventBasedGateway)
        });
    if after_event_based {
        return false;
    }

    defs.shape(host)
        .is_some_and(|s| boundary_side(point, &s.bounds).is_some())
}

pub(super) const ATTACH_PADDING: f64 = 15.0;

/// Border side(s) of `host` a point attaches to (bpmn-js
/// `getBoundaryAttachment`, `getOrientation` with -15 padding): `(dx, dy)`
/// with `-1` left/top, `1` right/bottom, `0` none; `None` when the point is
/// deeper than 15px inside the bounds.
pub fn boundary_side(point: Waypoint, host: &Bounds) -> Option<(i8, i8)> {
    let dx = if point.x <= host.x + ATTACH_PADDING {
        -1
    } else if point.x >= host.x + host.w - ATTACH_PADDING {
        1
    } else {
        0
    };
    let dy = if point.y <= host.y + ATTACH_PADDING {
        -1
    } else if point.y >= host.y + host.h - ATTACH_PADDING {
        1
    } else {
        0
    };

    (dx != 0 || dy != 0).then_some((dx, dy))
}

/// Whether the shape of `kind` may be resized (bpmn-js `BpmnRules.canResize`):
/// expanded sub-processes, participants, lanes, text annotations and groups.
pub fn can_resize(kind: &ElementKind, expanded: bool) -> bool {
    match kind {
        ElementKind::SubProcess { .. } => expanded,
        ElementKind::Participant { .. }
        | ElementKind::Lane
        | ElementKind::TextAnnotation { .. }
        | ElementKind::Group => true,
        _ => false,
    }
}

/// Minimum `(width, height)` of a resized shape (bpmn-js `ResizeBehavior`):
/// sub-process and group 140x120, participant 300x150, lane 300x60, text
/// annotation 50x30; `None` when the kind is not resizable.
pub fn min_size(kind: &ElementKind, expanded: bool) -> Option<(f64, f64)> {
    if !can_resize(kind, expanded) {
        return None;
    }

    Some(match kind {
        ElementKind::Participant { .. } => (300.0, 150.0),
        ElementKind::Lane => (300.0, 60.0),
        ElementKind::TextAnnotation { .. } => (50.0, 30.0),
        _ => (140.0, 120.0),
    })
}

/// Returns whether `connection` is of the kind `allowed`.
pub fn is_connection(kind: &ElementKind, allowed: Connection) -> bool {
    matches!(
        (kind, allowed),
        (ElementKind::SequenceFlow { .. }, Connection::Sequence)
            | (ElementKind::MessageFlow { .. }, Connection::Message)
            | (
                ElementKind::Association { .. },
                Connection::Association | Connection::Compensation
            )
            | (
                ElementKind::DataInputAssociation { .. },
                Connection::DataInput
            )
            | (
                ElementKind::DataOutputAssociation { .. },
                Connection::DataOutput
            )
    )
}

/// Nearest non-lane ancestor (the process or sub-process owning a node).
pub fn owner(defs: &Definitions, id: &ElementId) -> Option<ElementId> {
    let mut parent = defs.get(id)?.parent.clone();

    while let Some(p) = parent.clone() {
        match defs.get(&p) {
            Some(e) if e.kind == ElementKind::Lane => parent = e.parent.clone(),
            _ => break,
        }
    }

    parent
}

/// The participant `id` is, or whose process (transitively) contains `id`.
pub fn participant_of(defs: &Definitions, id: &ElementId) -> Option<ElementId> {
    let mut current = defs.get(id)?;

    loop {
        match &current.kind {
            ElementKind::Participant { .. } => return Some(current.id.clone()),

            ElementKind::Process => {
                return defs
                    .elements
                    .values()
                    .find(|e| {
                        matches!(&e.kind, ElementKind::Participant { process_ref: Some(p) } if *p == current.id)
                    })
                    .map(|e| e.id.clone());
            }

            _ => current = defs.get(current.parent.as_ref()?)?,
        }
    }
}

/// Container receiving a shape dropped at diagram point `p`: the topmost
/// lane, expanded sub-process or participant (its process) under `p`,
/// ignoring `exclude`, falling back to the plane's process.
pub fn container_at(defs: &Definitions, p: Waypoint, exclude: &[ElementId]) -> Option<ElementId> {
    let hit = defs
        .shapes()
        .filter(|s| !exclude.contains(&s.element_id))
        .filter(|s| {
            let b = &s.bounds;
            p.x >= b.x && p.x <= b.x + b.w && p.y >= b.y && p.y <= b.y + b.h
        })
        .filter_map(|s| match &defs.get(&s.element_id)?.kind {
            ElementKind::Participant { process_ref } => process_ref.clone(),
            ElementKind::Lane => Some(s.element_id.clone()),
            ElementKind::SubProcess { .. } if s.is_expanded == Some(true) => {
                Some(s.element_id.clone())
            }
            _ => None,
        })
        .last();

    hit.or_else(|| plane_process(defs))
}

/// The active plane's bound element when it is a process (not a
/// collaboration) or, drilled down, a collapsed sub-process.
pub fn plane_process(defs: &Definitions) -> Option<ElementId> {
    defs.plane().element_id.clone().filter(|p| {
        defs.get(p).is_some_and(|e| {
            matches!(
                e.kind,
                ElementKind::Process | ElementKind::SubProcess { .. }
            )
        })
    })
}

/// Whether `kind` is a BPMN flow node (event, activity or gateway).
pub fn is_flow_node(kind: &ElementKind) -> bool {
    use ElementKind::*;

    is_activity(kind)
        || matches!(
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
        )
}

/// Whether `kind` is an activity (task, sub-process or call activity).
pub fn is_activity(kind: &ElementKind) -> bool {
    use ElementKind::*;

    matches!(
        kind,
        Task | UserTask
            | ServiceTask
            | ScriptTask
            | SendTask
            | ReceiveTask
            | ManualTask
            | BusinessRuleTask
            | CallActivity
            | SubProcess { .. }
    )
}

/// Whether `kind` is a text annotation.
fn is_annotation(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::TextAnnotation { .. })
}

/// Whether `kind` is a data object or data store reference.
fn is_data(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::DataObjectReference | ElementKind::DataStoreReference
    )
}

/// Whether `kind` is a process or collaboration root.
fn is_root(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::Process | ElementKind::Collaboration)
}

/// Whether a message flow may start at `kind`.
fn is_message_source(kind: &ElementKind) -> bool {
    is_activity(kind)
        || matches!(
            kind,
            ElementKind::Participant { .. }
                | ElementKind::EndEvent(_)
                | ElementKind::IntermediateThrowEvent(_)
        )
}

/// Whether a message flow may end at `kind`.
fn is_message_target(kind: &ElementKind) -> bool {
    is_activity(kind)
        || matches!(
            kind,
            ElementKind::Participant { .. }
                | ElementKind::StartEvent(_)
                | ElementKind::IntermediateCatchEvent(_)
                | ElementKind::BoundaryEvent { .. }
        )
}

/// Why a gesture was refused, shown as feedback (bpmn-js
/// `ModelingFeedback`, which only words the first case; the others are
/// mangue's own wording).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A flow node dropped on the collaboration instead of a participant.
    OutsideParticipant,
    /// A connection the rules do not allow between the two elements.
    NotConnectable,
    /// A boundary event dropped on a border it may not attach to.
    NotAttachable,
}

impl Refusal {
    /// Text of the error tooltip.
    pub fn message(self) -> &'static str {
        match self {
            Refusal::OutsideParticipant => "flow elements must be children of pools/participants",
            Refusal::NotConnectable => "these elements cannot be connected",
            Refusal::NotAttachable => "boundary events cannot attach to this element",
        }
    }
}

/// Refusal of creating a shape of `kind` centered at `center`: a flow node
/// has no container there (diagram-js `create.rejected` on a collaboration).
pub fn create_refusal(defs: &Definitions, kind: &ElementKind, center: Waypoint) -> Option<Refusal> {
    (is_flow_node(kind) && container_at(defs, center, &[]).is_none())
        .then_some(Refusal::OutsideParticipant)
}

/// Refusal of moving `ids` by `(dx, dy)`: a moved flow node (not one that
/// travels inside a moved container) whose center would land outside every
/// container (diagram-js `shape.move.rejected`), or a single boundary event
/// dropped on the border of an activity it cannot attach to.
pub fn drop_refusal(defs: &Definitions, ids: &[ElementId], dx: f64, dy: f64) -> Option<Refusal> {
    let inside_moved = |id: &ElementId| {
        if participant_of(defs, id).is_some_and(|p| ids.contains(&p)) {
            return true;
        }

        let mut current = defs.get(id).and_then(|e| e.parent.clone());
        while let Some(parent) = current {
            if ids.contains(&parent) {
                return true;
            }
            current = defs.get(&parent).and_then(|e| e.parent.clone());
        }
        false
    };
    let moved_center = |id: &ElementId| {
        let c = super::layout::center(&defs.shape(id)?.bounds);
        Some(Waypoint {
            x: c.x + dx,
            y: c.y + dy,
        })
    };

    for id in ids.iter().filter(|id| !inside_moved(id)) {
        let (Some(element), Some(p)) = (defs.get(id), moved_center(id)) else {
            continue;
        };
        if !is_flow_node(&element.kind) || matches!(element.kind, ElementKind::BoundaryEvent { .. })
        {
            continue;
        }

        if container_at(defs, p, ids).is_none() {
            return Some(Refusal::OutsideParticipant);
        }
    }

    let [id] = ids else {
        return None;
    };
    if !matches!(
        defs.get(id).map(|e| &e.kind),
        Some(ElementKind::BoundaryEvent { .. })
    ) {
        return None;
    }

    let p = moved_center(id)?;
    let pad = ATTACH_PADDING;
    let refused = defs.shapes().any(|s| {
        let b = &s.bounds;
        s.element_id != *id
            && defs
                .get(&s.element_id)
                .is_some_and(|e| is_activity(&e.kind))
            && p.x >= b.x - pad
            && p.x <= b.x + b.w + pad
            && p.y >= b.y - pad
            && p.y <= b.y + b.h + pad
            && boundary_side(p, b).is_some()
            && !can_attach(defs, id, &s.element_id, p)
    });

    refused.then_some(Refusal::NotAttachable)
}

/// Refusal of connecting `source` to the shape `target`: the rules allow no
/// connection between them.
pub fn connect_refusal(
    defs: &Definitions,
    source: &ElementId,
    target: &ElementId,
) -> Option<Refusal> {
    can_connect(defs, source, target)
        .is_none()
        .then_some(Refusal::NotConnectable)
}

/// Refuses reconnecting a connection to a target rejected by the factory.
pub fn reconnect_refusal(
    defs: &Definitions,
    id: &ElementId,
    start: bool,
    to: &ElementId,
) -> Option<Refusal> {
    super::factory::reconnect(defs, id, start, to)
        .is_none()
        .then_some(Refusal::NotConnectable)
}

/// Returns whether a kind may start a connection from its context pad.
pub fn can_start_connection(kind: &ElementKind) -> bool {
    !matches!(kind, ElementKind::Lane | ElementKind::Group)
}

/// `ids` plus, transitively, their children (a participant's are its
/// process's) and the boundary events attached to them.
pub(super) fn closure(defs: &Definitions, ids: &[ElementId]) -> Vec<ElementId> {
    let mut out: Vec<ElementId> = Vec::new();
    let mut queue: Vec<ElementId> = ids.to_vec();

    while let Some(id) = queue.pop() {
        if out.contains(&id) {
            continue;
        }

        let mut parents = vec![id.clone()];
        if let Some(ElementKind::Participant {
            process_ref: Some(p),
        }) = defs.get(&id).map(|e| &e.kind)
        {
            parents.push(p.clone());
        }

        for e in defs.elements.values() {
            let child = e.parent.as_ref().is_some_and(|p| parents.contains(p));
            let attached = matches!(&e.kind, ElementKind::BoundaryEvent { attached_to, .. } if *attached_to == id);
            if child || attached {
                queue.push(e.id.clone());
            }
        }

        out.push(id);
    }

    out
}

/// Whether the local modeling direction of element `id` is horizontal
/// (bpmn-js `isDirectionHorizontal`): false inside a participant or lane
/// whose DI says `isHorizontal="false"`, and for such a pool or lane itself;
/// a process referenced by a participant takes that participant's direction.
pub fn is_direction_horizontal(defs: &Definitions, id: &ElementId) -> bool {
    let is_pool =
        |kind: &ElementKind| matches!(kind, ElementKind::Participant { .. } | ElementKind::Lane);
    let horizontal = |id: &ElementId| {
        defs.shape(id)
            .is_none_or(|s| s.is_horizontal != Some(false))
    };

    let Some(element) = defs.get(id) else {
        return true;
    };
    if is_pool(&element.kind) {
        return horizontal(id);
    }

    let mut current = element.parent.clone();
    let mut process = None;
    while let Some(parent) = current {
        let Some(owner) = defs.get(&parent) else {
            break;
        };
        if is_pool(&owner.kind) {
            return horizontal(&parent);
        }
        if matches!(owner.kind, ElementKind::Process) {
            process = Some(parent);
        }

        current = owner.parent.clone();
    }

    process
        .and_then(|process| {
            defs.elements.values().find(|e| {
                matches!(&e.kind, ElementKind::Participant { process_ref: Some(p) } if *p == process)
            })
        })
        .is_none_or(|pool| horizontal(&pool.id))
}
