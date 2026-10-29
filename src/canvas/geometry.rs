//! GPUI-independent canvas geometry: viewport transform and hit-testing.

use crate::model::{Bounds, Edge, ElementId, Shape, Waypoint};

/// Smallest allowed zoom factor.
pub const MIN_SCALE: f64 = 0.2;

/// Largest allowed zoom factor.
pub const MAX_SCALE: f64 = 4.0;

/// Maps diagram coordinates to screen coordinates: `screen = diagram * scale + offset`.
/// Screen coordinates are relative to the canvas origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub offset: Waypoint,
    pub scale: f64,
}

impl Default for Viewport {
    /// Identity transform: no offset, scale 1.
    fn default() -> Self {
        Viewport {
            offset: Waypoint { x: 0.0, y: 0.0 },
            scale: 1.0,
        }
    }
}

impl Viewport {
    /// Converts a diagram point to screen space.
    pub fn to_screen(&self, p: Waypoint) -> Waypoint {
        Waypoint {
            x: p.x * self.scale + self.offset.x,
            y: p.y * self.scale + self.offset.y,
        }
    }

    /// Converts a screen point to diagram space.
    pub fn to_diagram(&self, p: Waypoint) -> Waypoint {
        Waypoint {
            x: (p.x - self.offset.x) / self.scale,
            y: (p.y - self.offset.y) / self.scale,
        }
    }

    /// Converts diagram bounds to screen bounds.
    pub fn bounds_to_screen(&self, b: Bounds) -> Bounds {
        let origin = self.to_screen(Waypoint { x: b.x, y: b.y });

        Bounds {
            x: origin.x,
            y: origin.y,
            w: b.w * self.scale,
            h: b.h * self.scale,
        }
    }

    /// Moves the view by a screen-space delta.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.offset.x += dx;
        self.offset.y += dy;
    }

    /// Multiplies the scale by `factor` (clamped to `MIN_SCALE..=MAX_SCALE`)
    /// keeping the diagram point under `cursor` fixed on screen.
    pub fn zoom_at(&mut self, cursor: Waypoint, factor: f64) {
        let anchor = self.to_diagram(cursor);

        self.scale = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
        self.offset.x = cursor.x - anchor.x * self.scale;
        self.offset.y = cursor.y - anchor.y * self.scale;
    }

    /// One zoom step in or out (`direction` > 0 zooms in) around `center`,
    /// on diagram-js `ZoomScroll.stepZoom`'s log scale: 10 steps between
    /// `MIN_SCALE` and `MAX_SCALE`.
    pub fn step_zoom(&mut self, center: Waypoint, direction: f64) {
        let step = (MAX_SCALE / MIN_SCALE).ln() / 10.0;

        self.zoom_at(center, (step * direction.signum()).exp());
    }

    /// Viewport showing all of `content` centered in a `width` x `height`
    /// view, never zoomed in past 1 (diagram-js `canvas.zoom('fit-viewport',
    /// 'auto')`).
    pub fn fit(content: Bounds, width: f64, height: f64) -> Viewport {
        let scale = (width / content.w)
            .min(height / content.h)
            .min(1.0)
            .clamp(MIN_SCALE, MAX_SCALE);

        Viewport {
            offset: Waypoint {
                x: width / 2.0 - (content.x + content.w / 2.0) * scale,
                y: height / 2.0 - (content.y + content.h / 2.0) * scale,
            },
            scale,
        }
    }
}

/// Returns whether `p` lies inside `b` (edges inclusive).
pub fn contains(b: &Bounds, p: Waypoint) -> bool {
    p.x >= b.x && p.x <= b.x + b.w && p.y >= b.y && p.y <= b.y + b.h
}

/// Returns the topmost shape (last in paint order) containing the diagram point.
pub fn hit_shape<'a>(
    shapes: impl IntoIterator<Item = &'a Shape>,
    p: Waypoint,
) -> Option<&'a ElementId> {
    shapes
        .into_iter()
        .filter(|s| contains(&s.bounds, p))
        .last()
        .map(|s| &s.element_id)
}

/// Distance from `p` to the segment `a-b`.
pub fn segment_distance(p: Waypoint, a: Waypoint, b: Waypoint) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };

    let (cx, cy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// Returns the topmost edge with a segment within `tolerance` of `p`.
pub fn hit_edge<'a>(
    edges: impl IntoIterator<Item = &'a Edge>,
    p: Waypoint,
    tolerance: f64,
) -> Option<&'a ElementId> {
    edges
        .into_iter()
        .filter(|e| {
            e.waypoints
                .windows(2)
                .any(|w| segment_distance(p, w[0], w[1]) <= tolerance)
        })
        .last()
        .map(|e| &e.element_id)
}

/// Normalized rectangle spanned by two corner points.
pub fn rect_from(a: Waypoint, b: Waypoint) -> Bounds {
    Bounds {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        w: (a.x - b.x).abs(),
        h: (a.y - b.y).abs(),
    }
}

/// Returns whether `inner` lies completely inside `outer`.
pub fn encloses(outer: &Bounds, inner: &Bounds) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.w <= outer.x + outer.w
        && inner.y + inner.h <= outer.y + outer.h
}

/// Returns the two base corners of an arrowhead whose tip is `to`,
/// pointing along `from -> to`, with the given length and half-width.
pub fn arrowhead(from: Waypoint, to: Waypoint, length: f64, half_width: f64) -> [Waypoint; 2] {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = (dx * dx + dy * dy).sqrt().max(f64::EPSILON);
    let (ux, uy) = (dx / len, dy / len);

    let base = Waypoint {
        x: to.x - ux * length,
        y: to.y - uy * length,
    };

    [
        Waypoint {
            x: base.x - uy * half_width,
            y: base.y + ux * half_width,
        },
        Waypoint {
            x: base.x + uy * half_width,
            y: base.y - ux * half_width,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shorthand point constructor.
    fn pt(x: f64, y: f64) -> Waypoint {
        Waypoint { x, y }
    }

    /// Shorthand shape constructor.
    fn shape(id: &str, x: f64, y: f64, w: f64, h: f64) -> Shape {
        Shape {
            element_id: id.into(),
            bounds: Bounds { x, y, w, h },
            is_expanded: None,
            is_horizontal: None,
            label_bounds: None,
        }
    }

    /// Screen and diagram conversions are inverse of each other.
    #[test]
    fn screen_diagram_roundtrip() {
        let vp = Viewport {
            offset: pt(30.0, -10.0),
            scale: 2.0,
        };

        assert_eq!(vp.to_screen(pt(10.0, 5.0)), pt(50.0, 0.0));
        assert_eq!(vp.to_diagram(pt(50.0, 0.0)), pt(10.0, 5.0));
    }

    /// Bounds scale in size and translate in origin.
    #[test]
    fn bounds_to_screen_scales() {
        let vp = Viewport {
            offset: pt(5.0, 5.0),
            scale: 0.5,
        };
        let b = vp.bounds_to_screen(Bounds {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 80.0,
        });

        assert_eq!(
            b,
            Bounds {
                x: 10.0,
                y: 15.0,
                w: 50.0,
                h: 40.0
            }
        );
    }

    /// Panning shifts the offset by the screen delta.
    #[test]
    fn pan_moves_offset() {
        let mut vp = Viewport::default();
        vp.pan(12.0, -4.0);

        assert_eq!(vp.offset, pt(12.0, -4.0));
    }

    /// Zooming keeps the diagram point under the cursor fixed.
    #[test]
    fn zoom_keeps_cursor_anchor() {
        let mut vp = Viewport {
            offset: pt(40.0, 20.0),
            scale: 1.0,
        };
        let cursor = pt(200.0, 150.0);
        let before = vp.to_diagram(cursor);

        vp.zoom_at(cursor, 1.5);

        assert_eq!(vp.scale, 1.5);
        let after = vp.to_diagram(cursor);
        assert!((before.x - after.x).abs() < 1e-9 && (before.y - after.y).abs() < 1e-9);
    }

    /// Zoom is clamped to the allowed range.
    #[test]
    fn zoom_is_clamped() {
        let mut vp = Viewport::default();

        vp.zoom_at(pt(0.0, 0.0), 100.0);
        assert_eq!(vp.scale, MAX_SCALE);

        vp.zoom_at(pt(0.0, 0.0), 0.0001);
        assert_eq!(vp.scale, MIN_SCALE);
    }

    /// Hit-testing picks the last (topmost) shape and misses empty space.
    #[test]
    fn hit_shape_picks_topmost() {
        let shapes = vec![
            shape("pool", 0.0, 0.0, 500.0, 300.0),
            shape("task", 100.0, 100.0, 100.0, 80.0),
        ];

        assert_eq!(
            hit_shape(&shapes, pt(150.0, 140.0)).map(|i| i.as_str()),
            Some("task")
        );
        assert_eq!(
            hit_shape(&shapes, pt(10.0, 10.0)).map(|i| i.as_str()),
            Some("pool")
        );
        assert_eq!(hit_shape(&shapes, pt(600.0, 10.0)), None);
    }

    /// Edge hit-testing uses segment distance with a tolerance.
    #[test]
    fn hit_edge_by_distance() {
        let edges = vec![Edge {
            element_id: "f".into(),
            waypoints: vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 50.0)],
            label_bounds: None,
        }];

        assert!(hit_edge(&edges, pt(50.0, 3.0), 5.0).is_some());
        assert!(hit_edge(&edges, pt(104.0, 25.0), 5.0).is_some());
        assert!(hit_edge(&edges, pt(50.0, 20.0), 5.0).is_none());
    }

    /// Lasso rectangles normalize and enclose fully contained bounds only.
    #[test]
    fn rect_from_and_encloses() {
        let r = rect_from(pt(10.0, 50.0), pt(0.0, 0.0));
        let inner = |x, y| Bounds {
            x,
            y,
            w: 5.0,
            h: 5.0,
        };

        assert_eq!(
            r,
            Bounds {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 50.0
            }
        );
        assert!(encloses(&r, &inner(1.0, 1.0)));
        assert!(!encloses(&r, &inner(8.0, 1.0)));
    }

    /// Arrowhead corners sit behind the tip, symmetric around the direction.
    #[test]
    fn arrowhead_corners() {
        let [a, b] = arrowhead(pt(0.0, 0.0), pt(10.0, 0.0), 4.0, 2.0);

        assert_eq!(a, pt(6.0, 2.0));
        assert_eq!(b, pt(6.0, -2.0));
    }

    /// Fit centers large content scaled down, and small content at scale 1.
    #[test]
    fn fit_viewport() {
        let big = Viewport::fit(
            Bounds {
                x: 0.0,
                y: 0.0,
                w: 2000.0,
                h: 500.0,
            },
            1000.0,
            800.0,
        );
        assert_eq!(big.scale, 0.5);
        assert_eq!(big.to_screen(pt(1000.0, 250.0)), pt(500.0, 400.0));

        let small = Viewport::fit(
            Bounds {
                x: 100.0,
                y: 100.0,
                w: 100.0,
                h: 100.0,
            },
            1000.0,
            800.0,
        );
        assert_eq!(small.scale, 1.0);
        assert_eq!(small.to_screen(pt(150.0, 150.0)), pt(500.0, 400.0));
    }

    /// Ten zoom steps in from the minimum reach the maximum scale.
    #[test]
    fn step_zoom_log_scale() {
        let mut v = Viewport {
            offset: pt(0.0, 0.0),
            scale: MIN_SCALE,
        };

        for _ in 0..10 {
            v.step_zoom(pt(0.0, 0.0), 1.0);
        }
        assert!((v.scale - MAX_SCALE).abs() < 1e-9);
    }
}
