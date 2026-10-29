//! Copy and paste after bpmn-js `BpmnCopyPaste` / diagram-js `CopyPaste`:
//! a copy keeps the selected shapes with their descendants, boundary events
//! and the connections between them; a paste recreates them with fresh ids
//! centered at a point, dropped into the container under it.

use super::Command;
use super::factory::{id_prefix, new_id_excluding};
use super::lanes::{self, LaneSide};
use super::layout::center;
use super::rules::closure;
use super::rules::plane_process;
use crate::model::{
    Bounds, Definitions, Edge, Element, ElementId, ElementKind, Plane, Shape, Waypoint,
};

/// Copied elements with their DI, in document and paint order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clipboard {
    /// Top-level copied shapes (the selection minus nested ones).
    roots: Vec<ElementId>,
    elements: Vec<Element>,
    shapes: Vec<Shape>,
    edges: Vec<Edge>,
    /// Planes of copied collapsed sub-processes, outer ones first.
    planes: Vec<Plane>,
}

impl Clipboard {
    /// Whether nothing was copied.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

/// Copies the shapes of `ids` with their descendants (a pool with its
/// process, lanes and content; a lane with its sub-lanes and flow nodes),
/// the boundary events of copied hosts and every connection whose both
/// ends are copied; `None` when nothing is copyable.
pub fn copy(defs: &Definitions, ids: &[ElementId]) -> Option<Clipboard> {
    let copyable: Vec<ElementId> = ids
        .iter()
        .filter(|id| defs.shape(id).is_some())
        .cloned()
        .collect();

    let mut set = closure(defs, &copyable);
    let processes: Vec<ElementId> = set
        .iter()
        .filter_map(|id| match &defs.get(id)?.kind {
            ElementKind::Participant { process_ref } => process_ref.clone(),
            _ => None,
        })
        .collect();
    set.extend(processes);
    set.retain(|id| match defs.get(id).map(|e| &e.kind) {
        Some(ElementKind::BoundaryEvent { attached_to, .. }) => {
            set_has(&copyable, defs, attached_to)
        }
        _ => true,
    });

    let roots: Vec<ElementId> = copyable
        .iter()
        .filter(|id| {
            !copyable.iter().any(|other| {
                other != *id && closure(defs, std::slice::from_ref(other)).contains(id)
            })
        })
        .cloned()
        .collect();
    if roots.is_empty() {
        return None;
    }

    let connections: Vec<ElementId> = defs
        .elements
        .values()
        .filter(|e| {
            e.kind
                .endpoints()
                .is_some_and(|(s, t)| set.contains(s) && set.contains(t))
        })
        .map(|e| e.id.clone())
        .collect();
    set.extend(connections);

    let mut planes: Vec<Plane> = Vec::new();
    let mut owners: Vec<ElementId> = defs.shapes().map(|s| s.element_id.clone()).collect();
    while let Some(plane) = defs.diagrams.iter().skip(1).map(|d| &d.plane).find(|p| {
        p.element_id
            .as_ref()
            .is_some_and(|o| set.contains(o) && owners.contains(o) && !planes.contains(p))
    }) {
        owners.extend(plane.shapes.iter().map(|s| s.element_id.clone()));
        planes.push(plane.clone());
    }

    Some(Clipboard {
        roots,
        elements: defs
            .elements
            .values()
            .filter(|e| set.contains(&e.id))
            .cloned()
            .collect(),
        shapes: defs
            .shapes()
            .filter(|s| set.contains(&s.element_id))
            .cloned()
            .collect(),
        edges: defs
            .edges()
            .filter(|e| set.contains(&e.element_id))
            .cloned()
            .collect(),
        planes,
    })
}

/// Whether `host` is copied: listed itself or inside a listed shape.
fn set_has(copyable: &[ElementId], defs: &Definitions, host: &ElementId) -> bool {
    closure(defs, copyable).contains(host)
}

/// Command pasting `clip` with fresh ids, its bounding box centered at
/// `at`, plus the ids of the pasted top-level shapes (the new selection).
/// The copies are created in place, then moved to `at` so they reparent to
/// the container there and lane membership is recomputed. Pools paste only
/// into a collaboration (each with a fresh process); lanes go through
/// `paste_lanes`.
pub fn paste(
    defs: &Definitions,
    clip: &Clipboard,
    at: Waypoint,
) -> Option<(Command, Vec<ElementId>)> {
    let is_lane = |id: &ElementId| {
        clip.elements
            .iter()
            .any(|e| e.id == *id && e.kind == ElementKind::Lane)
    };
    if clip.roots.iter().any(is_lane) {
        return paste_lanes(defs, clip, at);
    }

    paste_nodes(defs, clip, at)
}

/// `paste` of a clipboard without lane roots.
fn paste_nodes(
    defs: &Definitions,
    clip: &Clipboard,
    at: Waypoint,
) -> Option<(Command, Vec<ElementId>)> {
    let bbox = clip
        .shapes
        .iter()
        .filter(|s| clip.roots.contains(&s.element_id))
        .map(|s| s.bounds)
        .reduce(|a, b| union(&a, &b))?;

    let mut taken: Vec<ElementId> = Vec::new();
    let ids: Vec<(ElementId, ElementId)> = clip
        .elements
        .iter()
        .map(|e| {
            let prefix = id_prefix(&e.kind);
            let id = new_id_excluding(defs, prefix, &taken);
            taken.push(id.clone());
            (e.id.clone(), id)
        })
        .collect();
    let map = |old: &ElementId| ids.iter().find(|(o, _)| o == old).map(|(_, n)| n.clone());

    let collaboration = defs.plane().element_id.clone().filter(|c| {
        defs.get(c)
            .is_some_and(|e| e.kind == ElementKind::Collaboration)
    });
    let pools = clip
        .elements
        .iter()
        .any(|e| matches!(e.kind, ElementKind::Participant { .. }));
    if pools && (collaboration.is_none() || defs.plane_owner().is_some()) {
        return None;
    }

    let fallback = plane_process(defs);
    let elements: Vec<Element> = clip
        .elements
        .iter()
        .map(|e| {
            let mut copy = e.clone();
            copy.id = map(&e.id).expect("mapped above");
            copy.parent = match &e.parent {
                Some(p) => map(p).or_else(|| defs.get(p).map(|_| p.clone())),
                None => None,
            }
            .or_else(|| fallback.clone());

            match &mut copy.kind {
                ElementKind::Participant { process_ref } => {
                    copy.parent = collaboration.clone();
                    let process = process_ref.as_ref().and_then(map);
                    *process_ref = process;
                }
                ElementKind::Process => copy.parent = None,
                _ => {}
            }

            match &mut copy.kind {
                ElementKind::BoundaryEvent { attached_to, .. } => {
                    *attached_to = map(attached_to).expect("host copied")
                }
                ElementKind::SequenceFlow { source, target, .. }
                | ElementKind::MessageFlow { source, target }
                | ElementKind::Association { source, target }
                | ElementKind::DataInputAssociation { source, target }
                | ElementKind::DataOutputAssociation { source, target } => {
                    *source = map(source).expect("source copied");
                    *target = map(target).expect("target copied");
                }
                _ => {}
            }

            copy
        })
        .collect();
    let element = |old: &ElementId| {
        let new = map(old)?;
        elements.iter().find(|e| e.id == new).cloned()
    };

    let mut commands = Vec::new();
    for shape in &clip.shapes {
        let Some(element) = element(&shape.element_id) else {
            continue;
        };
        let mut shape = shape.clone();
        shape.element_id = element.id.clone();
        match &element.kind {
            ElementKind::Participant { process_ref } => {
                let process = process_ref
                    .as_ref()
                    .and_then(|p| elements.iter().find(|e| e.id == *p))
                    .cloned();
                commands.push(Command::CreateParticipant {
                    participant: element,
                    shape,
                    process,
                    collaboration: None,
                });
            }
            _ => commands.push(Command::CreateShape { element, shape }),
        }
    }

    for edge in &clip.edges {
        let Some(element) = element(&edge.element_id) else {
            continue;
        };
        let mut edge = edge.clone();
        edge.element_id = element.id.clone();
        commands.push(Command::CreateConnection { element, edge });
    }

    for plane in &clip.planes {
        let Some(owner) = plane.element_id.as_ref().and_then(map) else {
            continue;
        };
        let shapes: Vec<Shape> = plane
            .shapes
            .iter()
            .filter_map(|s| {
                Some(Shape {
                    element_id: map(&s.element_id)?,
                    ..s.clone()
                })
            })
            .collect();
        let edges: Vec<Edge> = plane
            .edges
            .iter()
            .filter_map(|e| {
                Some(Edge {
                    element_id: map(&e.element_id)?,
                    ..e.clone()
                })
            })
            .collect();
        let content: Vec<Element> = shapes
            .iter()
            .map(|s| &s.element_id)
            .chain(edges.iter().map(|e| &e.element_id))
            .filter_map(|id| elements.iter().find(|e| e.id == *id).cloned())
            .collect();

        commands.push(Command::CreatePlaneContent {
            owner,
            elements: content,
            plane: Plane {
                shapes,
                edges,
                ..Plane::default()
            },
        });
    }

    let roots: Vec<ElementId> = clip.roots.iter().filter_map(map).collect();
    let c = center(&bbox);
    commands.push(Command::MoveShapes {
        ids: roots.clone(),
        dx: at.x - c.x,
        dy: at.y - c.y,
    });

    commands.extend(
        defs.elements
            .values()
            .filter(|e| matches!(e.kind, ElementKind::Participant { .. }))
            .chain(
                elements
                    .iter()
                    .filter(|e| matches!(e.kind, ElementKind::Participant { .. })),
            )
            .map(|e| Command::UpdateFlowNodeRefs {
                participant: e.id.clone(),
            }),
    );

    Some((Command::Compound(commands), roots))
}

/// `paste` of a clipboard with lane roots (bpmn-js drops lanes only on a
/// pool): each lane becomes a new lane below the pool under `at`, named
/// like the copy, with its flow nodes pasted centered in it; without a pool
/// there only the lanes' content is pasted at `at`. Other roots paste at
/// `at`. Selects the new lanes, else the pasted shapes.
fn paste_lanes(
    defs: &Definitions,
    clip: &Clipboard,
    at: Waypoint,
) -> Option<(Command, Vec<ElementId>)> {
    let pool = defs
        .shapes()
        .filter(|s| {
            let b = &s.bounds;
            at.x >= b.x && at.x <= b.x + b.w && at.y >= b.y && at.y <= b.y + b.h
        })
        .filter(|s| {
            defs.get(&s.element_id)
                .is_some_and(|e| matches!(e.kind, ElementKind::Participant { .. }))
        })
        .last()
        .map(|s| s.element_id.clone());

    let lane_roots: Vec<&Element> = clip
        .roots
        .iter()
        .filter_map(|id| {
            clip.elements
                .iter()
                .find(|e| e.id == *id && e.kind == ElementKind::Lane)
        })
        .collect();
    let others: Vec<ElementId> = clip
        .roots
        .iter()
        .filter(|id| !lane_roots.iter().any(|l| l.id == **id))
        .cloned()
        .collect();

    let mut after = defs.clone();
    let mut commands = Vec::new();
    let mut selection = Vec::new();

    for lane in lane_roots {
        let mut lanes_in = vec![lane.id.clone()];
        for _ in 0..clip.elements.len() {
            let deeper: Vec<ElementId> = clip
                .elements
                .iter()
                .filter(|e| e.kind == ElementKind::Lane && !lanes_in.contains(&e.id))
                .filter(|e| e.parent.as_ref().is_some_and(|p| lanes_in.contains(p)))
                .map(|e| e.id.clone())
                .collect();
            if deeper.is_empty() {
                break;
            }
            lanes_in.extend(deeper);
        }
        let content: Vec<ElementId> = clip
            .shapes
            .iter()
            .filter_map(|s| clip.elements.iter().find(|e| e.id == s.element_id))
            .filter(|e| e.kind != ElementKind::Lane)
            .filter(|e| e.parent.as_ref().is_some_and(|p| lanes_in.contains(p)))
            .map(|e| e.id.clone())
            .collect();

        let mut target = at;
        if let Some(pool) = &pool {
            let Some(add) = lanes::add_lane(&after, pool, LaneSide::Below) else {
                continue;
            };
            let known: Vec<ElementId> = after.elements.keys().cloned().collect();
            add.execute(&mut after)?;
            commands.push(add);

            let created = after
                .elements
                .values()
                .filter(|e| e.kind == ElementKind::Lane && !known.contains(&e.id))
                .filter_map(|e| Some((e.id.clone(), after.shape(&e.id)?.bounds)))
                .max_by(|a, b| a.1.y.total_cmp(&b.1.y));
            if let Some((id, bounds)) = created {
                if let Some(name) = &lane.name {
                    let rename = Command::UpdateLabel {
                        id: id.clone(),
                        name: name.clone(),
                    };
                    let _ = rename.execute(&mut after);
                    commands.push(rename);
                }
                target = center(&bounds);
                selection.push(id);
            }
        }

        if content.is_empty() {
            continue;
        }
        if let Some((paste, roots)) = paste_nodes(&after, &subset(clip, content), target) {
            let _ = paste.execute(&mut after);
            commands.push(paste);
            if pool.is_none() {
                selection.extend(roots);
            }
        }
    }

    if !others.is_empty()
        && let Some((paste, roots)) = paste_nodes(&after, &subset(clip, others), at)
    {
        commands.push(paste);
        selection.extend(roots);
    }

    if commands.is_empty() {
        return None;
    }

    Some((Command::Compound(commands), selection))
}

/// The part of `clip` under `roots`: their descendants (a pool's through
/// its process) and the connections between kept elements.
fn subset(clip: &Clipboard, roots: Vec<ElementId>) -> Clipboard {
    let find = |id: &ElementId| clip.elements.iter().find(|e| e.id == *id);

    let mut tops = roots.clone();
    for root in &roots {
        if let Some(ElementKind::Participant {
            process_ref: Some(p),
        }) = find(root).map(|e| &e.kind)
        {
            tops.push(p.clone());
        }
    }

    let inside = |e: &Element| {
        let mut current = Some(e);
        for _ in 0..=clip.elements.len() {
            let Some(c) = current else {
                return false;
            };
            if tops.contains(&c.id) {
                return true;
            }
            current = c.parent.as_ref().and_then(find);
        }
        false
    };

    let mut keep: Vec<ElementId> = clip
        .elements
        .iter()
        .filter(|e| e.kind.endpoints().is_none() && inside(e))
        .map(|e| e.id.clone())
        .collect();
    let connections: Vec<ElementId> = clip
        .elements
        .iter()
        .filter(|e| {
            e.kind
                .endpoints()
                .is_some_and(|(s, t)| keep.contains(s) && keep.contains(t))
        })
        .map(|e| e.id.clone())
        .collect();
    keep.extend(connections);

    Clipboard {
        roots,
        elements: clip
            .elements
            .iter()
            .filter(|e| keep.contains(&e.id))
            .cloned()
            .collect(),
        shapes: clip
            .shapes
            .iter()
            .filter(|s| keep.contains(&s.element_id))
            .cloned()
            .collect(),
        edges: clip
            .edges
            .iter()
            .filter(|e| keep.contains(&e.element_id))
            .cloned()
            .collect(),
        planes: clip
            .planes
            .iter()
            .filter(|p| p.element_id.as_ref().is_some_and(|o| keep.contains(o)))
            .cloned()
            .collect(),
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
