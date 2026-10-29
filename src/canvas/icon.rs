//! `icon_at`: a Lucide SVG icon centered in a palette or context-pad cell.

use gpui::{Hsla, Svg, div, prelude::*, px, svg};

use crate::model::Bounds;

/// Absolutely positioned cell `b` (relative to its container) holding the
/// asset `path` rendered `side` px square, tinted with `color`.
pub fn icon_at(path: &'static str, b: Bounds, side: f32, color: Hsla) -> impl IntoElement {
    let icon: Svg = svg().path(path).size(px(side)).text_color(color);

    div()
        .absolute()
        .left(px(b.x as f32))
        .top(px(b.y as f32))
        .w(px(b.w as f32))
        .h(px(b.h as f32))
        .flex()
        .items_center()
        .justify_center()
        .child(icon)
}
