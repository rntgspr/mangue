//! Embedded app assets served to GPUI: the vendored Lucide icons
//! (`assets/icons/`, lucide-static 0.460.0, ISC license), plus vertical
//! label SVGs generated on demand under `vlabel/` (see `vertical_label_path`),
//! and the built-in themes (`assets/themes/*.yaml`).

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// Embedded files by asset path.
const FILES: &[(&str, &[u8])] = &[
    ("icons/hand.svg", include_bytes!("../assets/icons/hand.svg")),
    (
        "icons/chevron-down.svg",
        include_bytes!("../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/lasso.svg",
        include_bytes!("../assets/icons/lasso.svg"),
    ),
    (
        "icons/move-horizontal.svg",
        include_bytes!("../assets/icons/move-horizontal.svg"),
    ),
    (
        "icons/spline.svg",
        include_bytes!("../assets/icons/spline.svg"),
    ),
    (
        "icons/trash-2.svg",
        include_bytes!("../assets/icons/trash-2.svg"),
    ),
    (
        "icons/wrench.svg",
        include_bytes!("../assets/icons/wrench.svg"),
    ),
    (
        "icons/arrow-up-right.svg",
        include_bytes!("../assets/icons/arrow-up-right.svg"),
    ),
    (
        "icons/message-square-text.svg",
        include_bytes!("../assets/icons/message-square-text.svg"),
    ),
    (
        "icons/arrow-up-to-line.svg",
        include_bytes!("../assets/icons/arrow-up-to-line.svg"),
    ),
    (
        "icons/arrow-down-to-line.svg",
        include_bytes!("../assets/icons/arrow-down-to-line.svg"),
    ),
    (
        "icons/rows-2.svg",
        include_bytes!("../assets/icons/rows-2.svg"),
    ),
    (
        "icons/rows-3.svg",
        include_bytes!("../assets/icons/rows-3.svg"),
    ),
    ("icons/plus.svg", include_bytes!("../assets/icons/plus.svg")),
    (
        "themes/solarized-light.yaml",
        include_bytes!("../assets/themes/solarized-light.yaml"),
    ),
    (
        "themes/solarized-dark.yaml",
        include_bytes!("../assets/themes/solarized-dark.yaml"),
    ),
    (
        "themes/tokyonight-day.yaml",
        include_bytes!("../assets/themes/tokyonight-day.yaml"),
    ),
    (
        "themes/tokyonight-night.yaml",
        include_bytes!("../assets/themes/tokyonight-night.yaml"),
    ),
];

/// Asset path of the embedded icon `name` (`"plus"` is `icons/plus.svg`), or
/// `None` when there is no such icon.
pub fn icon_path(name: &str) -> Option<String> {
    let path = format!("icons/{name}.svg");

    FILES.iter().any(|(p, _)| *p == path).then_some(path)
}

/// Asset path prefix of generated vertical (rotated -90 degrees) labels.
const VERTICAL_LABEL: &str = "vlabel/";

/// Asset path of a label reading bottom-to-top, centered in a `w` x `h`
/// band (diagram units) at `font_size`; served as SVG by `Assets`.
pub fn vertical_label_path(text: &str, w: f64, h: f64, font_size: f64) -> String {
    format!("{VERTICAL_LABEL}{w}x{h}x{font_size}/{text}")
}

/// SVG for a `vlabel/` path: `<text>` rotated -90 degrees about the band
/// center (GPUI cannot rotate text, resvg can), `None` when malformed.
fn vertical_label_svg(spec: &str) -> Option<String> {
    let (dims, text) = spec.split_once('/')?;
    let mut dims = dims.split('x').map(|d| d.parse::<f64>().ok());
    let (w, h, size) = (dims.next()??, dims.next()??, dims.next()??);

    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let (cx, cy) = (w / 2.0, h / 2.0);

    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\"><text x=\"{cx}\" y=\"{cy}\" \
         transform=\"rotate(-90 {cx} {cy})\" text-anchor=\"middle\" \
         dominant-baseline=\"central\" font-family=\"Arial, Helvetica, sans-serif\" \
         font-size=\"{size}\">{escaped}</text></svg>"
    ))
}

/// `AssetSource` over the files embedded at compile time.
pub struct Assets;

impl AssetSource for Assets {
    /// Returns the embedded bytes of `path`, or `None` when unknown.
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(spec) = path.strip_prefix(VERTICAL_LABEL) {
            return Ok(vertical_label_svg(spec).map(|svg| Cow::Owned(svg.into_bytes())));
        }

        Ok(FILES
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    /// Lists the embedded paths under the `path` prefix.
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every embedded icon loads and is an SVG; unknown paths load nothing.
    #[test]
    fn icons_load() {
        let icons: Vec<_> = FILES
            .iter()
            .filter(|(p, _)| p.starts_with("icons/"))
            .collect();
        for (path, _) in &icons {
            let bytes = Assets.load(path).unwrap().unwrap();
            assert!(String::from_utf8_lossy(&bytes).contains("<svg"));
        }

        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), icons.len());
        assert_eq!(Assets.list("themes/").unwrap().len(), 4);
    }

    /// Icon names resolve to embedded paths; unknown names and path tricks
    /// resolve to nothing.
    #[test]
    fn icon_names_resolve() {
        assert_eq!(icon_path("plus").as_deref(), Some("icons/plus.svg"));
        assert_eq!(icon_path("missing"), None);
        assert_eq!(icon_path("../themes/solarized-light"), None);
    }

    /// Vertical label paths load as SVG text rotated about the band center,
    /// with XML-escaped content; malformed specs load nothing.
    #[test]
    fn vertical_labels_load() {
        let path = vertical_label_path("A&B/C", 30.0, 200.0, 12.0);
        let bytes = Assets.load(&path).unwrap().unwrap();
        let svg = String::from_utf8_lossy(&bytes);

        assert!(svg.contains("rotate(-90 15 100)"));
        assert!(svg.contains(">A&amp;B/C</text>"));

        assert!(Assets.load("vlabel/bad/x").unwrap().is_none());
    }
}
