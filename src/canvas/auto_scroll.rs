//! Auto-scroll while dragging after diagram-js `AutoScroll`: a pointer
//! within the threshold of a canvas edge scrolls the canvas toward that
//! edge by a fixed step, repeated on a short timer while it stays there.

/// Distance (screen px) from an edge that starts scrolling (diagram-js
/// `scrollThresholdIn`).
pub const THRESHOLD: f64 = 20.0;

/// Pan per step in screen px (diagram-js `scrollStep`).
pub const STEP: f64 = 10.0;

/// Delay between repeated steps in ms (diagram-js `scrollRepeatTimeout`).
pub const REPEAT_MS: u64 = 15;

/// Viewport pan for a pointer at canvas-local `(x, y)` on a canvas of
/// `size`: `+STEP` near the left/top edge (content moves right/down to
/// reveal what lies beyond), `-STEP` near the right/bottom; `None` when
/// the pointer is away from every edge or outside the canvas.
pub fn delta(x: f64, y: f64, size: (f64, f64)) -> Option<(f64, f64)> {
    let (w, h) = size;
    if w <= 0.0 || h <= 0.0 || x < 0.0 || y < 0.0 || x > w || y > h {
        return None;
    }

    let axis = |v: f64, len: f64| {
        if v < THRESHOLD {
            STEP
        } else if v > len - THRESHOLD {
            -STEP
        } else {
            0.0
        }
    };
    let (dx, dy) = (axis(x, w), axis(y, h));

    (dx != 0.0 || dy != 0.0).then_some((dx, dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each edge scrolls toward itself; the middle and outside do not.
    #[test]
    fn edges_scroll() {
        let size = (800.0, 600.0);

        assert_eq!(delta(5.0, 300.0, size), Some((STEP, 0.0)));
        assert_eq!(delta(795.0, 300.0, size), Some((-STEP, 0.0)));
        assert_eq!(delta(400.0, 10.0, size), Some((0.0, STEP)));
        assert_eq!(delta(795.0, 595.0, size), Some((-STEP, -STEP)));
        assert_eq!(delta(400.0, 300.0, size), None);
        assert_eq!(delta(-5.0, 300.0, size), None);
    }
}
