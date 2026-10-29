use super::id::ElementId;

/// Optional per-element BPMN attributes kept for round trips. They live
/// beside `ElementKind` so adding them does not change kind constructors.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElementDetails {
    /// `isInterrupting` of a start event (`None` = absent, BPMN default true).
    pub is_interrupting: Option<bool>,
    /// `isExecutable` of a process (`None` exports as `false`).
    pub is_executable: Option<bool>,
    /// `messageRef`/`signalRef`/`errorRef`/`escalationRef` of the event definition.
    pub event_ref: Option<ElementId>,
    /// Timer expression of a timer event definition.
    pub timer: Option<TimerExpression>,
    /// Condition body of a conditional event definition.
    pub condition: Option<String>,
    /// `associationDirection` of an association (`None` = absent).
    pub association_direction: Option<AssociationDirection>,
    /// Id of the `laneSet`/`childLaneSet` owned by a process or lane.
    pub lane_set_id: Option<String>,
    /// `isForCompensation` of an activity (`None` = absent, BPMN default false).
    pub is_for_compensation: Option<bool>,
}

/// Timer event definition expression (`timeDate`, `timeDuration`, `timeCycle`).
#[derive(Debug, Clone, PartialEq)]
pub enum TimerExpression {
    Date(String),
    Duration(String),
    Cycle(String),
}

/// `associationDirection` values of `bpmn:association`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationDirection {
    None,
    One,
    Both,
}

/// Kind of a root-level element referenced by event definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    Message,
    Signal,
    Error,
    Escalation,
}

/// A root `bpmn:message`, `bpmn:signal`, `bpmn:error` or `bpmn:escalation`.
#[derive(Debug, Clone, PartialEq)]
pub struct RootElement {
    pub id: ElementId,
    pub kind: RootKind,
    pub name: Option<String>,
}
