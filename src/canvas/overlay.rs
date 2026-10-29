//! Canvas overlays (diagram-js `Overlays`, plus the timed tooltips of
//! `Tooltips`): small themed badges and tooltips anchored to an element
//! corner or to a diagram point. They are stored in diagram coordinates, so
//! the viewport carries them along; `OverlayBox` draws them.

use std::rc::Rc;
use std::time::Duration;

use gpui::{App, Window};

use crate::model::{Definitions, ElementId, Waypoint};

/// Time an overlay takes to fade out at the end of its life.
pub const FADE: Duration = Duration::from_millis(500);

/// Life of an error tooltip (bpmn-js `ModelingFeedback`: 2000 ms).
pub const ERROR_TTL: Duration = Duration::from_millis(2000);

/// Corner of an element's bounds an overlay is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// What an overlay is positioned against.
#[derive(Debug, Clone, PartialEq)]
pub enum Anchor {
    /// A corner of the shape with this id; hidden when the shape is gone.
    Element(ElementId, Corner),
    /// A fixed diagram point.
    Point(Waypoint),
}

/// Colors an overlay takes from the theme (`overlay.badge`, `overlay.error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Badge,
    Error,
}

/// Handle returned by `Overlays::add`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayId(u64);

/// Click handler of an overlay, called with the window and app like a GPUI
/// listener; clones share the closure and compare by identity.
#[derive(Clone)]
pub struct Click(Rc<dyn Fn(&mut Window, &mut App)>);

impl Click {
    /// Wraps `handler`.
    pub fn new(handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Click(Rc::new(handler))
    }

    /// Runs the handler.
    pub fn call(&self, window: &mut Window, cx: &mut App) {
        (self.0)(window, cx);
    }
}

impl std::fmt::Debug for Click {
    /// Prints a placeholder; closures have no useful `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Click(..)")
    }
}

impl PartialEq for Click {
    /// Two handlers are equal when they share the same closure.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Zoom scales an overlay is shown at (diagram-js `show: { minZoom,
/// maxZoom }`); a missing bound is open, both bounds are inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ZoomRange {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl ZoomRange {
    /// Whether `scale` lies within the range.
    pub fn contains(&self, scale: f64) -> bool {
        self.min.is_none_or(|min| scale >= min) && self.max.is_none_or(|max| scale <= max)
    }
}

/// One overlay: structured content (optional icon from `assets/icons`, text,
/// optional theme color key) at an anchor, living for `remaining` (forever
/// when `None`), shown only within `show` and clickable when `on_click` is
/// set.
#[derive(Debug, Clone, PartialEq)]
pub struct Overlay {
    pub id: OverlayId,
    pub anchor: Anchor,
    pub text: String,
    pub tone: Tone,
    pub remaining: Option<Duration>,
    /// Icon name without folder or extension (`"plus"` for
    /// `icons/plus.svg`); unknown names draw no icon.
    pub icon: Option<String>,
    /// Theme color key (see `Theme::color_for`) tinting the content in place
    /// of the tone's text color; unknown keys keep the tone's.
    pub color: Option<String>,
    pub show: ZoomRange,
    pub on_click: Option<Click>,
}

impl Overlay {
    /// Overlay with a text at `anchor`, always shown, not clickable. Its id
    /// is assigned by `Overlays::insert`.
    pub fn new(anchor: Anchor, text: impl Into<String>, tone: Tone) -> Self {
        Overlay {
            id: OverlayId(u64::MAX),
            anchor,
            text: text.into(),
            tone,
            remaining: None,
            icon: None,
            color: None,
            show: ZoomRange::default(),
            on_click: None,
        }
    }

    /// Whether the overlay is shown at viewport zoom `scale`.
    pub fn visible_at(&self, scale: f64) -> bool {
        self.show.contains(scale)
    }

    /// Diagram point the overlay sits at, `None` when its element is gone.
    pub fn position(&self, defs: &Definitions) -> Option<Waypoint> {
        match &self.anchor {
            Anchor::Point(p) => Some(*p),
            Anchor::Element(id, corner) => {
                let b = defs.shape(id)?.bounds;
                let (right, bottom) = (b.x + b.w, b.y + b.h);
                let (x, y) = match corner {
                    Corner::TopLeft => (b.x, b.y),
                    Corner::TopRight => (right, b.y),
                    Corner::BottomLeft => (b.x, bottom),
                    Corner::BottomRight => (right, bottom),
                };

                Some(Waypoint { x, y })
            }
        }
    }

    /// Opacity from 1 down to 0 over the last `FADE` of a timed overlay.
    pub fn opacity(&self) -> f32 {
        match self.remaining {
            Some(left) if left < FADE => left.as_secs_f32() / FADE.as_secs_f32(),
            _ => 1.0,
        }
    }
}

/// Registry of the canvas overlays (diagram-js `Overlays` add / remove /
/// get).
#[derive(Debug, Default)]
pub struct Overlays {
    items: Vec<Overlay>,
    next: u64,
}

impl Overlays {
    /// Adds a plain text overlay and returns its handle; `ttl` is how long
    /// it lives.
    pub fn add(
        &mut self,
        anchor: Anchor,
        text: impl Into<String>,
        tone: Tone,
        ttl: Option<Duration>,
    ) -> OverlayId {
        self.insert(Overlay {
            remaining: ttl,
            ..Overlay::new(anchor, text, tone)
        })
    }

    /// Adds `overlay` (its own `id` is replaced) and returns its handle.
    pub fn insert(&mut self, mut overlay: Overlay) -> OverlayId {
        let id = OverlayId(self.next);
        self.next += 1;
        overlay.id = id;
        self.items.push(overlay);

        id
    }

    /// Removes an overlay; returns whether it existed.
    pub fn remove(&mut self, id: OverlayId) -> bool {
        let before = self.items.len();
        self.items.retain(|o| o.id != id);

        self.items.len() != before
    }

    /// The overlay with this handle.
    pub fn get(&self, id: OverlayId) -> Option<&Overlay> {
        self.items.iter().find(|o| o.id == id)
    }

    /// Every overlay, oldest first.
    pub fn all(&self) -> &[Overlay] {
        &self.items
    }

    /// Overlays anchored to shape `id`.
    pub fn on(&self, id: &ElementId) -> impl Iterator<Item = &Overlay> {
        self.items
            .iter()
            .filter(move |o| matches!(&o.anchor, Anchor::Element(e, _) if e == id))
    }

    /// Drops the overlays anchored to shapes that no longer exist.
    pub fn prune(&mut self, defs: &Definitions) {
        self.items.retain(|o| match &o.anchor {
            Anchor::Element(id, _) => defs.shape(id).is_some(),
            Anchor::Point(_) => true,
        });
    }

    /// Whether any overlay is timed, so a clock is needed.
    pub fn is_timed(&self) -> bool {
        self.items.iter().any(|o| o.remaining.is_some())
    }

    /// Ages timed overlays by `dt` and drops the expired ones; returns
    /// whether anything changed (a repaint is due).
    pub fn tick(&mut self, dt: Duration) -> bool {
        let mut changed = false;
        for overlay in &mut self.items {
            if let Some(left) = &mut overlay.remaining {
                *left = left.saturating_sub(dt);
                changed = true;
            }
        }

        self.items.retain(|o| o.remaining != Some(Duration::ZERO));

        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Bounds, Element, ElementKind, Shape};

    /// Document with one task at (10, 20) of size 100 x 80.
    fn defs() -> Definitions {
        let mut defs = Definitions::default();
        defs.add_element(Element::new("t", ElementKind::Task));
        defs.add_shape(Shape {
            element_id: "t".into(),
            bounds: Bounds {
                x: 10.0,
                y: 20.0,
                w: 100.0,
                h: 80.0,
            },
            is_expanded: None,
            is_horizontal: None,
            label_bounds: None,
        });

        defs
    }

    /// add / get / remove round-trip, and `on` filters by element.
    #[test]
    fn add_get_remove() {
        let mut overlays = Overlays::default();
        let a = overlays.add(
            Anchor::Element("t".into(), Corner::TopRight),
            "1",
            Tone::Badge,
            None,
        );
        let b = overlays.add(
            Anchor::Point(Waypoint { x: 0.0, y: 0.0 }),
            "x",
            Tone::Error,
            None,
        );

        assert_eq!(overlays.get(a).map(|o| o.text.as_str()), Some("1"));
        assert_eq!(overlays.on(&"t".into()).count(), 1);
        assert!(overlays.remove(a));
        assert!(!overlays.remove(a));
        assert!(overlays.get(a).is_none());
        assert!(overlays.get(b).is_some());
    }

    /// Element anchors resolve to the bounds corner, in diagram space.
    #[test]
    fn element_corners() {
        let defs = defs();
        let mut overlays = Overlays::default();
        let corner = |overlays: &mut Overlays, c| {
            let id = overlays.add(Anchor::Element("t".into(), c), "", Tone::Badge, None);
            overlays.get(id).and_then(|o| o.position(&defs))
        };

        assert_eq!(
            corner(&mut overlays, Corner::TopLeft),
            Some(Waypoint { x: 10.0, y: 20.0 })
        );
        assert_eq!(
            corner(&mut overlays, Corner::TopRight),
            Some(Waypoint { x: 110.0, y: 20.0 })
        );
        assert_eq!(
            corner(&mut overlays, Corner::BottomLeft),
            Some(Waypoint { x: 10.0, y: 100.0 })
        );
        assert_eq!(
            corner(&mut overlays, Corner::BottomRight),
            Some(Waypoint { x: 110.0, y: 100.0 })
        );
    }

    /// A zoom range is inclusive and open-ended on a missing bound.
    #[test]
    fn zoom_range_bounds() {
        let both = ZoomRange {
            min: Some(0.5),
            max: Some(2.0),
        };
        assert!(both.contains(0.5) && both.contains(1.0) && both.contains(2.0));
        assert!(!both.contains(0.4) && !both.contains(2.1));

        assert!(ZoomRange::default().contains(0.01));
        assert!(
            ZoomRange {
                min: Some(1.0),
                max: None
            }
            .contains(50.0)
        );
        assert!(
            !ZoomRange {
                min: None,
                max: Some(1.0)
            }
            .contains(1.1)
        );
    }

    /// `insert` assigns the handle and keeps the structured content; clones
    /// of a click handler compare equal, distinct handlers do not.
    #[test]
    fn insert_keeps_structured_content() {
        let mut overlays = Overlays::default();
        let click = Click::new(|_, _| {});
        let mut overlay =
            Overlay::new(Anchor::Point(Waypoint { x: 0.0, y: 0.0 }), "n", Tone::Badge);
        overlay.icon = Some("plus".to_string());
        overlay.color = Some("selection".to_string());
        overlay.show.min = Some(1.0);
        overlay.on_click = Some(click.clone());

        let id = overlays.insert(overlay);
        let stored = overlays.get(id).expect("inserted");

        assert_eq!(stored.id, id);
        assert_eq!(stored.icon.as_deref(), Some("plus"));
        assert!(stored.visible_at(1.0) && !stored.visible_at(0.9));
        assert_eq!(stored.on_click, Some(click));
        assert_ne!(stored.on_click, Some(Click::new(|_, _| {})));
    }

    /// Overlays of a removed element are pruned; point overlays stay.
    #[test]
    fn prune_drops_overlays_of_missing_elements() {
        let mut overlays = Overlays::default();
        overlays.add(
            Anchor::Element("gone".into(), Corner::TopLeft),
            "",
            Tone::Badge,
            None,
        );
        overlays.add(
            Anchor::Point(Waypoint { x: 1.0, y: 1.0 }),
            "",
            Tone::Badge,
            None,
        );
        overlays.add(
            Anchor::Element("t".into(), Corner::TopLeft),
            "",
            Tone::Badge,
            None,
        );

        overlays.prune(&defs());
        assert_eq!(overlays.all().len(), 2);
    }

    /// Timed overlays fade over their last `FADE` and expire at zero.
    #[test]
    fn timed_overlays_fade_and_expire() {
        let mut overlays = Overlays::default();
        let id = overlays.add(
            Anchor::Point(Waypoint { x: 0.0, y: 0.0 }),
            "e",
            Tone::Error,
            Some(ERROR_TTL),
        );

        assert!(overlays.is_timed());
        assert!(overlays.tick(Duration::from_millis(1000)));
        assert_eq!(overlays.get(id).map(Overlay::opacity), Some(1.0));

        overlays.tick(Duration::from_millis(750));
        let faded = overlays.get(id).map(Overlay::opacity).unwrap();
        assert!(faded > 0.4 && faded < 0.6, "{faded}");

        overlays.tick(Duration::from_millis(250));
        assert!(overlays.get(id).is_none());
        assert!(!overlays.is_timed());
    }
}
