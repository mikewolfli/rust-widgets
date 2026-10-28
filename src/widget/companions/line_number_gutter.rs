//! `LineNumberGutter` — the shared line-number rail for code and log views (COMP-A22).
//!
//! # Why a companion
//!
//! A code editor, a log viewer and a diff view all paint the same rail: a column of line
//! numbers whose visible window is aligned with a sibling text viewport. Each control used to
//! keep its own numbers, its own current-line rule and its own breakpoint markers, so the rail
//! moved differently when the text viewport scrolled and a marker was drawn by only one of them.
//! This type owns the rail once. A host supplies the document range and the visible window,
//! then mirrors the two signals: [`LineNumberGutter::line_clicked`] for a row hit and
//! [`LineNumberGutter::current_line_changed`] for the caret line.
//!
//! The gutter never asks the text viewport for its geometry. `visible_lines` is the shared
//! contract: the host is the only party that knows the row height and the scroll offset, so it
//! tells the rail which document lines the viewport currently shows.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// Horizontal inset between the gutter edge and a number.
const GUTTER_INSET: u32 = 6;
/// Width of the breakpoint marker bar drawn at the gutter's outer edge.
const MARKER_BAR_WIDTH: u32 = 3;

/// Which edge a number is aligned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberAlign {
    /// Numbers hug the left edge, as in a log with a leading timestamp column.
    Left,
    /// Numbers hug the right edge, the usual code-editor gutter.
    #[default]
    Right,
}

impl LineNumberAlign {
    /// The token used by the property contract.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// Parses the token used by the property contract.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// A line-number column with breakpoint markers and a current-line highlight.
///
/// The rail paints only the window in `visible_lines`; a click in that window reports the
/// document line under the pointer rather than the row index inside the widget.
pub struct LineNumberGutter {
    base: BaseWidget,
    /// First document line represented by the gutter.
    first_line: u64,
    /// Number of document lines in the gutter's range.
    line_count: u64,
    /// First document line the connected viewport currently shows.
    visible_first: u64,
    /// Number of document lines the connected viewport currently shows.
    visible_count: u64,
    /// Document line highlighted as the caret line.
    current_line: u64,
    /// Edge the numbers are aligned to.
    align: LineNumberAlign,
    /// Document lines drawn as marked rows.
    marker_lines: Vec<u64>,
    /// Caller-supplied font; the theme font is used until one is set.
    font: Option<Font>,
    /// Emitted with the document line a press landed on.
    pub line_clicked: Signal1<u64>,
    /// Emitted with the new caret line when it differs from the previous one.
    pub current_line_changed: Signal1<u64>,
}

impl LineNumberGutter {
    /// Creates an empty gutter starting at document line 1.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "LineNumberGutter"),
            first_line: 1,
            line_count: 0,
            visible_first: 1,
            visible_count: 0,
            current_line: 0,
            align: LineNumberAlign::Right,
            marker_lines: Vec::new(),
            font: None,
            line_clicked: Signal1::new(),
            current_line_changed: Signal1::new(),
        }
    }

    /// Sets the document range the gutter addresses.
    pub fn set_range(&mut self, first: u64, count: u64) {
        if self.first_line == first && self.line_count == count {
            return;
        }
        self.first_line = first;
        self.line_count = count;
        self.base.request_redraw();
    }

    /// Sets the document window the connected viewport shows.
    pub fn set_visible_lines(&mut self, first: u64, count: u64) {
        if self.visible_first == first && self.visible_count == count {
            return;
        }
        self.visible_first = first;
        self.visible_count = count;
        self.base.request_redraw();
    }

    /// Sets the caret line.
    ///
    /// The value is stored as given so a host can report a caret just outside the visible
    /// window; only painting is bounded by the window.
    pub fn set_current_line(&mut self, line: u64) {
        if self.current_line == line {
            return;
        }
        self.current_line = line;
        self.base.request_redraw();
        self.current_line_changed.emit(line);
    }

    /// Replaces the marked rows. The list is sorted and deduplicated so painting and hit
    /// queries do not depend on the caller's ordering.
    pub fn set_markers(&mut self, mut lines: Vec<u64>) {
        lines.sort_unstable();
        lines.dedup();
        if self.marker_lines == lines {
            return;
        }
        self.marker_lines = lines;
        self.base.request_redraw();
    }

    /// Sets the font used for the numbers.
    pub fn set_font(&mut self, font: Font) {
        if self.font.as_ref() == Some(&font) {
            return;
        }
        self.font = Some(font);
        self.base.request_redraw();
    }

    /// Sets the edge the numbers are aligned to.
    pub fn set_align(&mut self, align: LineNumberAlign) {
        if self.align == align {
            return;
        }
        self.align = align;
        self.base.request_redraw();
    }

    /// Returns the first document line in the gutter's range.
    pub fn first_line(&self) -> u64 {
        self.first_line
    }

    /// Returns the number of document lines in the gutter's range.
    pub fn line_count(&self) -> u64 {
        self.line_count
    }

    /// Returns the visible window as `(first, count)`.
    pub fn visible_lines(&self) -> (u64, u64) {
        (self.visible_first, self.visible_count)
    }

    /// Returns the caret line.
    pub fn current_line(&self) -> u64 {
        self.current_line
    }

    /// Returns the marked document lines in ascending order.
    pub fn markers(&self) -> &[u64] {
        &self.marker_lines
    }

    /// Returns the alignment edge.
    pub fn align(&self) -> LineNumberAlign {
        self.align
    }

    /// Returns the caller-supplied font, if any.
    pub fn font(&self) -> Option<&Font> {
        self.font.as_ref()
    }

    /// Maps a widget-relative y coordinate to the document line painted there.
    ///
    /// Returns `None` above or below the widget, outside the visible window, or outside the
    /// document range. The row height is the widget height divided evenly by the visible line
    /// count, which is the same division `draw` uses, so painting and hit testing cannot
    /// disagree.
    pub fn line_at(&self, y: i32) -> Option<u64> {
        let rect = self.geometry();
        if self.visible_count == 0 || rect.height == 0 {
            return None;
        }
        if y < rect.y || y >= rect.bottom() {
            return None;
        }
        let rows = self.visible_count.min(u32::MAX as u64) as u32;
        let row_height = (rect.height / rows).max(1);
        let index = (y - rect.y) as u32 / row_height;
        if index >= rows {
            return None;
        }
        let line = self.visible_first.checked_add(u64::from(index))?;
        let end = self.first_line.saturating_add(self.line_count);
        if line < self.first_line || line >= end {
            return None;
        }
        Some(line)
    }

    /// The ink a number is painted with.
    ///
    /// A marker and the caret line use the accent; every other number uses the ordinary ink.
    /// The current line additionally receives a tinted row background in `draw`.
    fn line_ink(&self, line: u64, ink: Color, accent: Color) -> Color {
        if self.marker_lines.binary_search(&line).is_ok() || line == self.current_line {
            accent
        } else {
            ink
        }
    }
}

impl EventHandler for LineNumberGutter {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if !self.geometry().contains_point(*pos) {
                return;
            }
            let Some(line) = self.line_at(pos.y) else {
                return;
            };
            self.set_current_line(line);
            self.line_clicked.emit(line);
        }
    }
}

impl Draw for LineNumberGutter {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = self.font.as_ref().unwrap_or_else(|| effective_font(style));

        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        if self.visible_count == 0 || self.line_count == 0 || rect.height == 0 {
            return;
        }

        let rows = self.visible_count.min(u32::MAX as u64) as u32;
        let row_height = (rect.height / rows).max(1);
        let end = self.first_line.saturating_add(self.line_count);
        let alignment = match self.align {
            LineNumberAlign::Left => HorizontalAlignment::Left,
            LineNumberAlign::Right => HorizontalAlignment::Right,
        };
        let mut label = String::new();

        for index in 0..rows {
            let offset =
                u64::from(index).saturating_mul(u64::from(row_height)).min(i32::MAX as u64) as i32;
            let y = rect.y.saturating_add(offset);
            if y >= rect.bottom() {
                break;
            }
            let Some(line) = self.visible_first.checked_add(u64::from(index)) else {
                break;
            };
            if line < self.first_line {
                continue;
            }
            if line >= end {
                break;
            }

            let remaining = rect.bottom().saturating_sub(y).max(0) as u32;
            let height = row_height.min(remaining);
            if height == 0 {
                break;
            }
            let row = Rect::new(rect.x, y, rect.width, height);

            if line == self.current_line {
                context.fill_rect(row, surface.blend(&accent, 0.16));
            }

            if self.marker_lines.binary_search(&line).is_ok() {
                let marker_x = match self.align {
                    LineNumberAlign::Left => rect.x,
                    LineNumberAlign::Right => rect.right().saturating_sub(MARKER_BAR_WIDTH as i32),
                };
                context.fill_rect(Rect::new(marker_x, y, MARKER_BAR_WIDTH, height), accent);
            }

            label.clear();
            let _ = core::fmt::Write::write_fmt(&mut label, format_args!("{line}"));
            let text_bounds = Rect::new(
                rect.x.saturating_add(GUTTER_INSET as i32),
                y,
                rect.width.saturating_sub(GUTTER_INSET * 2 + MARKER_BAR_WIDTH),
                height,
            );
            context.draw_text_fitted_cow(
                text_bounds,
                &label,
                font,
                self.line_ink(line, ink, accent),
                alignment,
            );
        }
    }
}

impl Widget for LineNumberGutter {
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
        let rows = self.visible_count.min(u32::MAX as u64) as u32;
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(48, rows.saturating_mul(20).max(20)),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "line_clicked" => {
                Some(EventSignalRef::mapped("line_clicked", &self.line_clicked, |line| {
                    CapabilityValue::UInt(*line)
                }))
            }
            "current_line_changed" => Some(EventSignalRef::mapped(
                "current_line_changed",
                &self.current_line_changed,
                |line| CapabilityValue::UInt(*line),
            )),
            _ => None,
        }
    }
}

/// Decodes a line-number list.
///
/// Integer values are accepted in both signed and unsigned form because a document produced by
/// a different binding may spell `3` as an `Int`. A float is accepted only when it is finite,
/// non-negative and integral, so a list of row positions that arrived as numbers still
/// round-trips while `1.5` is refused rather than silently truncated to line 1.
fn expect_line_list(value: CapabilityValue) -> Result<Vec<u64>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut lines = Vec::with_capacity(items.len());
    for item in items {
        let line = match item {
            CapabilityValue::UInt(value) => value,
            CapabilityValue::Int(value) if value >= 0 => value as u64,
            CapabilityValue::Float(value)
                if value.is_finite()
                    && value >= 0.0
                    && value.trunc() == value
                    && value <= u64::MAX as f64 =>
            {
                value as u64
            }
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        lines.push(line);
    }
    Ok(lines)
}

impl WidgetProperties for LineNumberGutter {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "first_line" => Ok(CapabilityValue::UInt(self.first_line)),
            "line_count" => Ok(CapabilityValue::UInt(self.line_count)),
            "current_line" => Ok(CapabilityValue::UInt(self.current_line)),
            "align" => Ok(CapabilityValue::String(self.align.as_str().to_string())),
            "marker_lines" => Ok(CapabilityValue::Tuple(
                self.marker_lines.iter().map(|line| CapabilityValue::UInt(*line)).collect(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "first_line" => {
                let first = expect_usize(value)? as u64;
                self.set_range(first, self.line_count);
                Ok(())
            }
            "line_count" => {
                let count = expect_usize(value)? as u64;
                self.set_range(self.first_line, count);
                Ok(())
            }
            "current_line" => {
                self.set_current_line(expect_usize(value)? as u64);
                Ok(())
            }
            "align" => {
                let token = expect_string(value)?;
                self.set_align(
                    LineNumberAlign::from_name(&token)
                        .ok_or(CapabilityAccessError::TypeMismatch)?,
                );
                Ok(())
            }
            "marker_lines" => {
                self.set_markers(expect_line_list(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "first_line",
            "line_count",
            "current_line",
            "align",
            "marker_lines",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Arc;
    use crate::core::Point;
    use std::sync::Mutex;

    /// Counts the payloads delivered by a `u64` signal.
    fn recorder(signal: &Signal1<u64>) -> Arc<Mutex<Vec<u64>>> {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let handle = Arc::clone(&sink);
        signal.connect(move |value| {
            if let Ok(mut guard) = handle.lock() {
                guard.push(*value);
            }
        });
        sink
    }

    fn recorded(sink: &Arc<Mutex<Vec<u64>>>) -> Vec<u64> {
        sink.lock().map(|guard| guard.clone()).unwrap_or_default()
    }

    #[test]
    fn rows_map_to_document_lines() {
        let mut gutter = LineNumberGutter::new(Rect::new(0, 0, 40, 100));
        gutter.set_range(10, 100);
        gutter.set_visible_lines(10, 5);

        // 100 px / 5 rows = 20 px per row.
        assert_eq!(gutter.line_at(0), Some(10));
        assert_eq!(gutter.line_at(19), Some(10));
        assert_eq!(gutter.line_at(20), Some(11));
        assert_eq!(gutter.line_at(45), Some(12));
        assert_eq!(gutter.line_at(99), Some(14));
        assert_eq!(gutter.line_at(100), None, "the bottom edge is outside the widget");
        assert_eq!(gutter.line_at(-1), None, "the top edge is outside the widget");

        gutter.set_visible_lines(10, 0);
        assert_eq!(gutter.line_at(10), None, "an empty window has no row to map");
    }

    #[test]
    fn a_click_reports_the_line_and_moves_the_caret() {
        let mut gutter = LineNumberGutter::new(Rect::new(0, 0, 40, 100));
        gutter.set_range(10, 100);
        gutter.set_visible_lines(10, 5);
        let clicked = recorder(&gutter.line_clicked);
        let changed = recorder(&gutter.current_line_changed);

        gutter.handle_event(&Event::MousePress {
            pos: Point::new(10, 45),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(gutter.current_line(), 12);
        assert_eq!(recorded(&clicked), vec![12]);
        assert_eq!(recorded(&changed), vec![12]);

        // A second click on the same row still reports the hit, but is not a caret change.
        gutter.handle_event(&Event::MousePress {
            pos: Point::new(10, 50),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(recorded(&clicked), vec![12, 12]);
        assert_eq!(recorded(&changed), vec![12]);
    }

    #[test]
    fn marker_rows_are_painted_with_a_different_ink() {
        let mut gutter = LineNumberGutter::new(Rect::new(0, 0, 40, 100));
        gutter.set_current_line(0);
        gutter.set_markers(Vec::from([12u64, 12, 8]));

        assert_eq!(gutter.markers(), &[8u64, 12], "markers are sorted and deduplicated");
        let ink = Color::BLACK;
        let accent = Color::RED;
        assert_eq!(gutter.line_ink(12, ink, accent), accent);
        assert_eq!(gutter.line_ink(11, ink, accent), ink);
        assert_ne!(
            gutter.line_ink(12, ink, accent),
            gutter.line_ink(11, ink, accent),
            "a marked row must not paint like an ordinary row"
        );
    }

    #[test]
    fn properties_round_trip() {
        let mut gutter = LineNumberGutter::new(Rect::new(0, 0, 40, 100));
        gutter.set("first_line", CapabilityValue::UInt(7)).expect("writable");
        gutter.set("line_count", CapabilityValue::UInt(30)).expect("writable");
        gutter.set("current_line", CapabilityValue::UInt(9)).expect("writable");
        gutter.set("align", CapabilityValue::String("left".to_string())).expect("writable");
        gutter
            .set(
                "marker_lines",
                CapabilityValue::Tuple(vec![
                    CapabilityValue::UInt(9),
                    CapabilityValue::Int(11),
                    CapabilityValue::Float(11.0),
                ]),
            )
            .expect("a line list is writable");

        assert_eq!(gutter.first_line(), 7);
        assert_eq!(gutter.line_count(), 30);
        assert_eq!(gutter.current_line(), 9);
        assert_eq!(gutter.align(), LineNumberAlign::Left);
        assert_eq!(gutter.markers(), &[9u64, 11]);
        assert_eq!(gutter.get("first_line"), Ok(CapabilityValue::UInt(7)));
        assert_eq!(gutter.get("align"), Ok(CapabilityValue::String("left".to_string())));
        assert!(gutter.property_names().contains(&"marker_lines"));
    }
}
