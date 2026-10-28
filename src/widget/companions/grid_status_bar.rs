// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `GridStatusBar` — the shared count/filter strip for table-family controls (COMP-A23).
//!
//! # Why a companion
//!
//! `DataGrid`, the special-purpose `GridWidget` and the finance tables each painted their own
//! "N rows, M selected" line. The wording, the filter indicator and the click target therefore
//! differed per control. This type owns that strip once: a host sets the two counts and the
//! filter flag, and the control paints and reports activation.
//!
//! `status_clicked` is a unit signal: the status text is a summary, not a per-cell hit target, so
//! a subscriber that wants to open a filter editor or a selection dialog gets one notification
//! and can decide what the click means.

use crate::compat::{format, lock, Arc, Mutex, String, ToString};
use crate::core::{HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// Horizontal inset around the text.
const STATUS_INSET: u32 = 8;

/// A one-line summary of a table's row and selection counts.
pub struct GridStatusBar {
    base: BaseWidget,
    /// The row and selection counts, shared with a bound table.
    ///
    /// A grid attaches with `&GridStatusBar`, so the counts live behind a lock: the grid writes
    /// the same cell this control reads, which is what lets `bar.row_count()` answer immediately
    /// after a grid push instead of one frame later.
    counts: Arc<Mutex<(u64, u64)>>,
    filter_active: bool,
    /// Optional caller-supplied text, shown instead of the generated summary.
    text: String,
    /// Emitted when the strip is activated by a click, Enter or Space.
    pub status_clicked: GenericSignal,
}

impl GridStatusBar {
    /// Creates an empty status bar: zero rows, no selection, no filter.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StatusBar, geometry, "GridStatusBar"),
            counts: Arc::new(Mutex::new((0, 0))),
            filter_active: false,
            text: String::new(),
            status_clicked: GenericSignal::new(),
        }
    }

    /// The number of rows the grid reports.
    pub fn row_count(&self) -> u64 {
        lock(&self.counts).0
    }

    /// The number of selected rows.
    pub fn selected_count(&self) -> u64 {
        lock(&self.counts).1
    }

    /// The shared count cell a bound table writes, plus the signal that repaints this strip.
    ///
    /// A table attaches with `&GridStatusBar` and cannot call [`Self::set_counts`], which needs
    /// `&mut self`. This is the writable half: the cell is the same storage the accessors read, and
    /// the returned signal is the strip's own redraw request, so a write becomes visible without
    /// the host forwarding a frame. The pair is `(rows, selected)`; callers clamp `selected` to
    /// `rows` the way [`Self::set_counts`] does.
    pub fn counts_handle(&self) -> (Arc<Mutex<(u64, u64)>>, GenericSignal) {
        (Arc::clone(&self.counts), self.base.redraw_requested_signal().clone())
    }

    /// Whether the grid has an active filter.
    pub fn filter_active(&self) -> bool {
        self.filter_active
    }

    /// The explicit text, or `None` when the summary is generated.
    pub fn text(&self) -> Option<&str> {
        if self.text.is_empty() {
            None
        } else {
            Some(&self.text)
        }
    }

    /// Records both counts in one call, which is how a grid refreshes the strip.
    pub fn set_counts(&mut self, rows: u64, selected: u64) {
        let selected = selected.min(rows);
        {
            let mut counts = lock(&self.counts);
            if *counts == (rows, selected) {
                return;
            }
            *counts = (rows, selected);
        }
        self.base.request_redraw();
    }

    /// Shows or hides the filter indicator.
    pub fn set_filter_active(&mut self, active: bool) {
        if self.filter_active == active {
            return;
        }
        self.filter_active = active;
        self.base.request_redraw();
    }

    /// Replaces the summary text. An empty string restores the generated summary.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
    }

    /// The text the strip paints: the explicit string, or the generated summary.
    pub fn summary(&self) -> String {
        if !self.text.is_empty() {
            return self.text.clone();
        }
        let (row_count, selected_count) = *lock(&self.counts);
        let rows = if row_count == 1 { "1 row".to_string() } else { format!("{row_count} rows") };
        let selection = if selected_count == 0 {
            String::new()
        } else {
            format!(", {selected_count} selected")
        };
        let filter = if self.filter_active { ", filtered" } else { "" };
        format!("{rows}{selection}{filter}")
    }
}

impl Draw for GridStatusBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        let summary = self.summary();
        if summary.is_empty() {
            return;
        }
        let text_rect = Rect::new(
            rect.x + STATUS_INSET as i32,
            rect.y,
            rect.width.saturating_sub(STATUS_INSET * 2),
            rect.height,
        );
        let color = if self.filter_active { accent_color() } else { surface.blend(&ink, 0.7) };
        context.draw_text_fitted_cow(text_rect, &summary, font, color, HorizontalAlignment::Left);
    }
}

impl EventHandler for GridStatusBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseRelease { pos, button: 1, .. } if self.geometry().contains_point(*pos) => {
                self.status_clicked.emit();
            }
            Event::KeyPress { key, .. } if *key == 13 || *key == 32 => {
                self.status_clicked.emit();
            }
            _ => {}
        }
    }
}

impl Widget for GridStatusBar {
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
        Size::new(240, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "status_clicked" => Some(EventSignalRef::unit("status_clicked", &self.status_clicked)),
            _ => None,
        }
    }
}

impl WidgetProperties for GridStatusBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "row_count" => Ok(CapabilityValue::UInt(self.row_count())),
            "selected_count" => Ok(CapabilityValue::UInt(self.selected_count())),
            "filter_active" => Ok(CapabilityValue::Bool(self.filter_active)),
            "text" => Ok(CapabilityValue::String(self.text.clone())),
            "summary" => Ok(CapabilityValue::String(self.summary())),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "row_count" => {
                let rows = expect_usize(value)? as u64;
                self.set_counts(rows, self.selected_count());
                Ok(())
            }
            "selected_count" => {
                let selected = expect_usize(value)? as u64;
                self.set_counts(self.row_count(), selected);
                Ok(())
            }
            "filter_active" => {
                self.set_filter_active(expect_bool(value)?);
                Ok(())
            }
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "summary" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "row_count",
            "selected_count",
            "filter_active",
            "text",
            "summary",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use crate::core::Point;

    #[test]
    fn the_generated_summary_reports_rows_selection_and_filter() {
        let mut bar = GridStatusBar::new(Rect::new(0, 0, 240, 28));
        assert_eq!(bar.summary(), "0 rows");
        bar.set_counts(12, 3);
        assert_eq!(bar.summary(), "12 rows, 3 selected");
        bar.set_filter_active(true);
        assert_eq!(bar.summary(), "12 rows, 3 selected, filtered");
    }

    #[test]
    fn explicit_text_wins_over_the_generated_summary() {
        let mut bar = GridStatusBar::new(Rect::new(0, 0, 240, 28));
        bar.set_counts(5, 0);
        bar.set_text("Loading…");
        assert_eq!(bar.summary(), "Loading…");
        bar.set_text("");
        assert_eq!(bar.summary(), "5 rows");
    }

    #[test]
    fn a_click_inside_the_strip_emits_status_clicked() {
        use crate::compat::Arc;
        use core::sync::atomic::{AtomicU32, Ordering};
        let mut bar = GridStatusBar::new(Rect::new(0, 0, 240, 28));
        let clicks = Arc::new(AtomicU32::new(0));
        let counter = clicks.clone();
        bar.status_clicked.connect(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        bar.handle_event(&Event::MouseRelease { pos: Point::new(4, 4), button: 1 });
        assert_eq!(clicks.load(Ordering::SeqCst), 1);
        bar.handle_event(&Event::MouseRelease { pos: Point::new(400, 400), button: 1 });
        assert_eq!(clicks.load(Ordering::SeqCst), 1, "an outside release is not a click");
    }

    #[test]
    fn the_selection_count_is_clamped_to_the_row_count() {
        let mut bar = GridStatusBar::new(Rect::new(0, 0, 240, 28));
        bar.set_counts(2, 9);
        assert_eq!(bar.selected_count(), 2);
    }

    #[test]
    fn the_property_contract_round_trips() {
        let mut bar = GridStatusBar::new(Rect::new(0, 0, 240, 28));
        bar.set("row_count", CapabilityValue::UInt(7)).expect("writable");
        bar.set("selected_count", CapabilityValue::UInt(2)).expect("writable");
        bar.set("filter_active", CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(bar.get("row_count"), Ok(CapabilityValue::UInt(7)));
        assert_eq!(bar.get("selected_count"), Ok(CapabilityValue::UInt(2)));
        assert_eq!(bar.get("filter_active"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(
            bar.get("summary"),
            Ok(CapabilityValue::String("7 rows, 2 selected, filtered".to_string()))
        );
        let _ = Vec::<u8>::new();
    }
}
