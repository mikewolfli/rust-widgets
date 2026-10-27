//! `SummaryRow` — one aggregate cell per table column (COMP-A24).
//!
//! # Why a companion
//!
//! The table family (`DataGrid`, `GridWidget`, the finance tables) each painted its own
//! bottom summary line, so the aggregate wording, the value formatting and the column-to-cell
//! mapping differed per control. This type owns the row once: a host configures one
//! [`ColumnAggregate`] per column, pushes the calculated number through [`SummaryRow::set_value`]
//! and calls [`SummaryRow::update_from_source`] once after the batch.
//!
//! # Painted value versus source
//!
//! The row does **not** read the table model. The host is the source: it calculates the aggregate
//! and calls [`SummaryRow::set_value`]. `set_value` is deliberately silent so a refresh of twenty
//! columns does not emit twenty signals; `update_from_source` is the batch boundary that emits
//! [`SummaryRow::summary_recalculated`] exactly once.
//!
//! # Sticky
//!
//! `sticky` says whether the host pins the row to the bottom of the viewport. It changes **no**
//! pixel in this control's own paint; it is exposed only as a query (`sticky()`, and the
//! `sticky` property) so the host can make that decision.

use crate::compat::{format, String, ToString, Vec};
use crate::core::{HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    effective_font, estimate_line_height, estimate_text_width, ControlMetrics,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// Inset around a summary cell.
const SUMMARY_INSET: u32 = 4;

/// The narrowest one summary cell may be, so an empty row still has a usable strip.
const SUMMARY_MIN_WIDTH: u32 = 120;

/// The row's own minimum height.
const SUMMARY_MIN_HEIGHT: u32 = 32;

/// The aggregate function a summary cell applies to its column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAggregateKind {
    /// Adds the column's values.
    Sum,
    /// Reports the smallest value.
    Min,
    /// Reports the largest value.
    Max,
    /// Counts the populated values.
    Count,
    /// Reports the arithmetic mean.
    Average,
    /// A host-defined aggregate; the value is pushed through `set_value` like the built-ins.
    Custom,
}

impl ColumnAggregateKind {
    /// The published token used by the property contract.
    pub fn as_str(self) -> &'static str {
        match self {
            ColumnAggregateKind::Sum => "sum",
            ColumnAggregateKind::Min => "min",
            ColumnAggregateKind::Max => "max",
            ColumnAggregateKind::Count => "count",
            ColumnAggregateKind::Average => "average",
            ColumnAggregateKind::Custom => "custom",
        }
    }

    /// Parses the published token. `avg` is accepted as an alias for `average`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sum" => Some(ColumnAggregateKind::Sum),
            "min" => Some(ColumnAggregateKind::Min),
            "max" => Some(ColumnAggregateKind::Max),
            "count" => Some(ColumnAggregateKind::Count),
            "average" | "avg" => Some(ColumnAggregateKind::Average),
            "custom" => Some(ColumnAggregateKind::Custom),
            _ => None,
        }
    }
}

/// One column summary: which column, how it is aggregated, and the label painted beside the value.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnAggregate {
    /// The table's column token, used to match [`SummaryRow::set_value`] writes.
    pub column_token: String,
    /// The aggregate function the host applied.
    pub kind: ColumnAggregateKind,
    /// The label painted in the cell. An empty label falls back to the kind token.
    pub label: String,
}

impl ColumnAggregate {
    /// Creates one column summary.
    pub fn new(
        column_token: impl Into<String>,
        kind: ColumnAggregateKind,
        label: impl Into<String>,
    ) -> Self {
        Self { column_token: column_token.into(), kind, label: label.into() }
    }
}

/// A row of aggregate cells, one per configured column.
///
/// The layout is an equal-width cell per aggregate, in the order the aggregates were supplied.
/// [`SummaryRow::column_rect`] answers the same geometry the paint pass used, so a host can align
/// a menu or a tooltip with a cell without duplicating the calculation.
pub struct SummaryRow {
    base: BaseWidget,
    aggregates: Vec<ColumnAggregate>,
    sticky: bool,
    values: Vec<(String, f64)>,
    /// Emitted once per [`SummaryRow::update_from_source`] call after values were refreshed.
    pub summary_recalculated: GenericSignal,
}

impl SummaryRow {
    /// Creates an empty summary row.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "SummaryRow"),
            aggregates: Vec::new(),
            sticky: false,
            values: Vec::new(),
            summary_recalculated: GenericSignal::new(),
        }
    }

    /// Replaces the aggregate list.
    ///
    /// Values for tokens that survive the replacement are kept; values for tokens that are no
    /// longer configured are dropped, because painting or querying them would report a column the
    /// row no longer shows.
    pub fn set_aggregates(&mut self, aggregates: Vec<ColumnAggregate>) {
        self.aggregates = aggregates;
        let known = &self.aggregates;
        self.values.retain(|(token, _)| {
            known.iter().any(|item| item.column_token.as_str() == token.as_str())
        });
        self.base.request_redraw();
    }

    /// Returns the configured aggregates.
    pub fn aggregates(&self) -> &[ColumnAggregate] {
        &self.aggregates
    }

    /// Stores the value painted for `token`.
    ///
    /// Returns `false` — and changes nothing — when `token` is not one of the configured
    /// aggregates or when `value` is not finite. The write is silent: a host refreshing several
    /// columns calls this once per column and then calls [`Self::update_from_source`] once.
    pub fn set_value(&mut self, token: impl AsRef<str>, value: f64) -> bool {
        let token = token.as_ref();
        if !value.is_finite()
            || !self.aggregates.iter().any(|item| item.column_token.as_str() == token)
        {
            return false;
        }
        match self.values.iter_mut().find(|(existing, _)| existing.as_str() == token) {
            Some((_, current)) => {
                if *current == value {
                    return true;
                }
                *current = value;
            }
            None => self.values.push((token.to_string(), value)),
        }
        self.base.request_redraw();
        true
    }

    /// Finalizes a batch of [`Self::set_value`] writes after the host re-read its source.
    ///
    /// Drops values whose token is no longer configured, requests a repaint and emits
    /// [`Self::summary_recalculated`] exactly once. The host remains the source of the numbers;
    /// this method is the notification boundary, not a hidden table read.
    pub fn update_from_source(&mut self) {
        let known = &self.aggregates;
        self.values.retain(|(token, _)| {
            known.iter().any(|item| item.column_token.as_str() == token.as_str())
        });
        self.base.request_redraw();
        self.summary_recalculated.emit();
    }

    /// Returns the value stored for `token`, if one has been written.
    pub fn value(&self, token: impl AsRef<str>) -> Option<f64> {
        let token = token.as_ref();
        self.values.iter().find(|(existing, _)| existing.as_str() == token).map(|(_, value)| *value)
    }

    /// Returns every stored value, in the order the values were first written.
    pub fn values(&self) -> &[(String, f64)] {
        &self.values
    }

    /// Returns the rectangle of the cell that paints `token`, or `None` for an unknown token.
    ///
    /// The answer is the same rectangle the paint pass uses: equal-width cells inset by
    /// four pixels, in aggregate order.
    pub fn column_rect(&self, token: impl AsRef<str>) -> Option<Rect> {
        let token = token.as_ref();
        let index = self.aggregates.iter().position(|item| item.column_token.as_str() == token)?;
        self.cell_rect(index)
    }

    /// Returns the text painted in the cell for `token`, or `None` for an unknown token.
    ///
    /// Exposed so a test or a host tooltip can ask "what is on screen?" without running a paint
    /// pass. The paint method calls the same formatter.
    pub fn cell_text(&self, token: impl AsRef<str>) -> Option<String> {
        let token = token.as_ref();
        let aggregate = self.aggregates.iter().find(|item| item.column_token.as_str() == token)?;
        Some(self.aggregate_text(aggregate))
    }

    /// Whether the host pins this row to the bottom of the viewport.
    ///
    /// Paint is identical for both answers; this is a query only.
    pub fn sticky(&self) -> bool {
        self.sticky
    }

    /// Records the host's sticky decision. Deliberately does not request a repaint, because the
    /// flag changes no pixel in this control.
    pub fn set_sticky(&mut self, sticky: bool) {
        self.sticky = sticky;
    }

    /// The rectangle of one cell, using the layout shared with [`Self::column_rect`].
    fn cell_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.aggregates.len() {
            return None;
        }
        let rect = self.geometry();
        let width = (rect.width / self.aggregates.len() as u32).max(1);
        let offset = (index as u32).saturating_mul(width);
        let x = rect.x.saturating_add(offset.min(i32::MAX as u32) as i32);
        Some(Rect::new(
            x.saturating_add(SUMMARY_INSET as i32),
            rect.y.saturating_add(SUMMARY_INSET as i32),
            width.saturating_sub(SUMMARY_INSET * 2),
            rect.height.saturating_sub(SUMMARY_INSET * 2),
        ))
    }

    /// Formats one cell as `label: value`, falling back to the aggregate kind when the label is
    /// empty.
    fn aggregate_text(&self, aggregate: &ColumnAggregate) -> String {
        let value = self
            .value(aggregate.column_token.as_str())
            .map(format_value)
            .unwrap_or_else(|| "-".to_string());
        if aggregate.label.is_empty() {
            format!("{}: {}", aggregate.kind.as_str(), value)
        } else {
            format!("{}: {}", aggregate.label, value)
        }
    }
}

impl EventHandler for SummaryRow {
    fn handle_event(&mut self, event: &Event) {
        // A summary row owns no gesture, but the base still records hover/focus bookkeeping so a
        // host's generic state channel sees the same facts as for any other control. The enabled
        // guard keeps a disabled row from receiving even that bookkeeping.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
    }
}

impl Draw for SummaryRow {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        if self.aggregates.is_empty() {
            let empty_text_rect =
                Rect::new(rect.x + 8, rect.y, rect.width.saturating_sub(16), rect.height);
            context.draw_text_fitted_cow(
                empty_text_rect,
                "No summary columns",
                font,
                surface.blend(&ink, 0.55),
                HorizontalAlignment::Left,
            );
            return;
        }

        for index in 0..self.aggregates.len() {
            let Some(cell) = self.cell_rect(index) else {
                continue;
            };
            context.fill_rect(cell, surface.blend(&ink, 0.05));
            let border = if index == 0 { accent } else { surface.blend(&ink, 0.22) };
            context.draw_rect(cell, border);
            let text = self.aggregate_text(&self.aggregates[index]);
            context.draw_text_fitted_cow(cell, &text, font, ink, HorizontalAlignment::Left);
        }
    }
}

impl Widget for SummaryRow {
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
        let font = effective_font(self.style());
        let line_height = estimate_line_height(font, 1.0);
        let content_width = self.aggregates.iter().fold(0u32, |width, aggregate| {
            let text = self.aggregate_text(aggregate);
            width.saturating_add(
                estimate_text_width(&text, font, 1.0).saturating_add(SUMMARY_INSET * 2),
            )
        });
        let padding = EdgeOffsets::symmetric(SUMMARY_INSET, SUMMARY_INSET);
        let floor = Size::new(
            SUMMARY_MIN_WIDTH,
            line_height.saturating_add(SUMMARY_INSET * 2).max(SUMMARY_MIN_HEIGHT),
        );
        ControlMetrics::implicit_size(Size::new(content_width, line_height), padding, floor)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "summary_recalculated" => {
                Some(EventSignalRef::unit("summary_recalculated", &self.summary_recalculated))
            }
            _ => None,
        }
    }
}

/// Formats a stored aggregate value for painting.
///
/// Whole numbers lose the `.0` tail (`42`, not `42.00`); other finite values keep two decimals.
/// A non-finite value paints as `-` rather than as `NaN`.
fn format_value(value: f64) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }
    if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

/// Encodes one aggregate as `[column_token, kind, label]`.
fn aggregate_value(aggregate: &ColumnAggregate) -> CapabilityValue {
    CapabilityValue::Tuple(vec![
        CapabilityValue::String(aggregate.column_token.clone()),
        CapabilityValue::String(aggregate.kind.as_str().to_string()),
        CapabilityValue::String(aggregate.label.clone()),
    ])
}

/// Decodes the aggregate-list property.
fn expect_aggregates(
    value: CapabilityValue,
) -> Result<Vec<ColumnAggregate>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut aggregates = Vec::with_capacity(items.len());
    for item in items {
        let CapabilityValue::Tuple(parts) = item else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if parts.len() != 3 {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let mut parts = parts.into_iter();
        let column_token = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let kind_token = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let kind = ColumnAggregateKind::from_name(&kind_token.to_ascii_lowercase())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let label = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        aggregates.push(ColumnAggregate { column_token, kind, label });
    }
    Ok(aggregates)
}

impl WidgetProperties for SummaryRow {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "aggregates" => {
                Ok(CapabilityValue::Tuple(self.aggregates.iter().map(aggregate_value).collect()))
            }
            "sticky" => Ok(CapabilityValue::Bool(self.sticky)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "aggregates" => {
                self.set_aggregates(expect_aggregates(value)?);
                Ok(())
            }
            "sticky" => {
                self.set_sticky(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["aggregates", "sticky", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Arc, Mutex};
    use core::sync::atomic::{AtomicUsize, Ordering};

    fn row_with_columns() -> SummaryRow {
        let mut row = SummaryRow::new(Rect::new(0, 0, 360, 32));
        row.set_aggregates(vec![
            ColumnAggregate::new("revenue", ColumnAggregateKind::Sum, "Revenue"),
            ColumnAggregate::new("orders", ColumnAggregateKind::Count, "Orders"),
            ColumnAggregate::new("margin", ColumnAggregateKind::Average, "Margin"),
        ]);
        row
    }

    #[test]
    fn set_value_updates_the_painted_and_queried_value() {
        let mut row = row_with_columns();
        assert!(row.set_value("revenue", 1250.0));
        assert_eq!(row.value("revenue"), Some(1250.0));
        assert_eq!(row.cell_text("revenue").as_deref(), Some("Revenue: 1250"));

        // A second write replaces the first rather than appending a duplicate.
        assert!(row.set_value("revenue", 1300.5));
        assert_eq!(row.value("revenue"), Some(1300.5));
        assert_eq!(row.values().len(), 1);
        assert_eq!(row.cell_text("revenue").as_deref(), Some("Revenue: 1300.50"));
    }

    #[test]
    fn column_rect_maps_tokens_to_the_painted_cells() {
        let row = row_with_columns();
        let first = row.column_rect("revenue").expect("the first cell exists");
        let second = row.column_rect("orders").expect("the second cell exists");
        let third = row.column_rect("margin").expect("the third cell exists");

        // Three equal 120-pixel cells, inset by four pixels on every side.
        assert_eq!(first, Rect::new(4, 4, 112, 24));
        assert_eq!(second, Rect::new(124, 4, 112, 24));
        assert_eq!(third, Rect::new(244, 4, 112, 24));
        assert_eq!(row.column_rect("missing"), None);
    }

    #[test]
    fn an_unknown_token_is_ignored_by_set_value() {
        let mut row = row_with_columns();
        assert!(!row.set_value("missing", 9.0));
        assert_eq!(row.value("missing"), None);
        assert!(row.values().is_empty());
        assert!(!row.set_value("revenue", f64::NAN));
        assert_eq!(row.value("revenue"), None);
    }

    #[test]
    fn update_from_source_emits_once_and_drops_stale_values() {
        let mut row = row_with_columns();
        row.set_value("revenue", 10.0);
        row.set_value("orders", 2.0);

        let emissions = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&emissions);
        row.summary_recalculated.connect(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        row.set_aggregates(vec![ColumnAggregate::new(
            "revenue",
            ColumnAggregateKind::Sum,
            "Revenue",
        )]);
        row.update_from_source();
        assert_eq!(emissions.load(Ordering::SeqCst), 1);
        assert_eq!(row.value("revenue"), Some(10.0));
        assert_eq!(row.value("orders"), None);
    }

    #[test]
    fn aggregates_round_trip_through_the_property_route() {
        let mut row = SummaryRow::new(Rect::new(0, 0, 360, 32));
        let value = CapabilityValue::Tuple(vec![CapabilityValue::Tuple(vec![
            CapabilityValue::String("revenue".to_string()),
            CapabilityValue::String("average".to_string()),
            CapabilityValue::String("Mean revenue".to_string()),
        ])]);
        row.set("aggregates", value).expect("an aggregate list is writable");
        assert_eq!(row.aggregates().len(), 1);
        assert_eq!(row.aggregates()[0].kind, ColumnAggregateKind::Average);
        assert_eq!(
            row.get("aggregates").expect("readable"),
            CapabilityValue::Tuple(vec![CapabilityValue::Tuple(vec![
                CapabilityValue::String("revenue".to_string()),
                CapabilityValue::String("average".to_string()),
                CapabilityValue::String("Mean revenue".to_string()),
            ])])
        );

        row.set("sticky", CapabilityValue::Bool(true)).expect("sticky is writable");
        assert!(row.sticky());

        // The query-only flag must not be mistaken for a paint input.
        let seen = Arc::new(Mutex::new(Vec::<u8>::new()));
        let sink = Arc::clone(&seen);
        row.base.redraw_requested_signal().connect(move || {
            lock(&sink).push(1);
        });
        row.set_sticky(false);
        assert!(lock(&seen).is_empty());
    }
}
