//! Builders for the commands issued by the palette, the context pad and the
//! replace menu, after bpmn-js `ElementFactory`, `AutoPlace` and
//! `ReplaceMenuProvider`. They read the model and return a `Command`;
//! executing it is the caller's job.

use super::command::Command;
use super::label::{has_external_label, shape_label_bounds};
use super::rules::{Connection, can_connect, container_at, is_connection, owner, plane_process};
use crate::model::{
    AssociationDirection, Bounds, Definitions, Edge, Element, ElementId, ElementKind,
    EventDefinition, Shape, Waypoint,
};

/// Horizontal gap between a source and a shape appended to it (bpmn-js autoPlace).
pub const APPEND_GAP: f64 = 50.0;

/// Vertical gap kept when an appended shape is shifted down past an obstacle.
pub const APPEND_MARGIN: f64 = 30.0;

/// Padding around existing content when the first pool wraps a process.
const POOL_PADDING: f64 = 30.0;

/// Width of a pool's name band.
const POOL_BAND: f64 = 30.0;

/// Returns `<prefix>_<n>` with the smallest `n >= 1` not used by any element.
pub fn new_id(defs: &Definitions, prefix: &str) -> ElementId {
    new_id_excluding(defs, prefix, &[])
}

/// Returns the first id unused by the model and the caller's reservations.
pub(super) fn new_id_excluding(defs: &Definitions, prefix: &str, taken: &[ElementId]) -> ElementId {
    (1..)
        .map(|n| ElementId(format!("{prefix}_{n}")))
        .find(|id| defs.get(id).is_none() && !taken.contains(id))
        .expect("unbounded range")
}

#[cfg(test)]
mod allocation_tests {
    use super::*;

    /// Existing and reserved ids are both skipped when allocating an id.
    #[test]
    fn skips_existing_and_reserved_ids() {
        let mut defs = crate::io::new_diagram();
        defs.add_element(Element::new("Activity_1", ElementKind::Task));
        let taken = [ElementId::from("Activity_2")];

        assert_eq!(
            new_id_excluding(&defs, "Activity", &taken),
            ElementId::from("Activity_3")
        );
    }
}

/// bpmn-js id prefix of a new element of `kind`.
pub fn id_prefix(kind: &ElementKind) -> &'static str {
    use ElementKind::*;

    match kind {
        Process => "Process",
        Collaboration => "Collaboration",
        StartEvent(_)
        | EndEvent(_)
        | IntermediateCatchEvent(_)
        | IntermediateThrowEvent(_)
        | BoundaryEvent { .. } => "Event",
        ExclusiveGateway | ParallelGateway | InclusiveGateway | EventBasedGateway
        | ComplexGateway => "Gateway",
        DataObjectReference => "DataObjectReference",
        DataStoreReference => "DataStoreReference",
        TextAnnotation { .. } => "TextAnnotation",
        Group => "Group",
        Participant { .. } => "Participant",
        Lane => "Lane",
        SequenceFlow { .. } | MessageFlow { .. } => "Flow",
        Association { .. } => "Association",
        DataInputAssociation { .. } => "DataInputAssociation",
        DataOutputAssociation { .. } => "DataOutputAssociation",
        _ => "Activity",
    }
}

/// Command creating a shape of `kind` (default size, expanded when
/// `expanded`) centered at `center`, inside the container under it.
///
/// A participant goes through `create_participant`. Returns `None` when
/// the kind has no shape or a flow node is dropped outside every pool of a
/// collaboration.
pub fn create_shape(
    defs: &Definitions,
    kind: ElementKind,
    expanded: bool,
    center: Waypoint,
) -> Option<Command> {
    if matches!(kind, ElementKind::Participant { .. }) {
        return create_participant(defs, center);
    }

    let (w, h) = kind.default_size(expanded)?;
    let bounds = Bounds {
        x: center.x - w / 2.0,
        y: center.y - h / 2.0,
        w,
        h,
    };

    let artifact = matches!(
        kind,
        ElementKind::TextAnnotation { .. } | ElementKind::Group
    );
    let parent = container_at(defs, center, &[])
        .or_else(|| artifact.then(|| defs.plane().element_id.clone()).flatten())?;

    let mut element = Element::new(new_id(defs, id_prefix(&kind)).as_str(), kind);
    element.parent = Some(parent);

    let is_expanded = matches!(element.kind, ElementKind::SubProcess { .. }).then_some(expanded);
    let shape = Shape {
        element_id: element.id.clone(),
        bounds,
        is_expanded,
        is_horizontal: None,
        label_bounds: external_label(&element.kind, &bounds),
    };

    Some(Command::CreateShape { element, shape })
}

/// Command creating a pool. On a process diagram the pool wraps the existing
/// content (or sits at `center` when empty) and the diagram becomes a
/// collaboration; in a collaboration a new pool with its own process is
/// centered at `center`. Sub-process planes take no pools (`None`).
pub fn create_participant(defs: &Definitions, center: Waypoint) -> Option<Command> {
    if defs.plane_owner().is_some() {
        return None;
    }

    let (w, h) = ElementKind::Participant { process_ref: None }.default_size(false)?;
    let at_center = Bounds {
        x: center.x - w / 2.0,
        y: center.y - h / 2.0,
        w,
        h,
    };

    let id = new_id(defs, "Participant");
    let shape = |bounds| Shape {
        element_id: id.clone(),
        bounds,
        is_expanded: None,
        is_horizontal: Some(true),
        label_bounds: None,
    };

    if let Some(process) = plane_process(defs) {
        let collaboration = Element::new(
            new_id(defs, "Collaboration").as_str(),
            ElementKind::Collaboration,
        );
        let participant = Element::new(
            id.as_str(),
            ElementKind::Participant {
                process_ref: Some(process),
            },
        )
        .with_parent(collaboration.id.as_str());
        let bounds = content_bounds(defs)
            .map(|b| Bounds {
                x: b.x - POOL_PADDING - POOL_BAND,
                y: b.y - POOL_PADDING,
                w: (b.w + 2.0 * POOL_PADDING + POOL_BAND).max(w),
                h: (b.h + 2.0 * POOL_PADDING).max(h),
            })
            .unwrap_or(at_center);

        return Some(Command::CreateParticipant {
            participant,
            shape: shape(bounds),
            process: None,
            collaboration: Some(collaboration),
        });
    }

    let collaboration = defs.plane().element_id.clone().filter(|c| {
        defs.get(c)
            .is_some_and(|e| e.kind == ElementKind::Collaboration)
    })?;
    let process = Element::new(new_id(defs, "Process").as_str(), ElementKind::Process);
    let participant = Element::new(
        id.as_str(),
        ElementKind::Participant {
            process_ref: Some(process.id.clone()),
        },
    )
    .with_parent(collaboration.as_str());

    Some(Command::CreateParticipant {
        participant,
        shape: shape(at_center),
        process: Some(process),
        collaboration: None,
    })
}

/// Bounding box of every DI shape, or `None` when the plane is empty.
fn content_bounds(defs: &Definitions) -> Option<Bounds> {
    defs.shapes().map(|s| s.bounds).reduce(|a, b| {
        let (x, y) = (a.x.min(b.x), a.y.min(b.y));
        Bounds {
            x,
            y,
            w: (a.x + a.w).max(b.x + b.w) - x,
            h: (a.y + a.h).max(b.y + b.h) - y,
        }
    })
}

/// Command connecting `source` to `target` with the connection the rules
/// allow (auto-routed), or `None` when none is allowed.
pub fn connect(defs: &Definitions, source: &ElementId, target: &ElementId) -> Option<Command> {
    let rule = can_connect(defs, source, target)?;
    let (s, t) = (source.clone(), target.clone());

    let kind = match rule {
        Connection::Sequence => ElementKind::SequenceFlow {
            source: s,
            target: t,
            is_default: false,
            condition: None,
        },
        Connection::Message => ElementKind::MessageFlow {
            source: s,
            target: t,
        },
        Connection::Association | Connection::Compensation => ElementKind::Association {
            source: s,
            target: t,
        },
        Connection::DataInput => ElementKind::DataInputAssociation {
            source: s,
            target: t,
        },
        Connection::DataOutput => ElementKind::DataOutputAssociation {
            source: s,
            target: t,
        },
    };

    let parent = match rule {
        Connection::Sequence => owner(defs, source),
        Connection::Message => defs.plane().element_id.clone(),
        Connection::DataInput => owner(defs, target),
        Connection::DataOutput | Connection::Compensation => owner(defs, source),
        Connection::Association => {
            let (so, to) = (owner(defs, source), owner(defs, target));
            if so.is_some() && so == to {
                so
            } else {
                defs.plane().element_id.clone()
            }
        }
    };

    let mut element = Element::new(new_id(defs, id_prefix(&kind)).as_str(), kind);
    element.parent = parent;
    if rule == Connection::Compensation {
        element.details.association_direction = Some(AssociationDirection::One);
    }

    let edge = Edge {
        element_id: element.id.clone(),
        waypoints: Vec::new(),
        label_bounds: None,
    };

    Some(Command::CreateConnection { element, edge })
}

/// Bounds of a shape of size `(w, h)` appended to `source` (bpmn-js
/// autoPlace): text annotations go up-right; flow nodes go `APPEND_GAP`
/// right of it, vertically centered, shifted down below every overlapped
/// node until free.
pub fn append_bounds(
    defs: &Definitions,
    source: &Bounds,
    w: f64,
    h: f64,
    annotation: bool,
) -> Bounds {
    if annotation {
        return Bounds {
            x: source.x + source.w + APPEND_GAP,
            y: source.y - APPEND_GAP - h,
            w,
            h,
        };
    }

    let mut b = Bounds {
        x: source.x + source.w + APPEND_GAP,
        y: source.y + source.h / 2.0 - h / 2.0,
        w,
        h,
    };

    let obstacles: Vec<Bounds> = defs
        .shapes()
        .filter(|s| {
            defs.get(&s.element_id)
                .is_some_and(|e| super::rules::is_flow_node(&e.kind) || is_artifact_node(&e.kind))
                && s.is_expanded != Some(true)
        })
        .map(|s| s.bounds)
        .collect();

    while let Some(o) = obstacles.iter().find(|o| overlaps(o, &b)) {
        b.y = o.y + o.h + APPEND_MARGIN;
    }

    b
}

/// Whether `kind` is a small non-flow-node shape that blocks autoPlace.
fn is_artifact_node(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::DataObjectReference
            | ElementKind::DataStoreReference
            | ElementKind::TextAnnotation { .. }
    )
}

/// Default external label bounds of a new `kind` shape at `bounds` (bpmn-js
/// `ElementFactory` creates the label with the shape); `None` for kinds
/// labeled inside.
fn external_label(kind: &ElementKind, bounds: &Bounds) -> Option<Bounds> {
    has_external_label(kind).then(|| shape_label_bounds(kind, bounds))
}

/// Whether two bounds intersect with positive area.
fn overlaps(a: &Bounds, b: &Bounds) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// One-step command appending a new `kind` shape to `source` and connecting
/// them (sequence flow, or association for a text annotation). `None` when
/// the source has no shape or the rules reject the connection.
pub fn append(defs: &Definitions, source: &ElementId, kind: ElementKind) -> Option<Command> {
    let source_element = defs.get(source)?;
    let sb = defs.shape(source)?.bounds;
    let (w, h) = kind.default_size(false)?;
    let annotation = matches!(kind, ElementKind::TextAnnotation { .. });
    let bounds = append_bounds(defs, &sb, w, h, annotation);

    let mut element = Element::new(new_id(defs, id_prefix(&kind)).as_str(), kind);
    element.parent = if annotation {
        owner(defs, source).or_else(|| defs.plane().element_id.clone())
    } else {
        source_element.parent.clone()
    };
    let shape = Shape {
        element_id: element.id.clone(),
        bounds,
        is_expanded: None,
        is_horizontal: None,
        label_bounds: external_label(&element.kind, &bounds),
    };

    let (from, to) = (source.clone(), element.id.clone());
    let create = Command::CreateShape { element, shape };
    let mut after = defs.clone();
    create.execute(&mut after)?;
    let link = connect(&after, &from, &to)?;

    Some(Command::Compound(vec![create, link]))
}

/// Element kinds offered by the replace menu for `kind`, current one
/// excluded, after bpmn-js `ReplaceOptions`. Boundary events keep their host
/// and interrupting flag.
pub fn replace_options(kind: &ElementKind) -> Vec<ElementKind> {
    use ElementKind::*;
    use EventDefinition as D;

    let options: Vec<ElementKind> = match kind {
        StartEvent(_) => vec![
            StartEvent(D::None),
            IntermediateThrowEvent(D::None),
            EndEvent(D::None),
            StartEvent(D::Message),
            StartEvent(D::Timer),
            StartEvent(D::Conditional),
            StartEvent(D::Signal),
        ],

        IntermediateCatchEvent(_) | IntermediateThrowEvent(_) => vec![
            StartEvent(D::None),
            IntermediateThrowEvent(D::None),
            EndEvent(D::None),
            IntermediateCatchEvent(D::Message),
            IntermediateThrowEvent(D::Message),
            IntermediateCatchEvent(D::Timer),
            IntermediateThrowEvent(D::Escalation),
            IntermediateCatchEvent(D::Conditional),
            IntermediateCatchEvent(D::Link),
            IntermediateThrowEvent(D::Link),
            IntermediateThrowEvent(D::Compensation),
            IntermediateCatchEvent(D::Signal),
            IntermediateThrowEvent(D::Signal),
        ],

        EndEvent(_) => vec![
            StartEvent(D::None),
            IntermediateThrowEvent(D::None),
            EndEvent(D::None),
            EndEvent(D::Message),
            EndEvent(D::Escalation),
            EndEvent(D::Error),
            EndEvent(D::Compensation),
            EndEvent(D::Signal),
            EndEvent(D::Terminate),
        ],

        BoundaryEvent {
            attached_to,
            cancel_activity,
            ..
        } => [
            D::Message,
            D::Timer,
            D::Escalation,
            D::Conditional,
            D::Error,
            D::Signal,
            D::Compensation,
        ]
        .into_iter()
        .map(|definition| BoundaryEvent {
            definition,
            attached_to: attached_to.clone(),
            cancel_activity: *cancel_activity,
        })
        .collect(),

        Task | UserTask | ServiceTask | ScriptTask | SendTask | ReceiveTask | ManualTask
        | BusinessRuleTask | CallActivity => vec![
            Task,
            SendTask,
            ReceiveTask,
            UserTask,
            ManualTask,
            BusinessRuleTask,
            ServiceTask,
            ScriptTask,
            CallActivity,
            SubProcess {
                triggered_by_event: false,
            },
        ],

        SubProcess { triggered_by_event } => vec![
            SubProcess {
                triggered_by_event: !*triggered_by_event,
            },
            Task,
        ],

        ExclusiveGateway | ParallelGateway | InclusiveGateway | EventBasedGateway
        | ComplexGateway => vec![
            ExclusiveGateway,
            ParallelGateway,
            InclusiveGateway,
            ComplexGateway,
            EventBasedGateway,
        ],

        DataObjectReference => vec![DataStoreReference],
        DataStoreReference => vec![DataObjectReference],

        _ => Vec::new(),
    };

    options.into_iter().filter(|o| o != kind).collect()
}

/// Human label of a kind for menus ("Message Start Event", "User Task").
pub fn kind_label(kind: &ElementKind) -> String {
    use ElementKind::*;

    let event = |prefix: &str, def: &EventDefinition, base: &str| {
        let def = match def {
            EventDefinition::None => "",
            EventDefinition::Message => "Message ",
            EventDefinition::Timer => "Timer ",
            EventDefinition::Signal => "Signal ",
            EventDefinition::Error => "Error ",
            EventDefinition::Escalation => "Escalation ",
            EventDefinition::Conditional => "Conditional ",
            EventDefinition::Link => "Link ",
            EventDefinition::Compensation => "Compensation ",
            EventDefinition::Terminate => "Terminate ",
        };
        format!("{def}{prefix}{base}")
    };

    match kind {
        StartEvent(d) => event("Start ", d, "Event"),
        EndEvent(d) => event("End ", d, "Event"),
        IntermediateCatchEvent(d) => event("Intermediate Catch ", d, "Event"),
        IntermediateThrowEvent(d) => event("Intermediate Throw ", d, "Event"),
        BoundaryEvent {
            definition,
            cancel_activity,
            ..
        } => {
            let prefix = if *cancel_activity {
                "Boundary "
            } else {
                "Non-interrupting Boundary "
            };
            event(prefix, definition, "Event")
        }
        Task => "Task".into(),
        UserTask => "User Task".into(),
        ServiceTask => "Service Task".into(),
        ScriptTask => "Script Task".into(),
        SendTask => "Send Task".into(),
        ReceiveTask => "Receive Task".into(),
        ManualTask => "Manual Task".into(),
        BusinessRuleTask => "Business Rule Task".into(),
        CallActivity => "Call Activity".into(),
        SubProcess {
            triggered_by_event: true,
        } => "Event Sub-Process".into(),
        SubProcess { .. } => "Sub-Process".into(),
        ExclusiveGateway => "Exclusive Gateway".into(),
        ParallelGateway => "Parallel Gateway".into(),
        InclusiveGateway => "Inclusive Gateway".into(),
        EventBasedGateway => "Event-based Gateway".into(),
        ComplexGateway => "Complex Gateway".into(),
        DataObjectReference => "Data Object Reference".into(),
        DataStoreReference => "Data Store Reference".into(),
        TextAnnotation { .. } => "Text Annotation".into(),
        Group => "Group".into(),
        Participant { .. } => "Pool".into(),
        Lane => "Lane".into(),
        _ => format!("{kind:?}"),
    }
}

/// Command for the space tool (bpmn-js `SpaceTool`, simplified): moves every
/// shape whose left edge (horizontal drag) or top edge (vertical drag) lies
/// beyond the line through `start` by the dominant drag component. Lanes and
/// boundary events follow their containers/hosts; containers crossing the
/// line are not resized.
pub fn make_space(defs: &Definitions, start: Waypoint, dx: f64, dy: f64) -> Option<Command> {
    let horizontal = dx.abs() >= dy.abs();
    let (dx, dy) = if horizontal { (dx, 0.0) } else { (0.0, dy) };
    if dx == 0.0 && dy == 0.0 {
        return None;
    }

    let ids: Vec<ElementId> = defs
        .shapes()
        .filter(|s| {
            !matches!(
                defs.get(&s.element_id).map(|e| &e.kind),
                Some(ElementKind::Lane | ElementKind::BoundaryEvent { .. }) | None
            )
        })
        .filter(|s| {
            if horizontal {
                s.bounds.x >= start.x
            } else {
                s.bounds.y >= start.y
            }
        })
        .map(|s| s.element_id.clone())
        .collect();

    (!ids.is_empty()).then_some(Command::MoveShapes { ids, dx, dy })
}

/// Command moving one end of connection `id` (its source when `start`, else
/// its target) onto `to`, when the rules allow a connection of the same
/// kind between the new ends (bpmn-js `connection.reconnect` rule).
pub fn reconnect(
    defs: &Definitions,
    id: &ElementId,
    start: bool,
    to: &ElementId,
) -> Option<Command> {
    let element = defs.get(id)?;
    let (s, t) = element.kind.endpoints()?;
    let (source, target) = if start {
        (to.clone(), t.clone())
    } else {
        (s.clone(), to.clone())
    };

    let rule = can_connect(defs, &source, &target)?;
    if !is_connection(&element.kind, rule) {
        return None;
    }

    Some(Command::Reconnect {
        id: id.clone(),
        source,
        target,
    })
}
