use super::id::ElementId;

/// Event definition carried by an event (`None` means a plain event).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDefinition {
    None,
    Message,
    Timer,
    Signal,
    Error,
    Escalation,
    Conditional,
    Link,
    Compensation,
    Terminate,
}

/// Semantic type of a BPMN element, with its type-specific attributes.
#[derive(Debug, Clone, PartialEq)]
pub enum ElementKind {
    Process,
    Collaboration,

    StartEvent(EventDefinition),
    EndEvent(EventDefinition),
    IntermediateCatchEvent(EventDefinition),
    IntermediateThrowEvent(EventDefinition),
    BoundaryEvent {
        definition: EventDefinition,
        attached_to: ElementId,
        cancel_activity: bool,
    },

    Task,
    UserTask,
    ServiceTask,
    ScriptTask,
    SendTask,
    ReceiveTask,
    ManualTask,
    BusinessRuleTask,
    CallActivity,
    SubProcess {
        triggered_by_event: bool,
    },

    ExclusiveGateway,
    ParallelGateway,
    InclusiveGateway,
    EventBasedGateway,
    ComplexGateway,

    DataObjectReference,
    DataStoreReference,
    TextAnnotation {
        text: Option<String>,
    },
    Group,
    Participant {
        process_ref: Option<ElementId>,
    },
    Lane,

    SequenceFlow {
        source: ElementId,
        target: ElementId,
        is_default: bool,
        condition: Option<String>,
    },
    MessageFlow {
        source: ElementId,
        target: ElementId,
    },
    Association {
        source: ElementId,
        target: ElementId,
    },
    DataInputAssociation {
        source: ElementId,
        target: ElementId,
    },
    DataOutputAssociation {
        source: ElementId,
        target: ElementId,
    },
}

impl ElementKind {
    /// Returns `(source, target)` when the kind is a connection.
    pub fn endpoints(&self) -> Option<(&ElementId, &ElementId)> {
        match self {
            ElementKind::SequenceFlow { source, target, .. }
            | ElementKind::MessageFlow { source, target }
            | ElementKind::Association { source, target }
            | ElementKind::DataInputAssociation { source, target }
            | ElementKind::DataOutputAssociation { source, target } => Some((source, target)),
            _ => None,
        }
    }

    /// Default `(width, height)` of a new shape, as in bpmn-js `ElementFactory`.
    /// Returns `None` for connections and root containers.
    /// `expanded` only matters for sub-processes (collapsed ones use task size).
    pub fn default_size(&self, expanded: bool) -> Option<(f64, f64)> {
        use ElementKind::*;

        match self {
            StartEvent(_)
            | EndEvent(_)
            | IntermediateCatchEvent(_)
            | IntermediateThrowEvent(_)
            | BoundaryEvent { .. } => Some((36.0, 36.0)),

            Task | UserTask | ServiceTask | ScriptTask | SendTask | ReceiveTask | ManualTask
            | BusinessRuleTask | CallActivity => Some((100.0, 80.0)),

            SubProcess { .. } if expanded => Some((350.0, 200.0)),
            SubProcess { .. } => Some((100.0, 80.0)),

            ExclusiveGateway | ParallelGateway | InclusiveGateway | EventBasedGateway
            | ComplexGateway => Some((50.0, 50.0)),

            DataObjectReference => Some((36.0, 50.0)),
            DataStoreReference => Some((50.0, 50.0)),
            TextAnnotation { .. } => Some((100.0, 30.0)),
            Group => Some((300.0, 300.0)),
            Participant { .. } => Some((600.0, 250.0)),

            _ => None,
        }
    }
}
