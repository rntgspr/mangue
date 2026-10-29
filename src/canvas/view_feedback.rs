//! `Canvas` overlays and modeling feedback (diagram-js `Overlays`, bpmn-js
//! `ModelingFeedback`): the overlay API, the not-allowed cursor and marker
//! while a drag or connect would be refused, and the transient error
//! tooltip left near the cursor when a gesture was refused.

use std::time::Duration;

use gpui::Context;

use super::{Canvas, Gesture};
use crate::canvas::geometry::hit_shape;
use crate::canvas::handles::EdgeGrab;
use crate::canvas::overlay::{Anchor, ERROR_TTL, Overlay, OverlayId, Tone};
use crate::canvas::overlay_box::OverlayBox;
use crate::canvas::palette::{self, Tool};
use crate::model::ElementId;
use crate::modeling::rules::{
    Refusal, connect_refusal, create_refusal, drop_refusal, reconnect_refusal,
};

/// Period of the overlay clock that fades and drops timed overlays.
const TICK: Duration = Duration::from_millis(50);

/// Screen offset (px) of an error tooltip from the cursor (bpmn-js adds 5).
const TOOLTIP_OFFSET: f32 = 5.0;

/// Screen offset (px) of the not-allowed marker from the cursor.
const MARKER_OFFSET: f32 = 14.0;

impl Canvas {
    /// Adds an overlay (diagram-js `overlays.add`); a `ttl` makes it fade
    /// out and disappear on its own. Returns its handle.
    pub fn add_overlay(
        &mut self,
        anchor: Anchor,
        text: impl Into<String>,
        tone: Tone,
        ttl: Option<Duration>,
        cx: &mut Context<Self>,
    ) -> OverlayId {
        let id = self.overlay_store.add(anchor, text, tone, ttl);
        self.start_overlay_clock(cx);
        cx.notify();

        id
    }

    /// Adds a structured overlay (icon, text, theme color, zoom range, click
    /// handler; see `Overlay`) and returns its handle.
    pub fn insert_overlay(&mut self, overlay: Overlay, cx: &mut Context<Self>) -> OverlayId {
        let id = self.overlay_store.insert(overlay);
        self.start_overlay_clock(cx);
        cx.notify();

        id
    }

    /// Removes an overlay (diagram-js `overlays.remove`); returns whether
    /// it existed.
    pub fn remove_overlay(&mut self, id: OverlayId, cx: &mut Context<Self>) -> bool {
        let removed = self.overlay_store.remove(id);
        cx.notify();

        removed
    }

    /// The overlay with this handle (diagram-js `overlays.get`).
    pub fn overlay(&self, id: OverlayId) -> Option<&Overlay> {
        self.overlay_store.get(id)
    }

    /// Overlays anchored to shape `id`.
    pub fn overlays_on(&self, id: &ElementId) -> Vec<&Overlay> {
        self.overlay_store.on(id).collect()
    }

    /// Starts the clock that ages timed overlays, unless it is running; it
    /// stops itself once none is left.
    fn start_overlay_clock(&mut self, cx: &mut Context<Self>) {
        if self.overlay_clock || !self.overlay_store.is_timed() {
            return;
        }
        self.overlay_clock = true;

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let more = this
                    .update(cx, |canvas, cx| {
                        if canvas.overlay_store.tick(TICK) {
                            cx.notify();
                        }

                        let more = canvas.overlay_store.is_timed();
                        canvas.overlay_clock = more;
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

    /// Reports a refused gesture: an error tooltip near the cursor that
    /// fades after `ERROR_TTL`, replacing the previous one.
    pub(super) fn refuse(&mut self, refusal: Refusal, cx: &mut Context<Self>) {
        let stale: Vec<OverlayId> = self
            .overlay_store
            .all()
            .iter()
            .filter(|o| o.tone == Tone::Error)
            .map(|o| o.id)
            .collect();
        for id in stale {
            self.overlay_store.remove(id);
        }

        let at = Anchor::Point(self.cursor);
        self.add_overlay(at, refusal.message(), Tone::Error, Some(ERROR_TTL), cx);
    }

    /// Why `gesture`, as it stands, would be refused on release: a move
    /// dropping a flow node outside every pool or a boundary event on a
    /// border it cannot attach to, or a connection end dropped on a shape it
    /// cannot connect to.
    pub(super) fn gesture_refusal(&self, gesture: &Gesture) -> Option<Refusal> {
        let defs = &self.docs[self.active].definitions;

        match gesture {
            Gesture::Move {
                start,
                current,
                snap,
                preview: Some(_),
            } => {
                let snapped = self.snapped_move(*start, *current, *snap);

                drop_refusal(defs, &self.moving_ids(), snapped.dx, snapped.dy)
            }

            Gesture::Waypoints {
                id,
                grab: EdgeGrab::Endpoint(start),
                current,
                preview: Some(_),
                ..
            } => {
                let to = hit_shape(defs.shapes(), *current)?;

                reconnect_refusal(defs, id, *start, to)
            }

            _ => None,
        }
    }

    /// Why dropping the armed palette shape, or connecting to the shape
    /// under the cursor, would be refused right now.
    pub(super) fn armed_refusal(&self) -> Option<Refusal> {
        let defs = &self.docs[self.active].definitions;

        if let Some(Tool::Create(kind, expanded)) = &self.tool {
            if palette::covers(self.docs[self.active].viewport.to_screen(self.cursor)) {
                return None;
            }

            let (p, _) = self.snapped_create(kind, *expanded, self.cursor);
            return create_refusal(defs, kind, p);
        }

        let source = self.connect_source.as_ref()?;
        let target = hit_shape(defs.shapes(), self.cursor)?;

        connect_refusal(defs, source, target)
    }

    /// The refusal the pointer currently hovers over, if any: drives the
    /// not-allowed cursor and marker.
    pub(super) fn current_refusal(&self) -> Option<Refusal> {
        self.gesture_refusal(&self.gesture)
            .or_else(|| self.armed_refusal())
    }

    /// Overlays and the not-allowed marker as boxes at canvas-local screen
    /// positions, following the viewport.
    pub(super) fn overlay_boxes(&self, refused: bool) -> Vec<OverlayBox> {
        let mut boxes: Vec<OverlayBox> = self
            .overlay_store
            .all()
            .iter()
            .filter(|o| o.visible_at(self.docs[self.active].viewport.scale))
            .filter_map(|o| {
                let p = self.docs[self.active]
                    .viewport
                    .to_screen(o.position(&self.docs[self.active].definitions)?);
                let (x, y) = (p.x as f32, p.y as f32);

                let content = match o.anchor {
                    Anchor::Point(_) => OverlayBox::tooltip(
                        x + TOOLTIP_OFFSET,
                        y + TOOLTIP_OFFSET,
                        o.text.clone(),
                        o.tone,
                        o.opacity(),
                    ),
                    Anchor::Element(..) => {
                        OverlayBox::badge(x, y, o.text.clone(), o.tone, o.opacity())
                    }
                };

                Some(
                    content
                        .icon(o.icon.clone())
                        .color(o.color.clone())
                        .on_click(o.on_click.clone()),
                )
            })
            .collect();

        if refused {
            let p = self.docs[self.active].viewport.to_screen(self.cursor);
            boxes.push(OverlayBox::badge(
                p.x as f32 + MARKER_OFFSET,
                p.y as f32 + MARKER_OFFSET,
                "x".to_string(),
                Tone::Error,
                1.0,
            ));
        }

        boxes
    }
}
