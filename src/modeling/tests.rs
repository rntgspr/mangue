use super::*;
use crate::model::{
    Bounds, Definitions, Edge, Element, ElementId, ElementKind, EventDefinition, Shape, Waypoint,
};

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}

/// Shorthand point constructor.
fn pt(x: f64, y: f64) -> Waypoint {
    Waypoint { x, y }
}

/// Shorthand DI shape constructor.
fn shape(id: &str, bounds: Bounds) -> Shape {
    Shape {
        element_id: id.into(),
        bounds,
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    }
}

/// Shorthand sequence-flow kind.
fn flow(source: &str, target: &str) -> ElementKind {
    ElementKind::SequenceFlow {
        source: source.into(),
        target: target.into(),
        is_default: false,
        condition: None,
    }
}

/// Process plane: start(0,0) -> task(100,0) -> end(300,0), a boundary event
/// on the task and an expanded sub-process at (0, 300).
fn process() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());

    defs.add_element(Element::new("p", ElementKind::Process));
    defs.add_element(
        Element::new("start", ElementKind::StartEvent(EventDefinition::None)).with_parent("p"),
    );
    defs.add_element(Element::new("task", ElementKind::Task).with_parent("p"));
    defs.add_element(
        Element::new("end", ElementKind::EndEvent(EventDefinition::None)).with_parent("p"),
    );
    defs.add_element(
        Element::new(
            "boundary",
            ElementKind::BoundaryEvent {
                definition: EventDefinition::Timer,
                attached_to: "task".into(),
                cancel_activity: true,
            },
        )
        .with_parent("p"),
    );
    defs.add_element(
        Element::new(
            "sub",
            ElementKind::SubProcess {
                triggered_by_event: false,
            },
        )
        .with_parent("p"),
    );
    defs.add_element(Element::new("f1", flow("start", "task")).with_parent("p"));
    defs.add_element(Element::new("f2", flow("task", "end")).with_parent("p"));

    defs.add_shape(shape("start", b(0.0, 22.0, 36.0, 36.0)));
    defs.add_shape(shape("task", b(100.0, 0.0, 100.0, 80.0)));
    defs.add_shape(shape("end", b(300.0, 22.0, 36.0, 36.0)));
    defs.add_shape(shape("boundary", b(150.0, 62.0, 36.0, 36.0)));
    defs.add_shape(Shape {
        is_expanded: Some(true),
        ..shape("sub", b(0.0, 300.0, 350.0, 200.0))
    });

    defs.add_edge(Edge {
        element_id: "f1".into(),
        waypoints: vec![pt(36.0, 40.0), pt(100.0, 40.0)],
        label_bounds: None,
    });
    defs.add_edge(Edge {
        element_id: "f2".into(),
        waypoints: vec![pt(200.0, 40.0), pt(300.0, 40.0)],
        label_bounds: None,
    });

    for (id, di) in [("start", "S_di"), ("task", "T_di"), ("f1", "F1_di")] {
        defs.plane_mut().di_ids.insert(id.into(), di.into());
    }

    defs
}

/// Executes a move of `ids` by `(dx, dy)` on a fresh stack.
fn moved(defs: &mut Definitions, ids: &[&str], dx: f64, dy: f64) -> CommandStack {
    let mut stack = CommandStack::default();
    let ids = ids.iter().map(|i| ElementId::from(*i)).collect();

    assert!(stack.execute(Command::MoveShapes { ids, dx, dy }, defs));
    stack
}

/// Bounds of the shape of `id`.
fn bounds(defs: &Definitions, id: &str) -> Bounds {
    defs.shape(&id.into()).unwrap().bounds
}

/// Waypoints of the edge of `id`.
fn waypoints(defs: &Definitions, id: &str) -> Vec<Waypoint> {
    defs.edges()
        .find(|e| e.element_id.as_str() == id)
        .unwrap()
        .waypoints
        .clone()
}

/// Moving a shape drags its boundary event and re-routes one-sided edges.
#[test]
fn move_drags_boundary_and_reroutes() {
    let mut defs = process();
    let before = defs.clone();
    let mut stack = moved(&mut defs, &["task"], 0.0, 100.0);

    assert_eq!(bounds(&defs, "task"), b(100.0, 100.0, 100.0, 80.0));
    assert_eq!(bounds(&defs, "boundary"), b(150.0, 162.0, 36.0, 36.0));
    assert_eq!(
        waypoints(&defs, "f1"),
        [
            pt(36.0, 40.0),
            pt(68.0, 40.0),
            pt(68.0, 140.0),
            pt(100.0, 140.0)
        ]
    );

    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);

    assert!(stack.redo(&mut defs));
    assert_eq!(bounds(&defs, "task").y, 100.0);
}

/// When both ends move, edge waypoints are translated, not re-routed.
#[test]
fn move_both_ends_translates_edge() {
    let mut defs = process();
    moved(&mut defs, &["start", "task"], 10.0, 5.0);

    assert_eq!(waypoints(&defs, "f1"), [pt(46.0, 45.0), pt(110.0, 45.0)]);
}

/// Dropping a node into an expanded sub-process reparents it; undo restores.
#[test]
fn move_into_subprocess_reparents() {
    let mut defs = process();
    let before = defs.clone();
    let mut stack = moved(&mut defs, &["end"], 0.0, 350.0);

    assert_eq!(defs.get(&"end".into()).unwrap().parent, Some("sub".into()));

    stack.undo(&mut defs);
    assert_eq!(defs, before);
}

/// Moving an expanded sub-process moves its children.
#[test]
fn move_container_moves_children() {
    let mut defs = process();
    defs.elements
        .get_mut(&ElementId::from("end"))
        .unwrap()
        .parent = Some("sub".into());
    moved(&mut defs, &["sub"], 20.0, 0.0);

    assert_eq!(bounds(&defs, "end").x, 320.0);
    assert_eq!(bounds(&defs, "sub").x, 20.0);
}

/// Moving a participant moves the content of its process, including lanes.
#[test]
fn move_participant_moves_process_content() {
    let mut defs = Definitions::default();
    defs.add_element(Element::new("c", ElementKind::Collaboration));
    defs.add_element(
        Element::new(
            "pool",
            ElementKind::Participant {
                process_ref: Some("p".into()),
            },
        )
        .with_parent("c"),
    );
    defs.add_element(Element::new("p", ElementKind::Process));
    defs.add_element(Element::new("lane", ElementKind::Lane).with_parent("p"));
    defs.add_element(Element::new("t", ElementKind::Task).with_parent("lane"));
    defs.add_shape(shape("pool", b(0.0, 0.0, 600.0, 250.0)));
    defs.add_shape(shape("lane", b(30.0, 0.0, 570.0, 250.0)));
    defs.add_shape(shape("t", b(100.0, 50.0, 100.0, 80.0)));

    moved(&mut defs, &["pool"], 0.0, 40.0);

    assert_eq!(bounds(&defs, "lane").y, 40.0);
    assert_eq!(bounds(&defs, "t").y, 90.0);
    assert_eq!(defs.get(&"t".into()).unwrap().parent, Some("lane".into()));
}

/// A zero move is a no-op and is not recorded.
#[test]
fn zero_move_not_recorded() {
    let mut defs = process();
    let mut stack = CommandStack::default();
    let cmd = Command::MoveShapes {
        ids: vec!["task".into()],
        dx: 0.0,
        dy: 0.0,
    };

    assert!(!stack.execute(cmd, &mut defs));
    assert!(!stack.can_undo());
}

/// Deleting a node removes its boundary event and flows; undo restores order.
#[test]
fn delete_cascades_and_undo_restores_order() {
    let mut defs = process();
    let before = defs.clone();
    let mut stack = CommandStack::default();

    let cmd = Command::DeleteElements {
        ids: vec!["task".into()],
    };
    assert!(stack.execute(cmd, &mut defs));

    for id in ["task", "boundary", "f1", "f2"] {
        assert!(defs.get(&id.into()).is_none(), "{id} still present");
    }
    assert_eq!(defs.shapes().count(), 3);
    assert_eq!(defs.edges().count(), 0);

    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);
}

/// Creating a shape and a routed connection; undo removes both.
#[test]
fn create_shape_and_connection() {
    let mut defs = process();
    let before = defs.clone();
    let mut stack = CommandStack::default();

    stack.execute(
        Command::CreateShape {
            element: Element::new("t2", ElementKind::UserTask).with_parent("p"),
            shape: shape("t2", b(100.0, 150.0, 100.0, 80.0)),
        },
        &mut defs,
    );
    stack.execute(
        Command::CreateConnection {
            element: Element::new("f3", flow("task", "t2")).with_parent("p"),
            edge: Edge {
                element_id: "f3".into(),
                waypoints: Vec::new(),
                label_bounds: None,
            },
        },
        &mut defs,
    );

    assert_eq!(waypoints(&defs, "f3"), [pt(150.0, 80.0), pt(150.0, 150.0)]);

    stack.undo(&mut defs);
    stack.undo(&mut defs);
    assert_eq!(defs, before);
    assert!(stack.can_redo());
}

/// Creating an element with an existing id is a no-op.
#[test]
fn create_duplicate_is_noop() {
    let mut defs = process();
    let cmd = Command::CreateShape {
        element: Element::new("task", ElementKind::Task),
        shape: shape("task", b(0.0, 0.0, 1.0, 1.0)),
    };

    assert!(cmd.execute(&mut defs).is_none());
}

/// Label updates set the name, or the text of a text annotation.
#[test]
fn update_label() {
    let mut defs = process();
    defs.add_element(Element::new(
        "note",
        ElementKind::TextAnnotation { text: None },
    ));
    let mut stack = CommandStack::default();

    stack.execute(
        Command::UpdateLabel {
            id: "task".into(),
            name: "Review".into(),
        },
        &mut defs,
    );
    stack.execute(
        Command::UpdateLabel {
            id: "note".into(),
            name: "Hint".into(),
        },
        &mut defs,
    );

    assert_eq!(
        defs.get(&"task".into()).unwrap().name.as_deref(),
        Some("Review")
    );
    assert_eq!(
        defs.get(&"note".into()).unwrap().kind,
        ElementKind::TextAnnotation {
            text: Some("Hint".into())
        }
    );

    stack.undo(&mut defs);
    stack.undo(&mut defs);
    assert_eq!(defs.get(&"task".into()).unwrap().name, None);
}

/// Resizing re-routes attached connections.
#[test]
fn resize_reroutes() {
    let mut defs = process();
    let cmd = Command::ResizeShape {
        id: "task".into(),
        bounds: b(100.0, 0.0, 150.0, 80.0),
    };
    cmd.execute(&mut defs).unwrap();

    assert_eq!(waypoints(&defs, "f2"), [pt(250.0, 40.0), pt(300.0, 40.0)]);
}

/// A new command after undo clears the redo history.
#[test]
fn execute_clears_redo() {
    let mut defs = process();
    let mut stack = moved(&mut defs, &["task"], 5.0, 0.0);

    stack.undo(&mut defs);
    assert!(stack.can_redo());

    let cmd = Command::MoveShapes {
        ids: vec!["end".into()],
        dx: 1.0,
        dy: 0.0,
    };
    stack.execute(cmd, &mut defs);
    assert!(!stack.can_redo());
    assert!(!stack.redo(&mut defs));
}

/// Undo of an entry whose element another side changed is a no-op.
#[test]
fn undo_of_stale_entry_is_noop() {
    let mut defs = process();
    let mut stack = moved(&mut defs, &["task"], 5.0, 0.0);
    let before = defs.clone();
    defs.elements
        .get_mut(&ElementId::from("task"))
        .unwrap()
        .name = Some("edited".into());

    stack.invalidate(&before, &defs);

    let edited = defs.clone();
    assert!(!stack.undo(&mut defs));
    assert_eq!(defs, edited);
}

/// Undo of an entry untouched by the other side still reverts it.
#[test]
fn undo_of_untouched_entry_still_works() {
    let mut defs = process();
    let mut stack = moved(&mut defs, &["task"], 5.0, 0.0);
    let before = defs.clone();
    defs.elements.get_mut(&ElementId::from("end")).unwrap().name = Some("edited".into());

    stack.invalidate(&before, &defs);

    assert!(stack.undo(&mut defs));
    assert_eq!(bounds(&defs, "task").x, 100.0);
}

/// Redo of an entry whose element another side changed is a no-op.
#[test]
fn redo_of_stale_entry_is_noop() {
    let mut defs = process();
    let mut stack = moved(&mut defs, &["task"], 5.0, 0.0);
    stack.undo(&mut defs);
    let before = defs.clone();
    defs.elements
        .get_mut(&ElementId::from("task"))
        .unwrap()
        .name = Some("edited".into());

    stack.invalidate(&before, &defs);

    let edited = defs.clone();
    assert!(!stack.redo(&mut defs));
    assert_eq!(defs, edited);
}

/// Commands executed after an invalidation are not stale.
#[test]
fn entries_after_invalidate_are_fresh() {
    let mut defs = process();
    let mut stack = CommandStack::default();
    let before = defs.clone();
    defs.elements
        .get_mut(&ElementId::from("task"))
        .unwrap()
        .name = Some("edited".into());
    stack.invalidate(&before, &defs);

    let ids = vec![ElementId::from("task")];
    assert!(stack.execute(
        Command::MoveShapes {
            ids,
            dx: 5.0,
            dy: 0.0
        },
        &mut defs
    ));

    assert!(stack.undo(&mut defs));
}
