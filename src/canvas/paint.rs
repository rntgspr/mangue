//! Immediate-mode painting of a `Definitions` diagram with bpmn-js styling.

use gpui::{
    App, Hsla, PathBuilder, Pixels, Point, SharedString, TextAlign, TextRun, TransformationMatrix,
    Window, font, point, px,
};

use super::geometry::{Viewport, arrowhead};
use super::handles::{CORNERS, HANDLE_SIZE, corner_point};
use crate::assets::vertical_label_path;
use crate::model::{
    Bounds, Definitions, Edge, ElementId, ElementKind, EventDefinition, Shape, Waypoint,
};
use crate::modeling::label::has_external_label;
use crate::modeling::ordering::{DiItem, paint_order};
use crate::modeling::rules::can_resize;
use crate::theme::Theme;

/// Side of the square box glyphs are designed in (diagram units).
const GLYPH_BOX: f64 = 40.0;

/// Thickness of a participant or lane name band (diagram units).
const NAME_BAND: f64 = 30.0;

/// Name band of a participant or lane: the top strip for a vertical pool
/// (`isHorizontal=false`), the left strip for a horizontal one (bpmn-js
/// `BpmnRenderer`).
fn name_band(b: Bounds, vertical: bool) -> Bounds {
    if vertical {
        Bounds { h: NAME_BAND, ..b }
    } else {
        Bounds { w: NAME_BAND, ..b }
    }
}

/// Paints the whole diagram in bpmn-js z-order (`ordering::paint_order`),
/// then every external label on top (bpmn-js label level 10).
pub fn paint_definitions(
    defs: &Definitions,
    viewport: Viewport,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut painter = Painter::new(viewport, origin, window, cx);
    let plane = defs.plane();
    let order = paint_order(defs, plane);

    for item in &order {
        match *item {
            DiItem::Shape(i) => {
                let shape = &plane.shapes[i];
                if let Some(element) = defs.get(&shape.element_id) {
                    let name = element
                        .name
                        .as_deref()
                        .filter(|_| !has_external_label(&element.kind));
                    painter.shape(shape, &element.kind, name);
                }
            }

            DiItem::Edge(i) => {
                let edge = &plane.edges[i];
                if let Some(element) = defs.get(&edge.element_id) {
                    painter.edge(edge, &element.kind, None);
                }
            }
        }
    }

    for item in &order {
        match *item {
            DiItem::Shape(i) => {
                let shape = &plane.shapes[i];
                if let Some(element) = defs
                    .get(&shape.element_id)
                    .filter(|e| has_external_label(&e.kind))
                {
                    painter.shape_label(shape, &element.kind, element.name.as_deref());
                }
            }

            DiItem::Edge(i) => {
                let edge = &plane.edges[i];
                if let Some(name) = defs.get(&edge.element_id).and_then(|e| e.name.as_deref()) {
                    painter.edge_label(edge, name);
                }
            }
        }
    }
}

/// Paints the hover and selection outlines (bpmn-js `djs-outline`, 6px
/// offset), the lasso rectangle, the connect preview line and the space
/// tool guide lines (dashed) and the corner resize handles of a single
/// selected resizable shape, in screen-constant widths.
pub fn paint_overlay(
    defs: &Definitions,
    selected: &[ElementId],
    hover: Option<&ElementId>,
    lasso: Option<Bounds>,
    connect: Option<(Waypoint, Waypoint)>,
    guides: &[(Waypoint, Waypoint)],
    viewport: Viewport,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut painter = Painter::new(viewport, origin, window, cx);
    let unit = 1.0 / viewport.scale;
    let pad = 6.0 * unit;

    if let Some(shape) = hover
        .filter(|h| !selected.contains(h))
        .and_then(|h| defs.shape(h))
    {
        let b = shape.bounds;
        let outline = Bounds {
            x: b.x - pad,
            y: b.y - pad,
            w: b.w + 2.0 * pad,
            h: b.h + 2.0 * pad,
        };
        painter.draw_colored(
            |p| p.rounded_rect(outline, 4.0 * unit),
            painter.theme.hover.hsla(),
            unit,
        );
    }

    if let Some((from, to)) = connect {
        painter.draw_colored(
            |p| p.poly(&[(from.x, from.y), (to.x, to.y)], false),
            painter.theme.selection.hsla(),
            2.0 * unit,
        );
    }

    for (from, to) in guides {
        let mut builder = PathBuilder::stroke(px(1.)).dash_array(&[px(5.), px(5.)]);
        builder.move_to(painter.screen(*from));
        builder.line_to(painter.screen(*to));
        if let Ok(path) = builder.build() {
            painter
                .window
                .paint_path(path, painter.theme.selection.hsla());
        }
    }

    for shape in defs.shapes().filter(|s| selected.contains(&s.element_id)) {
        let b = shape.bounds;
        let outline = Bounds {
            x: b.x - pad,
            y: b.y - pad,
            w: b.w + 2.0 * pad,
            h: b.h + 2.0 * pad,
        };
        painter.draw_colored(
            |p| p.rounded_rect(outline, 4.0 * unit),
            painter.theme.selection.hsla(),
            unit,
        );
    }

    if let [id] = selected {
        let resizable = defs.shape(id).filter(|s| {
            defs.get(id)
                .is_some_and(|e| can_resize(&e.kind, s.is_expanded == Some(true)))
        });

        if let Some(shape) = resizable {
            for c in CORNERS.map(|c| corner_point(&shape.bounds, c)) {
                let half = HANDLE_SIZE / 2.0 * unit;
                let square = Bounds {
                    x: c.x - half,
                    y: c.y - half,
                    w: 2.0 * half,
                    h: 2.0 * half,
                };
                let mut pen = painter.pen(PathBuilder::fill());
                pen.rounded_rect(square, 0.0);
                if let Ok(path) = pen.builder.build() {
                    painter
                        .window
                        .paint_path(path, painter.theme.canvas.background.hsla());
                }

                painter.draw_colored(
                    |p| p.rounded_rect(square, 0.0),
                    painter.theme.selection.hsla(),
                    unit,
                );
            }
        }
    }

    for edge in defs.edges().filter(|e| selected.contains(&e.element_id)) {
        let points: Vec<(f64, f64)> = edge.waypoints.iter().map(|w| (w.x, w.y)).collect();
        painter.draw_colored(
            |p| p.poly(&points, false),
            painter.theme.selection.hsla(),
            3.0 * unit,
        );

        for w in &edge.waypoints {
            painter.draw_colored(
                |p| p.circle(w.x, w.y, 3.0 * unit),
                painter.theme.selection.hsla(),
                unit,
            );
        }
    }

    if let Some(r) = lasso {
        let mut pen = painter.pen(PathBuilder::fill());
        pen.rounded_rect(r, 0.0);
        if let Ok(path) = pen.builder.build() {
            painter
                .window
                .paint_path(path, painter.theme.lasso.hsla().opacity(0.1));
        }

        painter.draw_colored(|p| p.rounded_rect(r, 0.0), painter.theme.lasso.hsla(), unit);
    }
}

/// Paints the palette create preview (bpmn-js drag ghost): `shape` of
/// `kind` without label; the caller fades it via element opacity.
pub fn paint_ghost(
    kind: &ElementKind,
    shape: &Shape,
    viewport: Viewport,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut painter = Painter::new(viewport, origin, window, cx);

    painter.shape(shape, kind, None);
}

/// Icon of a palette or context-pad entry: a Lucide icon for generic tools,
/// a BPMN miniature for element kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum Glyph {
    /// Lucide icon at this asset path, rendered by the owning element as `svg()`.
    Icon(&'static str),
    /// Miniature of an element kind (`true` = expanded sub-process).
    Element(ElementKind, bool),
}

/// Paints `glyph` fitted into the window-space square `rect`, highlighted
/// with a light background when `active`. Icons only get the highlight.
pub fn paint_glyph(
    glyph: &Glyph,
    rect: gpui::Bounds<Pixels>,
    active: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if active {
        let hover = Theme::current(cx).palette.hover.hsla();
        window.paint_quad(gpui::fill(rect, hover.opacity(0.35)));
    }

    let side = f64::from(rect.size.width.min(rect.size.height));
    let viewport = Viewport {
        offset: Waypoint { x: 0.0, y: 0.0 },
        scale: side / GLYPH_BOX,
    };
    let mut painter = Painter::new(viewport, rect.origin, window, cx);

    painter.glyph(glyph);
}

/// Path under construction in diagram coordinates, projected to screen on the fly.
struct Pen {
    builder: PathBuilder,
    viewport: Viewport,
    origin: Point<Pixels>,
}

impl Pen {
    /// Projects a diagram point to window pixels.
    fn project(&self, x: f64, y: f64) -> Point<Pixels> {
        let p = self.viewport.to_screen(Waypoint { x, y });
        point(
            self.origin.x + px(p.x as f32),
            self.origin.y + px(p.y as f32),
        )
    }

    /// Starts a new sub-path.
    fn move_to(&mut self, x: f64, y: f64) {
        let p = self.project(x, y);
        self.builder.move_to(p);
    }

    /// Adds a straight segment.
    fn line_to(&mut self, x: f64, y: f64) {
        let p = self.project(x, y);
        self.builder.line_to(p);
    }

    /// Adds a clockwise circular arc of radius `r` ending at `(x, y)`.
    fn arc_to(&mut self, r: f64, x: f64, y: f64) {
        let p = self.project(x, y);
        let r = px((r * self.viewport.scale) as f32);
        self.builder.arc_to(point(r, r), px(0.), false, true, p);
    }

    /// Closes the current sub-path.
    fn close(&mut self) {
        self.builder.close();
    }

    /// Adds a polyline through `points`, closed when `closed`.
    fn poly(&mut self, points: &[(f64, f64)], closed: bool) {
        for (i, (x, y)) in points.iter().enumerate() {
            if i == 0 {
                self.move_to(*x, *y);
            } else {
                self.line_to(*x, *y);
            }
        }

        if closed {
            self.close();
        }
    }

    /// Adds a full circle.
    fn circle(&mut self, cx: f64, cy: f64, r: f64) {
        self.move_to(cx + r, cy);
        self.arc_to(r, cx - r, cy);
        self.arc_to(r, cx + r, cy);
        self.close();
    }

    /// Adds a rectangle with rounded corners (`r = 0` for sharp corners).
    fn rounded_rect(&mut self, b: Bounds, r: f64) {
        let (x, y, w, h) = (b.x, b.y, b.w, b.h);

        if r <= 0.0 {
            self.poly(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], true);
            return;
        }

        self.move_to(x + r, y);
        self.line_to(x + w - r, y);
        self.arc_to(r, x + w, y + r);
        self.line_to(x + w, y + h - r);
        self.arc_to(r, x + w - r, y + h);
        self.line_to(x + r, y + h);
        self.arc_to(r, x, y + h - r);
        self.line_to(x, y + r);
        self.arc_to(r, x + r, y);
        self.close();
    }
}

/// Stroke parameters in diagram units.
#[derive(Clone, Copy)]
struct Stroke {
    width: f64,
    dash: Option<[f64; 2]>,
}

/// Solid stroke of the given width.
fn solid(width: f64) -> Stroke {
    Stroke { width, dash: None }
}

/// Dashed stroke of the given width, dash and gap.
fn dashed(width: f64, dash: f64, gap: f64) -> Stroke {
    Stroke {
        width,
        dash: Some([dash, gap]),
    }
}

/// Paint context bound to one frame, with the colors of the shape or edge
/// being painted.
struct Painter<'a> {
    viewport: Viewport,
    origin: Point<Pixels>,
    window: &'a mut Window,
    cx: &'a mut App,
    theme: Theme,
    /// Fill of the current shape.
    fill: Hsla,
    /// Stroke of the current shape or edge.
    stroke: Hsla,
}

impl<'a> Painter<'a> {
    /// Creates a painter with the app's current theme.
    fn new(
        viewport: Viewport,
        origin: Point<Pixels>,
        window: &'a mut Window,
        cx: &'a mut App,
    ) -> Self {
        let theme = Theme::current(cx);
        let (fill, stroke) = (theme.shape.fill.hsla(), theme.shape.stroke.hsla());

        Painter {
            viewport,
            origin,
            window,
            cx,
            theme,
            fill,
            stroke,
        }
    }

    /// Projects a diagram point to window pixels.
    fn screen(&self, p: Waypoint) -> Point<Pixels> {
        let s = self.viewport.to_screen(p);

        point(
            self.origin.x + px(s.x as f32),
            self.origin.y + px(s.y as f32),
        )
    }

    /// Creates an empty pen sharing this painter's transform.
    fn pen(&self, builder: PathBuilder) -> Pen {
        Pen {
            builder,
            viewport: self.viewport,
            origin: self.origin,
        }
    }

    /// Fills and/or strokes the outline produced by `outline`.
    fn draw(&mut self, outline: impl Fn(&mut Pen), fill: Option<Hsla>, stroke: Option<Stroke>) {
        let scale = self.viewport.scale;

        if let Some(color) = fill {
            let mut pen = self.pen(PathBuilder::fill());
            outline(&mut pen);

            if let Ok(path) = pen.builder.build() {
                self.window.paint_path(path, color);
            }
        }

        if let Some(stroke) = stroke {
            let mut builder = PathBuilder::stroke(px((stroke.width * scale) as f32));
            if let Some([on, off]) = stroke.dash {
                builder = builder.dash_array(&[px((on * scale) as f32), px((off * scale) as f32)]);
            }

            let mut pen = self.pen(builder);
            outline(&mut pen);

            if let Ok(path) = pen.builder.build() {
                self.window.paint_path(path, self.stroke);
            }
        }
    }

    /// Paints text wrapped to `b`, horizontally aligned by `align` and
    /// vertically centered when `middle`, else top-aligned with padding.
    fn text(&mut self, text: &str, b: Bounds, align: TextAlign, middle: bool) {
        let scale = self.viewport.scale;
        let size = self.theme.font.size;
        let font_size = px((size * scale) as f32);
        let line_height = px((size * 1.2 * scale) as f32);
        if text.is_empty() || f32::from(font_size) < 2.0 {
            return;
        }

        let screen = self.viewport.bounds_to_screen(b);
        let left = self.origin.x + px(screen.x as f32);
        let width = px(screen.w as f32);
        let run = TextRun {
            len: text.len(),
            font: font(self.theme.font.family.clone()),
            color: self.theme.shape.label.hsla(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let Ok(lines) = self.window.text_system().shape_text(
            SharedString::from(text.to_string()),
            font_size,
            &[run],
            Some(width),
            None,
        ) else {
            return;
        };

        let total: f32 = lines
            .iter()
            .map(|l| f32::from(l.size(line_height).height))
            .sum();
        let top = if middle {
            screen.y as f32 + (screen.h as f32 - total) / 2.0
        } else {
            screen.y as f32 + (5.0 * scale) as f32
        };

        let mut y = self.origin.y + px(top);
        for line in &lines {
            let line_bounds = gpui::Bounds {
                origin: point(left, y),
                size: gpui::size(width, line_height),
            };
            let _ = line.paint(
                point(left, y),
                line_height,
                align,
                Some(line_bounds),
                self.window,
                self.cx,
            );
            y += line.size(line_height).height;
        }
    }

    /// Paints text reading bottom-to-top, centered in a vertical band (bpmn-js
    /// rotated pool/lane labels): GPUI cannot rotate text, so the label is an
    /// SVG generated by `Assets` with a rotated `<text>` and painted as a
    /// tinted monochrome sprite.
    fn vertical_text(&mut self, text: &str, band: Bounds) {
        let scale = self.viewport.scale;
        let size = self.theme.font.size;
        if text.is_empty() || size * scale < 2.0 {
            return;
        }

        let screen = self.viewport.bounds_to_screen(band);
        let bounds = gpui::Bounds {
            origin: point(
                self.origin.x + px(screen.x as f32),
                self.origin.y + px(screen.y as f32),
            ),
            size: gpui::size(px(screen.w as f32), px(screen.h as f32)),
        };
        let path = vertical_label_path(text, band.w, band.h, size);

        let _ = self.window.paint_svg(
            bounds,
            SharedString::from(path),
            None,
            TransformationMatrix::unit(),
            self.theme.shape.label.hsla(),
            self.cx,
        );
    }

    /// Paints one DI shape according to its element kind.
    fn shape(&mut self, shape: &Shape, kind: &ElementKind, name: Option<&str>) {
        use ElementKind::*;

        let b = shape.bounds;
        let width = self.theme.stroke_width.shape;
        self.fill = self.theme.fill_for(kind);
        self.stroke = self.theme.shape.stroke.hsla();

        match kind {
            StartEvent(def) => self.event(b, *def, EventStyle::Start),
            EndEvent(def) => self.event(b, *def, EventStyle::End),
            IntermediateCatchEvent(def) => self.event(b, *def, EventStyle::Catch),
            IntermediateThrowEvent(def) => self.event(b, *def, EventStyle::Throw),
            BoundaryEvent {
                definition,
                cancel_activity,
                ..
            } => self.event(
                b,
                *definition,
                if *cancel_activity {
                    EventStyle::Catch
                } else {
                    EventStyle::NonInterrupting
                },
            ),

            Task | UserTask | ServiceTask | ScriptTask | SendTask | ReceiveTask | ManualTask
            | BusinessRuleTask | CallActivity => {
                let width = if matches!(kind, CallActivity) {
                    5.0
                } else {
                    width
                };
                self.draw(
                    |p| p.rounded_rect(b, 10.0),
                    Some(self.fill),
                    Some(solid(width)),
                );
                self.task_marker(b, kind);
            }

            SubProcess { triggered_by_event } => {
                let stroke = if *triggered_by_event {
                    dashed(1.0, 4.0, 3.0)
                } else {
                    solid(width)
                };
                self.draw(|p| p.rounded_rect(b, 10.0), Some(self.fill), Some(stroke));

                if shape.is_expanded != Some(true) {
                    let m = Bounds {
                        x: b.x + b.w / 2.0 - 7.0,
                        y: b.y + b.h - 20.0,
                        w: 14.0,
                        h: 14.0,
                    };
                    let (cx, cy) = (m.x + 7.0, m.y + 7.0);
                    self.draw(|p| p.rounded_rect(m, 0.0), None, Some(solid(1.0)));
                    self.draw(
                        |p| {
                            p.poly(&[(cx - 4.0, cy), (cx + 4.0, cy)], false);
                            p.poly(&[(cx, cy - 4.0), (cx, cy + 4.0)], false);
                        },
                        None,
                        Some(solid(1.5)),
                    );
                }
            }

            ExclusiveGateway | ParallelGateway | InclusiveGateway | EventBasedGateway
            | ComplexGateway => self.gateway(b, kind),

            DataObjectReference => {
                let fold = 10.0;
                let (x, y, w, h) = (b.x, b.y, b.w, b.h);
                self.draw(
                    |p| {
                        p.poly(
                            &[
                                (x, y),
                                (x + w - fold, y),
                                (x + w, y + fold),
                                (x + w, y + h),
                                (x, y + h),
                            ],
                            true,
                        )
                    },
                    Some(self.fill),
                    Some(solid(width)),
                );
                self.draw(
                    |p| {
                        p.poly(
                            &[
                                (x + w - fold, y),
                                (x + w - fold, y + fold),
                                (x + w, y + fold),
                            ],
                            false,
                        )
                    },
                    None,
                    Some(solid(2.0)),
                );
            }

            DataStoreReference => self.data_store(b),

            TextAnnotation { text } => {
                let (x, y, h) = (b.x, b.y, b.h);
                self.draw(
                    |p| {
                        p.poly(
                            &[(x + 10.0, y), (x, y), (x, y + h), (x + 10.0, y + h)],
                            false,
                        )
                    },
                    None,
                    Some(solid(1.0)),
                );
                if let Some(text) = text {
                    let inner = Bounds {
                        x: x + 5.0,
                        w: b.w - 10.0,
                        ..b
                    };
                    self.text(text, inner, TextAlign::Left, false);
                }
            }

            Group => self.draw(
                |p| p.rounded_rect(b, 10.0),
                None,
                Some(dashed(1.0, 8.0, 4.0)),
            ),

            Participant { process_ref } => {
                let vertical = shape.is_horizontal == Some(false);
                self.draw(
                    |p| p.rounded_rect(b, 0.0),
                    Some(self.fill),
                    Some(solid(width)),
                );

                if process_ref.is_some() {
                    let band = name_band(b, vertical);
                    let rule = if vertical {
                        [(b.x, band.y + band.h), (b.x + b.w, band.y + band.h)]
                    } else {
                        [(band.x + band.w, b.y), (band.x + band.w, b.y + b.h)]
                    };
                    self.draw(|p| p.poly(&rule, false), None, Some(solid(2.0)));
                }

                if let Some(name) = name {
                    if process_ref.is_some() && vertical {
                        self.text(name, name_band(b, true), TextAlign::Center, true);
                    } else if process_ref.is_some() {
                        self.vertical_text(name, name_band(b, false));
                    } else {
                        self.text(name, b, TextAlign::Center, true);
                    }
                }
            }

            Lane => {
                self.draw(|p| p.rounded_rect(b, 0.0), None, Some(solid(1.5)));
                if let Some(name) = name {
                    if shape.is_horizontal == Some(false) {
                        self.text(name, name_band(b, true), TextAlign::Center, true);
                    } else {
                        self.vertical_text(name, name_band(b, false));
                    }
                }
            }

            _ => {}
        }

        self.shape_label(shape, kind, name);
    }

    /// Paints a glyph in the `GLYPH_BOX` square at the diagram origin.
    fn glyph(&mut self, glyph: &Glyph) {
        use ElementKind::*;

        let centered = |w: f64, h: f64| Bounds {
            x: (GLYPH_BOX - w) / 2.0,
            y: (GLYPH_BOX - h) / 2.0,
            w,
            h,
        };

        match glyph {
            Glyph::Icon(_) => {}

            Glyph::Element(Participant { .. }, _) => {
                let b = centered(32.0, 22.0);
                self.draw(
                    |p| p.rounded_rect(b, 0.0),
                    Some(self.fill),
                    Some(solid(2.0)),
                );
                self.draw(
                    |p| p.poly(&[(b.x + 7.0, b.y), (b.x + 7.0, b.y + b.h)], false),
                    None,
                    Some(solid(2.0)),
                );
            }

            Glyph::Element(kind, expanded) => {
                let bounds = match kind {
                    StartEvent(_)
                    | EndEvent(_)
                    | IntermediateCatchEvent(_)
                    | IntermediateThrowEvent(_)
                    | BoundaryEvent { .. } => centered(26.0, 26.0),
                    ExclusiveGateway | ParallelGateway | InclusiveGateway | EventBasedGateway
                    | ComplexGateway => centered(28.0, 28.0),
                    DataObjectReference => centered(20.0, 26.0),
                    DataStoreReference => centered(26.0, 24.0),
                    TextAnnotation { .. } => centered(18.0, 26.0),
                    SubProcess { .. } | Group => centered(32.0, 26.0),
                    _ => centered(30.0, 24.0),
                };
                let shape = Shape {
                    element_id: ElementId::from("glyph"),
                    bounds,
                    is_expanded: Some(*expanded),
                    is_horizontal: None,
                    label_bounds: None,
                };
                self.shape(&shape, kind, None);
            }
        }
    }

    /// Paints the name of a shape: inside activities, below/at label bounds otherwise.
    fn shape_label(&mut self, shape: &Shape, kind: &ElementKind, name: Option<&str>) {
        use ElementKind::*;

        let Some(name) = name else {
            return;
        };
        let b = shape.bounds;

        match kind {
            Task | UserTask | ServiceTask | ScriptTask | SendTask | ReceiveTask | ManualTask
            | BusinessRuleTask | CallActivity => {
                let inner = Bounds {
                    x: b.x + 5.0,
                    w: b.w - 10.0,
                    ..b
                };
                self.text(name, inner, TextAlign::Center, true);
            }

            SubProcess { .. } if shape.is_expanded == Some(true) => {
                let inner = Bounds {
                    x: b.x + 10.0,
                    w: b.w - 20.0,
                    ..b
                };
                self.text(name, inner, TextAlign::Left, false);
            }

            SubProcess { .. } => {
                let inner = Bounds {
                    x: b.x + 5.0,
                    w: b.w - 10.0,
                    h: b.h - 20.0,
                    ..b
                };
                self.text(name, inner, TextAlign::Center, true);
            }

            StartEvent(_)
            | EndEvent(_)
            | IntermediateCatchEvent(_)
            | IntermediateThrowEvent(_)
            | BoundaryEvent { .. }
            | ExclusiveGateway
            | ParallelGateway
            | InclusiveGateway
            | EventBasedGateway
            | ComplexGateway
            | DataObjectReference
            | DataStoreReference => {
                let label = shape.label_bounds.unwrap_or(Bounds {
                    x: b.x + b.w / 2.0 - 45.0,
                    y: b.y + b.h + 5.0,
                    w: 90.0,
                    h: 14.0,
                });
                let wide = Bounds {
                    x: label.x - 10.0,
                    w: label.w + 20.0,
                    y: label.y - 5.0,
                    ..label
                };
                self.text(name, wide, TextAlign::Center, false);
            }

            _ => {}
        }
    }

    /// Paints an event circle with its definition glyph.
    fn event(&mut self, b: Bounds, def: EventDefinition, style: EventStyle) {
        let (cx, cy, r) = (b.x + b.w / 2.0, b.y + b.h / 2.0, b.w / 2.0);

        match style {
            EventStyle::Start => {
                let width = self.theme.stroke_width.shape;
                self.draw(|p| p.circle(cx, cy, r), Some(self.fill), Some(solid(width)))
            }
            EventStyle::End => {
                let width = self.theme.stroke_width.end_event;
                self.draw(|p| p.circle(cx, cy, r), Some(self.fill), Some(solid(width)))
            }
            EventStyle::Catch | EventStyle::Throw | EventStyle::NonInterrupting => {
                let stroke = if matches!(style, EventStyle::NonInterrupting) {
                    dashed(1.5, 5.5, 3.0)
                } else {
                    solid(1.5)
                };
                self.draw(|p| p.circle(cx, cy, r), Some(self.fill), Some(stroke));
                self.draw(|p| p.circle(cx, cy, r - 3.0), None, Some(stroke));
            }
        }

        let filled = matches!(style, EventStyle::End | EventStyle::Throw);
        self.event_glyph(cx, cy, r, def, filled);
    }

    /// Paints the glyph of an event definition centered at `(cx, cy)`.
    fn event_glyph(&mut self, cx: f64, cy: f64, r: f64, def: EventDefinition, filled: bool) {
        let fill = if filled { self.stroke } else { self.fill };

        match def {
            EventDefinition::None => {}

            EventDefinition::Message => {
                let (w, h) = (r * 1.0, r * 0.7);
                let env = Bounds {
                    x: cx - w / 2.0,
                    y: cy - h / 2.0,
                    w,
                    h,
                };
                self.draw(|p| p.rounded_rect(env, 0.0), Some(fill), Some(solid(1.0)));

                let flap = [(env.x, env.y), (cx, cy + h * 0.1), (env.x + w, env.y)];
                if filled {
                    self.draw_colored(|p| p.poly(&flap, false), self.fill, 1.0);
                } else {
                    self.draw(|p| p.poly(&flap, false), None, Some(solid(1.0)));
                }
            }

            EventDefinition::Timer => {
                let rr = r * 0.62;
                self.draw(|p| p.circle(cx, cy, rr), Some(self.fill), Some(solid(2.0)));
                self.draw(
                    |p| {
                        for i in 0..12 {
                            let a = i as f64 * std::f64::consts::PI / 6.0;
                            let (s, c) = a.sin_cos();
                            p.poly(
                                &[
                                    (cx + c * rr * 0.75, cy + s * rr * 0.75),
                                    (cx + c * rr * 0.92, cy + s * rr * 0.92),
                                ],
                                false,
                            );
                        }
                    },
                    None,
                    Some(solid(1.0)),
                );
                self.draw(
                    |p| {
                        p.poly(
                            &[(cx, cy - rr * 0.65), (cx, cy), (cx + rr * 0.45, cy)],
                            false,
                        )
                    },
                    None,
                    Some(solid(1.5)),
                );
            }

            EventDefinition::Terminate => {
                self.draw(|p| p.circle(cx, cy, r * 0.6), Some(self.stroke), None);
            }

            EventDefinition::Signal => {
                let s = r * 0.55;
                self.draw(
                    |p| {
                        p.poly(
                            &[
                                (cx, cy - s),
                                (cx + s * 0.95, cy + s * 0.65),
                                (cx - s * 0.95, cy + s * 0.65),
                            ],
                            true,
                        )
                    },
                    Some(fill),
                    Some(solid(1.0)),
                );
            }

            EventDefinition::Escalation => {
                let s = r * 0.55;
                self.draw(
                    |p| {
                        p.poly(
                            &[
                                (cx, cy - s),
                                (cx + s * 0.7, cy + s),
                                (cx, cy + s * 0.3),
                                (cx - s * 0.7, cy + s),
                            ],
                            true,
                        )
                    },
                    Some(fill),
                    Some(solid(1.0)),
                );
            }

            EventDefinition::Error
            | EventDefinition::Conditional
            | EventDefinition::Link
            | EventDefinition::Compensation => {
                let letter = match def {
                    EventDefinition::Error => "E",
                    EventDefinition::Conditional => "C",
                    EventDefinition::Link => "L",
                    _ => "<<",
                };
                let cell = Bounds {
                    x: cx - r,
                    y: cy - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                };
                self.text(letter, cell, TextAlign::Center, true);
            }
        }
    }

    /// Strokes an outline with an explicit color (used for glyph details on dark fills).
    fn draw_colored(&mut self, outline: impl Fn(&mut Pen), color: Hsla, width: f64) {
        let mut pen = self.pen(PathBuilder::stroke(
            px((width * self.viewport.scale) as f32),
        ));
        outline(&mut pen);

        if let Ok(path) = pen.builder.build() {
            self.window.paint_path(path, color);
        }
    }

    /// Paints a task-type marker in the top-left corner (placeholder letters
    /// except for send/receive envelopes).
    fn task_marker(&mut self, b: Bounds, kind: &ElementKind) {
        use ElementKind::*;

        let (cx, cy) = (b.x + 15.0, b.y + 13.0);

        let letter = match kind {
            SendTask => return self.event_glyph(cx, cy, 14.0, EventDefinition::Message, true),
            ReceiveTask => return self.event_glyph(cx, cy, 14.0, EventDefinition::Message, false),
            UserTask => "U",
            ServiceTask => "S",
            ScriptTask => "Sc",
            ManualTask => "M",
            BusinessRuleTask => "R",
            _ => return,
        };

        let cell = Bounds {
            x: b.x + 4.0,
            y: b.y + 4.0,
            w: 22.0,
            h: 16.0,
        };
        self.text(letter, cell, TextAlign::Center, true);
    }

    /// Paints a gateway diamond with its type marker.
    fn gateway(&mut self, b: Bounds, kind: &ElementKind) {
        use ElementKind::*;

        let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
        let (hw, hh) = (b.w / 2.0, b.h / 2.0);

        self.draw(
            |p| {
                p.poly(
                    &[(cx, b.y), (b.x + b.w, cy), (cx, b.y + b.h), (b.x, cy)],
                    true,
                )
            },
            Some(self.fill),
            Some(solid(self.theme.stroke_width.shape)),
        );

        let s = hw * 0.4;

        match kind {
            ExclusiveGateway => {
                let d = s * 0.75;
                self.draw(
                    |p| {
                        p.poly(&[(cx - d, cy - d), (cx + d, cy + d)], false);
                        p.poly(&[(cx + d, cy - d), (cx - d, cy + d)], false);
                    },
                    None,
                    Some(solid(4.0)),
                );
            }

            ParallelGateway => self.draw(
                |p| {
                    p.poly(&[(cx - s, cy), (cx + s, cy)], false);
                    p.poly(&[(cx, cy - s), (cx, cy + s)], false);
                },
                None,
                Some(solid(4.0)),
            ),

            InclusiveGateway => self.draw(|p| p.circle(cx, cy, hh * 0.47), None, Some(solid(2.5))),

            EventBasedGateway => {
                self.draw(|p| p.circle(cx, cy, hh * 0.5), None, Some(solid(1.0)));
                self.draw(|p| p.circle(cx, cy, hh * 0.42), None, Some(solid(1.0)));

                let pr = hh * 0.27;
                let pentagon: Vec<(f64, f64)> = (0..5)
                    .map(|i| {
                        let a = -std::f64::consts::FRAC_PI_2
                            + i as f64 * 2.0 * std::f64::consts::PI / 5.0;
                        (cx + pr * a.cos(), cy + pr * a.sin())
                    })
                    .collect();
                self.draw(|p| p.poly(&pentagon, true), None, Some(solid(1.5)));
            }

            ComplexGateway => self.draw(
                |p| {
                    let d = s * 0.75;
                    p.poly(&[(cx - s, cy), (cx + s, cy)], false);
                    p.poly(&[(cx, cy - s), (cx, cy + s)], false);
                    p.poly(&[(cx - d, cy - d), (cx + d, cy + d)], false);
                    p.poly(&[(cx + d, cy - d), (cx - d, cy + d)], false);
                },
                None,
                Some(solid(3.0)),
            ),

            _ => {}
        }
    }

    /// Paints a data store cylinder.
    fn data_store(&mut self, b: Bounds) {
        let (x, y, w, h) = (b.x, b.y, b.w, b.h);
        let ry = h * 0.12;
        let cx = x + w / 2.0;

        self.draw(
            |p| {
                p.move_to(x, y + ry);
                ellipse_half(p, cx, y + ry, w / 2.0, ry, true);
                p.line_to(x + w, y + h - ry);
                ellipse_half(p, cx, y + h - ry, w / 2.0, ry, false);
                p.close();
            },
            Some(self.fill),
            Some(solid(self.theme.stroke_width.shape)),
        );

        for k in 0..3 {
            let yy = y + ry + k as f64 * ry * 0.9;
            self.draw(
                |p| {
                    p.move_to(x + w, yy);
                    ellipse_half(p, cx, yy, w / 2.0, ry, false);
                },
                None,
                Some(solid(2.0)),
            );
        }
    }

    /// Paints one DI edge according to its connection kind, plus its label.
    fn edge(&mut self, edge: &Edge, kind: &ElementKind, name: Option<&str>) {
        use ElementKind::*;

        let pts: Vec<(f64, f64)> = edge.waypoints.iter().map(|w| (w.x, w.y)).collect();
        if pts.len() < 2 {
            return;
        }

        let n = pts.len();
        let (from, to) = (edge.waypoints[n - 2], edge.waypoints[n - 1]);
        self.fill = self.theme.canvas.background.hsla();
        self.stroke = self.theme.flow.stroke.hsla();

        match kind {
            SequenceFlow { .. } => {
                let width = self.theme.stroke_width.flow;
                self.draw(|p| p.poly(&pts, false), None, Some(solid(width)));
                let [a, b] = arrowhead(from, to, 11.0, 4.0);
                self.draw(
                    |p| p.poly(&[(to.x, to.y), (a.x, a.y), (b.x, b.y)], true),
                    Some(self.stroke),
                    Some(solid(1.0)),
                );
            }

            MessageFlow { .. } => {
                self.draw(|p| p.poly(&pts, false), None, Some(dashed(1.5, 10.0, 6.0)));

                let start = edge.waypoints[0];
                self.draw(
                    |p| p.circle(start.x, start.y, 4.0),
                    Some(self.fill),
                    Some(solid(1.5)),
                );

                let [a, b] = arrowhead(from, to, 11.0, 4.0);
                self.draw(
                    |p| p.poly(&[(to.x, to.y), (a.x, a.y), (b.x, b.y)], true),
                    Some(self.fill),
                    Some(solid(1.5)),
                );
            }

            Association { .. } => {
                self.draw(|p| p.poly(&pts, false), None, Some(dashed(1.5, 2.0, 4.0)));
            }

            DataInputAssociation { .. } | DataOutputAssociation { .. } => {
                self.draw(|p| p.poly(&pts, false), None, Some(dashed(1.5, 2.0, 4.0)));
                let [a, b] = arrowhead(from, to, 11.0, 5.0);
                self.draw(
                    |p| p.poly(&[(a.x, a.y), (to.x, to.y), (b.x, b.y)], false),
                    None,
                    Some(solid(1.5)),
                );
            }

            _ => {}
        }

        if let Some(name) = name {
            self.edge_label(edge, name);
        }
    }

    /// Paints the name of a connection at its DI label bounds, or above the
    /// middle of its path.
    fn edge_label(&mut self, edge: &Edge, name: &str) {
        let n = edge.waypoints.len();
        if n < 2 {
            return;
        }

        let label = edge.label_bounds.unwrap_or_else(|| {
            let (a, b) = (edge.waypoints[(n - 1) / 2], edge.waypoints[n / 2]);
            let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            Bounds {
                x: mx - 45.0,
                y: my - 18.0,
                w: 90.0,
                h: 14.0,
            }
        });
        let wide = Bounds {
            x: label.x - 20.0,
            w: label.w + 40.0,
            y: label.y - 5.0,
            ..label
        };

        self.text(name, wide, TextAlign::Center, false);
    }
}

/// Visual variant of an event circle.
#[derive(Clone, Copy)]
enum EventStyle {
    Start,
    End,
    Catch,
    Throw,
    NonInterrupting,
}

/// Approximates half an ellipse from the left to the right end (`top`) or
/// right to left via the bottom with a cubic-free polyline.
fn ellipse_half(p: &mut Pen, cx: f64, cy: f64, rx: f64, ry: f64, top: bool) {
    const STEPS: usize = 16;

    for i in 1..=STEPS {
        let t = std::f64::consts::PI * i as f64 / STEPS as f64;
        let (x, y) = if top {
            (cx - rx * t.cos(), cy - ry * t.sin())
        } else {
            (cx + rx * t.cos(), cy + ry * t.sin())
        };
        p.line_to(x, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vertical pools get a 30px band at the top, full width; the left edge
    /// stays untouched.
    #[test]
    fn vertical_band_is_top_strip() {
        let b = Bounds {
            x: 100.0,
            y: 60.0,
            w: 400.0,
            h: 460.0,
        };
        let band = name_band(b, true);

        assert_eq!((band.x, band.y, band.w, band.h), (100.0, 60.0, 400.0, 30.0));
    }

    /// Horizontal pools keep the 30px band on the left, full height.
    #[test]
    fn horizontal_band_is_left_strip() {
        let b = Bounds {
            x: 100.0,
            y: 60.0,
            w: 400.0,
            h: 460.0,
        };
        let band = name_band(b, false);

        assert_eq!((band.x, band.y, band.w, band.h), (100.0, 60.0, 30.0, 460.0));
    }
}
