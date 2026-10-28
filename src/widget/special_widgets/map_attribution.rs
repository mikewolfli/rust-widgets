// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MapAttributionOverlay` — the source/licence strip a tile map must display (COMP-A3).
//!
//! # Why this is a separate control
//!
//! Most tile services require the map to show where its data came from ("© OpenStreetMap
//! contributors", a licence link, a data-provider name). `MapView` owns the camera, the markers
//! and the three navigation companions, but attribution is *legal* content: a host with a
//! proprietary tile source shows a different string from one using an open dataset, and an
//! offline application may show none at all. Painting it inside the map would have made every
//! host that did not want the default strip hide it again.
//!
//! The overlay owns no network state. [`Self::attach_map_view`] reads the map's
//! [`TileLoader`](crate::widget::special_widgets::map_view::TileLoader) template once and derives
//! a source entry from it; everything else is set by the host.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_f32, expect_string, expect_u8_saturating};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::special_widgets::map_view::MapView;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Which corner of its rectangle an anchored overlay sits in.
///
/// The crate has no shared corner enum: the two map overlays here need the same four positions,
/// so this type is defined once in this file and reused by
/// [`MapOverviewPanel`](super::map_overview::MapOverviewPanel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Corner {
    /// The top-left corner.
    #[default]
    TopLeft,
    /// The top-right corner.
    TopRight,
    /// The bottom-left corner.
    BottomLeft,
    /// The bottom-right corner.
    BottomRight,
}

impl Corner {
    /// The stable token spelling used by properties and tests.
    pub fn token(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }

    /// Parses a token spelling; `None` for an unknown word.
    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "top_left" | "topleft" => Some(Self::TopLeft),
            "top_right" | "topright" => Some(Self::TopRight),
            "bottom_left" | "bottomleft" => Some(Self::BottomLeft),
            "bottom_right" | "bottomright" => Some(Self::BottomRight),
            _ => None,
        }
    }

    /// Whether the corner is on the right edge.
    pub(super) fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    /// Whether the corner is on the bottom edge.
    pub(super) fn is_bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }
}

/// Resolves the surface, border and ink a map overlay paints with.
///
/// Shared with [`MapOverviewPanel`](super::map_overview::MapOverviewPanel) so the two overlays
/// cannot drift apart when the theme changes.
pub(super) fn map_overlay_palette(
    style: &crate::style::WidgetStyle,
    role: &str,
) -> (Color, Color, Color) {
    let theme = crate::style::resolved_theme_style_shared(role);
    let ink = style
        .text_color
        .or_else(|| theme.as_ref().and_then(|t| t.text_color))
        .unwrap_or(Color::rgb(45, 58, 74));
    let border = style
        .border_color
        .or_else(|| theme.as_ref().and_then(|t| t.border_color))
        .unwrap_or(Color::rgb(168, 184, 201));
    let surface = style
        .background_color
        .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        .unwrap_or(Color::rgb(235, 243, 250));
    (surface, border, ink)
}

/// One line of map attribution.
///
/// `url` is the destination an activation reports. `None` means the line is informational and
/// activation reports its `text` instead, which is what a licence notice with no link needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionEntry {
    /// Stable id, used by [`MapAttributionOverlay::remove_entry`].
    pub id: String,
    /// The visible credit text.
    pub text: String,
    /// The link to open, or `None` for a purely informational line.
    pub url: Option<String>,
}

impl AttributionEntry {
    /// Creates an entry with no link.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self { id: id.into(), text: text.into(), url: None }
    }

    /// Adds a link; an empty URL is treated as no link at all.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        let url = url.into();
        self.url = if url.trim().is_empty() { None } else { Some(url) };
        self
    }
}

/// Inset between the overlay and the edge of its own rectangle.
const OVERLAY_INSET: i32 = 8;
/// Padding inside the overlay's background panel.
const OVERLAY_PADDING: i32 = 6;
/// Vertical gap between two attribution rows.
const OVERLAY_ROW_GAP: i32 = 2;
/// The narrowest background panel, so an entry with empty text is still clickable.
const MIN_PANEL_WIDTH: i32 = 48;
/// The smallest and largest accepted font scale.
const MIN_FONT_SCALE: f32 = 0.5;
const MAX_FONT_SCALE: f32 = 3.0;
/// The id [`Self::attach_map_view`] uses for the entry derived from the tile template.
const TILE_SOURCE_ID: &str = "tile_source";

/// A corner-anchored list of map source credits.
///
/// The overlay paints only when it has entries, so a map with no attribution content costs one
/// empty check per frame.
pub struct MapAttributionOverlay {
    base: BaseWidget,
    entries: Vec<AttributionEntry>,
    corner: Corner,
    background_opacity: u8,
    font_scale: f32,
    /// The tile template the last [`Self::attach_map_view`] read.
    tile_source: Option<String>,
    /// The next auto-generated id.
    next_id: u64,
    /// Emitted with the entry's URL, or its text when the entry has no URL.
    pub on_entry_activated: Signal1<String>,
}

impl MapAttributionOverlay {
    /// Creates an empty overlay in the top-left corner with a `160/255` background.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "MapAttributionOverlay"),
            entries: Vec::new(),
            corner: Corner::TopLeft,
            background_opacity: 160,
            font_scale: 1.0,
            tile_source: None,
            next_id: 1,
            on_entry_activated: Signal1::new(),
        }
    }

    /// Replaces every entry.
    pub fn set_entries(&mut self, entries: Vec<AttributionEntry>) {
        if self.entries == entries {
            return;
        }
        self.entries = entries;
        self.base.request_redraw();
    }

    /// Returns the entries, in painting order.
    pub fn entries(&self) -> &[AttributionEntry] {
        &self.entries
    }

    /// Moves the overlay to `corner`.
    pub fn set_corner(&mut self, corner: Corner) {
        if self.corner == corner {
            return;
        }
        self.corner = corner;
        self.base.request_redraw();
    }

    /// Returns the corner the overlay is anchored to.
    pub fn corner(&self) -> Corner {
        self.corner
    }

    /// Sets the alpha applied to the background panel (`0` is invisible, `255` is opaque).
    pub fn set_background_opacity(&mut self, opacity: u8) {
        if self.background_opacity == opacity {
            return;
        }
        self.background_opacity = opacity;
        self.base.request_redraw();
    }

    /// Returns the background panel's alpha.
    pub fn background_opacity(&self) -> u8 {
        self.background_opacity
    }

    /// Scales the entry text, clamped to `0.5 ..= 3.0`.
    ///
    /// A non-finite value is ignored, matching the other scale setters in the crate.
    pub fn set_font_scale(&mut self, scale: f32) {
        if !scale.is_finite() {
            return;
        }
        let scale = scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
        if (self.font_scale - scale).abs() < f32::EPSILON {
            return;
        }
        self.font_scale = scale;
        self.base.request_redraw();
    }

    /// Returns the font scale.
    pub fn font_scale(&self) -> f32 {
        self.font_scale
    }

    /// Appends an entry and returns the id it was given.
    ///
    /// The id is generated by the overlay, so a host that wants to remove the line later keeps
    /// the return value. Pass `None` for `url` for a purely informational notice.
    pub fn add_entry(&mut self, text: impl Into<String>, url: Option<impl Into<String>>) -> String {
        let id = format!("attribution_{}", self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let url = url.map(Into::into).filter(|url| !url.trim().is_empty());
        self.entries.push(AttributionEntry { id: id.clone(), text: text.into(), url });
        self.base.request_redraw();
        id
    }

    /// Removes the entry with `id`, returning whether it was present.
    pub fn remove_entry(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id);
        let removed = self.entries.len() != before;
        if removed {
            self.base.request_redraw();
        }
        removed
    }

    /// Reads `map`'s tile template and adds a source entry for it.
    ///
    /// The template's authority becomes the credit (`https://tile.example.org/{z}/{x}/{y}.png`
    /// credits `tile.example.org` and links to `https://tile.example.org`). A template with no
    /// scheme is credited verbatim with no link, because there is no origin to open. The entry is
    /// added only when one with the same id is not already present, so re-attaching does not
    /// duplicate the credit.
    ///
    /// The map is not modified: the template is read through
    /// [`MapView::tile_loader`](crate::widget::special_widgets::map_view::MapView::tile_loader).
    pub fn attach_map_view(&mut self, map: &mut MapView) {
        let template = map.tile_loader().url_template().trim().to_string();
        self.tile_source = if template.is_empty() { None } else { Some(template.clone()) };
        if let Some(entry) = attribution_entry_from_template(&template) {
            if !self.entries.iter().any(|existing| existing.id == TILE_SOURCE_ID) {
                self.entries.push(entry);
            }
        }
        self.base.request_redraw();
    }

    /// Returns the tile template the last attach read, if any.
    pub fn tile_source(&self) -> Option<&str> {
        self.tile_source.as_deref()
    }

    /// The font the entries are painted with.
    fn resolved_font(&self) -> Font {
        let base = effective_font(self.base.style());
        base.with_size((base.size() * self.font_scale).max(1.0))
    }

    /// The height of one entry row.
    fn row_height(&self) -> i32 {
        (self.resolved_font().size().ceil() as i32 + 3).max(12)
    }

    /// The visible rows and their rectangles, in painting order.
    ///
    /// Painting and hit testing both read this, so the box a click lands in is by construction
    /// the box that was drawn.
    fn layout_rows(&self) -> Vec<(usize, Rect)> {
        let area = self.geometry();
        if self.entries.is_empty() {
            return Vec::new();
        }
        let available_width = area.width as i32 - OVERLAY_INSET * 2;
        let available_height = area.height as i32 - OVERLAY_INSET * 2;
        if available_width <= OVERLAY_PADDING * 2 || available_height <= OVERLAY_PADDING * 2 {
            return Vec::new();
        }

        let font = self.resolved_font();
        let widest = self
            .entries
            .iter()
            .map(|entry| estimate_text_width(&entry.text, &font, 1.0) as i32)
            .max()
            .unwrap_or(0);
        let panel_width = (widest + OVERLAY_PADDING * 2).clamp(MIN_PANEL_WIDTH, available_width);

        let row_height = self.row_height();
        let step = row_height + OVERLAY_ROW_GAP;
        let rows_that_fit =
            ((available_height - OVERLAY_PADDING * 2 + OVERLAY_ROW_GAP) / step).max(0) as usize;
        let count = self.entries.len().min(rows_that_fit);
        if count == 0 {
            return Vec::new();
        }

        let block_height = count as i32 * step - OVERLAY_ROW_GAP + OVERLAY_PADDING * 2;
        let left = if self.corner.is_right() {
            area.right() - OVERLAY_INSET - panel_width
        } else {
            area.x + OVERLAY_INSET
        };
        let top = if self.corner.is_bottom() {
            area.bottom() - OVERLAY_INSET - block_height
        } else {
            area.y + OVERLAY_INSET
        };
        let inner_x = left + OVERLAY_PADDING;
        let inner_width = (panel_width - OVERLAY_PADDING * 2).max(1) as u32;
        let row_height = row_height.max(1) as u32;

        (0..count)
            .map(|index| {
                (
                    index,
                    Rect::new(
                        inner_x,
                        top + OVERLAY_PADDING + index as i32 * step,
                        inner_width,
                        row_height,
                    ),
                )
            })
            .collect()
    }

    /// The background panel around the visible rows.
    fn panel_rect(&self) -> Option<Rect> {
        let rows = self.layout_rows();
        let first = rows.first()?.1;
        let last = rows.last()?.1;
        Some(Rect::new(
            first.x - OVERLAY_PADDING,
            first.y - OVERLAY_PADDING,
            first.width + (OVERLAY_PADDING * 2) as u32,
            (last.bottom() - first.y + OVERLAY_PADDING) as u32,
        ))
    }

    /// The rectangles of the entries that fit, one per visible row.
    pub fn entry_rects(&self) -> Vec<Rect> {
        self.layout_rows().into_iter().map(|(_, rect)| rect).collect()
    }

    /// The rectangle of the entry at `index`, when it is visible.
    pub fn entry_rect(&self, index: usize) -> Option<Rect> {
        self.layout_rows().into_iter().find(|(visible, _)| *visible == index).map(|(_, rect)| rect)
    }

    /// The index of the entry under `point`, when the point is on a visible row.
    pub fn entry_at(&self, point: Point) -> Option<usize> {
        self.layout_rows()
            .into_iter()
            .find(|(_, rect)| rect.contains_point(point))
            .map(|(index, _)| index)
    }
}

/// Builds the credit entry for a tile URL template.
fn attribution_entry_from_template(template: &str) -> Option<AttributionEntry> {
    let template = template.trim();
    if template.is_empty() {
        return None;
    }
    let (text, url) = match template.split_once("://") {
        Some((scheme, rest)) => {
            let authority = rest.split('/').next().unwrap_or(rest).trim();
            if authority.is_empty() {
                (template.to_string(), None)
            } else {
                (authority.to_string(), Some(format!("{scheme}://{authority}")))
            }
        }
        None => (template.to_string(), None),
    };
    Some(AttributionEntry { id: TILE_SOURCE_ID.to_string(), text, url })
}

impl Widget for MapAttributionOverlay {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> Size {
        Size::new(220, 18)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "entry_activated" | "on_entry_activated" => {
                Some(EventSignalRef::mapped("entry_activated", &self.on_entry_activated, |url| {
                    CapabilityValue::String(url.clone())
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MapAttributionOverlay {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entry_count" => Ok(CapabilityValue::UInt(self.entries().len() as u64)),
            "corner" => Ok(CapabilityValue::String(self.corner().token().to_string())),
            "background_opacity" => Ok(CapabilityValue::UInt(self.background_opacity() as u64)),
            "font_scale" => Ok(CapabilityValue::Float(self.font_scale() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "entry_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "corner" => {
                let token = expect_string(value)?;
                let corner = Corner::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_corner(corner);
                Ok(())
            }
            "background_opacity" => {
                self.set_background_opacity(expect_u8_saturating(value)?);
                Ok(())
            }
            "font_scale" => {
                self.set_font_scale(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "entry_count",
            "corner",
            "background_opacity",
            "font_scale",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for MapAttributionOverlay {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        let Event::MousePress { pos, button: 1, .. } = event else {
            return;
        };
        let Some(index) = self.entry_at(*pos) else {
            return;
        };
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let payload = entry.url.clone().unwrap_or_else(|| entry.text.clone());
        if !payload.is_empty() {
            self.on_entry_activated.emit(payload);
        }
    }
}

impl Draw for MapAttributionOverlay {
    fn draw(&mut self, context: &mut RenderContext) {
        let Some(panel) = self.panel_rect() else {
            return;
        };
        let style = self.base.style();
        let (surface, border, ink) = map_overlay_palette(style, "map_attribution");
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);
        let font = self.resolved_font();
        let opacity = self.background_opacity;

        context.fill_rounded_rect(panel, 3, surface.with_alpha(opacity));
        crate::widget::draw::stroke_surface_border(
            context,
            panel,
            [3, 3, 3, 3],
            border.with_alpha(opacity),
            1,
        );

        for (index, rect) in self.layout_rows() {
            let Some(entry) = self.entries.get(index) else {
                continue;
            };
            // A linked entry is painted in the accent so the one actionable line is distinguishable
            // from a plain licence notice.
            let color = if entry.url.is_some() { accent } else { ink };
            context.draw_text_line(rect, &entry.text, &font, color, HorizontalAlignment::Left);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn click(overlay: &mut MapAttributionOverlay, at: Point) -> Vec<String> {
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        overlay.on_entry_activated.connect(move |payload| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(payload.as_ref().clone());
            }
        });
        overlay.handle_event(&Event::mouse_press(at.x, at.y, 1));
        let out = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        out
    }

    /// A click reports the URL when the entry has one and the text otherwise.
    #[test]
    fn entries_are_clickable_and_report_url_or_text() {
        let mut overlay = MapAttributionOverlay::new(Rect::new(0, 0, 300, 120));
        overlay.add_entry("OpenStreetMap", Some("https://www.openstreetmap.org/copyright"));
        overlay.add_entry("Local data", None::<String>);

        let rects = overlay.entry_rects();
        assert_eq!(rects.len(), 2);
        assert_eq!(
            click(&mut overlay, Point::new(rects[0].x + 1, rects[0].y + 1)),
            vec!["https://www.openstreetmap.org/copyright".to_string()]
        );
        assert_eq!(
            click(&mut overlay, Point::new(rects[1].x + 1, rects[1].y + 1)),
            vec!["Local data".to_string()]
        );
    }

    /// The corner decides which side of the rectangle the panel occupies.
    #[test]
    fn a_corner_places_the_panel_on_the_requested_side() {
        let mut overlay = MapAttributionOverlay::new(Rect::new(0, 0, 300, 120));
        overlay.add_entry("Credit", None::<String>);

        overlay.set_corner(Corner::BottomRight);
        let bottom_right = overlay.entry_rect(0).expect("a visible row");
        assert!(bottom_right.x > 150, "the panel sits on the right half");
        assert!(bottom_right.y > 60, "the panel sits on the bottom half");

        overlay.set_corner(Corner::TopLeft);
        let top_left = overlay.entry_rect(0).expect("a visible row");
        assert!(top_left.x < 150);
        assert!(top_left.y < 60);
    }

    /// An overlay with nothing to credit paints nothing and reports nothing.
    #[test]
    fn empty_entries_have_no_hit_target() {
        let mut overlay = MapAttributionOverlay::new(Rect::new(0, 0, 300, 120));
        assert!(overlay.entry_rects().is_empty());
        assert!(overlay.entry_at(Point::new(10, 10)).is_none());
        assert!(click(&mut overlay, Point::new(10, 10)).is_empty());
    }

    /// A zero-sized or too-small overlay cannot host a row, so it must not emit from a press.
    #[test]
    fn degenerate_geometry_has_no_hit_target() {
        let mut zero = MapAttributionOverlay::new(Rect::new(0, 0, 0, 0));
        zero.add_entry("Credit", None::<String>);
        assert!(zero.entry_rects().is_empty());
        assert!(click(&mut zero, Point::new(0, 0)).is_empty());

        let mut tiny = MapAttributionOverlay::new(Rect::new(0, 0, 12, 12));
        tiny.add_entry("Credit", None::<String>);
        assert!(tiny.entry_rects().is_empty());
        assert!(click(&mut tiny, Point::new(5, 5)).is_empty());
    }

    /// Removing by id is a reported boolean and takes the row out of the layout.
    #[test]
    fn remove_entry_reports_whether_the_id_existed() {
        let mut overlay = MapAttributionOverlay::new(Rect::new(0, 0, 300, 120));
        let first = overlay.add_entry("One", None::<String>);
        let second = overlay.add_entry("Two", None::<String>);
        assert_ne!(first, second);

        assert!(overlay.remove_entry(&first));
        assert_eq!(overlay.entries().len(), 1);
        assert_eq!(overlay.entries()[0].id, second);
        assert!(!overlay.remove_entry(&first));
    }

    /// Attaching derives a source credit from the map's own tile template.
    #[test]
    fn attaching_reads_the_tile_template() {
        let mut map = MapView::new(Rect::new(0, 0, 300, 200));
        map.tile_loader_mut().set_url_template("https://tile.example.org/{z}/{x}/{y}.png");

        let mut overlay = MapAttributionOverlay::new(Rect::new(0, 0, 300, 120));
        overlay.attach_map_view(&mut map);

        let source = overlay
            .entries()
            .iter()
            .find(|entry| entry.id == TILE_SOURCE_ID)
            .expect("the tile source is credited");
        assert_eq!(source.text, "tile.example.org");
        assert_eq!(source.url.as_deref(), Some("https://tile.example.org"));
        assert_eq!(overlay.tile_source(), Some("https://tile.example.org/{z}/{x}/{y}.png"));

        let rect = overlay.entry_rect(0).expect("a visible row");
        assert_eq!(
            click(&mut overlay, Point::new(rect.x + 1, rect.y + 1)),
            vec!["https://tile.example.org".to_string()]
        );

        // Re-attaching does not duplicate the derived entry.
        overlay.attach_map_view(&mut map);
        assert_eq!(overlay.entries().iter().filter(|entry| entry.id == TILE_SOURCE_ID).count(), 1);
    }

    /// A scheme-less template has no origin to open, so it is credited verbatim.
    #[test]
    fn a_scheme_less_template_is_credited_without_a_link() {
        let mut map = MapView::new(Rect::new(0, 0, 300, 200));
        map.tile_loader_mut().set_url_template("tiles/{z}/{x}/{y}.png");

        let mut overlay = MapAttributionOverlay::new(Rect::new(0, 0, 300, 120));
        overlay.attach_map_view(&mut map);
        let source = overlay
            .entries()
            .iter()
            .find(|entry| entry.id == TILE_SOURCE_ID)
            .expect("the tile source is credited");
        assert_eq!(source.text, "tiles/{z}/{x}/{y}.png");
        assert_eq!(source.url, None);
    }

    /// Opacity is taken as given and the font scale is validated.
    #[test]
    fn opacity_and_font_scale_are_validated() {
        let mut overlay = MapAttributionOverlay::new(Rect::new(0, 0, 300, 120));
        overlay.set_background_opacity(0);
        assert_eq!(overlay.background_opacity(), 0);

        overlay.set_font_scale(9.0);
        assert_eq!(overlay.font_scale(), MAX_FONT_SCALE);
        overlay.set_font_scale(f32::NAN);
        assert_eq!(overlay.font_scale(), MAX_FONT_SCALE);
        overlay.set_font_scale(0.0);
        assert_eq!(overlay.font_scale(), MIN_FONT_SCALE);
    }

    /// The corner tokens round trip like the crate's other enumerated properties.
    #[test]
    fn corner_tokens_round_trip() {
        for corner in [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight] {
            assert_eq!(Corner::from_token(corner.token()), Some(corner));
        }
        assert_eq!(Corner::from_token("nowhere"), None);
    }
}
