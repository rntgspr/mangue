//! `TextElement`: the custom GPUI element painting a `TextInput`'s lines,
//! selection, caret and IME underline, and registering the input handler
//! (`window.handle_input`) while the input is focused.

use gpui::{
    App, Bounds, ElementInputHandler, Entity, GlobalElementId, InspectorElementId, LayoutId,
    PaintQuad, Pixels, ShapedLine, Style, TextRun, UnderlineStyle, Window, fill, point, prelude::*,
    px, relative, size,
};

use super::text_input::TextInput;
use crate::theme::Theme;

/// Caret width in px.
const CARET_WIDTH: f32 = 1.5;

/// Opacity of the selection highlight over the theme selection color.
const SELECTION_ALPHA: f32 = 0.3;

/// Opacity of the placeholder over the text color.
const PLACEHOLDER_ALPHA: f32 = 0.5;

/// Paints the text of `input` using the inherited text style.
pub struct TextElement {
    input: Entity<TextInput>,
}

impl TextElement {
    /// Element painting `input`.
    pub fn new(input: Entity<TextInput>) -> Self {
        TextElement { input }
    }
}

/// Shaped lines and quads computed in prepaint.
pub struct Prepainted {
    lines: Vec<(usize, ShapedLine)>,
    quads: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    /// The element itself.
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepainted;

    /// No element id (state lives in the input entity).
    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    /// No source location for the inspector.
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// Full width, one line height per text line.
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let lines = self.input.read(cx).edit.lines().len();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * lines as f32).into();

        (window.request_layout(style, [], cx), ())
    }

    /// Shapes each line (placeholder when empty) and lays out the
    /// selection, caret and composition underline.
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepainted {
        let input = self.input.read(cx);
        let edit = &input.edit;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let accent = Theme::current(cx).selection.hsla();

        let mut lines = Vec::new();
        let mut quads = Vec::new();
        let mut caret = None;
        let cursor = edit.cursor();

        for (row, range) in edit.lines().into_iter().enumerate() {
            let top = bounds.top() + line_height * row as f32;
            let (text, color) = if edit.text.is_empty() {
                (
                    input.placeholder.clone(),
                    style.color.opacity(PLACEHOLDER_ALPHA),
                )
            } else {
                (edit.text[range.clone()].to_string().into(), style.color)
            };

            let run = TextRun {
                len: text.len(),
                font: style.font(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let runs = match &edit.marked {
                Some(m) if m.start < range.end && m.end > range.start && !edit.text.is_empty() => {
                    let a = m.start.max(range.start) - range.start;
                    let b = m.end.min(range.end) - range.start;
                    [
                        TextRun {
                            len: a,
                            ..run.clone()
                        },
                        TextRun {
                            len: b - a,
                            underline: Some(UnderlineStyle {
                                color: Some(color),
                                thickness: px(1.),
                                wavy: false,
                            }),
                            ..run.clone()
                        },
                        TextRun {
                            len: text.len() - b,
                            ..run
                        },
                    ]
                    .into_iter()
                    .filter(|r| r.len > 0)
                    .collect()
                }
                _ => vec![run],
            };
            let line = window
                .text_system()
                .shape_line(text, font_size, &runs, None);

            let sel = &edit.selected;
            if !sel.is_empty() && sel.start <= range.end && sel.end >= range.start {
                let a = sel.start.max(range.start) - range.start;
                let b = sel.end.min(range.end) - range.start;
                let x0 = line.x_for_index(a);
                let x1 = line.x_for_index(b).max(x0 + px(CARET_WIDTH * 2.));
                quads.push(fill(
                    Bounds::from_corners(
                        point(bounds.left() + x0, top),
                        point(bounds.left() + x1, top + line_height),
                    ),
                    accent.opacity(SELECTION_ALPHA),
                ));
            }

            if sel.is_empty() && cursor >= range.start && cursor <= range.end && caret.is_none() {
                let x = if edit.text.is_empty() {
                    Pixels::ZERO
                } else {
                    line.x_for_index(cursor - range.start)
                };
                caret = Some(fill(
                    Bounds::new(
                        point(bounds.left() + x, top),
                        size(px(CARET_WIDTH), line_height),
                    ),
                    accent,
                ));
            }

            lines.push((range.start, line));
        }

        Prepainted {
            lines,
            quads,
            caret,
        }
    }

    /// Registers the input handler, paints selection, text and caret, and
    /// stores the layout in the input for hit-testing.
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepainted: &mut Prepainted,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        for quad in prepainted.quads.drain(..) {
            window.paint_quad(quad);
        }

        let line_height = window.line_height();
        for (row, (_, line)) in prepainted.lines.iter().enumerate() {
            let origin = point(bounds.left(), bounds.top() + line_height * row as f32);
            line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                .ok();
        }

        if focus.is_focused(window)
            && let Some(caret) = prepainted.caret.take()
        {
            window.paint_quad(caret);
        }

        let lines = std::mem::take(&mut prepainted.lines);
        let empty = self.input.read(cx).edit.text.is_empty();
        self.input.update(cx, |input, _| {
            input.record_layout(if empty { Vec::new() } else { lines }, bounds, line_height);
        });
    }
}
