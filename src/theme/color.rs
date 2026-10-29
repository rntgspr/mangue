//! `Color`: a theme color written as `"#rrggbb"` or `"#rrggbbaa"` in YAML.

use gpui::{Hsla, rgba};
use serde::{Deserialize, Deserializer, de::Error};

/// Theme color, converted to GPUI `Hsla` at parse time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color(pub Hsla);

impl Color {
    /// Parses `#rrggbb` (opaque) or `#rrggbbaa`; `None` when malformed.
    pub fn parse(text: &str) -> Option<Color> {
        let hex = text.strip_prefix('#')?;
        let value = u32::from_str_radix(hex, 16).ok()?;

        let rgba_value = match hex.len() {
            6 => (value << 8) | 0xff,
            8 => value,
            _ => return None,
        };

        Some(Color(rgba(rgba_value).into()))
    }

    /// The color as GPUI `Hsla`.
    pub fn hsla(self) -> Hsla {
        self.0
    }
}

impl<'de> Deserialize<'de> for Color {
    /// Reads a hex color string, rejecting anything else.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;

        Color::parse(&text).ok_or_else(|| D::Error::custom(format!("invalid color {text:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Six and eight digit hex colors parse; other forms do not.
    #[test]
    fn parses_hex() {
        assert_eq!(
            Color::parse("#ffffff"),
            Some(Color(rgba(0xffffffff).into()))
        );
        assert_eq!(Color::parse("#26 8bd2"), None);
        assert_eq!(
            Color::parse("#268bd280").map(|c| c.hsla().a),
            Some(128.0 / 255.0)
        );

        assert_eq!(Color::parse("268bd2"), None);
        assert_eq!(Color::parse("#fff"), None);
    }
}
