//! `Theme`: the complete set of canvas colors, stroke widths and font, as
//! deserialized from a theme YAML mapping (schema in `specs/modeler/theme.md`).

use gpui::{App, Global, Hsla};
use serde::Deserialize;
use serde_norway::Value;

use super::color::Color;
use crate::model::ElementKind;

/// Canvas surface.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasColors {
    pub background: Color,
}

/// Shape outline, fill and label colors, with optional per-kind fills that
/// fall back to `fill` when `null`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeColors {
    pub stroke: Color,
    pub fill: Color,
    pub label: Color,
    pub event_fill: Option<Color>,
    pub task_fill: Option<Color>,
    pub gateway_fill: Option<Color>,
    pub data_fill: Option<Color>,
    /// Pools and lanes.
    pub band_fill: Option<Color>,
}

/// Connection colors.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowColors {
    pub stroke: Color,
}

/// Palette frame, separator, icon tint and active-entry highlight.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteColors {
    pub background: Color,
    pub border: Color,
    pub icon: Color,
    pub hover: Color,
}

/// Context pad background.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PadColors {
    pub background: Color,
}

/// Popup or editor box colors (replace menu, label editor).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxColors {
    pub background: Color,
    pub border: Color,
    pub text: Color,
}

/// Canvas overlay colors (`canvas::overlay`): element badges and the error
/// tooltip / not-allowed marker of a refused gesture.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayColors {
    pub badge: BoxColors,
    pub error: BoxColors,
}

/// Stroke widths in diagram units.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrokeWidths {
    /// Main outline of activities, start events, gateways, data and pools.
    pub shape: f64,
    pub end_event: f64,
    /// Sequence flow line.
    pub flow: f64,
}

/// Label font.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Font {
    pub family: String,
    /// Size in diagram units.
    pub size: f64,
}

/// A complete theme; partial user files are merged over a built-in before
/// deserializing, so every key is present here.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub canvas: CanvasColors,
    pub shape: ShapeColors,
    pub flow: FlowColors,
    pub selection: Color,
    pub hover: Color,
    pub lasso: Color,
    /// Dots of the snap grid painted under the diagram.
    pub grid: Color,
    pub palette: PaletteColors,
    pub context_pad: PadColors,
    pub replace_menu: BoxColors,
    pub label_editor: BoxColors,
    pub overlay: OverlayColors,
    pub stroke_width: StrokeWidths,
    pub font: Font,
}

impl Global for Theme {}

impl Theme {
    /// Deserializes a complete theme mapping, naming the offending key on error.
    pub fn from_value(value: Value) -> Result<Theme, String> {
        serde_norway::from_value(value).map_err(|err| err.to_string())
    }

    /// The active theme of the app (the default when none was applied yet).
    pub fn current(cx: &App) -> Theme {
        cx.try_global::<Theme>().cloned().unwrap_or_default()
    }

    /// Color named by a dotted theme key (`selection`, `hover`, `lasso`,
    /// `grid`, `shape.stroke`, `shape.fill`, `shape.label`, `flow.stroke`,
    /// `palette.icon`, `overlay.badge.text`, `overlay.error.border`, ...), or
    /// `None` for an unknown key.
    pub fn color_for(&self, key: &str) -> Option<Color> {
        let boxed = |colors: &BoxColors, part: &str| match part {
            "background" => Some(colors.background),
            "border" => Some(colors.border),
            "text" => Some(colors.text),
            _ => None,
        };

        match key {
            "selection" => Some(self.selection),
            "hover" => Some(self.hover),
            "lasso" => Some(self.lasso),
            "grid" => Some(self.grid),
            "canvas.background" => Some(self.canvas.background),
            "shape.stroke" => Some(self.shape.stroke),
            "shape.fill" => Some(self.shape.fill),
            "shape.label" => Some(self.shape.label),
            "flow.stroke" => Some(self.flow.stroke),
            "palette.icon" => Some(self.palette.icon),
            _ => {
                let (group, part) = key.rsplit_once('.')?;

                match group {
                    "overlay.badge" => boxed(&self.overlay.badge, part),
                    "overlay.error" => boxed(&self.overlay.error, part),
                    "replace_menu" => boxed(&self.replace_menu, part),
                    "label_editor" => boxed(&self.label_editor, part),
                    _ => None,
                }
            }
        }
    }

    /// Fill of a shape of `kind`: its per-kind fill, else the shape fill.
    pub fn fill_for(&self, kind: &ElementKind) -> Hsla {
        use ElementKind::*;

        let shape = &self.shape;
        let specific = match kind {
            StartEvent(_)
            | EndEvent(_)
            | IntermediateCatchEvent(_)
            | IntermediateThrowEvent(_)
            | BoundaryEvent { .. } => shape.event_fill,
            Task
            | UserTask
            | ServiceTask
            | ScriptTask
            | SendTask
            | ReceiveTask
            | ManualTask
            | BusinessRuleTask
            | CallActivity
            | SubProcess { .. } => shape.task_fill,
            ExclusiveGateway | ParallelGateway | InclusiveGateway | EventBasedGateway
            | ComplexGateway => shape.gateway_fill,
            DataObjectReference | DataStoreReference => shape.data_fill,
            Participant { .. } | Lane => shape.band_fill,
            _ => None,
        };

        specific.unwrap_or(shape.fill).hsla()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dotted keys resolve to the matching theme colors; unknown keys and
    /// unknown parts resolve to nothing.
    #[test]
    fn color_keys_resolve() {
        let theme = Theme::default();

        assert_eq!(theme.color_for("selection"), Some(theme.selection));
        assert_eq!(theme.color_for("flow.stroke"), Some(theme.flow.stroke));
        assert_eq!(
            theme.color_for("overlay.error.text"),
            Some(theme.overlay.error.text)
        );
        assert_eq!(
            theme.color_for("overlay.badge.border"),
            Some(theme.overlay.badge.border)
        );

        assert_eq!(theme.color_for("nope"), None);
        assert_eq!(theme.color_for("overlay.badge.nope"), None);
        assert_eq!(theme.color_for("overlay.nope.text"), None);
    }
}
