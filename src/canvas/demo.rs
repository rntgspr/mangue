//! Hardcoded demo diagram used until XML import is wired into the app.

use crate::model::{
    Bounds, Definitions, Edge, Element, ElementKind, EventDefinition, Shape, Waypoint,
};

/// Builds a small collaboration exercising the core element set: a pool with
/// a lane, an exclusive split, a message flow to a black-box pool, a data
/// object, a data store and a text annotation.
pub fn demo_definitions() -> Definitions {
    use ElementKind::*;

    let mut defs = Definitions {
        id: "Definitions_demo".into(),
        ..Default::default()
    };
    defs.diagrams[0].id = "BPMNDiagram_1".into();
    defs.plane_mut().id = "BPMNPlane_1".into();
    defs.plane_mut().element_id = Some("Collaboration_1".into());

    let flow = |source: &str, target: &str| SequenceFlow {
        source: source.into(),
        target: target.into(),
        is_default: false,
        condition: None,
    };

    let nodes: Vec<(&str, ElementKind, Option<&str>, Option<&str>, Bounds)> = vec![
        (
            "Collaboration_1",
            Collaboration,
            None,
            None,
            b(0.0, 0.0, 0.0, 0.0),
        ),
        ("Process_1", Process, None, None, b(0.0, 0.0, 0.0, 0.0)),
        (
            "Participant_shop",
            Participant {
                process_ref: Some("Process_1".into()),
            },
            Some("Shop"),
            Some("Collaboration_1"),
            b(150.0, 80.0, 900.0, 380.0),
        ),
        (
            "Lane_sales",
            Lane,
            Some("Sales"),
            Some("Participant_shop"),
            b(180.0, 80.0, 870.0, 380.0),
        ),
        (
            "Participant_customer",
            Participant { process_ref: None },
            Some("Customer"),
            Some("Collaboration_1"),
            b(150.0, 520.0, 900.0, 100.0),
        ),
        (
            "Start_1",
            StartEvent(EventDefinition::Message),
            Some("Order received"),
            Some("Process_1"),
            b(232.0, 182.0, 36.0, 36.0),
        ),
        (
            "Task_check",
            UserTask,
            Some("Check order"),
            Some("Process_1"),
            b(320.0, 160.0, 100.0, 80.0),
        ),
        (
            "Gateway_ok",
            ExclusiveGateway,
            Some("Approved?"),
            Some("Process_1"),
            b(470.0, 175.0, 50.0, 50.0),
        ),
        (
            "Task_ship",
            ServiceTask,
            Some("Ship goods to the customer"),
            Some("Process_1"),
            b(580.0, 100.0, 100.0, 80.0),
        ),
        (
            "Task_reject",
            SendTask,
            Some("Notify rejection"),
            Some("Process_1"),
            b(580.0, 240.0, 100.0, 80.0),
        ),
        (
            "Timer_1",
            IntermediateCatchEvent(EventDefinition::Timer),
            Some("1 day"),
            Some("Process_1"),
            b(742.0, 122.0, 36.0, 36.0),
        ),
        (
            "End_1",
            EndEvent(EventDefinition::Terminate),
            Some("Done"),
            Some("Process_1"),
            b(862.0, 182.0, 36.0, 36.0),
        ),
        (
            "Data_order",
            DataObjectReference,
            Some("Order"),
            Some("Process_1"),
            b(352.0, 330.0, 36.0, 50.0),
        ),
        (
            "Store_crm",
            DataStoreReference,
            Some("CRM"),
            Some("Process_1"),
            b(460.0, 330.0, 50.0, 50.0),
        ),
        (
            "Annotation_1",
            TextAnnotation {
                text: Some("Terminates all paths".into()),
            },
            None,
            Some("Process_1"),
            b(920.0, 280.0, 100.0, 45.0),
        ),
    ];

    for (id, kind, name, parent, bounds) in nodes {
        let has_shape = kind.default_size(false).is_some() || matches!(kind, Lane);
        let is_label_below = matches!(
            kind,
            StartEvent(_)
                | EndEvent(_)
                | IntermediateCatchEvent(_)
                | ExclusiveGateway
                | DataObjectReference
                | DataStoreReference
        );

        let mut element = Element::new(id, kind);
        element.name = name.map(Into::into);
        element.parent = parent.map(Into::into);
        defs.add_element(element);

        if has_shape {
            defs.add_shape(Shape {
                element_id: id.into(),
                bounds,
                is_expanded: None,
                is_horizontal: Some(true),
                label_bounds: is_label_below.then(|| {
                    b(
                        bounds.x + bounds.w / 2.0 - 45.0,
                        bounds.y + bounds.h + 5.0,
                        90.0,
                        14.0,
                    )
                }),
            });
        }
    }

    for shape in &mut defs.plane_mut().shapes {
        match shape.element_id.as_str() {
            "Start_1" => shape.label_bounds = Some(b(205.0, 160.0, 90.0, 14.0)),
            "End_1" => shape.label_bounds = Some(b(902.0, 193.0, 36.0, 14.0)),
            _ => {}
        }
    }

    let edges: Vec<(
        &str,
        ElementKind,
        Option<&str>,
        Vec<(f64, f64)>,
        Option<Bounds>,
    )> = vec![
        (
            "Flow_1",
            flow("Start_1", "Task_check"),
            None,
            vec![(268.0, 200.0), (320.0, 200.0)],
            None,
        ),
        (
            "Flow_2",
            flow("Task_check", "Gateway_ok"),
            None,
            vec![(420.0, 200.0), (470.0, 200.0)],
            None,
        ),
        (
            "Flow_yes",
            flow("Gateway_ok", "Task_ship"),
            Some("yes"),
            vec![(495.0, 175.0), (495.0, 140.0), (580.0, 140.0)],
            Some(b(510.0, 122.0, 20.0, 14.0)),
        ),
        (
            "Flow_no",
            flow("Gateway_ok", "Task_reject"),
            Some("no"),
            vec![(495.0, 225.0), (495.0, 280.0), (580.0, 280.0)],
            Some(b(510.0, 262.0, 20.0, 14.0)),
        ),
        (
            "Flow_3",
            flow("Task_ship", "Timer_1"),
            None,
            vec![(680.0, 140.0), (742.0, 140.0)],
            None,
        ),
        (
            "Flow_4",
            flow("Timer_1", "End_1"),
            None,
            vec![(778.0, 140.0), (880.0, 140.0), (880.0, 182.0)],
            None,
        ),
        (
            "Flow_5",
            flow("Task_reject", "End_1"),
            None,
            vec![(680.0, 280.0), (880.0, 280.0), (880.0, 218.0)],
            None,
        ),
        (
            "Message_1",
            MessageFlow {
                source: "Task_reject".into(),
                target: "Participant_customer".into(),
            },
            Some("Rejection"),
            vec![(630.0, 320.0), (630.0, 520.0)],
            Some(b(640.0, 480.0, 60.0, 14.0)),
        ),
        (
            "Message_2",
            MessageFlow {
                source: "Participant_customer".into(),
                target: "Start_1".into(),
            },
            None,
            vec![(250.0, 520.0), (250.0, 218.0)],
            None,
        ),
        (
            "DataOut_1",
            DataOutputAssociation {
                source: "Task_check".into(),
                target: "Data_order".into(),
            },
            None,
            vec![(370.0, 240.0), (370.0, 330.0)],
            None,
        ),
        (
            "Association_1",
            Association {
                source: "End_1".into(),
                target: "Annotation_1".into(),
            },
            None,
            vec![(893.0, 210.0), (950.0, 280.0)],
            None,
        ),
    ];

    for (id, kind, name, points, label_bounds) in edges {
        let mut element = Element::new(id, kind);
        element.name = name.map(Into::into);
        defs.add_element(element);

        defs.add_edge(Edge {
            element_id: id.into(),
            waypoints: points.into_iter().map(|(x, y)| Waypoint { x, y }).collect(),
            label_bounds,
        });
    }

    defs
}

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}
