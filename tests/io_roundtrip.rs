//! Integration tests for BPMN XML import/export (`mangue::io`).

use mangue::io::{export_xml, import_xml, new_diagram};
use mangue::model::{
    AssociationDirection, Bounds, Definitions, ElementDetails, ElementId, ElementKind,
    EventDefinition, RootKind, TimerExpression, Waypoint,
};

const SIMPLE: &str = include_str!("fixtures/simple.bpmn");
const COLLABORATION: &str = include_str!("fixtures/collaboration.bpmn");
const EVENTS: &str = include_str!("fixtures/events.bpmn");
const MESSAGE_DELIVERY: &str = include_str!("fixtures/message-delivery.bpmn");
const COLLAPSED: &str = include_str!("fixtures/collapsed-subprocess.bpmn");
const VERTICAL: &str = include_str!("fixtures/vertical-pool.bpmn");

/// Imports a fixture, panicking with the error on failure.
fn load(xml: &str) -> Definitions {
    import_xml(xml).expect("fixture imports")
}

/// Returns the kind of element `id`, panicking when it is missing.
fn kind<'a>(defs: &'a Definitions, id: &str) -> &'a ElementKind {
    &defs
        .get(&ElementId::from(id))
        .unwrap_or_else(|| panic!("missing element {id}"))
        .kind
}

/// Returns the parent id of element `id` as a string.
fn parent(defs: &Definitions, id: &str) -> Option<String> {
    defs.get(&ElementId::from(id))
        .and_then(|e| e.parent.clone())
        .map(|p| p.0)
}

/// Asserts import -> export -> import yields an identical model.
fn assert_round_trip(xml: &str) {
    let first = load(xml);
    let exported = export_xml(&first);
    let second = import_xml(&exported).expect("exported XML re-imports");

    assert_eq!(first, second, "round trip changed the model:\n{exported}");
}

/// Simple process: names, documentation, default flow, condition, annotation.
#[test]
fn imports_simple_process() {
    let defs = load(SIMPLE);

    assert_eq!(defs.id, "Definitions_1");
    assert_eq!(defs.elements.len(), 12);
    assert_eq!(kind(&defs, "Process_1"), &ElementKind::Process);
    assert_eq!(kind(&defs, "Task_Check"), &ElementKind::UserTask);
    assert_eq!(parent(&defs, "Task_Check").as_deref(), Some("Process_1"));

    let task = defs.get(&"Task_Check".into()).unwrap();
    assert_eq!(task.name.as_deref(), Some("Check order"));
    assert_eq!(
        task.documentation.as_deref(),
        Some("Verify stock and payment.")
    );

    let process = defs.get(&"Process_1".into()).unwrap();
    assert_eq!(
        process.documentation.as_deref(),
        Some("Handles an incoming order & ships it.")
    );

    let ElementKind::SequenceFlow { is_default, .. } = kind(&defs, "Flow_Yes") else {
        panic!("Flow_Yes is not a sequence flow");
    };
    assert!(*is_default);

    let ElementKind::SequenceFlow {
        is_default,
        condition,
        ..
    } = kind(&defs, "Flow_No")
    else {
        panic!("Flow_No is not a sequence flow");
    };
    assert!(!*is_default);
    assert_eq!(condition.as_deref(), Some("${total < 0}"));

    assert_eq!(
        kind(&defs, "TextAnnotation_1"),
        &ElementKind::TextAnnotation {
            text: Some("Manual check for now".into())
        }
    );
    assert_eq!(
        kind(&defs, "Association_1"),
        &ElementKind::Association {
            source: "Task_Check".into(),
            target: "TextAnnotation_1".into()
        }
    );
}

/// DI: plane binding, shape bounds, label bounds and edge waypoints.
#[test]
fn imports_simple_di() {
    let defs = load(SIMPLE);
    let plane = defs.plane();

    assert_eq!(defs.diagrams[0].id, "BPMNDiagram_1");
    assert_eq!(plane.id, "BPMNPlane_1");
    assert_eq!(
        plane.element_id.as_ref().map(|e| e.as_str()),
        Some("Process_1")
    );
    assert_eq!(plane.shapes.len(), 6);
    assert_eq!(plane.edges.len(), 5);

    let start = defs.shape(&"StartEvent_1".into()).unwrap();
    assert_eq!(
        start.bounds,
        Bounds {
            x: 173.0,
            y: 102.0,
            w: 36.0,
            h: 36.0
        }
    );
    assert_eq!(
        start.label_bounds,
        Some(Bounds {
            x: 155.0,
            y: 145.0,
            w: 73.0,
            h: 14.0
        })
    );

    let assoc = defs
        .edges()
        .find(|e| e.element_id.as_str() == "Association_1")
        .unwrap();
    assert_eq!(assoc.waypoints[1], Waypoint { x: 403.5, y: 20.0 });
}

/// Collaboration: participants, process ref, message flows, lanes, data.
#[test]
fn imports_collaboration_with_lanes() {
    let defs = load(COLLABORATION);

    assert_eq!(kind(&defs, "Collaboration_1"), &ElementKind::Collaboration);
    assert_eq!(
        kind(&defs, "Participant_Shop"),
        &ElementKind::Participant {
            process_ref: Some("Process_Shop".into())
        }
    );
    assert_eq!(
        kind(&defs, "Participant_Customer"),
        &ElementKind::Participant { process_ref: None }
    );
    assert_eq!(
        parent(&defs, "Participant_Shop").as_deref(),
        Some("Collaboration_1")
    );
    assert_eq!(
        kind(&defs, "MessageFlow_1"),
        &ElementKind::MessageFlow {
            source: "Participant_Customer".into(),
            target: "Start_Shop".into()
        }
    );

    assert_eq!(kind(&defs, "Lane_Sales"), &ElementKind::Lane);
    assert_eq!(parent(&defs, "Lane_Sales").as_deref(), Some("Process_Shop"));
    assert_eq!(parent(&defs, "Task_Record").as_deref(), Some("Lane_Sales"));
    assert_eq!(parent(&defs, "End_Shop").as_deref(), Some("Lane_Shipping"));
    assert_eq!(
        parent(&defs, "DataStoreReference_1").as_deref(),
        Some("Process_Shop")
    );

    assert_eq!(
        kind(&defs, "Start_Shop"),
        &ElementKind::StartEvent(EventDefinition::Message)
    );
    assert_eq!(
        kind(&defs, "DataOutputAssociation_1"),
        &ElementKind::DataOutputAssociation {
            source: "Task_Record".into(),
            target: "DataStoreReference_1".into()
        }
    );
    assert_eq!(
        kind(&defs, "DataInputAssociation_1"),
        &ElementKind::DataInputAssociation {
            source: "DataObjectReference_1".into(),
            target: "Task_Send".into()
        }
    );
    assert!(defs.get(&"DataObject_1".into()).is_none());
    assert!(defs.get(&"Property_1".into()).is_none());

    let pool = defs.shape(&"Participant_Shop".into()).unwrap();
    assert_eq!(pool.is_horizontal, Some(true));
}

/// Default-namespace document: every event definition, boundary, sub-process.
#[test]
fn imports_events_default_namespace() {
    use EventDefinition::*;

    let defs = load(EVENTS);

    assert_eq!(kind(&defs, "Start_Timer"), &ElementKind::StartEvent(Timer));
    assert_eq!(kind(&defs, "Sub_End"), &ElementKind::EndEvent(Error));
    assert_eq!(
        kind(&defs, "Throw_Signal"),
        &ElementKind::IntermediateThrowEvent(Signal)
    );
    assert_eq!(
        kind(&defs, "Catch_Message"),
        &ElementKind::IntermediateCatchEvent(Message)
    );
    assert_eq!(
        kind(&defs, "Catch_Conditional"),
        &ElementKind::IntermediateCatchEvent(Conditional)
    );
    assert_eq!(
        kind(&defs, "Catch_Link"),
        &ElementKind::IntermediateCatchEvent(Link)
    );
    assert_eq!(
        kind(&defs, "Throw_Escalation"),
        &ElementKind::IntermediateThrowEvent(Escalation)
    );
    assert_eq!(
        kind(&defs, "Throw_Compensation"),
        &ElementKind::IntermediateThrowEvent(Compensation)
    );
    assert_eq!(
        kind(&defs, "Throw_None"),
        &ElementKind::IntermediateThrowEvent(None)
    );
    assert_eq!(
        kind(&defs, "End_Terminate"),
        &ElementKind::EndEvent(Terminate)
    );

    assert_eq!(
        kind(&defs, "Boundary_Timer"),
        &ElementKind::BoundaryEvent {
            definition: Timer,
            attached_to: "Sub_Work".into(),
            cancel_activity: false
        }
    );
    assert_eq!(
        kind(&defs, "Boundary_Error"),
        &ElementKind::BoundaryEvent {
            definition: Error,
            attached_to: "Sub_Work".into(),
            cancel_activity: true
        }
    );

    assert_eq!(
        kind(&defs, "Sub_Work"),
        &ElementKind::SubProcess {
            triggered_by_event: false
        }
    );
    assert_eq!(
        kind(&defs, "Event_Sub"),
        &ElementKind::SubProcess {
            triggered_by_event: true
        }
    );
    assert_eq!(parent(&defs, "Sub_Script").as_deref(), Some("Sub_Work"));
    assert_eq!(parent(&defs, "Sub_Flow_1").as_deref(), Some("Sub_Work"));

    let ElementKind::SequenceFlow { is_default, .. } = kind(&defs, "Flow_3") else {
        panic!("Flow_3 is not a sequence flow");
    };
    assert!(*is_default);

    for (id, expected) in [
        ("Call_1", ElementKind::CallActivity),
        ("Task_Plain", ElementKind::Task),
        ("Task_Receive", ElementKind::ReceiveTask),
        ("Task_Manual", ElementKind::ManualTask),
        ("Task_Rule", ElementKind::BusinessRuleTask),
        ("Gw_Parallel", ElementKind::ParallelGateway),
        ("Gw_Inclusive", ElementKind::InclusiveGateway),
        ("Gw_Event", ElementKind::EventBasedGateway),
        ("Gw_Complex", ElementKind::ComplexGateway),
        ("Group_1", ElementKind::Group),
    ] {
        assert_eq!(kind(&defs, id), &expected, "{id}");
    }

    let sub = defs.shape(&"Sub_Work".into()).unwrap();
    assert_eq!(sub.is_expanded, Some(true));
    let event_sub = defs.shape(&"Event_Sub".into()).unwrap();
    assert_eq!(event_sub.is_expanded, Some(false));
}

/// Returns the details of element `id`, panicking when it is missing.
fn details<'a>(defs: &'a Definitions, id: &str) -> &'a ElementDetails {
    &defs
        .get(&ElementId::from(id))
        .unwrap_or_else(|| panic!("missing element {id}"))
        .details
}

/// Event payloads, flags, root elements, lane set and DI ids are imported.
#[test]
fn imports_event_payloads_and_flags() {
    let events = load(EVENTS);

    assert_eq!(details(&events, "Process_Events").is_executable, Some(true));
    assert_eq!(
        details(&events, "Event_Sub_Start").is_interrupting,
        Some(false)
    );
    assert_eq!(details(&events, "Sub_Start").is_interrupting, None);

    let event_ref = |id| {
        details(&events, id)
            .event_ref
            .as_ref()
            .map(|r| r.0.as_str())
    };
    assert_eq!(event_ref("Catch_Message"), Some("Message_1"));
    assert_eq!(event_ref("Throw_Signal"), Some("Signal_1"));
    assert_eq!(event_ref("Boundary_Error"), Some("Error_1"));
    assert_eq!(event_ref("Event_Sub_Start"), Some("Escalation_1"));

    assert_eq!(
        details(&events, "Start_Timer").timer,
        Some(TimerExpression::Cycle("R/P1D".into()))
    );
    assert_eq!(
        details(&events, "Boundary_Timer").timer,
        Some(TimerExpression::Duration("PT1H".into()))
    );
    assert_eq!(
        details(&events, "Catch_Conditional").condition.as_deref(),
        Some("${ready}")
    );

    let roots: Vec<(RootKind, &str, Option<&str>)> = events
        .root_elements
        .iter()
        .map(|r| (r.kind, r.id.0.as_str(), r.name.as_deref()))
        .collect();
    assert_eq!(
        roots,
        [
            (RootKind::Message, "Message_1", Some("Ping")),
            (RootKind::Signal, "Signal_1", Some("Alarm")),
            (RootKind::Error, "Error_1", Some("Boom")),
            (RootKind::Escalation, "Escalation_1", Some("Up")),
        ]
    );

    let plane = events.plane();
    assert_eq!(
        plane
            .di_ids
            .get(&ElementId::from("Start_Timer"))
            .map(String::as_str),
        Some("Shape_Start_Timer")
    );
    assert!(!plane.di_ids.contains_key(&ElementId::from("Sub_Work")));

    let simple = load(SIMPLE);
    assert_eq!(
        details(&simple, "Association_1").association_direction,
        Some(AssociationDirection::None)
    );

    let collaboration = load(COLLABORATION);
    assert_eq!(
        details(&collaboration, "Process_Shop")
            .lane_set_id
            .as_deref(),
        Some("LaneSet_1")
    );
}

/// Export writes payloads, flags, root elements and original DI ids back.
#[test]
fn exports_event_payloads_and_flags() {
    let xml = export_xml(&load(EVENTS));

    assert!(xml.contains("<bpmn:process id=\"Process_Events\" isExecutable=\"true\">"));
    assert!(xml.contains("<bpmn:signal id=\"Signal_1\" name=\"Alarm\" />"));
    assert!(xml.contains("messageRef=\"Message_1\""));
    assert!(xml.contains("escalationRef=\"Escalation_1\""));
    assert!(xml.contains("isInterrupting=\"false\""));
    assert!(xml.contains(
        "<bpmn:timeDuration xsi:type=\"bpmn:tFormalExpression\">PT1H</bpmn:timeDuration>"
    ));
    assert!(
        xml.contains(
            "<bpmn:condition xsi:type=\"bpmn:tFormalExpression\">${ready}</bpmn:condition>"
        )
    );
    assert!(xml.contains("id=\"Shape_Start_Timer\" bpmnElement=\"Start_Timer\""));
    assert!(xml.contains("id=\"Sub_Work_di\""));

    let simple = export_xml(&load(SIMPLE));
    assert!(simple.contains("associationDirection=\"None\""));
}

/// Unknown elements (and their DI) are skipped without failing the import.
#[test]
fn skips_unknown_elements() {
    let defs = load(EVENTS);

    assert!(defs.get(&"Unknown_1".into()).is_none());
    assert!(defs.get(&"Message_1".into()).is_none());
    assert!(defs.shape(&"Unknown_1".into()).is_none());
}

/// Malformed XML and non-BPMN roots are rejected.
#[test]
fn rejects_invalid_documents() {
    assert!(import_xml("<bpmn:definitions").is_err());
    assert!(import_xml("<foo xmlns=\"urn:x\"/>").is_err());
}

/// Round trip preserves the simple process.
#[test]
fn round_trips_simple() {
    assert_round_trip(SIMPLE);
}

/// Round trip preserves the collaboration with lanes and data associations.
#[test]
fn round_trips_collaboration() {
    assert_round_trip(COLLABORATION);
}

/// Round trip preserves events, boundary events and sub-processes.
#[test]
fn round_trips_events() {
    assert_round_trip(EVENTS);
}

/// Export uses the bpmn-js header, prefixes and incoming/outgoing children.
#[test]
fn exports_bpmn_js_compatible_xml() {
    let xml = export_xml(&load(SIMPLE));

    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("xmlns:bpmn=\"http://www.omg.org/spec/BPMN/20100524/MODEL\""));
    assert!(xml.contains("xmlns:bpmndi=\"http://www.omg.org/spec/BPMN/20100524/DI\""));
    assert!(xml.contains("targetNamespace=\"http://bpmn.io/schema/bpmn\""));
    assert!(xml.contains("exporter=\"mangue\""));
    assert!(xml.contains(
        "<bpmn:exclusiveGateway id=\"Gateway_Ok\" name=\"Order ok?\" default=\"Flow_Yes\">"
    ));
    assert!(xml.contains("<bpmn:incoming>Flow_1</bpmn:incoming>"));
    assert!(xml.contains("<bpmn:outgoing>Flow_No</bpmn:outgoing>"));
    assert!(xml.contains("&lt; 0}</bpmn:conditionExpression>"));
    assert!(xml.contains("<dc:Bounds x=\"173\" y=\"102\" width=\"36\" height=\"36\" />"));
    assert!(xml.contains("<di:waypoint x=\"403.5\" y=\"20\" />"));
}

/// Export of data object references emits the backing `bpmn:dataObject`.
#[test]
fn exports_data_objects() {
    let xml = export_xml(&load(COLLABORATION));

    assert!(xml.contains("dataObjectRef=\"DataObject_DataObjectReference_1\""));
    assert!(xml.contains("<bpmn:dataObject id=\"DataObject_DataObjectReference_1\" />"));
    assert!(xml.contains("<bpmn:laneSet id=\"LaneSet_1\">"));
    assert!(xml.contains("<bpmn:flowNodeRef>Task_Record</bpmn:flowNodeRef>"));
}

/// `new_diagram` matches the bpmn-js initial diagram and survives a round trip.
#[test]
fn new_diagram_matches_bpmn_js() {
    let defs = new_diagram();

    assert_eq!(defs.elements.len(), 2);
    assert_eq!(kind(&defs, "Process_1"), &ElementKind::Process);
    assert_eq!(
        kind(&defs, "StartEvent_1"),
        &ElementKind::StartEvent(EventDefinition::None)
    );
    assert_eq!(
        defs.shape(&"StartEvent_1".into()).unwrap().bounds,
        Bounds {
            x: 173.0,
            y: 102.0,
            w: 36.0,
            h: 36.0
        }
    );

    let again = import_xml(&export_xml(&defs)).unwrap();
    assert_eq!(defs, again);
}

/// Real-world bpmn-js collaboration (4 pools, message flows, data store):
/// imports every element and round-trips unchanged.
#[test]
fn round_trips_message_delivery() {
    let defs = load(MESSAGE_DELIVERY);

    assert!(defs.shapes().count() > 0 && defs.edges().count() > 0);
    assert_eq!(
        kind(&defs, "Collaboration_MessageDelivery"),
        &ElementKind::Collaboration
    );

    assert_round_trip(MESSAGE_DELIVERY);
}

/// bpmn-js collapsed sub-process: its children live in a second
/// `BPMNDiagram` bound to the sub-process; both planes import, export in
/// order with their ids and round-trip unchanged.
#[test]
fn round_trips_collapsed_subprocess_plane() {
    let mut defs = load(COLLAPSED);

    assert_eq!(defs.diagrams.len(), 2);
    assert_eq!(defs.active, 0);
    assert!(defs.shape(&"Task_1".into()).is_none());
    assert_eq!(parent(&defs, "Task_1").as_deref(), Some("SubProcess_1"));

    let sub = &defs.diagrams[1];
    assert_eq!(sub.id, "BPMNDiagram_0p6a7cm");
    assert_eq!(sub.plane.id, "BPMNPlane_1h2xq9e");
    assert_eq!(sub.plane.element_id, Some("SubProcess_1".into()));
    assert_eq!(sub.plane.shapes.len(), 3);
    assert_eq!(sub.plane.edges.len(), 2);

    assert!(defs.open_plane(Some(&"SubProcess_1".into())));
    assert!(defs.shape(&"Task_1".into()).is_some());
    assert!(defs.shape(&"SubProcess_1".into()).is_none());

    let xml = export_xml(&load(COLLAPSED));
    let root = xml.find("BPMNPlane_1").expect("root plane");
    let plane = xml.find("BPMNPlane_1h2xq9e").expect("sub plane");
    assert!(root < plane && xml[plane..].contains("bpmnElement=\"Task_1\""));

    assert_round_trip(COLLAPSED);
}

/// Vertical pool and lanes keep `isHorizontal="false"`, a compensation
/// activity keeps `isForCompensation`, and both survive a round trip.
#[test]
fn round_trips_vertical_pool() {
    let defs = load(VERTICAL);

    for id in ["Participant_V", "Lane_Left", "Lane_Right"] {
        let shape = defs.shape(&ElementId::from(id)).expect("pool shape");
        assert_eq!(shape.is_horizontal, Some(false), "{id}");
    }
    assert_eq!(details(&defs, "Task_Undo").is_for_compensation, Some(true));
    assert_eq!(details(&defs, "Task_A").is_for_compensation, None);

    let xml = export_xml(&defs);
    assert_eq!(xml.matches("isHorizontal=\"false\"").count(), 3, "{xml}");
    assert!(xml.contains("isForCompensation=\"true\""), "{xml}");

    assert_round_trip(VERTICAL);
}
