//! `DrillButton`: the "open" overlay of a collapsed sub-process with its
//! own plane (bpmn-js `.bjs-drilldown`), a chevron on the selection color.

use std::rc::Rc;

use gpui::{App, CursorStyle, MouseButton, Window, div, prelude::*, px, svg};

use crate::model::Bounds;
use crate::theme::Theme;

/// Button at screen rect `rect`; a press goes to `on_open`.
#[derive(IntoElement)]
pub struct DrillButton {
    rect: Bounds,
    on_open: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl DrillButton {
    /// Builds the button at canvas-local screen rect `rect`.
    pub fn new(rect: Bounds, on_open: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        DrillButton {
            rect,
            on_open: Rc::new(on_open),
        }
    }
}

impl RenderOnce for DrillButton {
    /// Renders the chevron square; presses never reach the canvas.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let on_open = self.on_open;
        let side = self.rect.w as f32;

        div()
            .absolute()
            .left(px(self.rect.x as f32))
            .top(px(self.rect.y as f32))
            .size(px(side))
            .rounded_sm()
            .bg(theme.selection.hsla())
            .flex()
            .items_center()
            .justify_center()
            .cursor(CursorStyle::PointingHand)
            .child(
                svg()
                    .path("icons/chevron-down.svg")
                    .size(px(side - 4.))
                    .text_color(theme.canvas.background.hsla()),
            )
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                on_open(window, cx);
            })
    }
}
