//! Lane tools and lane-aware resizing, after bpmn-js `AddLaneHandler`,
//! `SplitLaneHandler`, `ResizeLaneBehavior` and `LaneUtil`. Builders read
//! the model and return a `Command`. Horizontal lanes only.

use super::command::{Command, shape_mut};
use super::factory::new_id_excluding;
use super::rules::{can_resize, is_flow_node, min_size, owner, participant_of};
use super::undo::Undo;
use crate::model::{Bounds, Definitions, Element, ElementId, ElementKind, Shape};

/// Indentation of lanes from the participant's left edge (name band).
pub const LANE_INDENTATION: f64 = 30.0;

/// Height of a lane added above or below another (bpmn-js `AddLaneHandler`).
const NEW_LANE_HEIGHT: f64 = 120.0;

/// Padding kept between a participant/lane border and its flow nodes on
/// resize (bpmn-js `LANE_PADDING`): top, right, bottom, left.
const LANE_PADDING: (f64, f64, f64, f64) = (20.0, 20.0, 20.0, 50.0);

/// Padding kept between a sub-process border and its children on resize.
const CHILD_PADDING: f64 = 20.0;

/// Where `add_lane` puts the new lane relative to the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneSide {
    Above,
    Below,
}

/// Command adding a 120px lane above or below `target` (a lane or a
/// participant), as the bpmn-js lane tools do: a participant without lanes
/// first gets one lane filling it. On a participant only the participant
/// grows by 120; on a lane, the participant and the other lanes crossing
/// the insertion line grow by 120 and the pool's shapes beyond it move
/// away. Lane membership is recomputed.
pub fn add_lane(defs: &Definitions, target: &ElementId, side: LaneSide) -> Option<Command> {
    let participant = participant_of(defs, target)?;
    let process = process_of(defs, &participant)?;
    let shape = defs.shape(target)?.bounds;
    let is_root = *target == participant;
    let lane_parent = if is_root {
        process.clone()
    } else {
        defs.get(target)?.parent.clone()?
    };

    let mut commands = Vec::new();
    let mut ids = fresh_lane_ids(defs, 2);

    if is_root && child_lanes(defs, &lane_parent).is_empty() {
        let bounds = Bounds {
            x: shape.x + LANE_INDENTATION,
            w: shape.w - LANE_INDENTATION,
            ..shape
        };
        commands.push(create_lane(ids.remove(0), &lane_parent, bounds));
    }

    let (offset, line) = match side {
        LaneSide::Above => (-NEW_LANE_HEIGHT, shape.y + 10.0),
        LaneSide::Below => (NEW_LANE_HEIGHT, shape.y + shape.h - 10.0),
    };
    if is_root {
        let mut bounds = shape;
        bounds.y += offset.min(0.0);
        bounds.h += offset.abs();
        commands.push(Command::ResizeShape {
            id: participant.clone(),
            bounds,
        });
    } else {
        commands.extend(make_lane_space(
            defs,
            &participant,
            &process,
            target,
            offset,
            line,
        ));
    }

    let x = shape.x + if is_root { LANE_INDENTATION } else { 0.0 };
    let bounds = Bounds {
        x,
        y: match side {
            LaneSide::Above => shape.y - NEW_LANE_HEIGHT,
            LaneSide::Below => shape.y + shape.h,
        },
        w: shape.w - (x - shape.x),
        h: NEW_LANE_HEIGHT,
    };
    commands.push(create_lane(ids.remove(0), &lane_parent, bounds));
    commands.push(Command::UpdateFlowNodeRefs { participant });

    Some(Command::Compound(commands))
}

/// Command splitting `target` (a participant or a lane) into `count` equal
/// horizontal lanes (bpmn-js `SplitLaneHandler`): existing child lanes are
/// resized in order, missing ones created, the last one takes the rounding
/// rest. `None` when it already has more than `count` child lanes.
pub fn split_lane(defs: &Definitions, target: &ElementId, count: usize) -> Option<Command> {
    let participant = participant_of(defs, target)?;
    let shape = defs.shape(target)?.bounds;
    let parent = if *target == participant {
        process_of(defs, &participant)?
    } else {
        target.clone()
    };

    let existing = child_lanes(defs, &parent);
    if count == 0 || existing.len() > count {
        return None;
    }

    let size = (shape.h / count as f64).round();
    let mut ids = fresh_lane_ids(defs, count);
    let mut commands = Vec::new();

    for index in 0..count {
        let h = if index + 1 == count {
            shape.h - size * index as f64
        } else {
            size
        };
        let bounds = Bounds {
            x: shape.x + LANE_INDENTATION,
            y: shape.y + index as f64 * size,
            w: shape.w - LANE_INDENTATION,
            h,
        };

        match existing.get(index) {
            Some(lane) => commands.push(Command::ResizeShape {
                id: lane.clone(),
                bounds,
            }),
            None => commands.push(create_lane(ids.remove(0), &parent, bounds)),
        }
    }

    commands.push(Command::UpdateFlowNodeRefs { participant });
    Some(Command::Compound(commands))
}

/// Command resizing `id` to `bounds` under the bpmn-js resize rules:
/// only resizable kinds (`rules::can_resize`), clamped to `rules::min_size`
/// and to the padded bounding box of the children (the edge that did not
/// move stays put). Participants and lanes also resize the lanes sharing
/// the moved edges (bpmn-js `computeLanesResize`) and recompute lane
/// membership; `None` when that would shrink a lane below its minimum.
pub fn resize(defs: &Definitions, id: &ElementId, bounds: Bounds) -> Option<Command> {
    let element = defs.get(id)?;
    let shape = defs.shape(id)?;
    let expanded = shape.is_expanded == Some(true);
    if !can_resize(&element.kind, expanded) {
        return None;
    }

    let old = shape.bounds;
    let (min_w, min_h) = min_size(&element.kind, expanded)?;
    let mut new = clamp_size(&old, bounds, min_w, min_h);
    if let Some(children) = children_box(defs, id) {
        new = contain(&old, new, &children);
    }

    if new == old {
        return None;
    }

    let resize = Command::ResizeShape {
        id: id.clone(),
        bounds: new,
    };
    let is_lanes = matches!(
        element.kind,
        ElementKind::Participant { .. } | ElementKind::Lane
    );
    if !is_lanes {
        return Some(resize);
    }

    let participant = participant_of(defs, id)?;
    let others = lanes_resize(defs, &participant, id, &old, &new);
    if others
        .iter()
        .any(|(lane, b)| *lane != participant && b.h < 60.0)
    {
        return None;
    }

    let mut commands = vec![resize];
    commands.extend(
        others
            .into_iter()
            .map(|(id, bounds)| Command::ResizeShape { id, bounds }),
    );
    commands.push(Command::UpdateFlowNodeRefs { participant });

    Some(Command::Compound(commands))
}

/// New bounds of the participant and lanes (other than `shape`) adjacent to
/// the edges of `shape` that move from `old` to `new` (bpmn-js
/// `computeLanesResize`): neighbors within 10px follow the moved edge,
/// lanes flush within 5px move that edge too.
pub fn lanes_resize(
    defs: &Definitions,
    participant: &ElementId,
    shape: &ElementId,
    old: &Bounds,
    new: &Bounds,
) -> Vec<(ElementId, Bounds)> {
    let Some(process) = process_of(defs, participant) else {
        return Vec::new();
    };

    let mut all = vec![participant.clone()];
    all.extend(
        defs.elements
            .values()
            .filter(|e| {
                e.kind == ElementKind::Lane && owner(defs, &e.id).as_ref() == Some(&process)
            })
            .map(|e| e.id.clone()),
    );

    let (ot, or, ob, ol) = trbl(old);
    let (nt, nr, nb, nl) = trbl(new);
    let mut out = Vec::new();

    for other in all.iter().filter(|o| *o != shape) {
        let Some(b) = defs.shape(other).map(|s| s.bounds) else {
            continue;
        };

        let (t, r, bo, l) = trbl(&b);
        let (mut dt, mut dr, mut db, mut dl) = (0.0, 0.0, 0.0, 0.0);

        if nt != ot {
            if (bo - ot).abs() < 10.0 {
                db = nt - bo;
            }
            if (t - ot).abs() < 5.0 {
                dt = nt - t;
            }
        }

        if nl != ol {
            if (r - ol).abs() < 10.0 {
                dr = nl - r;
            }
            if (l - ol).abs() < 5.0 {
                dl = nl - l;
            }
        }

        if nb != ob {
            if (t - ob).abs() < 10.0 {
                dt = nb - t;
            }
            if (bo - ob).abs() < 5.0 {
                db = nb - bo;
            }
        }

        if nr != or {
            if (l - or).abs() < 10.0 {
                dl = nr - l;
            }
            if (r - or).abs() < 5.0 {
                dr = nr - r;
            }
        }

        if dt != 0.0 || dr != 0.0 || db != 0.0 || dl != 0.0 {
            out.push((
                other.clone(),
                Bounds {
                    x: l + dl,
                    y: t + dt,
                    w: (r + dr) - (l + dl),
                    h: (bo + db) - (t + dt),
                },
            ));
        }
    }

    out
}

/// Lanes directly under `parent` (a process or a lane), top to bottom.
pub fn child_lanes(defs: &Definitions, parent: &ElementId) -> Vec<ElementId> {
    let mut lanes: Vec<(ElementId, f64)> = defs
        .children(parent)
        .filter(|e| e.kind == ElementKind::Lane)
        .map(|e| (e.id.clone(), defs.shape(&e.id).map_or(0.0, |s| s.bounds.y)))
        .collect();

    lanes.sort_by(|a, b| a.1.total_cmp(&b.1));
    lanes.into_iter().map(|(id, _)| id).collect()
}

/// Process referenced by `participant`.
fn process_of(defs: &Definitions, participant: &ElementId) -> Option<ElementId> {
    match &defs.get(participant)?.kind {
        ElementKind::Participant { process_ref } => process_ref.clone(),
        _ => None,
    }
}

/// `count` distinct unused `Lane_<n>` ids, smallest numbers first.
fn fresh_lane_ids(defs: &Definitions, count: usize) -> Vec<ElementId> {
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(new_id_excluding(defs, "Lane", &ids));
    }

    ids
}

/// `CreateShape` of a horizontal lane under `parent`.
fn create_lane(id: ElementId, parent: &ElementId, bounds: Bounds) -> Command {
    let element = Element::new(id.as_str(), ElementKind::Lane).with_parent(parent.as_str());
    let shape = Shape {
        element_id: id,
        bounds,
        is_expanded: None,
        is_horizontal: Some(true),
        label_bounds: None,
    };

    Command::CreateShape { element, shape }
}

/// Space-tool commands opening `offset` px at the horizontal line `line`
/// inside a participant (bpmn-js `SpaceTool.calculateAdjustments`): the
/// participant and lanes crossing the line (except `target`) grow toward
/// `offset`, every other shape of the pool beyond the line moves by it.
fn make_lane_space(
    defs: &Definitions,
    participant: &ElementId,
    process: &ElementId,
    target: &ElementId,
    offset: f64,
    line: f64,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut moving = Vec::new();

    let members = defs.shapes().filter(|s| {
        s.element_id == *participant
            || (owner(defs, &s.element_id).as_ref() == Some(process)
                && defs
                    .get(&s.element_id)
                    .is_some_and(|e| e.kind.endpoints().is_none()))
    });

    for s in members {
        let Some(kind) = defs.get(&s.element_id).map(|e| &e.kind) else {
            continue;
        };
        if matches!(kind, ElementKind::BoundaryEvent { .. }) {
            continue;
        }

        let (start, end) = (s.bounds.y, s.bounds.y + s.bounds.h);
        let beyond = if offset > 0.0 {
            start > line
        } else {
            end < line
        };
        let crossing = start < line && end > line;
        let is_lanes = matches!(kind, ElementKind::Participant { .. } | ElementKind::Lane);

        if beyond {
            moving.push(s.element_id.clone());
        } else if crossing && is_lanes && s.element_id != *target {
            let mut bounds = s.bounds;
            if offset < 0.0 {
                bounds.y += offset;
            }
            bounds.h += offset.abs();
            commands.push(Command::ResizeShape {
                id: s.element_id.clone(),
                bounds,
            });
        }
    }

    if !moving.is_empty() {
        commands.push(Command::MoveShapes {
            ids: moving,
            dx: 0.0,
            dy: offset,
        });
    }

    commands
}

/// Top, right, bottom, left of `b`.
fn trbl(b: &Bounds) -> (f64, f64, f64, f64) {
    (b.y, b.x + b.w, b.y + b.h, b.x)
}

/// `new` grown to at least `min_w` x `min_h`, keeping the edges of `old`
/// that did not move.
fn clamp_size(old: &Bounds, new: Bounds, min_w: f64, min_h: f64) -> Bounds {
    let mut out = new;

    if out.w < min_w {
        if out.x != old.x {
            out.x = out.x + out.w - min_w;
        }
        out.w = min_w;
    }

    if out.h < min_h {
        if out.y != old.y {
            out.y = out.y + out.h - min_h;
        }
        out.h = min_h;
    }

    out
}

/// `new` grown so that it still contains `inner`, moving only the edges
/// that moved from `old`.
fn contain(old: &Bounds, new: Bounds, inner: &Bounds) -> Bounds {
    let (mut t, mut r, mut b, mut l) = trbl(&new);
    let (it, ir, ib, il) = trbl(inner);

    if l != old.x {
        l = l.min(il);
    }
    if r != old.x + old.w {
        r = r.max(ir);
    }
    if t != old.y {
        t = t.min(it);
    }
    if b != old.y + old.h {
        b = b.max(ib);
    }

    Bounds {
        x: l,
        y: t,
        w: r - l,
        h: b - t,
    }
}

/// Padded bounding box of the content that must stay inside `id` on resize:
/// a participant's or lane's flow nodes (lane padding), a sub-process's
/// children (20px); boundary events, lanes and connections are ignored.
fn children_box(defs: &Definitions, id: &ElementId) -> Option<Bounds> {
    let kind = &defs.get(id)?.kind;
    let scope = match kind {
        ElementKind::Participant { process_ref } => process_ref.clone()?,
        _ => id.clone(),
    };
    let lane_like = matches!(kind, ElementKind::Participant { .. } | ElementKind::Lane);

    let inside = |e: &Element| {
        let mut parent = e.parent.clone();
        while let Some(p) = parent {
            if p == scope {
                return true;
            }
            parent = defs
                .get(&p)
                .filter(|x| x.kind == ElementKind::Lane)
                .and_then(|x| x.parent.clone());
        }
        false
    };

    let content = defs
        .elements
        .values()
        .filter(|e| e.kind.endpoints().is_none())
        .filter(|e| {
            !matches!(
                e.kind,
                ElementKind::Lane | ElementKind::BoundaryEvent { .. }
            )
        })
        .filter(|e| inside(e))
        .filter_map(|e| defs.shape(&e.id).map(|s| s.bounds))
        .reduce(|a, b| {
            let (x, y) = (a.x.min(b.x), a.y.min(b.y));
            Bounds {
                x,
                y,
                w: (a.x + a.w).max(b.x + b.w) - x,
                h: (a.y + a.h).max(b.y + b.h) - y,
            }
        })?;

    let (pt, pr, pb, pl) = if lane_like {
        LANE_PADDING
    } else {
        (CHILD_PADDING, CHILD_PADDING, CHILD_PADDING, CHILD_PADDING)
    };

    Some(Bounds {
        x: content.x - pl,
        y: content.y - pt,
        w: content.w + pl + pr,
        h: content.h + pt + pb,
    })
}

/// Grows the lanes of `process` adjacent to the removed lane `bounds` over
/// its space (horizontal lanes; bpmn-js `DeleteLaneBehavior`).
pub(super) fn close_lane_gap(
    defs: &mut Definitions,
    process: &ElementId,
    bounds: &Bounds,
    undo: &mut Undo,
) {
    let (top, bottom) = (bounds.y, bounds.y + bounds.h);
    let lanes: Vec<(ElementId, Bounds)> = defs
        .elements
        .values()
        .filter(|e| e.kind == ElementKind::Lane && owner(defs, &e.id).as_ref() == Some(process))
        .filter_map(|e| Some((e.id.clone(), defs.shape(&e.id)?.bounds)))
        .collect();

    let above: Vec<&(ElementId, Bounds)> = lanes
        .iter()
        .filter(|(_, b)| (b.y + b.h - top).abs() < 10.0)
        .collect();
    let below: Vec<&(ElementId, Bounds)> = lanes
        .iter()
        .filter(|(_, b)| (b.y - bottom).abs() < 10.0)
        .collect();
    let offset = if !above.is_empty() && !below.is_empty() {
        bounds.h / 2.0
    } else {
        bounds.h
    };

    for (id, _) in &above {
        undo.shape(defs, id);
        if let Some(shape) = shape_mut(defs, id) {
            shape.bounds.h += offset;
        }
    }

    for (id, _) in &below {
        undo.shape(defs, id);
        if let Some(shape) = shape_mut(defs, id) {
            shape.bounds.y -= offset;
            shape.bounds.h += offset;
        }
    }
}

/// Executes `UpdateFlowNodeRefs` for `participant`'s process.
pub(super) fn update_flow_node_refs(
    defs: &mut Definitions,
    participant: &ElementId,
    undo: &mut Undo,
) {
    let Some(ElementKind::Participant {
        process_ref: Some(process),
    }) = defs.get(participant).map(|e| e.kind.clone())
    else {
        return;
    };

    let lanes: Vec<(ElementId, Bounds, usize)> = defs
        .elements
        .values()
        .filter(|e| e.kind == ElementKind::Lane && owner(defs, &e.id).as_ref() == Some(&process))
        .filter_map(|e| {
            Some((
                e.id.clone(),
                defs.shape(&e.id)?.bounds,
                lane_depth(defs, &e.id),
            ))
        })
        .collect();

    let nodes: Vec<ElementId> = defs
        .elements
        .values()
        .filter(|e| is_flow_node(&e.kind) && owner(defs, &e.id).as_ref() == Some(&process))
        .map(|e| e.id.clone())
        .collect();

    for id in nodes {
        let Some(c) = defs.shape(&id).map(|s| super::layout::center(&s.bounds)) else {
            continue;
        };
        let lane = lanes
            .iter()
            .filter(|(_, b, _)| c.x >= b.x && c.x <= b.x + b.w && c.y >= b.y && c.y <= b.y + b.h)
            .max_by_key(|(_, _, depth)| *depth)
            .map(|(lane, _, _)| lane.clone());
        let parent = lane.unwrap_or_else(|| process.clone());

        if defs.get(&id).and_then(|e| e.parent.as_ref()) == Some(&parent) {
            continue;
        }

        undo.element(defs, &id);
        if let Some(e) = defs.elements.get_mut(&id) {
            e.parent = Some(parent);
        }
    }
}

/// Paint index for the new shape of `id`: right after its participant's
/// shape and existing lanes when `id` is a lane, else the end.
pub(super) fn lane_slot(defs: &Definitions, id: &ElementId) -> usize {
    let shapes = &defs.plane().shapes;
    let is_lane = |e: &ElementId| defs.get(e).is_some_and(|e| e.kind == ElementKind::Lane);

    let participant = is_lane(id)
        .then(|| super::rules::participant_of(defs, id))
        .flatten();
    let Some(start) = participant.and_then(|p| shapes.iter().position(|s| s.element_id == p))
    else {
        return shapes.len();
    };

    let mut at = start + 1;
    while at < shapes.len() && is_lane(&shapes[at].element_id) {
        at += 1;
    }
    at
}

/// Number of lane ancestors of lane `id`.
fn lane_depth(defs: &Definitions, id: &ElementId) -> usize {
    let mut depth = 0;
    let mut parent = defs.get(id).and_then(|e| e.parent.clone());

    while let Some(p) = parent {
        match defs.get(&p) {
            Some(e) if e.kind == ElementKind::Lane => {
                depth += 1;
                parent = e.parent.clone();
            }
            _ => break,
        }
    }

    depth
}
