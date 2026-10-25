//! `Legend` — a reusable chart legend with optional series toggling.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{expect_color, expect_token, surface_ink};

// ---------------------------------------------------------------------------
// COMP-07 Legend
// ---------------------------------------------------------------------------

/// How a legend lays its entries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendOrientation {
    /// Entries flow left to right.
    Horizontal,
    /// Entries stack top to bottom.
    #[default]
    Vertical,
}

impl LegendOrientation {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            LegendOrientation::Horizontal => "horizontal",
            LegendOrientation::Vertical => "vertical",
        }
    }
    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "horizontal" => Some(LegendOrientation::Horizontal),
            "vertical" => Some(LegendOrientation::Vertical),
            _ => None,
        }
    }
}

/// Where a legend sits relative to the plot it describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendPosition {
    /// Above the plot.
    #[default]
    Top,
    /// Right of the plot.
    Right,
    /// Below the plot.
    Bottom,
    /// Left of the plot.
    Left,
    /// Outside the plot's own surface (a detached legend).
    Outside,
}

impl LegendPosition {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            LegendPosition::Top => "top",
            LegendPosition::Right => "right",
            LegendPosition::Bottom => "bottom",
            LegendPosition::Left => "left",
            LegendPosition::Outside => "outside",
        }
    }
    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "top" => Some(LegendPosition::Top),
            "right" => Some(LegendPosition::Right),
            "bottom" => Some(LegendPosition::Bottom),
            "left" => Some(LegendPosition::Left),
            "outside" => Some(LegendPosition::Outside),
            _ => None,
        }
    }
}

/// One legend row: its series label, colour, and whether that series is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendEntry {
    /// Series label.
    pub label: String,
    /// Series colour.
    pub color: Color,
    /// Whether the series is currently visible.
    pub visible: bool,
}

impl LegendEntry {
    /// Creates a visible entry.
    pub fn new(label: impl Into<String>, color: Color) -> Self {
        Self { label: label.into(), color, visible: true }
    }
    /// Sets the initial visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

/// Inset around a legend's entries.
const LEGEND_INSET: u32 = 4;
/// Size of the colour marker before a label.
const LEGEND_MARKER: u32 = 9;

/// A legend that can be mounted beside a chart or a custom canvas.
///
/// A checkable legend owns the visibility bit and reports every toggle through
/// [`Self::series_toggled`]; a non-checkable legend only reports clicks.
pub struct Legend {
    base: BaseWidget,
    entries: Vec<LegendEntry>,
    orientation: LegendOrientation,
    checkable: bool,
    position: LegendPosition,
    /// Emitted with the entry index on any click.
    pub entry_clicked: Signal1<usize>,
    /// Emitted with `(index, visible)` after a checkable entry toggles.
    pub series_toggled: Signal1<(usize, bool)>,
}

impl Legend {
    /// Creates an empty vertical legend.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Legend, geometry, "Legend"),
            entries: Vec::new(),
            orientation: LegendOrientation::Vertical,
            checkable: false,
            position: LegendPosition::Top,
            entry_clicked: Signal1::new(),
            series_toggled: Signal1::new(),
        }
    }

    /// Replaces the entries.
    pub fn set_entries(&mut self, entries: Vec<LegendEntry>) {
        self.entries = entries;
        self.base.request_redraw();
    }

    /// Returns the entries.
    pub fn entries(&self) -> &[LegendEntry] {
        &self.entries
    }

    /// Shows or hides one series, publishing the change when it actually moved.
    ///
    /// Returns `false` for an index outside the entry list.
    pub fn set_entry_visible(&mut self, index: usize, visible: bool) -> bool {
        let Some(entry) = self.entries.get_mut(index) else {
            return false;
        };
        if entry.visible == visible {
            return true;
        }
        entry.visible = visible;
        self.base.request_redraw();
        self.series_toggled.emit((index, visible));
        true
    }

    /// Sets the layout orientation.
    pub fn set_orientation(&mut self, orientation: LegendOrientation) {
        if self.orientation != orientation {
            self.orientation = orientation;
            self.base.request_redraw();
        }
    }

    /// Returns the layout orientation.
    pub fn orientation(&self) -> LegendOrientation {
        self.orientation
    }

    /// Enables or disables click-to-toggle.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }

    /// Returns whether entries are checkable.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Sets the position hint (layout is the host's business).
    pub fn set_position(&mut self, position: LegendPosition) {
        self.position = position;
    }

    /// Returns the position hint.
    pub fn position(&self) -> LegendPosition {
        self.position
    }

    /// The rectangle of one entry, in window coordinates.
    fn entry_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.entries.len() {
            return None;
        }
        let rect = self.geometry();
        let count = self.entries.len() as u32;
        let inner_width = rect.width.saturating_sub(LEGEND_INSET * 2);
        let inner_height = rect.height.saturating_sub(LEGEND_INSET * 2);
        match self.orientation {
            LegendOrientation::Horizontal => {
                let width = (inner_width / count.max(1)).max(1);
                Some(Rect::new(
                    rect.x + LEGEND_INSET as i32 + index as u32 as i32 * width as i32,
                    rect.y + LEGEND_INSET as i32,
                    width,
                    inner_height,
                ))
            }
            LegendOrientation::Vertical => {
                let height = (inner_height / count.max(1)).max(1);
                Some(Rect::new(
                    rect.x + LEGEND_INSET as i32,
                    rect.y + LEGEND_INSET as i32 + index as u32 as i32 * height as i32,
                    inner_width,
                    height,
                ))
            }
        }
    }

    /// The entry under `pos`, if any.
    fn entry_at(&self, pos: Point) -> Option<usize> {
        (0..self.entries.len())
            .find(|&index| self.entry_rect(index).is_some_and(|rect| rect.contains(pos)))
    }
}

impl EventHandler for Legend {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some(index) = self.entry_at(*pos) {
                self.entry_clicked.emit(index);
                if self.checkable {
                    let visible = !self.entries[index].visible;
                    self.set_entry_visible(index, visible);
                }
            }
        }
    }
}

impl Draw for Legend {
    fn draw(&mut self, context: &mut RenderContext) {
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        context.fill_rect(self.geometry(), surface);
        for index in 0..self.entries.len() {
            let Some(cell) = self.entry_rect(index) else {
                continue;
            };
            let entry = &self.entries[index];
            let marker = Rect::new(
                cell.x + 4,
                cell.y + (cell.height.saturating_sub(LEGEND_MARKER) / 2) as i32,
                LEGEND_MARKER.min(cell.width),
                LEGEND_MARKER.min(cell.height),
            );
            let marker_color = if entry.visible { entry.color } else { entry.color.with_alpha(60) };
            context.fill_rect(marker, marker_color);
            let label_color = if entry.visible { ink } else { surface.blend(&ink, 0.45) };
            let label = Rect::new(
                marker.x + LEGEND_MARKER as i32 + 4,
                cell.y,
                cell.width.saturating_sub(LEGEND_MARKER + 8),
                cell.height,
            );
            context.draw_text_fitted(
                label,
                &entry.label,
                font,
                label_color,
                crate::core::HorizontalAlignment::Left,
            );
        }
    }
}

impl Widget for Legend {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        match self.orientation {
            LegendOrientation::Horizontal => {
                Size::new((self.entries.len() as u32 * 80).max(80), 24)
            }
            LegendOrientation::Vertical => Size::new(140, (self.entries.len() as u32 * 22).max(22)),
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "entry_clicked" => {
                Some(EventSignalRef::mapped("entry_clicked", &self.entry_clicked, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "series_toggled" => {
                Some(EventSignalRef::mapped("series_toggled", &self.series_toggled, |value| {
                    CapabilityValue::Tuple(vec![
                        CapabilityValue::UInt(value.0 as u64),
                        CapabilityValue::Bool(value.1),
                    ])
                }))
            }
            _ => None,
        }
    }
}

/// Encodes one legend entry as `[label, colour, visible]`.
fn legend_entry_value(entry: &LegendEntry) -> CapabilityValue {
    CapabilityValue::Tuple(vec![
        CapabilityValue::String(entry.label.clone()),
        CapabilityValue::Color(entry.color),
        CapabilityValue::Bool(entry.visible),
    ])
}

/// Decodes the entry-list property.
fn expect_legend_entries(
    value: CapabilityValue,
) -> Result<Vec<LegendEntry>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let CapabilityValue::Tuple(parts) = item else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if parts.len() != 3 {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let mut parts = parts.into_iter();
        let label = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let color = expect_color(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let visible = expect_bool(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        entries.push(LegendEntry { label, color, visible });
    }
    Ok(entries)
}

impl WidgetProperties for Legend {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entries" => {
                Ok(CapabilityValue::Tuple(self.entries.iter().map(legend_entry_value).collect()))
            }
            "orientation" => Ok(CapabilityValue::String(self.orientation.as_str().to_string())),
            "checkable" => Ok(CapabilityValue::Bool(self.checkable)),
            "position" => Ok(CapabilityValue::String(self.position.as_str().to_string())),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "entries" => {
                self.set_entries(expect_legend_entries(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_token(value, LegendOrientation::from_name)?);
                Ok(())
            }
            "checkable" => {
                self.set_checkable(expect_bool(value)?);
                Ok(())
            }
            "position" => {
                self.set_position(expect_token(value, LegendPosition::from_name)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["entries", "orientation", "checkable", "position", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_an_entry_reports_the_series_and_refuses_a_stale_index() {
        let mut legend = Legend::new(Rect::new(0, 0, 240, 80));
        legend.set_entries(crate::compat::Vec::from([
            LegendEntry::new("Revenue", Color::PRIMARY),
            LegendEntry::new("Cost", Color::ERROR),
        ]));
        assert_eq!(legend.entries().len(), 2);
        assert!(legend.set_entry_visible(1, false));
        assert!(!legend.entries()[1].visible);
        // An entry that was already visible is reported as a no-op success rather than a toggle.
        assert!(legend.set_entry_visible(0, true));
        assert!(legend.entries()[0].visible);
        assert!(!legend.set_entry_visible(5, false));
    }
}
