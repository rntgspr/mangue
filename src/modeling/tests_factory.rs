//! Tests for the command builders (create, append, connect, replace, space)
//! and compound undo.

use super::factory::{
    APPEND_GAP, append, append_bounds, connect, create_shape, kind_label, make_space, new_id,
    replace_options,
};
use super::layout::is_orthogonal;
use super::*;
use crate::model::{
    Bounds, Definitions, Element, ElementId, ElementKind, EventDefinition, Shape, Waypoint,
};

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}

/// Shorthand point constructor.
fn pt(x: f64, y: f64) -> Waypoint {
    Waypoint { x, y }
}

/// Adds element `id` of `kind` under `parent` with a DI shape at `bounds`.
fn node(defs: &mut Definitions, id: &str, kind: ElementKind, parent: &str, bounds: Bounds) {
    defs.add_element(Element::new(id, kind).with_parent(parent));
    defs.add_shape(Shape {
        element_id: id.into(),
        bounds,
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });
}

/// Collaboration with pool `a` (process `pa`: start, task, end, data, note)
/// at (0,0,600,250) and pool `b` (process `pb`: task `t2`) at (0,300,600,250).
fn collaboration() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("c".into());
    defs.add_element(Element::new("c", ElementKind::Collaboration));
    defs.add_element(Element::new("pa", ElementKind::Process));
    defs.add_element(Element::new("pb", ElementKind::Process));

    let pool = |p: &str| ElementKind::Participant {
        process_ref: Some(p.into()),
    };
    node(&mut defs, "a", pool("pa"), "c", b(0.0, 0.0, 600.0, 250.0));
    node(&mut defs, "b", pool("pb"), "c", b(0.0, 300.0, 600.0, 250.0));

    node(
        &mut defs,
        "start",
        ElementKind::StartEvent(EventDefinition::None),
        "pa",
        b(50.0, 100.0, 36.0, 36.0),
    );
    node(
        &mut defs,
        "task",
        ElementKind::Task,
        "pa",
        b(150.0, 78.0, 100.0, 80.0),
    );
    node(
        &mut defs,
        "end",
        ElementKind::EndEvent(EventDefinition::None),
        "pa",
        b(400.0, 100.0, 36.0, 36.0),
    );
    node(
        &mut defs,
        "data",
        ElementKind::DataObjectReference,
        "pa",
        b(150.0, 190.0, 36.0, 50.0),
    );
    node(
        &mut defs,
        "note",
        ElementKind::TextAnnotation { text: None },
        "pa",
        b(300.0, 10.0, 100.0, 30.0),
    );
    node(
        &mut defs,
        "t2",
        ElementKind::Task,
        "pb",
        b(150.0, 380.0, 100.0, 80.0),
    );

    defs
}

/// Plain process plane `p` with a single task at (100,100).
fn process() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));
    node(
        &mut defs,
        "task",
        ElementKind::Task,
        "p",
        b(100.0, 100.0, 100.0, 80.0),
    );
    defs
}

/// New ids use the bpmn-js prefix and the first free number.
#[test]
fn new_id_skips_used() {
    let mut defs = process();
    defs.add_element(Element::new("Activity_1", ElementKind::Task));

    assert_eq!(new_id(&defs, "Activity"), ElementId::from("Activity_2"));
    assert_eq!(new_id(&defs, "Flow"), ElementId::from("Flow_1"));
}

/// Palette create centers the default size at the cursor inside the pool.
#[test]
fn create_shape_centered_in_container() {
    let mut defs = collaboration();
    let command = create_shape(
        &defs,
        ElementKind::ExclusiveGateway,
        false,
        pt(500.0, 400.0),
    )
    .unwrap();
    let mut stack = CommandStack::default();
    assert!(stack.execute(command, &mut defs));

    let id = ElementId::from("Gateway_1");
    assert_eq!(defs.get(&id).unwrap().parent, Some("pb".into()));
    assert_eq!(defs.shape(&id).unwrap().bounds, b(475.0, 375.0, 50.0, 50.0));

    assert!(create_shape(&defs, ElementKind::Task, false, pt(10.0, 270.0)).is_none());
    let note = create_shape(
        &defs,
        ElementKind::TextAnnotation { text: None },
        false,
        pt(10.0, 270.0),
    );
    assert!(note.is_some());
}

/// The expanded sub-process gets its expanded size and DI flag.
#[test]
fn create_expanded_subprocess() {
    let defs = process();
    let Some(Command::CreateShape { shape, .. }) = create_shape(
        &defs,
        ElementKind::SubProcess {
            triggered_by_event: false,
        },
        true,
        pt(0.0, 0.0),
    ) else {
        panic!("expected CreateShape");
    };

    assert_eq!(shape.bounds, b(-175.0, -100.0, 350.0, 200.0));
    assert_eq!(shape.is_expanded, Some(true));
}

/// The first pool on a process wraps its content and makes a collaboration.
#[test]
fn create_participant_converts_process() {
    let mut defs = process();
    let before = defs.clone();
    let command = create_shape(
        &defs,
        ElementKind::Participant { process_ref: None },
        false,
        pt(0.0, 0.0),
    )
    .unwrap();
    let mut stack = CommandStack::default();
    assert!(stack.execute(command, &mut defs));

    let pool = ElementId::from("Participant_1");
    assert_eq!(defs.plane_mut().element_id, Some("Collaboration_1".into()));
    assert_eq!(
        defs.get(&pool).unwrap().kind,
        ElementKind::Participant {
            process_ref: Some("p".into())
        }
    );
    assert_eq!(defs.plane_mut().shapes[0].element_id, pool);
    let bounds = defs.shape(&pool).unwrap().bounds;
    assert!(bounds.x <= 100.0 - 30.0 && bounds.w >= 600.0);
    assert_eq!(rules::participant_of(&defs, &"task".into()), Some(pool));

    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);
}

/// A pool added to a collaboration gets its own new process.
#[test]
fn create_participant_in_collaboration() {
    let mut defs = collaboration();
    let command = create_shape(
        &defs,
        ElementKind::Participant { process_ref: None },
        false,
        pt(300.0, 800.0),
    )
    .unwrap();
    assert!(command.execute(&mut defs).is_some());

    assert_eq!(
        defs.get(&"Process_1".into()).unwrap().kind,
        ElementKind::Process
    );
    assert_eq!(
        defs.get(&"Participant_1".into()).unwrap().parent,
        Some("c".into())
    );
}

/// autoPlace: gap right of the source, centered, shifted below obstacles.
#[test]
fn append_bounds_avoids_overlap() {
    let mut defs = process();
    let source = b(100.0, 100.0, 100.0, 80.0);

    let free = append_bounds(&defs, &source, 100.0, 80.0, false);
    assert_eq!(free, b(200.0 + APPEND_GAP, 100.0, 100.0, 80.0));

    node(
        &mut defs,
        "blocker",
        ElementKind::Task,
        "p",
        b(260.0, 90.0, 100.0, 80.0),
    );
    let shifted = append_bounds(&defs, &source, 100.0, 80.0, false);
    assert_eq!(shifted, b(250.0, 200.0, 100.0, 80.0));

    let note = append_bounds(&defs, &source, 100.0, 30.0, true);
    assert_eq!(note, b(250.0, 20.0, 100.0, 30.0));
}

/// Append creates shape + flow as one undoable step.
#[test]
fn append_is_one_undo_step() {
    let mut defs = process();
    let before = defs.clone();
    let command = append(
        &defs,
        &"task".into(),
        ElementKind::EndEvent(EventDefinition::None),
    )
    .unwrap();
    let mut stack = CommandStack::default();
    assert!(stack.execute(command, &mut defs));

    let event = ElementId::from("Event_1");
    assert_eq!(
        defs.shape(&event).unwrap().bounds,
        b(250.0, 122.0, 36.0, 36.0)
    );
    let flow = defs.get(&"Flow_1".into()).unwrap();
    assert!(
        matches!(&flow.kind, ElementKind::SequenceFlow { source, target, .. } if source.as_str() == "task" && *target == event)
    );
    assert_eq!(flow.parent, Some("p".into()));
    assert_eq!(defs.edges().count(), 1);

    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);
    assert!(stack.redo(&mut defs));
    assert!(defs.get(&event).is_some());
}

/// Appending a text annotation links it with an association.
#[test]
fn append_annotation_uses_association() {
    let mut defs = process();
    let command = append(
        &defs,
        &"task".into(),
        ElementKind::TextAnnotation { text: None },
    )
    .unwrap();
    command.execute(&mut defs).unwrap();

    assert!(matches!(
        defs.get(&"Association_1".into()).unwrap().kind,
        ElementKind::Association { .. }
    ));
}

/// Nothing can be appended to an end event (no outgoing sequence flow).
#[test]
fn append_from_end_event_rejected() {
    let defs = collaboration();

    assert!(append(&defs, &"end".into(), ElementKind::Task).is_none());
}

/// Connect picks the allowed connection kind and its semantic parent.
#[test]
fn connect_parents() {
    let defs = collaboration();

    let Some(Command::CreateConnection { element, .. }) =
        connect(&defs, &"task".into(), &"t2".into())
    else {
        panic!("expected message flow");
    };
    assert!(matches!(element.kind, ElementKind::MessageFlow { .. }));
    assert_eq!(element.parent, Some("c".into()));

    let Some(Command::CreateConnection { element, .. }) =
        connect(&defs, &"start".into(), &"task".into())
    else {
        panic!("expected sequence flow");
    };
    assert_eq!(element.parent, Some("pa".into()));

    assert!(connect(&defs, &"end".into(), &"start".into()).is_none());
}

/// Replace keeps id, name, DI and valid flows; drops rejected flows; undo restores.
#[test]
fn replace_keeps_identity_and_prunes_flows() {
    let mut defs = collaboration();
    let mut stack = CommandStack::default();
    for (s, t) in [("start", "task"), ("task", "end")] {
        let command = connect(&defs, &s.into(), &t.into()).unwrap();
        assert!(stack.execute(command, &mut defs));
    }
    defs.elements
        .get_mut(&ElementId::from("task"))
        .unwrap()
        .name = Some("Do".into());
    let before = defs.clone();

    let replace = |id: &str, kind| Command::ReplaceElement {
        id: id.into(),
        kind,
    };
    assert!(stack.execute(replace("task", ElementKind::UserTask), &mut defs));
    let task = defs.get(&"task".into()).unwrap();
    assert_eq!(task.kind, ElementKind::UserTask);
    assert_eq!(task.name.as_deref(), Some("Do"));
    assert_eq!(defs.edges().count(), 2);

    assert!(stack.execute(
        replace("end", ElementKind::StartEvent(EventDefinition::None)),
        &mut defs
    ));
    assert_eq!(defs.edges().count(), 1);

    assert!(stack.undo(&mut defs));
    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);

    assert!(
        replace("task", ElementKind::Task)
            .execute(&mut defs)
            .is_none()
    );
}

/// Replace options are sibling kinds, never the current one.
#[test]
fn replace_options_siblings() {
    let task = replace_options(&ElementKind::Task);
    assert!(task.contains(&ElementKind::UserTask));
    assert!(!task.contains(&ElementKind::Task));

    let gateway = replace_options(&ElementKind::ExclusiveGateway);
    assert_eq!(gateway.len(), 4);

    let end = replace_options(&ElementKind::EndEvent(EventDefinition::None));
    assert!(end.contains(&ElementKind::EndEvent(EventDefinition::Terminate)));
    assert!(end.contains(&ElementKind::StartEvent(EventDefinition::None)));

    assert!(replace_options(&ElementKind::Lane).is_empty());
    assert_eq!(
        kind_label(&ElementKind::StartEvent(EventDefinition::Message)),
        "Message Start Event"
    );
}

/// Compound undo restores sub-commands in reverse; empty compounds are no-ops.
#[test]
fn compound_undo() {
    let mut defs = process();
    let before = defs.clone();
    let compound = Command::Compound(vec![
        Command::MoveShapes {
            ids: vec!["task".into()],
            dx: 10.0,
            dy: 0.0,
        },
        Command::DeleteElements {
            ids: vec!["task".into()],
        },
    ]);

    let mut stack = CommandStack::default();
    assert!(stack.execute(compound, &mut defs));
    assert!(defs.get(&"task".into()).is_none());
    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);

    assert!(!stack.execute(Command::Compound(Vec::new()), &mut defs));
}

/// The space tool moves shapes beyond the line by the dominant axis.
#[test]
fn space_tool_moves_beyond_line() {
    let defs = collaboration();
    let Some(Command::MoveShapes { ids, dx, dy }) = make_space(&defs, pt(300.0, 0.0), 40.0, 5.0)
    else {
        panic!("expected move");
    };

    assert_eq!((dx, dy), (40.0, 0.0));
    assert!(ids.contains(&"end".into()) && ids.contains(&"note".into()));
    assert!(!ids.contains(&"task".into()) && !ids.contains(&"a".into()));
    assert!(make_space(&defs, pt(300.0, 0.0), 0.0, 0.0).is_none());
}

/// Adds element `id` of `kind` under process `p` with a DI shape at `bounds`.
fn layout_node(defs: &mut Definitions, id: &str, kind: ElementKind, bounds: Bounds) {
    defs.add_element(Element::new(id, kind).with_parent("p"));
    defs.add_shape(Shape {
        element_id: id.into(),
        bounds,
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });
}

/// Creating flows through the command picks the rule from the kinds: a
/// gateway source leaves from its bottom, a gateway target is entered from
/// its top, a timer-less ordinary pair is `h:h`.
#[test]
fn command_applies_rule_by_kind() {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));
    layout_node(
        &mut defs,
        "gw",
        ElementKind::ExclusiveGateway,
        b(0.0, 0.0, 50.0, 50.0),
    );
    layout_node(
        &mut defs,
        "t",
        ElementKind::Task,
        b(200.0, 100.0, 100.0, 80.0),
    );
    layout_node(
        &mut defs,
        "e",
        ElementKind::EndEvent(EventDefinition::None),
        b(400.0, 300.0, 36.0, 36.0),
    );

    connect(&defs, &"gw".into(), &"t".into())
        .unwrap()
        .execute(&mut defs);
    connect(&defs, &"t".into(), &"e".into())
        .unwrap()
        .execute(&mut defs);

    let edges: Vec<_> = defs.edges().map(|e| e.waypoints.clone()).collect();
    assert_eq!(
        edges[0],
        vec![pt(25.0, 50.0), pt(25.0, 140.0), pt(200.0, 140.0)]
    );
    assert_eq!(edges[1].first(), Some(&pt(300.0, 140.0)));
    assert_eq!(edges[1].last(), Some(&pt(400.0, 318.0)));
    assert_ortho(&edges[1]);
}

/// Asserts that every segment of `route` is horizontal or vertical.
fn assert_ortho(route: &[Waypoint]) {
    assert!(route.len() >= 2, "{route:?}");
    for seg in route.windows(2) {
        assert!(is_orthogonal(seg[0], seg[1]), "diagonal in {route:?}");
    }
}

/// Adds `element` with a DI shape at `bounds`.
fn put(defs: &mut Definitions, element: Element, bounds: Bounds, horizontal: Option<bool>) {
    let id = element.id.clone();
    defs.add_element(element);
    defs.add_shape(Shape {
        element_id: id,
        bounds,
        is_expanded: None,
        is_horizontal: horizontal,
        label_bounds: None,
    });
}

/// A compensation association from a compensate boundary event follows the
/// boundary layout (leaves its attach side) and flags the target activity.
#[test]
fn compensation_association_follows_boundary_layout() {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));
    put(
        &mut defs,
        Element::new("host", ElementKind::Task).with_parent("p"),
        b(100.0, 100.0, 100.0, 80.0),
        None,
    );
    put(
        &mut defs,
        Element::new(
            "ev",
            ElementKind::BoundaryEvent {
                definition: EventDefinition::Compensation,
                attached_to: "host".into(),
                cancel_activity: true,
            },
        )
        .with_parent("p"),
        b(132.0, 162.0, 36.0, 36.0),
        None,
    );
    put(
        &mut defs,
        Element::new("comp", ElementKind::Task).with_parent("p"),
        b(500.0, 300.0, 100.0, 80.0),
        None,
    );

    let create = connect(&defs, &"ev".into(), &"comp".into()).expect("compensation");
    let undo = create.execute(&mut defs).expect("created");

    let route = defs.edges().next().expect("edge").waypoints.clone();
    assert_eq!(route.first(), Some(&pt(150.0, 198.0)), "{route:?}");
    assert_eq!(route.last(), Some(&pt(500.0, 340.0)), "{route:?}");
    assert_ortho(&route);
    assert_eq!(
        defs.get(&"comp".into())
            .unwrap()
            .details
            .is_for_compensation,
        Some(true)
    );

    undo.restore(&mut defs);
    assert_eq!(
        defs.get(&"comp".into())
            .unwrap()
            .details
            .is_for_compensation,
        None
    );
}
