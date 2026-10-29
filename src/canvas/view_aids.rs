//! `Canvas` drag aids: auto-scroll near the canvas edge (diagram-js
//! `AutoScroll`), shape/grid snapping with snap lines (diagram-js
//! `Snapping`, `GridSnapping`) and the replace-menu hover preview (bpmn-js
//! `ReplaceMenuProvider` preview).

use std::rc::Rc;
use std::time::Duration;

use gpui::Context;

use super::{Canvas, Gesture};
use crate::canvas::auto_scroll;
use crate::canvas::handles::{corner_point, resized};
use crate::model::{Bounds, Definitions, ElementKind, Waypoint};
use crate::modeling::snapping::{self, Line, Snapped};
use crate::modeling::{Command, factory};

/// Per-drag helper state of the canvas.
#[derive(Default)]
pub(super) struct Aids {
    /// Last canvas-local pointer of the drag in progress (auto-scroll).
    pointer: Option<Waypoint>,
    /// Whether the auto-scroll repeat timer is running.
    scrolling: bool,
    /// cmd held: snapping off for this drag (diagram-js modifier).
    pub(super) free: bool,
    /// Snap lines of the current drag preview.
    pub(super) lines: Vec<Line>,
    /// Kind hovered in the replace menu, previewed on the selection.
    pub(super) replace_preview: Option<ElementKind>,
}

impl Canvas {
    /// Grid size drags snap to: the settings grid when snapping is on and
    /// cmd is not held, else `None`.
    pub(super) fn snap_grid(&self) -> Option<f64> {
        (self.grid.snap && !self.aids.free).then_some(self.grid.size)
    }

    /// Snap distance in diagram units at the current zoom.
    fn tolerance(&self) -> f64 {
        snapping::TOLERANCE / self.docs[self.active].viewport.scale
    }

    /// Snapped delta of moving the selection from `start` to `current`;
    /// raw when `enabled` is off (cmd held).
    pub(super) fn snapped_move(
        &self,
        start: Waypoint,
        current: Waypoint,
        enabled: bool,
    ) -> Snapped {
        let (dx, dy) = (current.x - start.x, current.y - start.y);
        if !enabled {
            return Snapped {
                dx,
                dy,
                lines: Vec::new(),
            };
        }

        let ids = self.moving_ids();
        snapping::snap_move(
            &self.docs[self.active].definitions,
            &ids,
            dx,
            dy,
            self.tolerance(),
            self.snap_grid(),
        )
    }

    /// New bounds of resizing `b` of `id` by dragging `corner` from `start`
    /// to `current`, the corner snapped to sibling edges or the grid, plus
    /// the snap lines.
    pub(super) fn snapped_resize(
        &self,
        id: &crate::model::ElementId,
        b: &Bounds,
        corner: (i8, i8),
        start: Waypoint,
        current: Waypoint,
    ) -> (Bounds, Vec<Line>) {
        let from = corner_point(b, corner);
        let dragged = Waypoint {
            x: from.x + current.x - start.x,
            y: from.y + current.y - start.y,
        };
        let (to, lines) = if self.aids.free {
            (dragged, Vec::new())
        } else {
            snapping::snap_corner(
                &self.docs[self.active].definitions,
                id,
                dragged,
                self.tolerance(),
                self.snap_grid(),
            )
        };

        (resized(b, corner, to.x - from.x, to.y - from.y), lines)
    }

    /// Bendpoint drag target `p` on the grid (diagram-js `GridSnapping` on
    /// `bendpoint.move`).
    pub(super) fn snapped_bendpoint(&self, p: Waypoint) -> Waypoint {
        snapping::snap_point(p, self.snap_grid())
    }

    /// Segment drag delta along the segment normal, keeping the segment
    /// (at `at`) on the grid (diagram-js `connectionSegment.move`).
    pub(super) fn snapped_segment(&self, at: f64, delta: f64) -> f64 {
        snapping::quantize(at + delta, self.snap_grid()) - at
    }

    /// Center a palette create of `kind` at `p` snaps to: the would-be
    /// shape's center/edges to its new siblings' (diagram-js
    /// `CreateMoveSnapping` on `create.move`), else the grid, plus the
    /// snap lines.
    pub(super) fn snapped_create(
        &self,
        kind: &ElementKind,
        expanded: bool,
        p: Waypoint,
    ) -> (Waypoint, Vec<Line>) {
        let mut defs = (*self.docs[self.active].definitions).clone();
        let id = factory::create_shape(&defs, kind.clone(), expanded, p).and_then(|command| {
            let id = crate::modeling::created_shape(&command)?.1.element_id;
            command.execute(&mut defs).map(|_| id)
        });
        let Some(id) = id else {
            return (snapping::snap_point(p, self.snap_grid()), Vec::new());
        };

        let s = snapping::snap_move(&defs, &[id], 0.0, 0.0, self.tolerance(), self.snap_grid());
        (
            Waypoint {
                x: p.x + s.dx,
                y: p.y + s.dy,
            },
            s.lines,
        )
    }

    /// Space tool delta from `start` to `current` with each axis on the grid
    /// (diagram-js `GridSnapping` on `spaceTool.move`).
    pub(super) fn snapped_space(&self, start: Waypoint, current: Waypoint) -> (f64, f64) {
        let grid = self.snap_grid();

        (
            snapping::quantize(current.x - start.x, grid),
            snapping::quantize(current.y - start.y, grid),
        )
    }

    /// Snap lines of the drag gesture at its current point.
    pub(super) fn drag_lines(&self) -> Vec<Line> {
        let defs = &self.docs[self.active].definitions;

        match &self.gesture {
            Gesture::Move {
                start,
                current,
                snap,
                ..
            } => self.snapped_move(*start, *current, *snap).lines,

            Gesture::Resize {
                id,
                corner,
                start,
                current,
                ..
            } => defs
                .shape(id)
                .map(|s| {
                    self.snapped_resize(id, &s.bounds, *corner, *start, *current)
                        .1
                })
                .unwrap_or_default(),

            _ => Vec::new(),
        }
    }

    /// Whether a pointer drag that auto-scrolls is in progress: a gesture
    /// other than panning, or a palette shape being dragged.
    fn auto_scrolling_drag(&self) -> bool {
        match self.gesture {
            Gesture::Idle => matches!(self.tool, Some(crate::canvas::palette::Tool::Create(..))),
            Gesture::Pan { .. } => false,
            _ => true,
        }
    }

    /// Pointer moved to canvas-local `local` during a drag: scrolls one
    /// step when near an edge and starts the repeat timer.
    pub(super) fn auto_scroll(&mut self, local: Waypoint, cx: &mut Context<Self>) {
        self.aids.pointer = Some(local);
        if !self.auto_scrolling_drag() || !self.scroll_step() {
            return;
        }

        if self.aids.scrolling {
            return;
        }
        self.aids.scrolling = true;

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(auto_scroll::REPEAT_MS))
                    .await;
                let more = this
                    .update(cx, |canvas, cx| {
                        let more = canvas.auto_scrolling_drag() && canvas.scroll_step();
                        if more {
                            cx.notify();
                        } else {
                            canvas.aids.scrolling = false;
                        }
                        more
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
            }
        })
        .detach();
    }

    /// One auto-scroll step at the last pointer: pans, then moves the drag
    /// to the diagram point now under the pointer. Returns whether it
    /// scrolled.
    fn scroll_step(&mut self) -> bool {
        let Some(local) = self.aids.pointer else {
            return false;
        };
        let Some((dx, dy)) = auto_scroll::delta(local.x, local.y, self.size.get()) else {
            return false;
        };

        self.docs[self.active].viewport.pan(dx, dy);
        let p = self.docs[self.active].viewport.to_diagram(local);
        self.cursor = p;

        let previewing = match &mut self.gesture {
            Gesture::Lasso { current, .. } | Gesture::Space { current, .. } => {
                *current = p;
                false
            }

            Gesture::Move {
                current, preview, ..
            }
            | Gesture::Label {
                current, preview, ..
            }
            | Gesture::Resize {
                current, preview, ..
            }
            | Gesture::Waypoints {
                current, preview, ..
            } => {
                *current = p;
                preview.is_some()
            }

            Gesture::Idle | Gesture::Pan { .. } => false,
        };
        if previewing {
            self.refresh_preview();
        }

        true
    }

    /// Ends the drag aids when the pointer is released.
    pub(super) fn end_aids(&mut self) {
        self.aids.pointer = None;
        self.aids.lines.clear();
    }

    /// Hover on a replace-menu row: previews `kind` on the selection
    /// (`None` when the pointer leaves the row).
    pub(super) fn preview_replace(&mut self, kind: Option<&ElementKind>, cx: &mut Context<Self>) {
        let kind = kind.cloned();
        if self.aids.replace_preview != kind {
            self.aids.replace_preview = kind;
            cx.notify();
        }
    }

    /// The document with the hovered replace-menu kind applied to the
    /// single selected shape, while the menu is open.
    pub(super) fn replace_preview(&self) -> Option<Rc<Definitions>> {
        let kind = self
            .aids
            .replace_preview
            .clone()
            .filter(|_| self.replacing)?;
        let id = self.single_selected()?;

        let mut defs = (*self.docs[self.active].definitions).clone();
        Command::ReplaceElement { id, kind }.execute(&mut defs)?;
        Some(Rc::new(defs))
    }
}
