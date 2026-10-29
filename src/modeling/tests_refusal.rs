//! Tests for the refusal rules behind the modeling feedback
//! (`rules::create_refusal`, `drop_refusal`, `connect_refusal`).

use super::rules::{Refusal, connect_refusal, create_refusal, drop_refusal};
use crate::model::{
    Bounds, Definitions, Element, ElementId, ElementKind, EventDefinition, Shape, Waypoint,
};

/// Shorthand point constructor.
pub(super) fn pt(x: f64, y: f64) -> Waypoint {
    Waypoint { x, y }
}

/// Adds element `id` of `kind` under `parent` with a DI shape.
pub(super) fn node(defs: &mut Definitions, id: &str, kind: ElementKind, parent: &str, b: [f64; 4]) {
    defs.add_element(Element::new(id, kind).with_parent(parent));
    defs.add_shape(Shape {
        element_id: id.into(),
        bounds: Bounds {
            x: b[0],
            y: b[1],
            w: b[2],
            h: b[3],
        },
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });
}

/// Collaboration `c` with pool `pool` (0,0 600x300, process `pa`) holding
/// task `task` and start event `start`, plus a bare event sub-process
/// `esp` and a task `host` in the pool.
pub(super) fn collaboration() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("c".into());
    defs.add_element(Element::new("c", ElementKind::Collaboration));
    defs.add_element(Element::new("pa", ElementKind::Process));
    node(
        &mut defs,
        "pool",
        ElementKind::Participant {
            process_ref: Some("pa".into()),
        },
        "c",
        [0.0, 0.0, 600.0, 300.0],
    );
    node(
        &mut defs,
        "task",
        ElementKind::Task,
        "pa",
        [100.0, 100.0, 100.0, 80.0],
    );
    node(
        &mut defs,
        "start",
        ElementKind::StartEvent(EventDefinition::None),
        "pa",
        [20.0, 120.0, 36.0, 36.0],
    );
    node(
        &mut defs,
        "esp",
        ElementKind::SubProcess {
            triggered_by_event: true,
        },
        "pa",
        [300.0, 100.0, 100.0, 80.0],
    );

    defs
}

/// Shorthand id list.
fn ids(names: &[&str]) -> Vec<ElementId> {
    names.iter().map(|n| ElementId::from(*n)).collect()
}

/// Creating a flow node outside every pool of a collaboration is refused;
/// inside a pool, or an artifact or pool anywhere, is not.
#[test]
fn create_flow_node_outside_pool() {
    let defs = collaboration();

    assert_eq!(
        create_refusal(&defs, &ElementKind::Task, pt(300.0, 500.0)),
        Some(Refusal::OutsideParticipant)
    );
    assert_eq!(
        create_refusal(&defs, &ElementKind::Task, pt(300.0, 150.0)),
        None
    );
    assert_eq!(
        create_refusal(
            &defs,
            &ElementKind::TextAnnotation { text: None },
            pt(300.0, 500.0)
        ),
        None
    );
    assert_eq!(
        create_refusal(
            &defs,
            &ElementKind::Participant { process_ref: None },
            pt(300.0, 500.0)
        ),
        None
    );
}

/// Creating on a plain process never refuses.
#[test]
fn create_in_process_is_allowed() {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));

    assert_eq!(
        create_refusal(&defs, &ElementKind::Task, pt(5.0, 5.0)),
        None
    );
}

/// Moving a flow node out of the pool is refused; within it, or together
/// with its pool, is not.
#[test]
fn move_out_of_pool() {
    let defs = collaboration();

    assert_eq!(
        drop_refusal(&defs, &ids(&["task"]), 0.0, 400.0),
        Some(Refusal::OutsideParticipant)
    );
    assert_eq!(drop_refusal(&defs, &ids(&["task"]), 50.0, 50.0), None);
    assert_eq!(
        drop_refusal(&defs, &ids(&["pool", "task"]), 0.0, 400.0),
        None
    );
    assert_eq!(drop_refusal(&defs, &ids(&["pool"]), 0.0, 400.0), None);
}

/// A boundary event on an event sub-process border cannot attach; on a task
/// border or in free space it can.
#[test]
fn boundary_drop_on_unattachable_host() {
    let mut defs = collaboration();
    node(
        &mut defs,
        "b",
        ElementKind::BoundaryEvent {
            definition: EventDefinition::None,
            attached_to: "task".into(),
            cancel_activity: true,
        },
        "pa",
        [132.0, 162.0, 36.0, 36.0],
    );

    assert_eq!(
        drop_refusal(&defs, &ids(&["b"]), 200.0, -80.0),
        Some(Refusal::NotAttachable)
    );
    assert_eq!(drop_refusal(&defs, &ids(&["b"]), 10.0, 0.0), None);
    assert_eq!(drop_refusal(&defs, &ids(&["b"]), 0.0, 100.0), None);
}

/// A connection the rules forbid is refused; an allowed one is not.
#[test]
fn connect_refusals() {
    let defs = collaboration();
    let (task, start) = (ElementId::from("task"), ElementId::from("start"));

    assert_eq!(
        connect_refusal(&defs, &task, &start),
        Some(Refusal::NotConnectable)
    );
    assert_eq!(connect_refusal(&defs, &start, &task), None);
}

/// Reconnection refuses a missing connection and permits a valid target.
#[test]
fn reconnect_refusals() {
    let mut defs = collaboration();
    let start = ElementId::from("start");
    let task = ElementId::from("task");
    let command = super::factory::connect(&defs, &start, &task).unwrap();
    let id = super::command::created_id(&command).unwrap();
    command.execute(&mut defs).unwrap();

    assert_eq!(
        super::rules::reconnect_refusal(&defs, &id, false, &start),
        Some(Refusal::NotConnectable)
    );
    assert_eq!(
        super::rules::reconnect_refusal(&defs, &id, false, &task),
        None
    );
}

/// Every refusal has a distinct, non-empty ASCII message.
#[test]
fn messages() {
    let all = [
        Refusal::OutsideParticipant,
        Refusal::NotConnectable,
        Refusal::NotAttachable,
    ];

    for r in all {
        assert!(!r.message().is_empty() && r.message().is_ascii());
    }
    assert_eq!(
        Refusal::OutsideParticipant.message(),
        "flow elements must be children of pools/participants"
    );
    assert_ne!(all[0].message(), all[1].message());
}
