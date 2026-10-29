//! Tests for the T4 parity commands: align, distribute, grid snap, copy /
//! paste, reconnect and label move.

use super::arrange::{Alignment, Axis, align, distribute, snap};
use super::clipboard::{copy, paste};
use super::factory::reconnect;
use super::*;
use crate::model::{Bounds, Definitions, Edge, Element, ElementId, ElementKind, Shape, Waypoint};

/// Shorthand bounds constructor.
fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, w, h }
}

/// Adds task `id` to process `p` with a DI shape at `bounds`.
fn task(defs: &mut Definitions, id: &str, bounds: Bounds) {
    defs.add_element(Element::new(id, ElementKind::Task).with_parent("p"));
    defs.add_shape(Shape {
        element_id: id.into(),
        bounds,
        is_expanded: None,
        is_horizontal: None,
        label_bounds: None,
    });
}

/// Process `p` (plane root) with tasks `a`, `b`, `c` and flow `f` from `a`
/// to `b`.
fn diagram() -> Definitions {
    let mut defs = Definitions::default();
    defs.plane_mut().element_id = Some("p".into());
    defs.add_element(Element::new("p", ElementKind::Process));

    task(&mut defs, "a", b(0.0, 0.0, 100.0, 80.0));
    task(&mut defs, "b", b(200.0, 40.0, 100.0, 80.0));
    task(&mut defs, "c", b(500.0, 100.0, 100.0, 60.0));

    let kind = ElementKind::SequenceFlow {
        source: "a".into(),
        target: "b".into(),
        is_default: false,
        condition: None,
    };
    defs.add_element(Element::new("f", kind).with_parent("p"));
    defs.add_edge(Edge {
        element_id: "f".into(),
        waypoints: vec![
            Waypoint { x: 100.0, y: 40.0 },
            Waypoint { x: 200.0, y: 80.0 },
        ],
        label_bounds: None,
    });

    defs
}

/// Ids from string slices.
fn ids(list: &[&str]) -> Vec<ElementId> {
    list.iter().map(|s| ElementId::from(*s)).collect()
}

/// Executes `command` on `defs`, asserting it changed something.
fn run(command: Command, defs: &mut Definitions) -> Undo {
    command.execute(defs).expect("command changes the diagram")
}

/// Left/right/middle alignment moves every shape to the shared edge or the
/// middle of the range (bpmn-js `AlignElements`); undo restores.
#[test]
fn align_edges_and_middle() {
    let mut defs = diagram();
    let before = defs.clone();

    let undo = run(
        align(&defs, &ids(&["a", "b", "c"]), Alignment::Left).unwrap(),
        &mut defs,
    );
    for id in ["a", "b", "c"] {
        assert_eq!(defs.shape(&id.into()).unwrap().bounds.x, 0.0);
    }
    undo.restore(&mut defs);
    assert_eq!(defs, before);

    run(
        align(&defs, &ids(&["a", "c"]), Alignment::Bottom).unwrap(),
        &mut defs,
    );
    assert_eq!(defs.shape(&"a".into()).unwrap().bounds.y, 80.0);

    let mut defs = before.clone();
    run(
        align(&defs, &ids(&["a", "c"]), Alignment::Middle).unwrap(),
        &mut defs,
    );
    let a = defs.shape(&"a".into()).unwrap().bounds;
    let c = defs.shape(&"c".into()).unwrap().bounds;
    assert_eq!(a.y + a.h / 2.0, 80.0);
    assert_eq!(c.y + c.h / 2.0, 80.0);
}

/// Aligning fewer than two shapes is refused.
#[test]
fn align_needs_two() {
    let defs = diagram();

    assert!(align(&defs, &ids(&["a"]), Alignment::Left).is_none());
    assert!(align(&defs, &ids(&["a", "f"]), Alignment::Left).is_none());
}

/// Distribution keeps the outer shapes and equalizes the gaps between them
/// (bpmn-js `DistributeElements`); fewer than three is refused.
#[test]
fn distribute_equal_gaps() {
    let mut defs = diagram();

    assert!(distribute(&defs, &ids(&["a", "b"]), Axis::Horizontal).is_none());

    run(
        distribute(&defs, &ids(&["c", "a", "b"]), Axis::Horizontal).unwrap(),
        &mut defs,
    );
    let x = |id: &str| defs.shape(&id.into()).unwrap().bounds.x;
    assert_eq!((x("a"), x("b"), x("c")), (0.0, 250.0, 500.0));
}

/// Grid snapping rounds to the nearest 10px line.
#[test]
fn snap_to_grid() {
    assert_eq!(snap(174.0), 170.0);
    assert_eq!(snap(175.0), 180.0);
    assert_eq!(snap(-4.0), 0.0);
}

/// Pasting copies the selection and the flow between copied nodes with
/// fresh ids, centered at the paste point; undo removes them all.
#[test]
fn copy_paste_regenerates_ids() {
    let mut defs = diagram();
    let before = defs.clone();
    let clip = copy(&defs, &ids(&["a", "b"])).unwrap();

    let (command, roots) = paste(&defs, &clip, Waypoint { x: 150.0, y: 400.0 }).unwrap();
    let undo = run(command, &mut defs);

    assert_eq!(roots.len(), 2);
    assert!(roots.iter().all(|id| before.get(id).is_none()));
    assert_eq!(defs.elements.len(), before.elements.len() + 3);

    let pasted = defs.get(&roots[0]).unwrap();
    assert_eq!(pasted.parent, Some("p".into()));
    let first = defs.shape(&roots[0]).unwrap().bounds;
    assert_eq!((first.x, first.y), (0.0, 340.0));

    let flow = defs
        .elements
        .values()
        .find(|e| e.kind.endpoints() == Some((&roots[0], &roots[1])))
        .expect("pasted flow");
    assert!(defs.edges().any(|e| e.element_id == flow.id));

    undo.restore(&mut defs);
    assert_eq!(defs, before);
}

/// Pasting twice yields distinct ids each time.
#[test]
fn paste_twice_unique() {
    let mut defs = diagram();
    let clip = copy(&defs, &ids(&["a"])).unwrap();

    let (first, a) = paste(&defs, &clip, Waypoint { x: 0.0, y: 300.0 }).unwrap();
    run(first, &mut defs);
    let (second, b) = paste(&defs, &clip, Waypoint { x: 0.0, y: 500.0 }).unwrap();
    run(second, &mut defs);

    assert_ne!(a, b);
}

/// Reconnecting the target of a flow onto another task re-routes it; a
/// target the rules reject (the source's own start, here a process) is refused.
#[test]
fn reconnect_target() {
    let mut defs = diagram();

    let command = reconnect(&defs, &"f".into(), false, &"c".into()).unwrap();
    run(command, &mut defs);
    let flow = defs.get(&"f".into()).unwrap();
    assert_eq!(flow.kind.endpoints(), Some((&"a".into(), &"c".into())));

    let end = *defs
        .edges()
        .find(|e| e.element_id == "f".into())
        .unwrap()
        .waypoints
        .last()
        .unwrap();
    let c = defs.shape(&"c".into()).unwrap().bounds;
    assert!(end.x >= c.x && end.x <= c.x + c.w);

    assert!(reconnect(&defs, &"f".into(), false, &"p".into()).is_none());
}

/// Moving a label translates its bounds only, starting from the default
/// spot for a flow without DI label; undo restores it.
#[test]
fn move_label_alone() {
    let mut defs = diagram();
    let before = defs.clone();

    let undo = run(
        Command::MoveLabel {
            id: "f".into(),
            dx: 10.0,
            dy: 20.0,
        },
        &mut defs,
    );
    let edge = defs.edges().find(|e| e.element_id == "f".into()).unwrap();
    let default = super::label::flow_label_bounds(&edge.waypoints).unwrap();
    let label = edge.label_bounds.unwrap();
    assert_eq!((label.x, label.y), (default.x + 10.0, default.y + 20.0));

    undo.restore(&mut defs);
    assert_eq!(defs, before);
}
