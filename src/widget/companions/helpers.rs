//! Conversions and theme lookups shared by the companion controls.

use crate::compat::Vec;
use crate::core::{Color, Point, Rect};
use crate::style::WidgetStyle;
use crate::widget::capability::coercion::{expect_i64, expect_string};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};

// ---------------------------------------------------------------------------
// Shared conversion helpers
// ---------------------------------------------------------------------------

/// Resolves the surface and ink a companion paints with.
///
/// An explicit style wins, then the active theme, then a neutral light fallback — the same
/// precedence the other controls use, so a companion mounted beside them does not switch
/// appearance on its own.
pub(super) fn surface_ink(style: &WidgetStyle) -> (Color, Color) {
    let themed = crate::style::palette_shared().map(|colors| colors.background);
    let surface = style.background_color.or(themed).unwrap_or(Color::rgb(245, 246, 248));
    let ink = style
        .text_color
        .or_else(|| themed.map(|surface| surface.contrast_color()))
        .unwrap_or_else(|| surface.contrast_color());
    (surface, ink)
}

/// The active theme's accent, or the library's primary colour.
pub(super) fn accent_color() -> Color {
    crate::style::palette_shared().map(|colors| colors.primary).unwrap_or(Color::PRIMARY)
}

/// Encodes a point as the `[x, y]` tuple property representation.
pub(super) fn point_value(point: Point) -> CapabilityValue {
    CapabilityValue::Tuple(vec![
        CapabilityValue::Int(i64::from(point.x)),
        CapabilityValue::Int(i64::from(point.y)),
    ])
}

/// Decodes the `[x, y]` property representation.
pub(super) fn expect_point(value: CapabilityValue) -> Result<Point, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    if items.len() != 2 {
        return Err(CapabilityAccessError::TypeMismatch);
    }
    let mut items = items.into_iter();
    let first = items.next().ok_or(CapabilityAccessError::TypeMismatch)?;
    let second = items.next().ok_or(CapabilityAccessError::TypeMismatch)?;
    let x = expect_i64(first)?.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let y = expect_i64(second)?.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    Ok(Point::new(x, y))
}

/// Decodes a single colour from either a typed colour or a `#rrggbb[aa]` string.
pub(super) fn expect_color(value: CapabilityValue) -> Result<Color, CapabilityAccessError> {
    match value {
        CapabilityValue::Color(color) => Ok(color),
        CapabilityValue::String(text) => {
            Color::parse_hex(&text).ok_or(CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Decodes a colour list; one malformed entry refuses the whole list.
pub(super) fn expect_colors(value: CapabilityValue) -> Result<Vec<Color>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    items.into_iter().map(expect_color).collect()
}

/// Encodes colours as a tuple of typed colour values.
pub(super) fn colors_value(colors: &[Color]) -> CapabilityValue {
    CapabilityValue::Tuple(colors.iter().map(|color| CapabilityValue::Color(*color)).collect())
}

/// Encodes an optional rectangle: `Null` for "no ROI", `Rect` otherwise.
pub(super) fn optional_rect_value(rect: Option<Rect>) -> CapabilityValue {
    match rect {
        Some(rect) => CapabilityValue::Rect(rect),
        None => CapabilityValue::Null,
    }
}

/// Decodes an optional rectangle value.
pub(super) fn expect_optional_rect(
    value: CapabilityValue,
) -> Result<Option<Rect>, CapabilityAccessError> {
    match value {
        CapabilityValue::Null => Ok(None),
        CapabilityValue::Rect(rect) => Ok(Some(rect)),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Parses a string property into an enum through its own token parser.
pub(super) fn expect_token<T>(
    value: CapabilityValue,
    parse: fn(&str) -> Option<T>,
) -> Result<T, CapabilityAccessError> {
    let token = expect_string(value)?;
    parse(&token.to_ascii_lowercase()).ok_or(CapabilityAccessError::TypeMismatch)
}

/// Clamps a coordinate into an inclusive range, tolerating an inverted (empty) range.
pub(super) fn clamp_axis(value: i32, min: i32, max: i32) -> i32 {
    if max < min {
        min
    } else {
        value.clamp(min, max)
    }
}

/// Scales a content extent by a zoom factor, keeping a visible minimum and rejecting NaN.
pub(super) fn scaled_extent(extent: u32, zoom: f32) -> u32 {
    let value = extent as f32 * zoom;
    if !value.is_finite() {
        return 1;
    }
    value.round().clamp(1.0, u32::MAX as f32) as u32
}
