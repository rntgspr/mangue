//! Tests of sub-process planes: collapse/expand moving content between
//! planes (bpmn-js `SubProcessPlaneBehavior`), plane lifecycle on create
//! and delete, and undo/redo re-opening the plane a command ran on.

use super::{Command, CommandStack, factory};
use crate::io::import_xml;
use crate::model::{Definitions, ElementId, ElementKind, Waypoint};

/// bpmn-js collapsed sub-process fixture (children on `SubProcess_1`'s plane).
const COLLAPSED: &str = include_str!("../../tests/fixtures/collapsed-subprocess.bpmn");

/// The fixture, imported.
fn collapsed() -> Definitions {
    let mut defs = import_xml(COLLAPSED).expect("fixture imports");
    super::compat_planes(&mut defs);
    defs
}

/// Id `SubProcess_1`.
fn sub() -> ElementId {
    "SubProcess_1".into()
}

/// Expanding moves the plane content onto the root plane centered on the
/// new 350x200 shape (content box 180..468 x 160..240, so the task
/// center lands at 320 - 4) and drops the plane; collapsing again recreates
/// `SubProcess_1_plane` with the content's top-left at (180, 160).
#[test]
fn toggle_moves_content_between_planes() {
    let mut defs = collapsed();
    let mut stack = CommandStack::default();

    assert!(stack.execute(Command::ToggleCollapse { id: sub() }, &mut defs));
    assert_eq!(defs.diagrams.len(), 1);
    let shape = defs.shape(&sub()).unwrap();
    assert_eq!(shape.is_expanded, Some(true));
    assert_eq!((shape.bounds.w, shape.bounds.h), (350.0, 200.0));
    let task = defs
        .shape(&"Task_1".into())
        .expect("task on root plane")
        .bounds;
    assert_eq!(
        (task.x + task.w / 2.0, task.y + task.h / 2.0),
        (316.0, 120.0)
    );

    assert!(stack.execute(Command::ToggleCollapse { id: sub() }, &mut defs));
    assert_eq!(defs.diagrams.len(), 2);
    let plane = &defs.diagrams[1].plane;
    assert_eq!(plane.id, "SubProcess_1_plane");
    assert_eq!(defs.diagrams[1].id, "BPMNDiagram_SubProcess_1");
    let min_x = plane
        .shapes
        .iter()
        .map(|s| s.bounds.x)
        .fold(f64::MAX, f64::min);
    let min_y = plane
        .shapes
        .iter()
        .map(|s| s.bounds.y)
        .fold(f64::MAX, f64::min);
    assert_eq!((min_x, min_y), (180.0, 160.0));
    assert_eq!(plane.edges.len(), 2);
    assert!(defs.shape(&"Task_1".into()).is_none());
    assert_eq!(defs.shape(&sub()).unwrap().bounds.w, 100.0);
}

/// Undoing both toggles restores the imported document exactly; redo
/// replays them.
#[test]
fn toggle_undo_redo() {
    let original = collapsed();
    let mut defs = original.clone();
    let mut stack = CommandStack::default();

    stack.execute(Command::ToggleCollapse { id: sub() }, &mut defs);
    let expanded = defs.clone();
    stack.execute(Command::ToggleCollapse { id: sub() }, &mut defs);

    assert!(stack.undo(&mut defs));
    assert_eq!(defs, expanded);
    assert!(stack.undo(&mut defs));
    assert_eq!(defs, original);

    assert!(stack.redo(&mut defs));
    assert_eq!(defs, expanded);
}

/// Creating a collapsed sub-process adds its empty plane (bpmn-js creates
/// the drill-down root on `shape.create`); undo removes it.
#[test]
fn create_collapsed_adds_plane() {
    let mut defs = collapsed();
    let mut stack = CommandStack::default();
    let kind = ElementKind::SubProcess {
        triggered_by_event: false,
    };
    let command = factory::create_shape(&defs, kind, false, Waypoint { x: 600.0, y: 120.0 })
        .expect("create command");
    let Command::CreateShape { element, .. } = &command else {
        panic!("not a create");
    };
    let id = element.id.clone();

    assert!(stack.execute(command, &mut defs));
    let index = defs.plane_index(&id).expect("plane");
    assert!(defs.diagrams[index].plane.shapes.is_empty());

    stack.undo(&mut defs);
    assert_eq!(defs, collapsed());
}

/// Deleting a collapsed sub-process removes its children and plane; undo
/// restores both.
#[test]
fn delete_collapsed_removes_plane() {
    let mut defs = collapsed();
    let mut stack = CommandStack::default();

    stack.execute(Command::DeleteElements { ids: vec![sub()] }, &mut defs);
    assert_eq!(defs.diagrams.len(), 1);
    assert!(defs.get(&"Task_1".into()).is_none());

    stack.undo(&mut defs);
    assert_eq!(defs, collapsed());
}

/// Shapes created while drilled down go to the sub-process plane with the
/// sub-process as parent; undo from the root plane re-opens that plane.
#[test]
fn edits_follow_their_plane() {
    let mut defs = collapsed();
    let mut stack = CommandStack::default();
    assert!(defs.open_plane(Some(&sub())));

    let command = factory::create_shape(
        &defs,
        ElementKind::Task,
        false,
        Waypoint { x: 600.0, y: 400.0 },
    )
    .expect("create command");
    assert!(stack.execute(command, &mut defs));
    let created = defs.plane().shapes.last().unwrap().element_id.clone();
    assert_eq!(defs.get(&created).unwrap().parent, Some(sub()));
    assert!(factory::create_participant(&defs, Waypoint { x: 0.0, y: 0.0 }).is_none());

    defs.open_plane(None);
    assert!(defs.shape(&created).is_none());

    stack.undo(&mut defs);
    assert_eq!(defs.plane_owner(), Some(&sub()));
    assert!(defs.get(&created).is_none());
    assert_eq!(defs.diagrams[1].plane.shapes.len(), 3);
}

/// `layout::orthogonalize_all` rewrites diagonal edges on sub-process
/// planes too, keeping the active plane.
#[test]
fn orthogonalize_all_walks_every_plane() {
    let mut defs = collapsed();
    defs.diagrams[1].plane.edges[0].waypoints[1].y += 30.0;

    super::layout::orthogonalize_all(&mut defs);

    assert_eq!(defs.active, 0);
    let wps = &defs.diagrams[1].plane.edges[0].waypoints;
    assert!(wps.len() >= 2);
    assert!(wps.windows(2).all(|w| w[0].x == w[1].x || w[0].y == w[1].y));
}

/// Replacing the collapsed sub-process with a task drops its plane and the
/// content drawn on it (bpmn-js `SubProcessPlaneBehavior`); undo restores
/// both, redo drops them again.
#[test]
fn replace_with_task_drops_plane() {
    let mut defs = collapsed();
    let before = defs.clone();
    let mut stack = CommandStack::default();
    let task = ElementId::from("Task_1");

    let kind = ElementKind::Task;
    assert!(stack.execute(Command::ReplaceElement { id: sub(), kind }, &mut defs));
    assert_eq!(defs.diagrams.len(), 1);
    assert!(defs.get(&task).is_none());
    assert_eq!(defs.get(&sub()).unwrap().kind, ElementKind::Task);

    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);

    assert!(stack.redo(&mut defs));
    assert_eq!(defs.diagrams.len(), 1);
    assert!(defs.get(&task).is_none());
}

/// A legacy file drawing the collapsed sub-process content on the root
/// plane is normalized with that content moved to the sub-process's own plane,
/// top-left at (180, 160) (bpmn-js `SubprocessCompatibility`).
#[test]
fn import_moves_legacy_content_to_plane() {
    let seam = "    </bpmndi:BPMNPlane>\n  </bpmndi:BPMNDiagram>\n  \
        <bpmndi:BPMNDiagram id=\"BPMNDiagram_0p6a7cm\">\n    \
        <bpmndi:BPMNPlane id=\"BPMNPlane_1h2xq9e\" bpmnElement=\"SubProcess_1\">\n";
    assert!(COLLAPSED.contains(seam));
    let legacy = COLLAPSED.replace(seam, "");
    let mut defs = import_xml(&legacy).expect("legacy imports");

    assert_eq!(defs.diagrams.len(), 1);
    super::compat_planes(&mut defs);
    assert_eq!(defs.diagrams.len(), 2);
    assert!(defs.shape(&"Task_1".into()).is_none());
    assert!(defs.open_plane(Some(&sub())));
    let start = defs.shape(&"StartEvent_2".into()).unwrap().bounds;
    assert_eq!((start.x, start.y), (180.0, 182.0));
    assert!(defs.shape(&"Task_1".into()).is_some());
}

/// Copying the collapsed sub-process and pasting it gives the copy a plane
/// of its own holding fresh copies of the content (bpmn-js copies the
/// sub-process children); undo removes copy and plane.
#[test]
fn copy_paste_keeps_plane_content() {
    use crate::modeling::clipboard;

    let mut defs = collapsed();
    let before = defs.clone();
    let mut stack = CommandStack::default();

    let clip = clipboard::copy(&defs, &[sub()]).expect("copyable");
    let at = Waypoint { x: 320.0, y: 400.0 };
    let (command, roots) = clipboard::paste(&defs, &clip, at).expect("pastes");
    assert!(stack.execute(command, &mut defs));

    let copy = roots[0].clone();
    assert_ne!(copy, sub());
    assert_eq!(defs.diagrams.len(), 3);
    assert!(defs.open_plane(Some(&copy)));
    assert_eq!(defs.plane().shapes.len(), 3);
    assert_eq!(defs.plane().edges.len(), 2);
    assert!(
        defs.plane().shapes.iter().all(|s| {
            defs.get(&s.element_id).and_then(|e| e.parent.clone()) == Some(copy.clone())
        })
    );
    defs.open_plane(None);

    assert!(stack.undo(&mut defs));
    assert_eq!(defs, before);
}
