use crate::model::{
    Bounds, Definitions, Diagram, Element, ElementKind, EventDefinition, Plane, Shape,
};

/// Builds the bpmn-js initial diagram: non-executable `Process_1` with one
/// `StartEvent_1` at (173, 102), 36x36, DI id `_BPMNShape_StartEvent_2`.
pub fn new_diagram() -> Definitions {
    let mut defs = Definitions {
        id: "Definitions_1".into(),
        diagrams: vec![Diagram {
            id: "BPMNDiagram_1".into(),
            plane: Plane {
                id: "BPMNPlane_1".into(),
                element_id: Some("Process_1".into()),
                ..Plane::default()
            },
        }],
        ..Definitions::default()
    };

    let mut process = Element::new("Process_1", ElementKind::Process);
    process.details.is_executable = Some(false);
    defs.add_element(process);

    defs.add_element(
        Element::new(
            "StartEvent_1",
            ElementKind::StartEvent(EventDefinition::None),
        )
        .with_parent("Process_1"),
    );

    defs.add_shape(Shape {
        element_id: "StartEvent_1".into(),
        bounds: Bounds {
            x: 173.0,
            y: 102.0,
            w: 36.0,
            h: 36.0,
        },
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });

    defs.plane_mut()
        .di_ids
        .insert("StartEvent_1".into(), "_BPMNShape_StartEvent_2".into());

    defs
}
