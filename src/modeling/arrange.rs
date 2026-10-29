//! Arranging shapes after bpmn-js `AlignElements` / `DistributeElements`
//! and diagram-js `GridSnapping`: align a selection to an edge or center,
//! distribute it with equal gaps, and quantize coordinates to the grid.

use super::Command;
use super::rules::closure;
use crate::model::{Bounds, Definitions, ElementId, ElementKind};

/// Grid spacing in diagram units (diagram-js `GridUtil.SPACING`).
pub const GRID: f64 = 10.0;

/// Edge or center a selection is aligned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}

/// Axis along which shapes are distributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// `value` rounded to the nearest grid line (diagram-js `quantize`).
pub fn snap(value: f64) -> f64 {
    (value / GRID).round() * GRID
}

/// Command aligning the alignable shapes among `ids` (bpmn-js
/// `elements.align` rule: no connections, boundary events or lanes, and no
/// descendants of another selected shape); `None` for fewer than two.
pub fn align(defs: &Definitions, ids: &[ElementId], alignment: Alignment) -> Option<Command> {
    let shapes = arrangeable(defs, ids, false);
    if shapes.len() < 2 {
        return None;
    }

    let horizontal = matches!(
        alignment,
        Alignment::Left | Alignment::Center | Alignment::Right
    );
    let range = |b: &Bounds| if horizontal { (b.x, b.w) } else { (b.y, b.h) };
    let min = shapes
        .iter()
        .map(|(_, b)| range(b).0)
        .fold(f64::INFINITY, f64::min);
    let max = shapes
        .iter()
        .map(|(_, b)| range(b).0 + range(b).1)
        .fold(f64::NEG_INFINITY, f64::max);

    let target = |b: &Bounds| {
        let (pos, len) = range(b);
        match alignment {
            Alignment::Left | Alignment::Top => min - pos,
            Alignment::Right | Alignment::Bottom => max - (pos + len),
            Alignment::Center | Alignment::Middle => {
                ((min + max) / 2.0).round() - (pos + len / 2.0)
            }
        }
    };

    let moves = shapes
        .iter()
        .map(|(id, b)| {
            let delta = target(b);
            if horizontal {
                (id.clone(), delta, 0.0)
            } else {
                (id.clone(), 0.0, delta)
            }
        })
        .collect();

    moves_command(defs, moves)
}

/// Command distributing the distributable shapes among `ids` along `axis`
/// with equal gaps, the first and last staying put (bpmn-js
/// `elements.distribute` rule also excludes text annotations); `None` for
/// fewer than three.
pub fn distribute(defs: &Definitions, ids: &[ElementId], axis: Axis) -> Option<Command> {
    let mut shapes = arrangeable(defs, ids, true);
    if shapes.len() < 3 {
        return None;
    }

    let range = |b: &Bounds| match axis {
        Axis::Horizontal => (b.x, b.w),
        Axis::Vertical => (b.y, b.h),
    };
    shapes.sort_by(|a, b| range(&a.1).0.total_cmp(&range(&b.1).0));

    let first = range(&shapes[0].1);
    let last = range(&shapes[shapes.len() - 1].1);
    let sizes: f64 = shapes.iter().map(|(_, b)| range(b).1).sum();
    let gap = ((last.0 + last.1 - first.0) - sizes) / (shapes.len() - 1) as f64;

    let mut cursor = first.0;
    let mut moves = Vec::new();
    for (id, b) in &shapes {
        let (pos, len) = range(b);
        let delta = cursor.round() - pos;
        moves.push(match axis {
            Axis::Horizontal => (id.clone(), delta, 0.0),
            Axis::Vertical => (id.clone(), 0.0, delta),
        });
        cursor += len + gap;
    }

    moves_command(defs, moves)
}

/// Shapes of `ids` that may be aligned or distributed (no boundary events,
/// lanes or descendants of another listed shape; text annotations excluded
/// when `distributing`), with their bounds.
fn arrangeable(
    defs: &Definitions,
    ids: &[ElementId],
    distributing: bool,
) -> Vec<(ElementId, Bounds)> {
    ids.iter()
        .filter(|id| {
            defs.get(id).is_some_and(|e| match e.kind {
                ElementKind::BoundaryEvent { .. } | ElementKind::Lane => false,
                ElementKind::TextAnnotation { .. } => !distributing,
                _ => true,
            })
        })
        .filter(|id| {
            !ids.iter().any(|other| {
                other != *id && closure(defs, std::slice::from_ref(other)).contains(id)
            })
        })
        .filter_map(|id| Some((id.clone(), defs.shape(id)?.bounds)))
        .collect()
}

/// One undoable step moving each shape by its delta (zero moves skipped),
/// then recomputing lane membership of every participant; `None` when
/// nothing moves.
fn moves_command(defs: &Definitions, moves: Vec<(ElementId, f64, f64)>) -> Option<Command> {
    let mut commands: Vec<Command> = moves
        .into_iter()
        .filter(|(_, dx, dy)| *dx != 0.0 || *dy != 0.0)
        .map(|(id, dx, dy)| Command::MoveShapes {
            ids: vec![id],
            dx,
            dy,
        })
        .collect();
    if commands.is_empty() {
        return None;
    }

    refresh_participants(defs, &mut commands);

    Some(Command::Compound(commands))
}

/// Adds lane membership updates for all participants.
fn refresh_participants(defs: &Definitions, commands: &mut Vec<Command>) {
    commands.extend(
        defs.elements
            .values()
            .filter(|e| matches!(e.kind, ElementKind::Participant { .. }))
            .map(|e| Command::UpdateFlowNodeRefs {
                participant: e.id.clone(),
            }),
    );
}

/// Builds one selection move followed by participant membership updates.
pub fn move_selection(defs: &Definitions, ids: Vec<ElementId>, dx: f64, dy: f64) -> Command {
    let mut commands = vec![Command::MoveShapes { ids, dx, dy }];
    refresh_participants(defs, &mut commands);
    Command::Compound(commands)
}
