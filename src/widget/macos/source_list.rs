// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MacSourceList` — a sectioned sidebar with single-row selection.
//!
//! A source list is the sidebar a desktop application puts its navigation in: headed sections,
//! one row per destination, a single highlighted row. The rows are addressed by a flat index so
//! the same number answers both the click and the `selected_row` property, which keeps the hit
//! test, the paint pass and the published contract from disagreeing about "which row".

use crate::compat::Cow;
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{profile_metrics, role_font, ControlMetricsProfile, FOCUS_RING_WIDTH};
use crate::widget::text_utils::elide_to_width;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::{
    palette, MAC_SOURCE_LIST_DEFAULT_HEIGHT, MAC_SOURCE_LIST_DEFAULT_WIDTH,
    MAC_SOURCE_LIST_ROW_HEIGHT, MAC_SOURCE_LIST_SECTION_HEIGHT,
};

/// The horizontal inset of a section header's caption.
const SECTION_HEADER_INSET: i32 = 12;

/// The horizontal inset of a row's caption, which sits below the header's level.
const ROW_LABEL_INSET: i32 = 20;

/// The inset of the selection highlight from the sidebar's own edges.
const SELECTION_INSET: i32 = 4;

/// The width of the vertical scrollbar drawn when the content overflows.
const SCROLLBAR_WIDTH: i32 = 8;

/// The shortest a scrollbar thumb may be, so a very long list still has a grabbable target.
const SCROLLBAR_MIN_THUMB: u32 = 24;

/// One row of a source list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceListRow {
    /// The stable identifier the host recognises the row by.
    pub id: String,
    /// The caption shown in the row.
    pub label: String,
    /// The sidebar's signature leading icon.
    pub icon: Option<IconName>,
    /// A count shown in a trailing capsule (unread items, children, ...).
    pub badge: Option<u32>,
    /// The badge's caption, generated once when the badge changes (L-P12).
    pub badge_text: Option<String>,
    /// A small leading colour dot, for a tag or status.
    pub accent_dot: Option<crate::core::Color>,
}

impl SourceListRow {
    /// A row with an identifier and a caption.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            badge_text: None,
            accent_dot: None,
        }
    }

    /// The same row with a leading icon.
    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The same row with a trailing badge count.
    pub fn with_badge(mut self, badge: u32) -> Self {
        self.badge = Some(badge);
        self.badge_text = Some(badge.to_string());
        self
    }

    /// The same row with a leading accent dot.
    pub fn with_accent_dot(mut self, dot: crate::core::Color) -> Self {
        self.accent_dot = Some(dot);
        self
    }
}

/// One headed group of rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceListSection {
    /// The header caption. An empty caption still occupies the header row, so a section without
    /// a title keeps the same vertical rhythm as one with a title.
    pub title: String,
    /// The rows in this section, in display order.
    pub rows: Vec<SourceListRow>,
}

impl SourceListSection {
    /// A section with a header and no rows.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), rows: Vec::new() }
    }

    /// The same section with its rows attached.
    pub fn with_rows(mut self, rows: Vec<SourceListRow>) -> Self {
        self.rows = rows;
        self
    }
}

/// A sectioned sidebar with one selected row.
///
/// Rows are addressed by a flat index that walks the sections in order, which is the same
/// index [`Self::selected_row`] reports and the `selected_row` property publishes. Selecting a
/// different row emits [`Self::selection_changed`] with that row's identifier.
pub struct MacSourceList {
    base: BaseWidget,
    sections: Vec<SourceListSection>,
    selected_row: Option<usize>,
    hovered_row: Option<usize>,
    row_height: u32,
    /// Emitted when the selected row changes, with the row's identifier.
    pub selection_changed: Signal1<String>,
    /// Emitted when a row's badge changes, with `(row id, new badge)`.
    pub badge_changed: Signal1<(String, Option<u32>)>,
    /// Emitted when the selection goes from a row to nothing (MW-20).
    pub selection_cleared: GenericSignal,
    /// Emitted with `(row id, pointer position)` on a right click on a row (MW-21).
    pub row_context_menu: Signal1<(String, Point)>,
    /// Vertical scroll offset in pixels, clamped to the content height (MW-21).
    scroll_offset: i32,
    /// An in-flight scrollbar drag: `(press y, offset at press)`.
    scroll_drag: Option<(i32, i32)>,
}

impl MacSourceList {
    /// Creates an empty source list with the desktop row height.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "MacSourceList"),
            sections: Vec::new(),
            selected_row: None,
            hovered_row: None,
            row_height: MAC_SOURCE_LIST_ROW_HEIGHT,
            selection_changed: Signal1::new(),
            badge_changed: Signal1::new(),
            selection_cleared: GenericSignal::new(),
            row_context_menu: Signal1::new(),
            scroll_offset: 0,
            scroll_drag: None,
        }
    }

    /// Replaces the sections, keeping the selection when the flat index still addresses a row.
    ///
    /// A rebuilt section list is a reset, not a selection change, so no signal is emitted.
    pub fn set_sections(&mut self, sections: Vec<SourceListSection>) {
        if self.sections == sections {
            return;
        }
        self.sections = sections;
        let rows = self.row_count();
        self.selected_row = self.selected_row.filter(|row| *row < rows);
        self.hovered_row = self.hovered_row.filter(|row| *row < rows);
        self.scroll_offset = self.scroll_offset.clamp(0, self.max_scroll());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The sections, in display order.
    pub fn sections(&self) -> &[SourceListSection] {
        &self.sections
    }

    /// The number of sections.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// The number of rows across every section.
    pub fn row_count(&self) -> usize {
        self.sections.iter().map(|section| section.rows.len()).sum()
    }

    /// The height of one row.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// The flat index of the selected row, or `None` when nothing is selected.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row.filter(|row| *row < self.row_count())
    }

    /// Returns the identifier of the selected row, or `None` when nothing is selected.
    pub fn selected_id(&self) -> Option<&str> {
        self.row_and_section(self.selected_row()?).map(|(row, _)| row.id.as_str())
    }

    /// Sets a row's badge count, returning whether it changed.
    ///
    /// The badge is drawn in a trailing capsule. Refusing an out-of-range flat index returns
    /// `false` rather than silently updating nothing; a change is announced through
    /// [`Self::badge_changed`] with the row's identifier, so a host can update its own model
    /// without re-deriving the index.
    pub fn set_badge(&mut self, flat: usize, badge: Option<u32>) -> bool {
        let Some((row, _)) = self.row_and_section(flat) else {
            return false;
        };
        if row.badge == badge {
            return false;
        }
        let id = row.id.clone();
        // The borrow above is released before the section is mutated.
        let Some(row) =
            self.sections.iter_mut().flat_map(|section| section.rows.iter_mut()).nth(flat)
        else {
            return false;
        };
        row.badge = badge;
        // The caption is generated with the badge so the paint path never formats it (L-P12).
        row.badge_text = badge.map(|count| count.to_string());
        self.base.request_redraw();
        self.badge_changed.emit((id, badge));
        true
    }

    /// Selects the row at flat index `row`, or clears the selection when `row` is `None`.
    ///
    /// Returns whether the selection changed; emits [`Self::selection_changed`] with the row's
    /// identifier, so a host can switch destinations without re-deriving the index.
    pub fn set_selected_row(&mut self, row: Option<usize>) -> bool {
        let next = row.filter(|row| *row < self.row_count());
        if next == self.selected_row() {
            return false;
        }
        self.selected_row = next;
        self.base.request_redraw();
        match self.selected_id().map(str::to_string) {
            Some(id) => self.selection_changed.emit(id),
            // A cleared selection is its own fact: `selection_changed` carries an id, so it
            // cannot express "nothing is selected" (MW-20).
            None => self.selection_cleared.emit(),
        }
        true
    }

    /// The section and row a flat index addresses, or `None` when it is out of range.
    fn row_and_section(&self, flat: usize) -> Option<(&SourceListRow, &SourceListSection)> {
        let mut remaining = flat;
        for section in &self.sections {
            if remaining < section.rows.len() {
                return section.rows.get(remaining).map(|row| (row, section));
            }
            remaining -= section.rows.len();
        }
        None
    }

    /// The total height the sections need, including one header per section.
    ///
    /// A host that scrolls the sidebar, or sizes it to its content, asks here rather than
    /// re-deriving the row and header heights from the constants.
    pub fn content_height(&self) -> u32 {
        self.sections.len() as u32 * MAC_SOURCE_LIST_SECTION_HEIGHT
            + self.row_count() as u32 * self.row_height
    }

    /// The largest valid scroll offset: the content height beyond the visible box.
    fn max_scroll(&self) -> i32 {
        (self.content_height().saturating_sub(self.geometry().height)) as i32
    }

    /// The current vertical scroll offset.
    pub fn scroll_offset(&self) -> i32 {
        self.scroll_offset
    }

    /// Scrolls the list by `offset` pixels, clamped into the content (MW-21).
    pub fn set_scroll_offset(&mut self, offset: i32) {
        let clamped = offset.clamp(0, self.max_scroll());
        if clamped == self.scroll_offset {
            return;
        }
        self.scroll_offset = clamped;
        self.base.request_redraw();
    }

    /// The scrollbar's track, or `None` when the content fits.
    fn scrollbar_track(&self) -> Option<Rect> {
        let max = self.max_scroll();
        if max <= 0 {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(rect.right() - SCROLLBAR_WIDTH, rect.y, SCROLLBAR_WIDTH as u32, rect.height))
    }

    /// The scrollbar thumb, derived from the offset and the visible fraction.
    fn scrollbar_thumb(&self) -> Option<Rect> {
        let track = self.scrollbar_track()?;
        let content = self.content_height().max(1);
        let visible = self.geometry().height.min(content);
        let height = ((track.height as u64 * visible as u64) / content as u64)
            .max(SCROLLBAR_MIN_THUMB as u64) as u32;
        let travel = track.height.saturating_sub(height) as i32;
        let max = self.max_scroll().max(1);
        let y = track.y + (travel as i64 * self.scroll_offset as i64 / max as i64) as i32;
        Some(Rect::new(track.x, y, track.width, height.min(track.height)))
    }

    /// The rectangle of the row at flat index `flat`, or `None` when it is out of range.
    fn row_rect(&self, flat: usize) -> Option<Rect> {
        let rect = self.geometry();
        if flat >= self.row_count() {
            return None;
        }
        let mut y = rect.y - self.scroll_offset;
        let mut remaining = flat;
        for section in &self.sections {
            y += MAC_SOURCE_LIST_SECTION_HEIGHT as i32;
            if remaining < section.rows.len() {
                return Some(Rect::new(
                    rect.x,
                    y + remaining as i32 * self.row_height as i32,
                    rect.width,
                    self.row_height,
                ));
            }
            remaining -= section.rows.len();
            y += section.rows.len() as i32 * self.row_height as i32;
        }
        None
    }

    /// The rectangle of section `index`'s header, or `None` when it is out of range.
    fn header_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.sections.len() {
            return None;
        }
        let rect = self.geometry();
        let mut y = rect.y - self.scroll_offset;
        for section in self.sections.iter().take(index) {
            y += MAC_SOURCE_LIST_SECTION_HEIGHT as i32
                + section.rows.len() as i32 * self.row_height as i32;
        }
        Some(Rect::new(rect.x, y, rect.width, MAC_SOURCE_LIST_SECTION_HEIGHT))
    }

    /// The corner radius of the selection highlight, taken from the desktop profile's button
    /// radius so the sidebar and the other controls in this module share one shape token.
    fn selection_radius(&self) -> u32 {
        profile_metrics(ControlMetricsProfile::Macos).button_radius
    }

    /// The row under `pos`, if any.
    fn row_at(&self, pos: Point) -> Option<usize> {
        (0..self.row_count())
            .find(|row| self.row_rect(*row).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// Moves the selection by `delta` rows, clamped to the row list.
    fn move_selection(&mut self, delta: isize) {
        let count = self.row_count();
        if count == 0 {
            return;
        }
        let current = self.selected_row().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, count as isize - 1) as usize;
        self.set_selected_row(Some(next));
    }
}

impl Widget for MacSourceList {
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
        Size::new(MAC_SOURCE_LIST_DEFAULT_WIDTH, MAC_SOURCE_LIST_DEFAULT_HEIGHT)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::ListView
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves a published event name to the control's own signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "selection_changed" => Some(EventSignalRef::mapped(
                "selection_changed",
                &self.selection_changed,
                |value| CapabilityValue::String(value.clone()),
            )),
            "badge_changed" => {
                Some(EventSignalRef::mapped("badge_changed", &self.badge_changed, |value| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(value.0.clone()),
                        match value.1 {
                            Some(badge) => CapabilityValue::UInt(badge as u64),
                            None => CapabilityValue::Null,
                        },
                    ]))
                }))
            }
            "selection_cleared" => {
                Some(EventSignalRef::unit("selection_cleared", &self.selection_cleared))
            }
            "row_context_menu" => {
                Some(EventSignalRef::mapped(
                    "row_context_menu",
                    &self.row_context_menu,
                    // `(row id, pointer)` travels as two strings: the point has no carrier of its
                    // own, so it is delivered in the same `x,y` spelling the geometry properties use.
                    |(id, pos)| {
                        CapabilityValue::Tuple(crate::compat::Vec::from([
                            CapabilityValue::String(id.clone()),
                            CapabilityValue::String(format!("{},{}", pos.x, pos.y)),
                        ]))
                    },
                ))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MacSourceList {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "section_count" => Ok(CapabilityValue::UInt(self.section_count() as u64)),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "selected_row" => match self.selected_row() {
                Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_row" => match value {
                CapabilityValue::Null => {
                    self.set_selected_row(None);
                    Ok(())
                }
                other => {
                    let row = expect_usize(other)?;
                    if row >= self.row_count() {
                        return Err(CapabilityAccessError::OutOfRange);
                    }
                    self.set_selected_row(Some(row));
                    Ok(())
                }
            },
            // Derived from the section list, which owns them.
            "section_count" | "row_count" | "row_height" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "section_count",
            "row_count",
            "row_height",
            "selected_row",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for MacSourceList {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let colors = palette();
        let style = self.base.style();
        let header_font = role_font(style, 11.0);
        let row_font = role_font(style, 12.0);
        let ink = if self.base.is_enabled() { colors.ink } else { colors.muted };

        // The sidebar's own surface, plus the hairline that separates it from the content pane.
        context.fill_rect(rect, colors.surface);
        context.draw_line(
            Point::new(rect.right() - 1, rect.y),
            Point::new(rect.right() - 1, rect.bottom() - 1),
            colors.separator,
        );

        // Everything below is clipped to the sidebar: a long list must not paint over its
        // neighbour, and a caller that sizes the control smaller than its content gets a
        // cropped list rather than an overflowing one.
        context.push_clip(rect.x, rect.y, rect.width, rect.height);

        for (index, section) in self.sections.iter().enumerate() {
            let Some(header) = self.header_rect(index) else {
                break;
            };
            // Headers and rows are stacked in increasing y, so the first one below the sidebar
            // ends the pass: emitting a command for a caption that can only be clipped away is
            // wasted work, and it would also be a coordinate outside the control's own box.
            if header.y >= rect.bottom() {
                break;
            }
            if !section.title.is_empty() {
                let available = header.width.saturating_sub((SECTION_HEADER_INSET as u32) + 4);
                let mut measure = |text: &str| {
                    crate::widget::base_widgets::measure_cache::measure_text(
                        context,
                        text,
                        &header_font,
                    )
                    .width
                };
                let shown: Cow<'_, str> = elide_to_width(&section.title, available, &mut measure);
                let line = context.text_line(header, &header_font);
                context.draw_text(
                    Point::new(header.x + SECTION_HEADER_INSET, line.y),
                    shown.as_ref(),
                    &header_font,
                    colors.muted,
                    HorizontalAlignment::Left,
                );
            }
        }

        // Rows are painted in one pass so a selection highlight can never cover a label that
        // was drawn before it: every fill first, then every caption.
        for flat in 0..self.row_count() {
            let Some(row) = self.row_rect(flat) else {
                break;
            };
            if row.y >= rect.bottom() {
                break;
            }
            let selected = self.selected_row() == Some(flat);
            if selected {
                let inset = SELECTION_INSET.min(row.width as i32 / 2);
                let highlight = Rect::new(
                    row.x + inset,
                    row.y,
                    row.width.saturating_sub((2 * inset) as u32),
                    row.height,
                );
                context.fill_rounded_rect(highlight, self.selection_radius(), colors.accent);
            } else if self.hovered_row == Some(flat) {
                context.fill_rect(row, colors.raised);
            }
        }

        for flat in 0..self.row_count() {
            let Some(row) = self.row_rect(flat) else {
                break;
            };
            if row.y >= rect.bottom() {
                break;
            }
            let Some((entry, _)) = self.row_and_section(flat) else {
                continue;
            };
            let mut ink_x = row.x + ROW_LABEL_INSET;
            if let Some(dot) = entry.accent_dot {
                context.fill_circle(Point::new(row.x + 8, row.y + row.height as i32 / 2), 3, dot);
                ink_x += 10;
            }
            if let Some(icon) = entry.icon {
                let icon_side = 14u32;
                let icon_y = row.y + (row.height.saturating_sub(icon_side) / 2) as i32;
                let icon_ink = if self.selected_row() == Some(flat) {
                    colors.accent.contrast_color()
                } else {
                    ink
                };
                crate::widget::draw_icon_at(
                    context,
                    Rect::new(ink_x, icon_y, icon_side, icon_side),
                    icon_ink,
                    icon,
                );
                ink_x += icon_side as i32 + 4;
            }

            // A badge is a trailing capsule; its caption was generated when the badge was set
            // (L-P12), and its measurement is reused for the capsule and the ink.
            let badge_text = entry.badge_text.as_deref();
            let badge_metrics = badge_text.map(|text| {
                crate::widget::base_widgets::measure_cache::measure_text(context, text, &row_font)
            });
            let mut badge_width = 0u32;
            if let Some(metrics) = badge_metrics.as_ref() {
                badge_width = metrics.width.min(row.width.saturating_sub(8)) + 10;
            }
            let reserved = (ink_x - row.x) as u32 + badge_width + 8;
            let available = row.width.saturating_sub(reserved);
            if available == 0 {
                continue;
            }
            let mut measure = |text: &str| {
                crate::widget::base_widgets::measure_cache::measure_text(context, text, &row_font)
                    .width
            };
            let shown: Cow<'_, str> = elide_to_width(&entry.label, available, &mut measure);
            let label_ink = if self.selected_row() == Some(flat) {
                colors.accent.contrast_color()
            } else {
                ink
            };
            let line = context.text_line(row, &row_font);
            context.draw_text(
                Point::new(ink_x, line.y),
                shown.as_ref(),
                &row_font,
                label_ink,
                HorizontalAlignment::Left,
            );

            if badge_width > 0 {
                let Some(metrics) = badge_metrics.as_ref() else {
                    continue;
                };
                let Some(badge_text) = badge_text else {
                    continue;
                };
                let badge_height = line.height.min(row.height.saturating_sub(6)).max(12);
                let badge_rect = Rect::new(
                    row.right() - badge_width as i32 - 6,
                    row.y + (row.height.saturating_sub(badge_height) / 2) as i32,
                    badge_width,
                    badge_height,
                );
                let badge_fill = if self.selected_row() == Some(flat) {
                    colors.accent.contrast_color().with_alpha(45)
                } else {
                    colors.accent
                };
                context.fill_rounded_rect(badge_rect, badge_height / 2, badge_fill);
                context.draw_text(
                    Point::new(
                        badge_rect.x + (badge_rect.width as i32 - metrics.width as i32) / 2,
                        badge_rect.y + (badge_rect.height as i32 - metrics.height as i32) / 2,
                    ),
                    badge_text,
                    &row_font,
                    badge_fill.contrast_color(),
                    HorizontalAlignment::Left,
                );
            }
        }

        context.pop_clip();

        // ── Scrollbar ──
        // A track on the trailing edge and a thumb sized to the visible fraction. The thumb is
        // also the drag handle (MW-21).
        if let (Some(track), Some(thumb)) = (self.scrollbar_track(), self.scrollbar_thumb()) {
            context.fill_rect(track, colors.raised.with_alpha(80));
            context.fill_rounded_rect(
                thumb,
                (thumb.width / 2).max(1),
                colors.muted.with_alpha(150),
            );
        }

        // The focus ring is drawn on the selected row, which is the sidebar's focus indicator.
        if self.base.draws_focus_ring() {
            if let Some(row) = self.selected_row().and_then(|flat| self.row_rect(flat)) {
                let inset = SELECTION_INSET.min(row.width as i32 / 2);
                let highlight = Rect::new(
                    row.x + inset,
                    row.y,
                    row.width.saturating_sub((2 * inset) as u32),
                    row.height,
                );
                context.draw_rounded_rect_stroke(
                    highlight,
                    self.selection_radius(),
                    colors.accent,
                    FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

impl EventHandler for MacSourceList {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                let hovered = self.row_at(*pos);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                // A scrollbar drag takes precedence over row selection.
                if let (Some(track), Some(thumb)) = (self.scrollbar_track(), self.scrollbar_thumb())
                {
                    if thumb.contains_point(*pos) || track.contains_point(*pos) {
                        if !thumb.contains_point(*pos) {
                            // A click on the track jumps a page toward the click.
                            let page = self.geometry().height as i32;
                            let direction = if pos.y < thumb.y { -page } else { page };
                            self.set_scroll_offset(self.scroll_offset + direction);
                        }
                        self.scroll_drag = Some((pos.y, self.scroll_offset));
                        return;
                    }
                }
                if let Some(row) = self.row_at(*pos) {
                    self.set_selected_row(Some(row));
                }
            }
            // A secondary click reports the row it names without changing the selection: a
            // context menu acts on the row it was opened from (MW-21). The runtime delivers
            // `ContextMenu` in addition to the button press, so only this variant is handled
            // to avoid emitting the same request twice.
            Event::ContextMenu { pos } => {
                if let Some(row) = self.row_at(*pos) {
                    if let Some((entry, _)) = self.row_and_section(row) {
                        let id = entry.id.clone();
                        self.row_context_menu.emit((id, *pos));
                        return;
                    }
                }
                self.base.handle_event(event);
            }
            Event::Drag { pos, .. } => {
                if let Some((start_y, start_offset)) = self.scroll_drag {
                    let drag_distance = pos.y - start_y;
                    if let (Some(track), Some(thumb)) =
                        (self.scrollbar_track(), self.scrollbar_thumb())
                    {
                        let travel = track.height.saturating_sub(thumb.height).max(1) as i32;
                        let max = self.max_scroll().max(1);
                        let next = start_offset + drag_distance * max / travel;
                        self.set_scroll_offset(next);
                    }
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.scroll_drag = None;
            }
            Event::Wheel { delta, .. } => {
                // One notch scrolls three rows, the conventional line step.
                let step = self.row_height as i32 * 3;
                self.set_scroll_offset(self.scroll_offset - delta.y * step);
            }
            Event::KeyPress { key, .. } => match crate::shortcut::Key::from_key_code(*key) {
                Some(crate::shortcut::Key::Up) => self.move_selection(-1),
                Some(crate::shortcut::Key::Down) => self.move_selection(1),
                Some(crate::shortcut::Key::Home) => {
                    if self.row_count() > 0 {
                        self.set_selected_row(Some(0));
                    }
                }
                Some(crate::shortcut::Key::End) if self.row_count() > 0 => {
                    self.set_selected_row(Some(self.row_count() - 1));
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Recorder;
    use super::*;
    use crate::core::Color;
    use crate::render::RenderCommand;
    use std::sync::{Arc, Mutex};

    fn sample() -> MacSourceList {
        let mut list = MacSourceList::new(Rect::new(0, 0, 220, 240));
        list.set_sections(vec![
            SourceListSection::new("Favorites").with_rows(vec![
                SourceListRow::new("inbox", "Inbox"),
                SourceListRow::new("drafts", "Drafts"),
            ]),
            SourceListSection::new("Tags").with_rows(vec![SourceListRow::new("today", "Today")]),
        ]);
        list
    }

    #[test]
    fn a_fresh_list_has_no_sections_and_no_selection() {
        let list = MacSourceList::new(Rect::new(0, 0, 220, 240));
        assert_eq!(list.section_count(), 0);
        assert_eq!(list.row_count(), 0);
        assert!(list.selected_row().is_none());
        assert!(list.selected_id().is_none());
        assert_eq!(list.size_hint(), Size::new(220, 240));
    }

    #[test]
    fn the_row_count_spans_every_section() {
        let list = sample();
        assert_eq!(list.section_count(), 2);
        assert_eq!(list.row_count(), 3);
        assert_eq!(list.row_height(), MAC_SOURCE_LIST_ROW_HEIGHT);
    }

    #[test]
    fn rows_are_stacked_below_their_section_header() {
        let list = sample();
        let header = list.header_rect(0).expect("the first header exists");
        let first = list.row_rect(0).expect("the first row exists");
        let second = list.row_rect(1).expect("the second row exists");
        assert_eq!(first.y, header.bottom());
        assert_eq!(second.y, first.bottom());
        // The second section's header follows the first section's rows.
        let second_header = list.header_rect(1).expect("the second header exists");
        assert_eq!(second_header.y, second.bottom());
    }

    #[test]
    fn clicking_a_row_selects_it_and_emits_its_id() {
        let mut list = sample();
        let seen = Arc::new(Mutex::new(String::new()));
        list.selection_changed.connect({
            let seen = Arc::clone(&seen);
            move |id: Arc<String>| {
                *seen.lock().unwrap() = (*id).clone();
            }
        });
        let row = list.row_rect(2).expect("the third row exists");
        list.handle_event(&Event::MousePress { pos: row.center(), button: 1, modifiers: 0 });
        assert_eq!(list.selected_row(), Some(2));
        assert_eq!(list.selected_id(), Some("today"));
        assert_eq!(seen.lock().unwrap().as_str(), "today");
    }

    #[test]
    fn reselecting_the_same_row_is_not_a_change() {
        let mut list = sample();
        assert!(list.set_selected_row(Some(1)));
        assert!(!list.set_selected_row(Some(1)));
        // Clearing is a change, and a cleared list reports no id.
        assert!(list.set_selected_row(None));
        assert!(list.selected_id().is_none());
    }

    #[test]
    fn a_smaller_section_set_drops_a_selection_that_no_longer_exists() {
        let mut list = sample();
        list.set_selected_row(Some(2));
        list.set_sections(vec![
            SourceListSection::new("Only").with_rows(vec![SourceListRow::new("one", "One")])
        ]);
        assert!(list.selected_row().is_none());
    }

    #[test]
    fn the_property_route_reads_and_writes_every_published_name() {
        let mut list = sample();
        assert_eq!(list.get("section_count"), Ok(CapabilityValue::UInt(2)));
        assert_eq!(list.get("row_count"), Ok(CapabilityValue::UInt(3)));
        assert_eq!(
            list.get("row_height"),
            Ok(CapabilityValue::UInt(MAC_SOURCE_LIST_ROW_HEIGHT as u64))
        );
        assert_eq!(list.get("selected_row"), Ok(CapabilityValue::Null));
        list.set("selected_row", CapabilityValue::UInt(1)).unwrap();
        assert_eq!(list.get("selected_row"), Ok(CapabilityValue::UInt(1)));
        assert_eq!(
            list.set("selected_row", CapabilityValue::UInt(9)),
            Err(CapabilityAccessError::OutOfRange)
        );
        assert_eq!(
            list.set("row_count", CapabilityValue::UInt(0)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn the_sidebar_paints_its_surface_and_every_caption() {
        let mut list = sample();
        let mut recorder = Recorder::new(220, 240);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            list.draw(&mut context);
        }
        recorder.end_frame();

        let captions: Vec<&str> = recorder
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::DrawText { text, .. } => Some(text.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(captions, vec!["Favorites", "Tags", "Inbox", "Drafts", "Today"]);
    }

    #[test]
    fn a_selected_row_is_filled_before_its_caption_is_painted() {
        let mut list = sample();
        list.set_selected_row(Some(0));
        let mut recorder = Recorder::new(220, 240);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            list.draw(&mut context);
        }
        recorder.end_frame();

        let fill_at = recorder
            .commands
            .iter()
            .position(|command| matches!(command, RenderCommand::FillRoundedRect { .. }));
        let label_at = recorder.commands.iter().position(|command| {
            matches!(command, RenderCommand::DrawText { text, .. } if text.as_ref() == "Inbox")
        });
        assert!(fill_at.is_some(), "the selected row is highlighted");
        assert!(fill_at.unwrap() < label_at.unwrap(), "the caption stays legible");
    }

    #[test]
    fn rows_below_the_sidebar_are_not_painted() {
        // One header plus exactly one row fits in 46 points; the second row starts at the
        // sidebar's bottom edge and must not be emitted at all.
        let mut list = MacSourceList::new(Rect::new(0, 0, 220, 46));
        list.set_sections(vec![SourceListSection::new("S")
            .with_rows(vec![SourceListRow::new("a", "Alpha"), SourceListRow::new("b", "Beta")])]);

        let mut recorder = Recorder::new(220, 46);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            list.draw(&mut context);
        }
        recorder.end_frame();

        let captions: Vec<&str> = recorder
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::DrawText { text, .. } => Some(text.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(captions, vec!["S", "Alpha"]);
    }

    #[test]
    fn every_painted_caption_stays_inside_the_sidebar() {
        let mut list = sample();
        let mut recorder = Recorder::new(220, 240);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            list.draw(&mut context);
        }
        recorder.end_frame();
        for command in &recorder.commands {
            if let RenderCommand::DrawText { origin, .. } = command {
                assert!(origin.x >= 0 && origin.x < 220, "captions start inside the sidebar");
                assert!(origin.y >= 0 && origin.y < 240, "captions stay inside the sidebar");
            }
        }
    }

    #[test]
    fn rows_carry_icons_badges_and_accent_dots() {
        let mut list = MacSourceList::new(Rect::new(0, 0, 220, 240));
        list.set_sections(vec![SourceListSection::new("Mail").with_rows(vec![
            SourceListRow::new("inbox", "Inbox")
                .with_icon(IconName::Check)
                .with_badge(12)
                .with_accent_dot(crate::core::Color::rgb(255, 0, 0)),
            SourceListRow::new("sent", "Sent"),
        ])]);
        assert_eq!(list.sections()[0].rows[0].badge, Some(12));

        let badge = std::sync::Arc::new(std::sync::Mutex::new(None));
        let b = badge.clone();
        list.badge_changed.connect(move |value: std::sync::Arc<(String, Option<u32>)>| {
            *b.lock().unwrap() = Some((*value).clone());
        });
        assert!(list.set_badge(0, Some(7)));
        assert_eq!(*badge.lock().unwrap(), Some(("inbox".to_string(), Some(7))));
        assert!(!list.set_badge(9, Some(1)), "an unknown row is refused");

        let mut recorder = Recorder::new(220, 240);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            list.draw(&mut context);
        }
        recorder.end_frame();
        let texts: Vec<&str> = recorder
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::DrawText { text, .. } => Some(text.as_ref()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"7"), "the updated badge is painted: {texts:?}");
        assert!(texts.contains(&"Inbox"));
        assert!(list.event_signal_dyn("badge_changed").is_some());
    }
}
