use super::*;

// Builds start -> task -> end with DI for every element.
fn sample() -> Definitions {
    let mut defs = Definitions::default();

    defs.add_element(Element::new("p", ElementKind::Process));
    defs.add_element(
        Element::new("start", ElementKind::StartEvent(EventDefinition::None)).with_parent("p"),
    );
    defs.add_element(Element::new("task", ElementKind::UserTask).with_parent("p"));
    defs.add_element(
        Element::new("end", ElementKind::EndEvent(EventDefinition::Terminate)).with_parent("p"),
    );
    defs.add_element(Element::new("f1", flow("start", "task")).with_parent("p"));
    defs.add_element(Element::new("f2", flow("task", "end")).with_parent("p"));

    for id in ["start", "task", "end"] {
        defs.add_shape(Shape {
            element_id: id.into(),
            bounds: Bounds {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            is_expanded: None,
            is_horizontal: None,
            label_bounds: None,
        });
    }

    for id in ["f1", "f2"] {
        defs.add_edge(Edge {
            element_id: id.into(),
            waypoints: vec![Waypoint { x: 0.0, y: 0.0 }, Waypoint { x: 1.0, y: 1.0 }],
            label_bounds: None,
        });
    }

    defs
}

// Builds a plain sequence flow between two ids.
fn flow(source: &str, target: &str) -> ElementKind {
    ElementKind::SequenceFlow {
        source: source.into(),
        target: target.into(),
        is_default: false,
        condition: None,
    }
}

// Collects element ids from an iterator for assertions.
fn ids<'a>(it: impl Iterator<Item = &'a Element>) -> Vec<&'a str> {
    it.map(|e| e.id.as_str()).collect()
}

#[test]
// Default sizes follow bpmn-js ElementFactory.
fn default_sizes_match_bpmn_js() {
    assert_eq!(ElementKind::Task.default_size(false), Some((100.0, 80.0)));
    assert_eq!(
        ElementKind::StartEvent(EventDefinition::Timer).default_size(false),
        Some((36.0, 36.0))
    );
    assert_eq!(
        ElementKind::ExclusiveGateway.default_size(false),
        Some((50.0, 50.0))
    );
    assert_eq!(
        ElementKind::SubProcess {
            triggered_by_event: false
        }
        .default_size(true),
        Some((350.0, 200.0))
    );
    assert_eq!(
        ElementKind::SubProcess {
            triggered_by_event: false
        }
        .default_size(false),
        Some((100.0, 80.0))
    );
    assert_eq!(
        ElementKind::Participant { process_ref: None }.default_size(false),
        Some((600.0, 250.0))
    );
    assert_eq!(
        ElementKind::DataObjectReference.default_size(false),
        Some((36.0, 50.0))
    );
    assert_eq!(
        ElementKind::DataStoreReference.default_size(false),
        Some((50.0, 50.0))
    );
    assert_eq!(
        ElementKind::TextAnnotation { text: None }.default_size(false),
        Some((100.0, 30.0))
    );
    assert_eq!(flow("a", "b").default_size(false), None);
}

#[test]
// Only connection kinds expose endpoints.
fn endpoints_only_for_connections() {
    assert!(ElementKind::Task.endpoints().is_none());
    let mf = ElementKind::MessageFlow {
        source: "a".into(),
        target: "b".into(),
    };
    assert_eq!(mf.endpoints(), Some((&"a".into(), &"b".into())));
}

#[test]
// Duplicate ids are rejected and order is preserved.
fn add_rejects_duplicates_and_keeps_order() {
    let mut defs = sample();

    assert!(!defs.add_element(Element::new("task", ElementKind::Task)));
    assert_eq!(
        defs.get(&"task".into()).unwrap().kind,
        ElementKind::UserTask
    );

    let order: Vec<_> = defs.elements.keys().map(|k| k.as_str()).collect();
    assert_eq!(order, ["p", "start", "task", "end", "f1", "f2"]);
}

#[test]
// Incoming/outgoing resolve connections of a node.
fn incoming_and_outgoing() {
    let defs = sample();
    let task = "task".into();

    assert_eq!(ids(defs.incoming(&task)), ["f1"]);
    assert_eq!(ids(defs.outgoing(&task)), ["f2"]);
    assert_eq!(ids(defs.children(&"p".into())).len(), 5);
}

#[test]
// Removing a node cascades to its connections and all their DI.
fn remove_cascades_flows_and_di() {
    let mut defs = sample();

    let removed = defs.remove_element(&"task".into());
    assert_eq!(
        removed,
        vec![ElementId::from("task"), "f1".into(), "f2".into()]
    );

    assert!(defs.get(&"task".into()).is_none());
    assert!(defs.get(&"f1".into()).is_none());
    assert!(defs.get(&"start".into()).is_some());
    assert!(defs.shape(&"task".into()).is_none());
    assert_eq!(defs.shapes().count(), 2);
    assert_eq!(defs.edges().count(), 0);
}

#[test]
// Removing a connection keeps its endpoints.
fn remove_connection_only() {
    let mut defs = sample();

    assert_eq!(
        defs.remove_element(&"f1".into()),
        vec![ElementId::from("f1")]
    );
    assert!(defs.get(&"start".into()).is_some());
    assert_eq!(defs.edges().count(), 1);
}

#[test]
// Removing an unknown id is a no-op.
fn remove_unknown_is_noop() {
    let mut defs = sample();

    assert!(defs.remove_element(&"nope".into()).is_empty());
    assert_eq!(defs.elements.len(), 6);
}

#[test]
// A self-loop flow is removed once with its node.
fn remove_with_self_loop() {
    let mut defs = sample();
    defs.add_element(Element::new("loop", flow("task", "task")));

    let removed = defs.remove_element(&"task".into());
    assert_eq!(removed.len(), 4);
}
