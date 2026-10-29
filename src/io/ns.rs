use crate::model::EventDefinition;

/// BPMN 2.0 semantic model namespace.
pub const BPMN: &str = "http://www.omg.org/spec/BPMN/20100524/MODEL";
/// BPMN 2.0 diagram interchange namespace.
pub const BPMNDI: &str = "http://www.omg.org/spec/BPMN/20100524/DI";
/// OMG Diagram Definition common namespace (`dc:Bounds`).
pub const DC: &str = "http://www.omg.org/spec/DD/20100524/DC";
/// OMG Diagram Definition DI namespace (`di:waypoint`).
pub const DI: &str = "http://www.omg.org/spec/DD/20100524/DI";
/// XML Schema instance namespace (`xsi:type`).
pub const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Event definitions and their XML local names (`None` has no element).
pub const EVENT_DEFINITIONS: [(EventDefinition, &str); 9] = [
    (EventDefinition::Message, "messageEventDefinition"),
    (EventDefinition::Timer, "timerEventDefinition"),
    (EventDefinition::Signal, "signalEventDefinition"),
    (EventDefinition::Error, "errorEventDefinition"),
    (EventDefinition::Escalation, "escalationEventDefinition"),
    (EventDefinition::Conditional, "conditionalEventDefinition"),
    (EventDefinition::Link, "linkEventDefinition"),
    (EventDefinition::Compensation, "compensateEventDefinition"),
    (EventDefinition::Terminate, "terminateEventDefinition"),
];
