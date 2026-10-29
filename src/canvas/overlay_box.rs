//! `OverlayBox`: the small themed box a canvas overlay is drawn as (bpmn-js
//! `.djs-overlay` badge, `.djs-tooltip` error), also used as the
//! not-allowed marker that follows the cursor of a refused drag.

use gpui::{App, CursorStyle, MouseButton, Window, div, prelude::*, px, svg};

use super::overlay::{Click, Tone};
use crate::assets::icon_path;
use crate::theme::Theme;

/// Side (px) of a badge or marker.
pub const BOX_SIDE: f32 = 16.0;

/// Box with `text` at canvas-local screen point `(x, y)`.
#[derive(IntoElement)]
pub struct OverlayBox {
    x: f32,
    y: f32,
    text: String,
    tone: Tone,
    opacity: f32,
    centered: bool,
    icon: Option<String>,
    color: Option<String>,
    on_click: Option<Click>,
}

impl OverlayBox {
    /// Box whose top-left corner sits at `(x, y)`, e.g. a tooltip.
    pub fn tooltip(x: f32, y: f32, text: String, tone: Tone, opacity: f32) -> Self {
        OverlayBox {
            x,
            y,
            text,
            tone,
            opacity,
            centered: false,
            icon: None,
            color: None,
            on_click: None,
        }
    }

    /// Box centered on `(x, y)`, e.g. a badge on an element corner or a
    /// marker on the cursor.
    pub fn badge(x: f32, y: f32, text: String, tone: Tone, opacity: f32) -> Self {
        OverlayBox {
            centered: true,
            ..OverlayBox::tooltip(x, y, text, tone, opacity)
        }
    }

    /// Draws the `assets/icons` icon `name` before the text, when it exists.
    pub fn icon(mut self, name: Option<String>) -> Self {
        self.icon = name;
        self
    }

    /// Tints icon and text with the theme color `key` instead of the tone's.
    pub fn color(mut self, key: Option<String>) -> Self {
        self.color = key;
        self
    }

    /// Makes the box clickable: a left press runs `click` and never reaches
    /// the canvas.
    pub fn on_click(mut self, click: Option<Click>) -> Self {
        self.on_click = click;
        self
    }
}

impl RenderOnce for OverlayBox {
    /// Renders the box in the `overlay` theme colors; it takes input only
    /// when it has a click handler.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let colors = match self.tone {
            Tone::Badge => &theme.overlay.badge,
            Tone::Error => &theme.overlay.error,
        };
        let tint = self
            .color
            .as_deref()
            .and_then(|key| theme.color_for(key))
            .unwrap_or(colors.text)
            .hsla();
        let icon = self.icon.as_deref().and_then(icon_path);
        let (left, top) = if self.centered {
            (self.x - BOX_SIDE / 2.0, self.y - BOX_SIDE / 2.0)
        } else {
            (self.x, self.y)
        };

        let mut boxed = div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .min_w(px(BOX_SIDE))
            .h(px(BOX_SIDE))
            .px_1()
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded_sm()
            .border_1()
            .border_color(colors.border.hsla())
            .bg(colors.background.hsla())
            .text_color(tint)
            .text_xs()
            .whitespace_nowrap()
            .opacity(self.opacity)
            .children(icon.map(|path| svg().path(path).size(px(BOX_SIDE - 6.0)).text_color(tint)))
            .when(!self.text.is_empty(), |b| b.child(self.text));

        if let Some(click) = self.on_click {
            boxed = boxed.cursor(CursorStyle::PointingHand).on_mouse_down(
                MouseButton::Left,
                move |_, window, cx| {
                    cx.stop_propagation();
                    click.call(window, cx);
                },
            );
        }

        boxed
    }
}
