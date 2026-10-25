// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `StickyHeader` — the content a pinned scroll-area band carries (COMP-E01).
//!
//! # Why this exists beside `ScrollArea`'s sticky bands
//!
//! A sticky region only ever painted a plain colour band and a bottom rule
//! (`ScrollArea::draw_sticky_band`). The behaviour — a group header that follows the
//! scroll — was there, but the header's *content* was not: a host could not put a title,
//! an icon or an action in the band and had to approximate it with a translucent stripe.
//!
//! This control is that content. It carries a title, an optional icon and an optional
//! mounted control, and it is deliberately light: a fixed-height row, one signal, and a
//! geometry derived from the band it is pinned to.
//!
//! # How it attaches
//!
//! [`StickyHeader::set_band`] records the same `(group_top, group_bottom, height)` triple
//! a [`StickyRegion`](crate::widget::container_widgets::scrollarea::StickyRegion) is built
//! from, so the control and the band cannot drift. The host registers the band on the
//! scroll area and mounts the header through
//! [`ScrollArea::set_sticky_header`](crate::widget::container_widgets::ScrollArea::set_sticky_header);
//! the area then places this control in the band rectangle it derives and draws it there.
//! A band with no header keeps the historical plain fill.
//!
//! # Widget kind
//!
//! There is no `StickyHeader` variant in `WidgetKind`, and this module may not add one
//! (the enum is the capability layer's bookkeeping). The record therefore reuses the
//! closest existing kind, a header row: `WidgetKind::KanbanColumnHeader`. That is the
//! accounting the issue asks for ("`WidgetKind` 记账沿用最近似现有类型"), and it is stated
//! here rather than left for a reader to discover.

use crate::core::{Alignment, Color, HorizontalAlignment, ObjectId, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    alignment_to_str, expect_alignment, expect_string, expect_u32,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::theme_snapshot::ThemeSnapshot;

/// The horizontal padding inside the band.
const BAND_PADDING: i32 = 8;
/// Side of the icon tile, in logical pixels.
const ICON_SIDE: u32 = 14;
/// Gap between the icon tile and the title.
const ICON_GAP: u32 = 6;

/// The content of a pinned scroll-area band.
pub struct StickyHeader {
    base: BaseWidget,
    title: String,
    /// Optional control drawn instead of the title (COMP-E01).
    content_widget: Option<ObjectId>,
    /// The band's height, kept in step with the `StickyRegion` it belongs to.
    height: u32,
    /// The content-space top of the group this header follows.
    group_top: i32,
    /// The content-space bottom of that group.
    group_bottom: i32,
    /// How the title is aligned inside the band.
    alignment: Alignment,
    /// Optional icon drawn ahead of the title.
    icon: Option<IconSource>,
    /// Emitted when the band is clicked — the gesture a host turns into "collapse this group".
    pub header_clicked: Signal1<()>,
}

impl StickyHeader {
    /// Creates a header with the given geometry, no title and no band.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::KanbanColumnHeader, geometry, "StickyHeader"),
            title: String::new(),
            content_widget: None,
            height: geometry.height,
            group_top: 0,
            group_bottom: 0,
            alignment: Alignment::Left,
            icon: None,
            header_clicked: Signal1::new(),
        }
    }

    /// The header's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the header's title and requests a redraw on a real change.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// The band this header follows: `(group_top, group_bottom, height)`.
    pub fn band(&self) -> (i32, i32, u32) {
        (self.group_top, self.group_bottom, self.height)
    }

    /// Binds the header to a scroll-area group band.
    ///
    /// The three values are exactly what
    /// [`ScrollArea::add_sticky_region`](crate::widget::container_widgets::ScrollArea::add_sticky_region)
    /// takes, so a host can pass the same triple to both and the header cannot drift from the
    /// band it names. A zero height is rejected (a band with no extent cannot be seen), leaving
    /// the previous binding untouched.
    pub fn set_band(&mut self, group_top: i32, group_bottom: i32, height: u32) {
        if height == 0 {
            return;
        }
        if self.group_top == group_top && self.group_bottom == group_bottom && self.height == height
        {
            return;
        }
        self.group_top = group_top;
        self.group_bottom = group_bottom;
        self.height = height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The control drawn in the band instead of the title, if one is mounted.
    pub fn content(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Mounts (or clears, with `None`) the control drawn in the band.
    ///
    /// The control is linked as this header's child and placed in [`Self::content_rect`], the same
    /// two-step contract `SafeArea` uses for its own content: the tree painter draws a child at its
    /// own geometry, so the placement is what makes the mount visible.
    pub fn set_content(&mut self, child: Option<ObjectId>) {
        if self.content_widget == child {
            return;
        }
        if let Some(previous) = self.content_widget {
            self.base.remove_child_linked(previous);
        }
        self.content_widget = child;
        if let Some(child) = child {
            self.base.add_child_linked(child);
        }
        self.apply();
        self.base.request_redraw();
    }

    /// The icon drawn ahead of the title.
    pub fn icon(&self) -> Option<IconSource> {
        self.icon
    }

    /// Sets (or clears) the header icon.
    pub fn set_icon(&mut self, icon: Option<IconSource>) {
        if self.icon == icon {
            return;
        }
        self.icon = icon;
        self.base.request_redraw();
    }

    /// How the title is aligned inside the band.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }

    /// Sets how the title is aligned inside the band.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// The rectangle a mounted content control occupies: the band minus its padding.
    pub fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + BAND_PADDING,
            rect.y,
            rect.width.saturating_sub(BAND_PADDING as u32 * 2),
            rect.height,
        )
    }

    /// Places the mounted content control inside the band. Idempotent and cheap.
    pub fn apply(&mut self) {
        let Some(child) = self.content_widget else {
            return;
        };
        let rect = self.content_rect();
        #[cfg(not(alloc_frugal))]
        {
            crate::set_widget_geometry(child, rect.x, rect.y, rect.width, rect.height);
            crate::set_widget_visible(child, true);
        }
        #[cfg(alloc_frugal)]
        let _ = (child, rect);
    }
}

impl Widget for StickyHeader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        // The band is what the host derives from the scroll position; a new rectangle means the
        // content has to follow it rather than stay at the previous pin.
        self.apply();
    }

    fn size_hint(&self) -> Size {
        Size::new(240, self.height.max(1))
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for StickyHeader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "height" => Ok(CapabilityValue::UInt(u64::from(self.height))),
            "group_top" => Ok(CapabilityValue::Int(i64::from(self.group_top))),
            "group_bottom" => Ok(CapabilityValue::Int(i64::from(self.group_bottom))),
            "alignment" => Ok(CapabilityValue::String(alignment_to_str(self.alignment).into())),
            "content" => Ok(match self.content_widget {
                Some(id) => CapabilityValue::UInt(id),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "height" => {
                let height = expect_u32(value)?;
                self.set_band(self.group_top, self.group_bottom, height);
                Ok(())
            }
            "group_top" => {
                let top = expect_i64_as_i32(value)?;
                self.set_band(top, self.group_bottom, self.height);
                Ok(())
            }
            "group_bottom" => {
                let bottom = expect_i64_as_i32(value)?;
                self.set_band(self.group_top, bottom, self.height);
                Ok(())
            }
            "alignment" => {
                let alignment = expect_alignment(value)?;
                self.set_alignment(alignment);
                Ok(())
            }
            // The content is an object id, which a scalar property write cannot express without a
            // second payload convention; `set_content` is the supported entry.
            "content" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "height",
            "group_top",
            "group_bottom",
            "alignment",
            "content",
            BASE_PROPERTY_NAMES
        ]
    }
}

/// Reads a property integer as the `i32` the band coordinates are stored in.
fn expect_i64_as_i32(value: CapabilityValue) -> Result<i32, CapabilityAccessError> {
    match value {
        CapabilityValue::Int(raw) => Ok(raw.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        CapabilityValue::UInt(raw) => Ok(raw.min(i32::MAX as u64) as i32),
        CapabilityValue::Float(raw) => Ok(raw.clamp(i32::MIN as f64, i32::MAX as f64) as i32),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

impl Draw for StickyHeader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // One theme read for the frame (C-P01 shape): the band fill, the ink and the rule all
        // derive from this snapshot.
        let theme = ThemeSnapshot::capture("sticky_header");
        let style = self.base.style();
        let window_fill = theme.window_fill();
        let ink = style
            .text_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A band painted in the window fill would read as a hole rather than a pinned header, so a
        // resolved surface equal to the window fill is stepped away from it — the same rule
        // `ScrollArea`'s fallback band uses.
        let fill = match style
            .background_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.10),
        };
        context.fill_rect(rect, fill);
        // The bottom rule is what separates a pinned header from the content scrolling under it.
        let rule = style
            .border_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| fill.blend(&ink, 0.25));
        context.draw_line(
            crate::core::Point::new(rect.x, rect.y + rect.height as i32 - 1),
            crate::core::Point::new(rect.x + rect.width as i32, rect.y + rect.height as i32 - 1),
            rule,
        );

        let font = effective_font(style);
        let line = context.text_line(rect, font);
        let mut text_x = rect.x + BAND_PADDING;
        let mut text_width = rect.width.saturating_sub(BAND_PADDING as u32 * 2);
        if let Some(source) = self.icon {
            let side = ICON_SIDE.min(line.height.max(1));
            let icon_rect =
                Rect::new(text_x, line.y + line.height.saturating_sub(side) as i32 / 2, side, side);
            if let Err(error) = crate::widget::paint_icon(context, source.into(), icon_rect, ink) {
                log::warn!("sticky header icon could not be painted: {error:?}");
            }
            let consumed = (ICON_SIDE + ICON_GAP) as i32;
            text_x += consumed;
            text_width = text_width.saturating_sub(consumed as u32);
        }
        if self.content_widget.is_none() && !self.title.is_empty() {
            let alignment = match self.alignment {
                Alignment::Center => HorizontalAlignment::Center,
                Alignment::Right => HorizontalAlignment::Right,
                _ => HorizontalAlignment::Left,
            };
            // A custom content control owns the band; the title is only the built-in content.
            let band = Rect::new(text_x, line.y, text_width.max(1), line.height.max(1));
            context.draw_text_fitted_cow(band, &self.title, font, ink, alignment);
        }
    }
}

impl EventHandler for StickyHeader {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if self.geometry().contains(*pos) {
                self.header_clicked.emit(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_header_is_empty_and_clickable() {
        let header = StickyHeader::new(Rect::new(0, 0, 200, 28));
        assert_eq!(header.title(), "");
        assert_eq!(header.content(), None);
        assert_eq!(header.band(), (0, 0, 28));
        assert!(header.event_signal_dyn("nothing").is_none());
    }

    #[test]
    fn set_band_records_the_triple_and_rejects_a_zero_height() {
        let mut header = StickyHeader::new(Rect::new(0, 0, 200, 28));
        header.set_band(100, 400, 30);
        assert_eq!(header.band(), (100, 400, 30));
        header.set_band(0, 0, 0);
        assert_eq!(header.band(), (100, 400, 30), "a zero-height band is not a binding");
    }

    #[test]
    fn the_alignment_property_round_trips() {
        let mut header = StickyHeader::new(Rect::new(0, 0, 200, 28));
        assert!(header.set("alignment", CapabilityValue::String("center".into())).is_ok());
        let read = header.get("alignment").expect("readable");
        assert_eq!(read, CapabilityValue::String(alignment_to_str(Alignment::Center).into()));
    }
}
