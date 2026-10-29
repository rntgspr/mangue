//! `ContextPad`: the action pad next to a single selected shape (bpmn-js
//! `ContextPadProvider`): append, connect, change type and delete, plus its
//! GPUI-independent entry list, layout and hit-testing.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, CursorStyle, MouseButton, Pixels, Point, Window, canvas, div, point, prelude::*, px, size,
};

use super::icon::icon_at;
use super::paint::{Glyph, paint_glyph};
use crate::model::{Bounds, ElementKind, EventDefinition, Waypoint};
use crate::modeling::lanes::LaneSide;
use crate::modeling::rules::{can_start_connection, is_flow_node};
use crate::theme::Theme;

/// Side of one entry cell (px).
pub const CELL: f64 = 26.0;

/// Entries per row.
const COLUMNS: usize = 3;

/// Gap between the shape's right edge and the pad (px).
pub const GAP: f64 = 10.0;

/// One context-pad action.
#[derive(Debug, Clone, PartialEq)]
pub enum PadEntry {
    /// Append a new shape of this kind and connect it.
    Append(ElementKind),
    /// Open the append-anything menu.
    AppendAnything,
    /// Arm connect with the selected shape as source.
    Connect,
    /// Open the replace menu.
    Replace,
    /// Delete the selected shape.
    Delete,
    /// Add a lane above or below the selected participant or lane.
    AddLane(LaneSide),
    /// Split the selected participant or lane into this many lanes.
    SplitLane(usize),
}

impl PadEntry {
    /// Icon shown for this entry.
    pub fn glyph(&self) -> Glyph {
        match self {
            PadEntry::Append(ElementKind::TextAnnotation { .. }) => {
                Glyph::Icon("icons/message-square-text.svg")
            }
            PadEntry::Append(kind) => Glyph::Element(kind.clone(), false),
            PadEntry::AppendAnything => Glyph::Icon("icons/plus.svg"),
            PadEntry::Connect => Glyph::Icon("icons/arrow-up-right.svg"),
            PadEntry::Replace => Glyph::Icon("icons/wrench.svg"),
            PadEntry::Delete => Glyph::Icon("icons/trash-2.svg"),
            PadEntry::AddLane(LaneSide::Above) => Glyph::Icon("icons/arrow-up-to-line.svg"),
            PadEntry::AddLane(LaneSide::Below) => Glyph::Icon("icons/arrow-down-to-line.svg"),
            PadEntry::SplitLane(2) => Glyph::Icon("icons/rows-2.svg"),
            PadEntry::SplitLane(_) => Glyph::Icon("icons/rows-3.svg"),
        }
    }
}

/// Entry groups (each starts a new row) for a shape of `kind`.
/// `replaceable` tells whether the replace menu has options for it.
/// Participants and lanes get the bpmn-js lane tools first.
pub fn entries(kind: &ElementKind, replaceable: bool) -> Vec<Vec<PadEntry>> {
    use ElementKind::*;

    let annotation = PadEntry::Append(TextAnnotation { text: None });
    let mut groups = Vec::new();

    if matches!(kind, Participant { .. } | Lane) {
        groups.push(vec![
            PadEntry::AddLane(LaneSide::Above),
            PadEntry::AddLane(LaneSide::Below),
            PadEntry::SplitLane(2),
            PadEntry::SplitLane(3),
        ]);
    }

    if is_flow_node(kind) && !matches!(kind, EndEvent(_)) {
        groups.push(vec![
            PadEntry::Append(EndEvent(EventDefinition::None)),
            PadEntry::Append(ExclusiveGateway),
            PadEntry::Append(Task),
            PadEntry::Append(IntermediateThrowEvent(EventDefinition::None)),
            annotation,
            PadEntry::AppendAnything,
        ]);
    } else if is_flow_node(kind)
        || matches!(
            kind,
            Participant { .. } | DataObjectReference | DataStoreReference
        )
    {
        groups.push(vec![annotation]);
    }

    let mut edit = Vec::new();
    if replaceable {
        edit.push(PadEntry::Replace);
    }
    if can_start_connection(kind) {
        edit.push(PadEntry::Connect);
    }
    edit.push(PadEntry::Delete);
    groups.push(edit);

    groups
}

/// Each entry with its cell, relative to the pad origin.
pub fn layout(groups: &[Vec<PadEntry>]) -> Vec<(PadEntry, Bounds)> {
    let mut out = Vec::new();
    let mut row = 0usize;

    for group in groups {
        for (i, entry) in group.iter().enumerate() {
            let bounds = Bounds {
                x: (i % COLUMNS) as f64 * CELL,
                y: (row + i / COLUMNS) as f64 * CELL,
                w: CELL,
                h: CELL,
            };
            out.push((entry.clone(), bounds));
        }
        row += group.len().div_ceil(COLUMNS);
    }

    out
}

/// `(width, height)` of the pad for `groups`.
pub fn pad_size(groups: &[Vec<PadEntry>]) -> (f64, f64) {
    let bottom = layout(groups)
        .iter()
        .map(|(_, b)| b.y + b.h)
        .fold(0.0, f64::max);

    (COLUMNS as f64 * CELL, bottom)
}

/// Entry under `p` (relative to the pad origin), if any.
pub fn entry_at(groups: &[Vec<PadEntry>], p: Waypoint) -> Option<PadEntry> {
    layout(groups)
        .into_iter()
        .find(|(_, b)| p.x >= b.x && p.x < b.x + b.w && p.y >= b.y && p.y < b.y + b.h)
        .map(|(entry, _)| entry)
}

/// Canvas-local origin of the pad for a shape with screen bounds `shape`.
pub fn origin_for(shape: &Bounds) -> Waypoint {
    Waypoint {
        x: shape.x + shape.w + GAP,
        y: shape.y,
    }
}

/// Context pad element at a canvas-local `origin`; picks go to `on_pick`.
#[derive(IntoElement)]
pub struct ContextPad {
    origin: Waypoint,
    groups: Vec<Vec<PadEntry>>,
    on_pick: Rc<dyn Fn(&PadEntry, &mut Window, &mut App)>,
}

impl ContextPad {
    /// Builds a pad showing `groups` at canvas-local `origin`.
    pub fn new(
        origin: Waypoint,
        groups: Vec<Vec<PadEntry>>,
        on_pick: impl Fn(&PadEntry, &mut Window, &mut App) + 'static,
    ) -> Self {
        ContextPad {
            origin,
            groups,
            on_pick: Rc::new(on_pick),
        }
    }
}

impl RenderOnce for ContextPad {
    /// Renders the pad entries; presses never reach the canvas below.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let icon = theme.palette.icon.hsla();
        let (w, h) = pad_size(&self.groups);
        let origin: Rc<Cell<Point<Pixels>>> = Rc::new(Cell::new(point(px(0.), px(0.))));
        let paint_origin = origin.clone();
        let groups = Rc::new(self.groups);
        let paint_groups = groups.clone();
        let on_pick = self.on_pick;
        let icons: Vec<_> = layout(&groups)
            .into_iter()
            .filter_map(|(entry, b)| match entry.glyph() {
                Glyph::Icon(path) => Some(icon_at(path, b, 16.0, icon)),
                _ => None,
            })
            .collect();

        div()
            .absolute()
            .left(px(self.origin.x as f32))
            .top(px(self.origin.y as f32))
            .bg(theme.context_pad.background.hsla())
            .rounded_sm()
            .cursor(CursorStyle::PointingHand)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        paint_origin.set(bounds.origin);

                        for (entry, b) in layout(&paint_groups) {
                            let rect = gpui::Bounds {
                                origin: bounds.origin + point(px(b.x as f32), px(b.y as f32)),
                                size: size(px(b.w as f32), px(b.h as f32)),
                            };
                            paint_glyph(&entry.glyph(), rect, false, window, cx);
                        }
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
                if let Some(entry) = entry_at(&groups, p) {
                    on_pick(&entry, window, cx);
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flow nodes get the five appends plus append-anything, then
    /// replace/connect/delete.
    #[test]
    fn task_entries() {
        let groups = entries(&ElementKind::Task, true);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 6);
        assert_eq!(groups[0][5], PadEntry::AppendAnything);
        assert_eq!(
            groups[1],
            [PadEntry::Replace, PadEntry::Connect, PadEntry::Delete]
        );
    }

    /// End events cannot append flow nodes; groups only delete.
    #[test]
    fn restricted_entries() {
        let end = entries(&ElementKind::EndEvent(EventDefinition::None), true);
        assert_eq!(
            end[0],
            [PadEntry::Append(ElementKind::TextAnnotation { text: None })]
        );

        let group = entries(&ElementKind::Group, false);
        assert_eq!(group, [vec![PadEntry::Delete]]);
    }

    /// Groups start new rows; hit-testing follows the layout.
    #[test]
    fn layout_and_hit() {
        let groups = entries(&ElementKind::Task, true);
        let cells = layout(&groups);

        assert_eq!(
            cells[3].1,
            Bounds {
                x: 0.0,
                y: CELL,
                w: CELL,
                h: CELL
            }
        );
        assert_eq!(
            cells[6].1,
            Bounds {
                x: 0.0,
                y: 2.0 * CELL,
                w: CELL,
                h: CELL
            }
        );
        assert_eq!(pad_size(&groups), (3.0 * CELL, 3.0 * CELL));

        let p = Waypoint {
            x: 2.5 * CELL,
            y: 2.5 * CELL,
        };
        assert_eq!(entry_at(&groups, p), Some(PadEntry::Delete));
        assert_eq!(
            entry_at(
                &groups,
                Waypoint {
                    x: 2.5 * CELL,
                    y: 1.5 * CELL
                }
            ),
            Some(PadEntry::AppendAnything)
        );
    }

    /// The pad sits `GAP` right of the shape, aligned with its top.
    #[test]
    fn pad_origin() {
        let shape = Bounds {
            x: 100.0,
            y: 50.0,
            w: 100.0,
            h: 80.0,
        };

        assert_eq!(origin_for(&shape), Waypoint { x: 210.0, y: 50.0 });
    }
}
