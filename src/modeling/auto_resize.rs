//! Auto-resize after diagram-js `AutoResize` with bpmn-js
//! `BpmnAutoResize` / `BpmnAutoResizeProvider`: when moved or created
//! shapes end up closer than the padding to (or beyond) the border of their
//! participant or expanded sub-process, that container grows by the offset
//! on those sides, in the same undoable step; growth propagates upward.

use super::Command;
use super::lanes;
use crate::model::{Bounds, Definitions, ElementId, ElementKind};

/// Minimum distance kept between content and container border (diagram-js
/// `AutoResize.getPadding`): top, right, bottom, left.
const PADDING: [f64; 4] = [2.0, 15.0, 2.0, 15.0];

/// Room added past the content on a side that grows (diagram-js
/// `AutoResize.getOffset`): top, right, bottom, left.
const OFFSET: [f64; 4] = [60.0, 100.0, 60.0, 100.0];

/// `command` extended with the container resizes the shapes it moves or
/// creates call for, evaluated on the document after it runs; the command
/// itself when nothing needs to grow.
pub fn with_auto_resize(defs: &Definitions, command: Command) -> Command {
    let shapes = touched(&command);
    if shapes.is_empty() {
        return command;
    }

    let mut after = defs.clone();
    if command.execute(&mut after).is_none() {
        return command;
    }

    let mut resizes = Vec::new();
    let mut moved = shapes;
    for _ in 0..after.elements.len() {
        let grown = grow(&after, &moved);
        if grown.is_empty() {
            break;
        }

        moved.clear();
        for (container, resize) in grown {
            if resize.execute(&mut after).is_some() {
                resizes.push(resize);
                moved.push(container);
            }
        }
    }

    if resizes.is_empty() {
        return command;
    }

    let mut commands = vec![command];
    commands.extend(resizes);
    Command::Compound(commands)
}

/// Resize commands, with their container, for every container whose
/// listed children (lanes excluded, like bpmn-js) come within `PADDING` of
/// its border.
fn grow(defs: &Definitions, ids: &[ElementId]) -> Vec<(ElementId, Command)> {
    let mut groups: Vec<(ElementId, Bounds)> = Vec::new();

    for id in ids {
        let Some(element) = defs.get(id) else {
            continue;
        };
        if matches!(element.kind, ElementKind::Lane) {
            continue;
        }
        let (Some(container), Some(shape)) = (container(defs, id), defs.shape(id)) else {
            continue;
        };

        match groups.iter_mut().find(|(c, _)| *c == container) {
            Some((_, b)) => *b = union(b, &shape.bounds),
            None => groups.push((container, shape.bounds)),
        }
    }

    groups
        .into_iter()
        .filter_map(|(container, content)| {
            let target = defs.shape(&container)?.bounds;
            let bounds = optimal(&target, &content);
            if bounds == target {
                return None;
            }

            Some((container.clone(), lanes::resize(defs, &container, bounds)?))
        })
        .collect()
}

/// diagram-js `AutoResize._getOptimalBounds`: each side of `target` closer
/// than the padding to `content` moves to the content plus the offset.
fn optimal(target: &Bounds, content: &Bounds) -> Bounds {
    let [pt, pr, pb, pl] = PADDING;
    let [ot, or, ob, ol] = OFFSET;

    let mut top = target.y;
    let mut right = target.x + target.w;
    let mut bottom = target.y + target.h;
    let mut left = target.x;

    if content.y - top < pt {
        top = content.y - ot;
    }
    if content.x - left < pl {
        left = content.x - ol;
    }
    if right - (content.x + content.w) < pr {
        right = content.x + content.w + or;
    }
    if bottom - (content.y + content.h) < pb {
        bottom = content.y + content.h + ob;
    }

    Bounds {
        x: left,
        y: top,
        w: right - left,
        h: bottom - top,
    }
}

/// Auto-resizable container `id` is painted in: its expanded sub-process
/// shape or the participant of its process (lanes skipped); `None` on the
/// plane root.
fn container(defs: &Definitions, id: &ElementId) -> Option<ElementId> {
    let element = defs.get(id)?;
    let mut parent = match &element.kind {
        ElementKind::BoundaryEvent { attached_to, .. } => defs.get(attached_to)?.parent.clone(),
        _ => element.parent.clone(),
    };

    for _ in 0..defs.elements.len() {
        let p = parent?;
        let e = defs.get(&p)?;

        match &e.kind {
            ElementKind::Lane => parent = e.parent.clone(),

            ElementKind::Process => {
                return defs
                    .elements
                    .values()
                    .find(|o| {
                        matches!(&o.kind, ElementKind::Participant { process_ref: Some(r) } if *r == p)
                    })
                    .map(|o| o.id.clone())
                    .filter(|o| defs.shape(o).is_some());
            }

            ElementKind::SubProcess { .. } => {
                let expanded = defs.shape(&p).is_some_and(|s| s.is_expanded == Some(true));
                return expanded.then_some(p);
            }

            _ => return None,
        }
    }

    None
}

/// Shapes a command moves or creates (top level of compounds), for which
/// containers may need to grow.
fn touched(command: &Command) -> Vec<ElementId> {
    match command {
        Command::MoveShapes { ids, .. } => ids.clone(),
        Command::CreateShape { element, .. } => vec![element.id.clone()],
        Command::AttachBoundary { id, .. } => vec![id.clone()],
        Command::Compound(commands) => commands.iter().flat_map(touched).collect(),
        _ => Vec::new(),
    }
}

/// Smallest bounds containing `a` and `b`.
fn union(a: &Bounds, b: &Bounds) -> Bounds {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));

    Bounds {
        x,
        y,
        w: (a.x + a.w).max(b.x + b.w) - x,
        h: (a.y + a.h).max(b.y + b.h) - y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Waypoint;
    use crate::modeling::{CommandStack, factory};

    /// The initial diagram wrapped in a pool; returns it with the pool id.
    fn pooled() -> (Definitions, ElementId) {
        let mut defs = crate::io::new_diagram();
        factory::create_participant(&defs, Waypoint { x: 0.0, y: 0.0 })
            .expect("pool")
            .execute(&mut defs)
            .expect("created");
        let pool = defs
            .elements
            .values()
            .find(|e| matches!(e.kind, ElementKind::Participant { .. }))
            .expect("participant")
            .id
            .clone();

        (defs, pool)
    }

    /// Moving the start event past the pool's right border grows the pool
    /// to the event plus 100, as one undoable step.
    #[test]
    fn move_beyond_pool_grows_it() {
        let (mut defs, pool) = pooled();
        let start = ElementId::from("StartEvent_1");
        let before = defs.shape(&pool).unwrap().bounds;
        let right = before.x + before.w;
        let b = defs.shape(&start).unwrap().bounds;
        let dx = right - b.x - 20.0;

        let command = with_auto_resize(
            &defs,
            Command::MoveShapes {
                ids: vec![start.clone()],
                dx,
                dy: 0.0,
            },
        );
        let mut stack = CommandStack::default();
        assert!(stack.execute(command, &mut defs));

        let after = defs.shape(&pool).unwrap().bounds;
        let moved = defs.shape(&start).unwrap().bounds;
        assert_eq!(after.x + after.w, moved.x + moved.w + 100.0);
        assert_eq!(after.x, before.x);

        assert!(stack.undo(&mut defs));
        assert_eq!(defs.shape(&pool).unwrap().bounds, before);
    }

    /// A move that stays well inside leaves the container alone.
    #[test]
    fn move_inside_keeps_pool() {
        let (defs, _) = pooled();
        let command = Command::MoveShapes {
            ids: vec![ElementId::from("StartEvent_1")],
            dx: 10.0,
            dy: 0.0,
        };

        assert_eq!(with_auto_resize(&defs, command.clone()), command);
    }

    /// Optimal bounds only move the sides the content crowds.
    #[test]
    fn optimal_bounds() {
        let target = Bounds {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let content = Bounds {
            x: 50.0,
            y: 99.0,
            w: 10.0,
            h: 10.0,
        };

        let b = optimal(&target, &content);
        assert_eq!((b.x, b.y, b.w, b.h), (0.0, 0.0, 100.0, 169.0));
    }
}
