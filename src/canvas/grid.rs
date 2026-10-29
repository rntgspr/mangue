//! Dot grid under the diagram: one dot per snap-grid intersection, in
//! diagram coordinates, thinned to major steps when dots get too dense.

use std::cell::RefCell;
use std::sync::Arc;

use gpui::{Bounds, Hsla, Pixels, Point, RenderImage, Window, px, size};
use image::{Frame, Rgba, RgbaImage};

use super::geometry::Viewport;
use crate::model::Waypoint;

/// Smallest on-screen distance between painted dots, in screen px.
const MIN_SPACING: f64 = 20.0;

/// Dot radius in screen px (constant at every zoom).
const DOT_RADIUS: f64 = 1.0;

/// Multiple of the grid size between painted dots: the first of 1, 2, 5,
/// 10, 20, 50... whose on-screen spacing is at least `MIN_SPACING`.
pub fn dot_step(grid: f64, scale: f64) -> f64 {
    let mut decade = 1.0;
    loop {
        for factor in [1.0, 2.0, 5.0] {
            let step = factor * decade;
            if grid * step * scale >= MIN_SPACING {
                return step;
            }
        }

        decade *= 10.0;
    }
}

/// Target edge (screen px) of a cached tile; the tile holds whole dot
/// cells, so its edge is the first multiple of the spacing past this.
const TILE_EDGE: f64 = 256.0;

/// Margin (device-independent px) between a tile's edge and its first dot,
/// so no dot is cut by the tile border.
const PAD: f64 = 2.0;

/// What a cached tile was drawn for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TileKey {
    spacing: f64,
    color: [f32; 4],
    scale_factor: f32,
}

/// A rasterized square of dots and the key it was drawn for.
struct Tile {
    key: TileKey,
    image: Arc<RenderImage>,
    /// Edge of the tile in screen px.
    side: f64,
}

thread_local! {
    /// The tile of the last grid painted: panning reuses it, a zoom or
    /// theme change replaces it.
    static TILE: RefCell<Option<Tile>> = const { RefCell::new(None) };
}

/// Pixels (BGRA, straight alpha) of a tile of `cells` x `cells` dots
/// `spacing` screen px apart, drawn at `scale_factor` device px per px.
/// The first dot sits `PAD` px from the tile's top-left corner.
pub fn raster(spacing: f64, cells: usize, scale_factor: f64, color: [f32; 4]) -> RgbaImage {
    let side = ((cells as f64 * spacing * scale_factor).round() as u32).max(1);
    let mut image = RgbaImage::new(side, side);
    let radius = DOT_RADIUS * scale_factor;
    let [r, g, b, a] = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);

    for iy in 0..cells {
        for ix in 0..cells {
            let cx = (ix as f64 * spacing + PAD) * scale_factor;
            let cy = (iy as f64 * spacing + PAD) * scale_factor;
            let (x0, x1) = (
                (cx - radius - 1.0).floor().max(0.0),
                (cx + radius + 1.0).ceil(),
            );
            let (y0, y1) = (
                (cy - radius - 1.0).floor().max(0.0),
                (cy + radius + 1.0).ceil(),
            );

            for y in y0 as u32..(y1 as u32).min(side) {
                for x in x0 as u32..(x1 as u32).min(side) {
                    let dist =
                        ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt();
                    let coverage = (radius + 0.5 - dist).clamp(0.0, 1.0);
                    let alpha = (coverage * f64::from(a)).round() as u8;
                    if alpha > image.get_pixel(x, y)[3] {
                        image.put_pixel(x, y, Rgba([b, g, r, alpha]));
                    }
                }
            }
        }
    }

    image
}

/// Inclusive range of tile indices on one axis whose `side`-wide tiles,
/// starting at screen position `origin + index * side`, touch `0..=extent`.
pub fn tile_range(origin: f64, side: f64, extent: f64) -> (i64, i64) {
    let first = ((0.0 - origin) / side).floor() as i64;
    let last = ((extent - origin) / side).floor() as i64;

    (first, last)
}

/// Paints the dots of a `grid`-unit grid in `color` over a canvas of
/// `width` x `height` screen px at `origin`: a cached raster tile of dots,
/// tiled across the viewport, so a frame paints a few images instead of one
/// quad per dot.
pub fn paint_grid(
    grid: f64,
    color: Hsla,
    viewport: Viewport,
    origin: Point<Pixels>,
    (width, height): (f64, f64),
    window: &mut Window,
) {
    let spacing = grid * dot_step(grid, viewport.scale) * viewport.scale;
    let rgba = color.to_rgb();
    let key = TileKey {
        spacing,
        color: [rgba.r, rgba.g, rgba.b, rgba.a],
        scale_factor: window.scale_factor(),
    };

    let tile = TILE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.as_ref().is_none_or(|t| t.key != key) {
            if let Some(old) = cache.take() {
                let _ = window.drop_image(old.image);
            }

            let cells = (TILE_EDGE / spacing).ceil().max(1.0) as usize;
            let image = raster(spacing, cells, f64::from(key.scale_factor), key.color);
            let render = RenderImage::new(vec![Frame::new(image)]);

            *cache = Some(Tile {
                key,
                image: Arc::new(render),
                side: cells as f64 * spacing,
            });
        }

        cache.as_ref().map(|t| (t.image.clone(), t.side))
    });
    let Some((image, side)) = tile else {
        return;
    };

    let lattice = viewport.to_screen(Waypoint { x: 0.0, y: 0.0 });
    let (ox, oy) = (lattice.x - PAD, lattice.y - PAD);
    let (x0, x1) = tile_range(ox, side, width);
    let (y0, y1) = tile_range(oy, side, height);

    for ty in y0..=y1 {
        for tx in x0..=x1 {
            let at = gpui::point(
                origin.x + px((ox + tx as f64 * side) as f32),
                origin.y + px((oy + ty as f64 * side) as f32),
            );
            let bounds = Bounds::new(at, size(px(side as f32), px(side as f32)));

            let _ = window.paint_image(bounds, bounds, Default::default(), image.clone(), 0, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every 2nd dot at normal zoom; sparser major steps as zoom drops.
    #[test]
    fn step_by_zoom() {
        assert_eq!(dot_step(10.0, 1.0), 2.0);
        assert_eq!(dot_step(10.0, 0.8), 5.0);
        assert_eq!(dot_step(10.0, 0.5), 5.0);
        assert_eq!(dot_step(10.0, 0.2), 10.0);
        assert_eq!(dot_step(10.0, 0.1), 20.0);
        assert_eq!(dot_step(5.0, 1.0), 5.0);
        assert_eq!(dot_step(20.0, 4.0), 1.0);
    }

    /// A tile holds one dot per cell with a transparent gap between them,
    /// at the requested scale factor.
    #[test]
    fn raster_draws_dots_in_cells() {
        let image = raster(20.0, 2, 1.0, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(image.dimensions(), (40, 40));

        assert!(image.get_pixel(2, 2)[3] > 150);
        assert!(image.get_pixel(22, 22)[3] > 150);
        assert_eq!(image.get_pixel(12, 12)[3], 0);

        let hidpi = raster(20.0, 1, 2.0, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(hidpi.dimensions(), (40, 40));
        assert_eq!(hidpi.get_pixel(4, 4)[3], 255);
    }

    /// A viewport is covered by a few tiles whatever its size in dots.
    #[test]
    fn tiles_cover_viewport() {
        assert_eq!(tile_range(-2.0, 260.0, 1000.0), (0, 3));
        assert_eq!(tile_range(-300.0, 260.0, 100.0), (1, 1));
        assert_eq!(tile_range(50.0, 260.0, 100.0), (-1, 0));
    }
}
