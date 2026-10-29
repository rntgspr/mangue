//! `Palette`: the left vertical toolbar (bpmn-js `PaletteProvider`) with
//! the hand, lasso, space and global connect tools and the create entries,
//! plus its GPUI-independent layout and hit-testing.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, CursorStyle, MouseButton, Pixels, Point, Window, canvas, div, point, prelude::*, px, size,
};

use super::icon::icon_at;
use super::paint::{Glyph, paint_glyph};
use crate::model::{Bounds, ElementKind, EventDefinition, Waypoint};
use crate::theme::Theme;

/// Palette position inside the canvas (canvas-local px).
pub const ORIGIN: Waypoint = Waypoint { x: 10.0, y: 10.0 };

/// Side of one entry cell (px).
const CELL: f64 = 40.0;

/// Entries per row.
const COLUMNS: usize = 2;

/// Inner padding of the palette frame (px).
const PADDING: f64 = 4.0;

/// Height of the separator between tools and create entries (px).
const SEPARATOR: f64 = 9.0;

/// Number of tool entries (before the separator).
const TOOLS: usize = 4;

/// Editing tool armed from the palette (or the context pad's connect).
#[derive(Debug, Clone, PartialEq)]
pub enum Tool {
    /// Drag pans the canvas.
    Hand,
    /// Drag lasso-selects, even over shapes.
    Lasso,
    /// Drag inserts (or removes) space.
    Space,
    /// Click a source, then a target, to connect them.
    Connect,
    /// Click (or drop) on the canvas creates this kind (`true` = expanded).
    Create(ElementKind, bool),
}

impl Tool {
    /// Icon shown for this tool.
    pub fn glyph(&self) -> Glyph {
        match self {
            Tool::Hand => Glyph::Icon("icons/hand.svg"),
            Tool::Lasso => Glyph::Icon("icons/lasso.svg"),
            Tool::Space => Glyph::Icon("icons/move-horizontal.svg"),
            Tool::Connect => Glyph::Icon("icons/spline.svg"),
            Tool::Create(kind, expanded) => Glyph::Element(kind.clone(), *expanded),
        }
    }
}

/// Palette entries in display order: four tools, then the create entries.
pub fn entries() -> Vec<Tool> {
    use ElementKind::*;

    vec![
        Tool::Hand,
        Tool::Lasso,
        Tool::Space,
        Tool::Connect,
        Tool::Create(StartEvent(EventDefinition::None), false),
        Tool::Create(IntermediateThrowEvent(EventDefinition::None), false),
        Tool::Create(EndEvent(EventDefinition::None), false),
        Tool::Create(ExclusiveGateway, false),
        Tool::Create(Task, false),
        Tool::Create(
            SubProcess {
                triggered_by_event: false,
            },
            true,
        ),
        Tool::Create(DataObjectReference, false),
        Tool::Create(DataStoreReference, false),
        Tool::Create(Participant { process_ref: None }, false),
        Tool::Create(Group, false),
    ]
}

/// Each entry with its cell, relative to the palette content origin.
pub fn layout() -> Vec<(Tool, Bounds)> {
    entries()
        .into_iter()
        .enumerate()
        .map(|(i, tool)| {
            let (group, index) = if i < TOOLS { (0, i) } else { (1, i - TOOLS) };
            let rows_before = if group == 0 {
                0.0
            } else {
                TOOLS.div_ceil(COLUMNS) as f64 * CELL + SEPARATOR
            };
            let bounds = Bounds {
                x: PADDING + (index % COLUMNS) as f64 * CELL,
                y: PADDING + rows_before + (index / COLUMNS) as f64 * CELL,
                w: CELL,
                h: CELL,
            };
            (tool, bounds)
        })
        .collect()
}

/// `(width, height)` of the palette content.
pub fn content_size() -> (f64, f64) {
    let bottom = layout().iter().map(|(_, b)| b.y + b.h).fold(0.0, f64::max);

    (2.0 * PADDING + COLUMNS as f64 * CELL, bottom + PADDING)
}

/// Y of the separator line, relative to the content origin.
fn separator_y() -> f64 {
    PADDING + TOOLS.div_ceil(COLUMNS) as f64 * CELL + SEPARATOR / 2.0
}

/// Entry under `p` (relative to the content origin), if any.
pub fn entry_at(p: Waypoint) -> Option<Tool> {
    layout()
        .into_iter()
        .find(|(_, b)| p.x >= b.x && p.x < b.x + b.w && p.y >= b.y && p.y < b.y + b.h)
        .map(|(tool, _)| tool)
}

/// Whether canvas-local `p` lies over the palette frame.
pub fn covers(p: Waypoint) -> bool {
    let (w, h) = content_size();

    p.x >= ORIGIN.x && p.x <= ORIGIN.x + w + 2.0 && p.y >= ORIGIN.y && p.y <= ORIGIN.y + h + 2.0
}

/// Palette element: paints the entries and reports picks to `on_pick`.
#[derive(IntoElement)]
pub struct Palette {
    active: Option<Tool>,
    on_pick: Rc<dyn Fn(&Tool, &mut Window, &mut App)>,
}

impl Palette {
    /// Builds a palette highlighting `active`; `on_pick` receives clicked entries.
    pub fn new(
        active: Option<Tool>,
        on_pick: impl Fn(&Tool, &mut Window, &mut App) + 'static,
    ) -> Self {
        Palette {
            active,
            on_pick: Rc::new(on_pick),
        }
    }
}

impl RenderOnce for Palette {
    /// Renders the framed palette; presses never reach the canvas below.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx).palette;
        let separator = theme.border.hsla();
        let (w, h) = content_size();
        let origin: Rc<Cell<Point<Pixels>>> = Rc::new(Cell::new(point(px(0.), px(0.))));
        let paint_origin = origin.clone();
        let active = self.active;
        let on_pick = self.on_pick;
        let icons = layout()
            .into_iter()
            .filter_map(|(tool, b)| match tool.glyph() {
                Glyph::Icon(path) => Some(icon_at(path, b, 20.0, theme.icon.hsla())),
                _ => None,
            });

        div()
            .absolute()
            .left(px(ORIGIN.x as f32))
            .top(px(ORIGIN.y as f32))
            .bg(theme.background.hsla())
            .border_1()
            .border_color(theme.border.hsla())
            .rounded_sm()
            .cursor(CursorStyle::PointingHand)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        paint_origin.set(bounds.origin);

                        for (tool, b) in layout() {
                            let rect = gpui::Bounds {
                                origin: bounds.origin + point(px(b.x as f32), px(b.y as f32)),
                                size: size(px(b.w as f32), px(b.h as f32)),
                            };
                            let is_active = active.as_ref() == Some(&tool);
                            paint_glyph(&tool.glyph(), rect, is_active, window, cx);
                        }

                        let y = bounds.origin.y + px(separator_y() as f32);
                        let line = gpui::Bounds {
                            origin: point(bounds.origin.x + px(PADDING as f32), y),
                            size: size(px((w - 2.0 * PADDING) as f32), px(1.)),
                        };
                        window.paint_quad(gpui::fill(line, separator));
                    },
                )
                .w(px(w as f32))
                .h(px(h as f32)),
            )
            .child(div().absolute().top_0().left_0().children(icons))
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                cx.stop_propagation();

                let o = origin.get();
                let p = Waypoint {
                    x: f64::from(event.position.x - o.x),
                    y: f64::from(event.position.y - o.y),
                };
                if let Some(tool) = entry_at(p) {
                    on_pick(&tool, window, cx);
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cell centers hit their own entry; padding and the separator hit nothing.
    #[test]
    fn hit_test_matches_layout() {
        for (tool, b) in layout() {
            let center = Waypoint {
                x: b.x + b.w / 2.0,
                y: b.y + b.h / 2.0,
            };
            assert_eq!(entry_at(center), Some(tool));
        }

        assert_eq!(entry_at(Waypoint { x: 1.0, y: 1.0 }), None);
        assert_eq!(
            entry_at(Waypoint {
                x: 20.0,
                y: separator_y()
            }),
            None
        );
    }

    /// Two columns: tools on two rows, create entries below the separator.
    #[test]
    fn layout_rows() {
        let cells = layout();

        assert_eq!(cells.len(), 14);
        assert_eq!(
            cells[0].1,
            Bounds {
                x: 4.0,
                y: 4.0,
                w: 40.0,
                h: 40.0
            }
        );
        assert_eq!(
            cells[3].1,
            Bounds {
                x: 44.0,
                y: 44.0,
                w: 40.0,
                h: 40.0
            }
        );
        assert_eq!(cells[4].1.y, 4.0 + 80.0 + SEPARATOR);
        assert_eq!(content_size(), (88.0, 4.0 + 80.0 + SEPARATOR + 200.0 + 4.0));
    }

    /// `covers` reports canvas-local points over the palette frame.
    #[test]
    fn covers_frame() {
        assert!(covers(Waypoint { x: 20.0, y: 20.0 }));
        assert!(!covers(Waypoint { x: 200.0, y: 20.0 }));
    }
}
