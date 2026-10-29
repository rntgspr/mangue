//! Editor actions of the `Canvas` view after bpmn-js `BpmnEditorActions`:
//! select all, copy/paste, zoom steps and fit, align/distribute, arrow-key
//! moves and the single-letter tool shortcuts of `BpmnKeyboardBindings`.

use gpui::{Context, KeyDownEvent, Window};

use super::super::actions::{
    AlignBottom, AlignCenter, AlignLeft, AlignMiddle, AlignRight, AlignTop, Copy,
    DistributeHorizontally, DistributeVertically, Paste, SelectAll, ZoomFit, ZoomIn, ZoomOut,
    ZoomReset,
};
use super::super::geometry::Viewport;
use super::super::palette::Tool;
use super::Canvas;
use crate::model::{Bounds, Waypoint};
use crate::modeling::arrange::{self, Alignment, Axis};
use crate::modeling::{auto_resize, clipboard, factory};

/// Arrow-key move distance (diagram-js `KeyboardMoveSelection.moveSpeed`).
const MOVE_SPEED: f64 = 1.0;

/// Arrow-key move distance with shift (`moveSpeedAccelerated`).
const MOVE_SPEED_ACCELERATED: f64 = 10.0;

/// Cmd + arrow canvas scroll distance (diagram-js `KeyboardMove.moveSpeed`).
const SCROLL_SPEED: f64 = 50.0;

/// Cmd + shift + arrow canvas scroll distance (`moveSpeedAccelerated`).
const SCROLL_SPEED_ACCELERATED: f64 = 200.0;

impl Canvas {
    /// Single-key shortcuts outside label editing: arrows move the selection
    /// (cmd + arrows scroll the canvas), `h`/`l`/`s`/`c` arm the hand, lasso,
    /// space and global connect tools, `e` edits the selected label, `a`
    /// opens the append menu and `r` the replace menu. Returns whether the key was handled.
    pub(super) fn on_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;

        let arrow = match keystroke.key.as_str() {
            "left" => Some((-1.0, 0.0)),
            "right" => Some((1.0, 0.0)),
            "up" => Some((0.0, -1.0)),
            "down" => Some((0.0, 1.0)),
            _ => None,
        };
        if let Some((x, y)) = arrow {
            if m.platform || m.control {
                let speed = if m.shift {
                    SCROLL_SPEED_ACCELERATED
                } else {
                    SCROLL_SPEED
                };
                self.docs[self.active].viewport.pan(-x * speed, -y * speed);
                return true;
            }

            let speed = if m.shift {
                MOVE_SPEED_ACCELERATED
            } else {
                MOVE_SPEED
            };
            if let Some(command) = self.move_command(x * speed, y * speed) {
                self.execute(command, cx);
            }
            return true;
        }

        if m.modified() {
            return false;
        }

        let tool = match keystroke.key.as_str() {
            "h" => Some(Tool::Hand),
            "l" => Some(Tool::Lasso),
            "s" => Some(Tool::Space),
            "c" => Some(Tool::Connect),
            _ => None,
        };
        if let Some(tool) = tool {
            self.reset_tools();
            self.tool = Some(tool);
            return true;
        }

        match keystroke.key.as_str() {
            "e" => {
                let [id] = self.docs[self.active].selection.as_slice() else {
                    return false;
                };
                self.start_edit(id.clone(), cx);
                true
            }

            "a" => self.open_append_menu(cx),

            "r" => {
                let replaceable = self.single_selected().is_some_and(|id| {
                    self.docs[self.active]
                        .definitions
                        .get(&id)
                        .is_some_and(|e| !factory::replace_options(&e.kind).is_empty())
                });
                self.replacing = replaceable;
                self.replace_active = None;
                replaceable
            }

            _ => false,
        }
    }

    /// Selects every shape and connection (bpmn-js `selectElements`).
    pub(super) fn on_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }

        let defs = &self.docs[self.active].definitions;
        self.docs[self.active].selection = defs
            .shapes()
            .map(|s| s.element_id.clone())
            .chain(defs.edges().map(|e| e.element_id.clone()))
            .collect();
        cx.notify();
    }

    /// Copies the selection into the in-app clipboard.
    pub(super) fn on_copy(&mut self, _: &Copy, _: &mut Window, _: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }

        if let Some(clip) = clipboard::copy(
            &self.docs[self.active].definitions,
            &self.docs[self.active].selection,
        ) {
            self.clipboard = Some(clip);
        }
    }

    /// Pastes the clipboard centered at the cursor (grid snapped) with fresh
    /// ids, growing the container it lands in, and selects the pasted shapes.
    pub(super) fn on_paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        let Some(clip) = self.clipboard.as_ref().filter(|c| !c.is_empty()) else {
            return;
        };

        let at = Waypoint {
            x: self.grid.snap(self.cursor.x),
            y: self.grid.snap(self.cursor.y),
        };
        let Some((command, roots)) =
            clipboard::paste(&self.docs[self.active].definitions, clip, at)
        else {
            return;
        };

        let command = auto_resize::with_auto_resize(&self.docs[self.active].definitions, command);
        if self.execute(command, cx) {
            self.docs[self.active].selection = roots;
        }
    }

    /// Center of the canvas in screen px.
    fn screen_center(&self) -> Waypoint {
        let (w, h) = self.size.get();

        Waypoint {
            x: w / 2.0,
            y: h / 2.0,
        }
    }

    /// Zooms one step in around the canvas center.
    pub(super) fn on_zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        let center = self.screen_center();
        self.docs[self.active].viewport.step_zoom(center, 1.0);
        cx.notify();
    }

    /// Zooms one step out around the canvas center.
    pub(super) fn on_zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        let center = self.screen_center();
        self.docs[self.active].viewport.step_zoom(center, -1.0);
        cx.notify();
    }

    /// Resets the zoom to 1 around the canvas center (diagram-js `zoom(1)`).
    pub(super) fn on_zoom_reset(&mut self, _: &ZoomReset, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        let factor = 1.0 / self.docs[self.active].viewport.scale;
        let center = self.screen_center();
        self.docs[self.active].viewport.zoom_at(center, factor);
        cx.notify();
    }

    /// Fits the whole diagram into the canvas on the next render.
    pub(super) fn on_zoom_fit(&mut self, _: &ZoomFit, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        self.fit_pending = true;
        cx.notify();
    }

    /// Applies a pending fit once the canvas size is known (it is written on
    /// the first paint, so the first frame requests another one).
    pub(super) fn apply_fit(&mut self, window: &mut Window) {
        if !self.fit_pending || self.docs.is_empty() {
            return;
        }

        let (w, h) = self.size.get();
        if w <= 0.0 || h <= 0.0 {
            window.request_animation_frame();
            return;
        }

        self.fit_pending = false;
        if let Some(content) = content_bounds(self) {
            self.docs[self.active].viewport = Viewport::fit(content, w, h);
        }
    }

    /// Aligns the selection (bpmn-js `alignElements`).
    fn align(&mut self, alignment: Alignment, cx: &mut Context<Self>) {
        if let Some(command) = arrange::align(
            &self.docs[self.active].definitions,
            &self.docs[self.active].selection,
            alignment,
        ) {
            self.execute(command, cx);
        }
    }

    /// Distributes the selection (bpmn-js `distributeElements`).
    fn distribute(&mut self, axis: Axis, cx: &mut Context<Self>) {
        if let Some(command) = arrange::distribute(
            &self.docs[self.active].definitions,
            &self.docs[self.active].selection,
            axis,
        ) {
            self.execute(command, cx);
        }
    }

    /// Aligns the selection's left edges.
    pub(super) fn on_align_left(&mut self, _: &AlignLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.align(Alignment::Left, cx);
    }

    /// Aligns the selection's horizontal centers.
    pub(super) fn on_align_center(
        &mut self,
        _: &AlignCenter,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.align(Alignment::Center, cx);
    }

    /// Aligns the selection's right edges.
    pub(super) fn on_align_right(
        &mut self,
        _: &AlignRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.align(Alignment::Right, cx);
    }

    /// Aligns the selection's top edges.
    pub(super) fn on_align_top(&mut self, _: &AlignTop, _: &mut Window, cx: &mut Context<Self>) {
        self.align(Alignment::Top, cx);
    }

    /// Aligns the selection's vertical middles.
    pub(super) fn on_align_middle(
        &mut self,
        _: &AlignMiddle,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.align(Alignment::Middle, cx);
    }

    /// Aligns the selection's bottom edges.
    pub(super) fn on_align_bottom(
        &mut self,
        _: &AlignBottom,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.align(Alignment::Bottom, cx);
    }

    /// Distributes the selection with equal horizontal gaps.
    pub(super) fn on_distribute_horizontally(
        &mut self,
        _: &DistributeHorizontally,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.distribute(Axis::Horizontal, cx);
    }

    /// Distributes the selection with equal vertical gaps.
    pub(super) fn on_distribute_vertically(
        &mut self,
        _: &DistributeVertically,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.distribute(Axis::Vertical, cx);
    }
}

/// Bounds of every shape and waypoint of the canvas document.
fn content_bounds(canvas: &Canvas) -> Option<Bounds> {
    let defs = &canvas.docs[canvas.active].definitions;
    let points = defs
        .shapes()
        .flat_map(|s| {
            let b = s.bounds;
            [
                Waypoint { x: b.x, y: b.y },
                Waypoint {
                    x: b.x + b.w,
                    y: b.y + b.h,
                },
            ]
        })
        .chain(defs.edges().flat_map(|e| e.waypoints.iter().copied()));

    let (mut min, mut max) = (
        Waypoint {
            x: f64::INFINITY,
            y: f64::INFINITY,
        },
        Waypoint {
            x: f64::NEG_INFINITY,
            y: f64::NEG_INFINITY,
        },
    );
    for p in points {
        min = Waypoint {
            x: min.x.min(p.x),
            y: min.y.min(p.y),
        };
        max = Waypoint {
            x: max.x.max(p.x),
            y: max.y.max(p.y),
        };
    }

    (min.x.is_finite() && max.x > min.x && max.y > min.y).then_some(Bounds {
        x: min.x,
        y: min.y,
        w: max.x - min.x,
        h: max.y - min.y,
    })
}
