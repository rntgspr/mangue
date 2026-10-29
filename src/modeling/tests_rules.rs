//! Tests for connection and attachment rules.

use super::rules::{Connection, can_connect, container_at};
use super::tests_refusal::{
    collaboration as refusal_collaboration, node as refusal_node, pt as refusal_pt,
};
use crate::model::{Bounds, Definitions, Element, ElementKind, EventDefinition, Shape, Waypoint};

/// Connection starts exclude lanes and groups while allowing tasks.
#[test]
fn connection_start_rules() {
    assert!(!super::rules::can_start_connection(&ElementKind::Lane));
    assert!(!super::rules::can_start_connection(&ElementKind::Group));
    assert!(super::rules::can_start_connection(&ElementKind::Task));
}

/// Host lookup selects the topmost eligible activity within the attachment padding.
#[test]
fn attachment_host_is_topmost() {
    let mut defs = refusal_collaboration();
    refusal_node(
        &mut defs,
        "upper",
        ElementKind::Task,
        "pa",
        [100.0, 100.0, 100.0, 80.0],
    );
    refusal_node(
        &mut defs,
        "event",
        ElementKind::IntermediateThrowEvent(EventDefinition::None),
        "pa",
        [0.0, 0.0, 36.0, 36.0],
    );

    assert_eq!(
        super::attach::host_at(&defs, &"event".into(), refusal_pt(100.0, 90.0)),
        Some("upper".into())
    );
    assert_eq!(
        super::attach::host_at(&defs, &"event".into(), refusal_pt(100.0, 84.0)),
        None
    );
}

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

/// Shorthand for `can_connect` over string ids.
fn rule(defs: &Definitions, s: &str, t: &str) -> Option<Connection> {
    can_connect(defs, &s.into(), &t.into())
}

/// Sequence flows stay in one pool and respect start/end/boundary roles.
#[test]
fn rules_sequence_flow() {
    let mut defs = collaboration();
    node(
        &mut defs,
        "bnd",
        ElementKind::BoundaryEvent {
            definition: EventDefinition::Timer,
            attached_to: "task".into(),
            cancel_activity: true,
        },
        "pa",
        b(200.0, 140.0, 36.0, 36.0),
    );

    assert_eq!(rule(&defs, "start", "task"), Some(Connection::Sequence));
    assert_eq!(rule(&defs, "task", "end"), Some(Connection::Sequence));
    assert_eq!(rule(&defs, "bnd", "end"), Some(Connection::Sequence));
    assert_eq!(rule(&defs, "end", "task"), None);
    assert_eq!(rule(&defs, "task", "start"), None);
    assert_eq!(rule(&defs, "start", "bnd"), None);
    assert_eq!(rule(&defs, "task", "task"), Some(Connection::Sequence));
}

/// Message flows join different pools only, from senders to receivers.
#[test]
fn rules_message_flow() {
    let defs = collaboration();

    assert_eq!(rule(&defs, "task", "t2"), Some(Connection::Message));
    assert_eq!(rule(&defs, "end", "t2"), Some(Connection::Message));
    assert_eq!(rule(&defs, "t2", "start"), Some(Connection::Message));
    assert_eq!(rule(&defs, "a", "b"), Some(Connection::Message));
    assert_eq!(rule(&defs, "start", "t2"), None);
    assert_eq!(rule(&defs, "t2", "end"), None);
    assert_eq!(rule(&defs, "a", "task"), None);
}

/// Annotations get associations; data gets data associations with activities.
#[test]
fn rules_associations() {
    let defs = collaboration();

    assert_eq!(rule(&defs, "task", "note"), Some(Connection::Association));
    assert_eq!(rule(&defs, "note", "t2"), Some(Connection::Association));
    assert_eq!(rule(&defs, "data", "task"), Some(Connection::DataInput));
    assert_eq!(rule(&defs, "task", "data"), Some(Connection::DataOutput));
    assert_eq!(rule(&defs, "start", "data"), Some(Connection::DataOutput));
    assert_eq!(rule(&defs, "note", "note"), None);
}

/// The drop container is the topmost pool/sub-process under the point.
#[test]
fn container_hit_test() {
    let defs = collaboration();

    assert_eq!(container_at(&defs, pt(10.0, 10.0), &[]), Some("pa".into()));
    assert_eq!(container_at(&defs, pt(10.0, 400.0), &[]), Some("pb".into()));
    assert_eq!(container_at(&defs, pt(10.0, 270.0), &[]), None);
    assert_eq!(
        container_at(&process(), pt(-500.0, 0.0), &[]),
        Some("p".into())
    );
}
