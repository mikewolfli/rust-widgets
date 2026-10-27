// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `PresetChipBar` — a row (or column) of selectable preset chips.
//!
//! # Why this is its own control
//!
//! A "quick range" strip is not specific to a date range: a report header, a filter row and a
//! calendar all offer the same affordance — a row of labelled chips of which one is current, and a
//! second click on the current one means "apply it again". That layout, the hit test and the selected
//! chip's paint were all inline inside `date_range_picker` (COMP-K01), so no other control could use
//! them without a copy.
//!
//! # What it owns
//!
//! The chips, their ids and which id is selected. It does not own what a chip *means*: a host
//! listens to [`PresetChipBar::preset_selected`] and applies its own range, filter or page.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconItemError, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How a chip bar lays its chips out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChipOrientation {
    /// Left to right, wrapping is not attempted: the row clips at its own edge.
    #[default]
    Horizontal,
    /// Top to bottom, one chip per row.
    Vertical,
}

impl ChipOrientation {
    /// The published token: `"horizontal"` or `"vertical"`.
    pub fn as_str(self) -> &'static str {
        match self {
            ChipOrientation::Horizontal => "horizontal",
            ChipOrientation::Vertical => "vertical",
        }
    }

    /// Parses [`Self::as_str`].
    // An inherent token parser returning `Option`; part of the published API surface.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "horizontal" => Some(ChipOrientation::Horizontal),
            "vertical" => Some(ChipOrientation::Vertical),
            _ => None,
        }
    }
}

/// One preset chip: a stable id and the caption it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetChip {
    /// The id the selection and the signals carry.
    pub id: u64,
    /// The caption painted on the chip.
    pub label: String,
}

impl PresetChip {
    /// Creates a chip.
    pub fn new(id: u64, label: impl Into<String>) -> Self {
        Self { id, label: label.into() }
    }
}

/// A font's identity, as far as text measurement is concerned.
type FontKey = (u32, u16, u64);

/// The facts a chip layout is a pure function of.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ChipLayoutKey {
    rect: Rect,
    /// The chip-list revision, bumped whenever a chip is added or removed.
    revision: u64,
    orientation: ChipOrientation,
    chip_height: u32,
    chip_padding: u32,
    spacing: u32,
    font: FontKey,
}

/// The cached chip boxes.
#[derive(Default)]
struct ChipLayoutCache {
    key: Option<ChipLayoutKey>,
    rects: Vec<(u64, Rect)>,
}

/// The identity of `font` for measurement caches.
fn font_key(font: &Font) -> FontKey {
    let mut family_hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in font.family().as_bytes() {
        family_hash ^= u64::from(*byte);
        family_hash = family_hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (font.size().to_bits(), font.weight(), family_hash)
}

/// A bar of selectable preset chips.
pub struct PresetChipBar {
    base: BaseWidget,
    presets: Vec<PresetChip>,
    /// Bumped by every change to `presets`, so the layout cache can invalidate on content.
    layout_revision: u64,
    /// The cached chip boxes, rebuilt only when their inputs change (MISC-11).
    chip_layout: crate::compat::RefCell<ChipLayoutCache>,
    /// The selected chip's id, if one is selected.
    selected_preset_id: Option<u64>,
    /// The id the next [`Self::add_preset`] hands out.
    next_id: u64,
    orientation: ChipOrientation,
    chip_height: u32,
    chip_padding: u32,
    spacing: u32,
    /// The chip the pointer is over, for the hover fill.
    hovered_id: Option<u64>,
    /// Emitted with the id of the chip the user chose.
    pub preset_selected: Signal1<u64>,
    /// Emitted when the user clicks the chip that is already selected.
    ///
    /// "Apply the current preset again" is a distinct intent from "choose this preset": a host that
    /// refreshes a report on re-selection needs to hear about it, and folding it into
    /// `preset_selected` would make every host handle a case it did not ask for.
    pub preset_reselected: GenericSignal,
}

impl PresetChipBar {
    /// Creates an empty bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chip, geometry, "PresetChipBar"),
            presets: Vec::new(),
            layout_revision: 0,
            chip_layout: crate::compat::RefCell::new(ChipLayoutCache::default()),
            selected_preset_id: None,
            next_id: 1,
            orientation: ChipOrientation::Horizontal,
            chip_height: 24,
            chip_padding: 10,
            spacing: 6,
            hovered_id: None,
            preset_selected: Signal1::new(),
            preset_reselected: GenericSignal::new(),
        }
    }

    /// The chips, in bar order.
    pub fn presets(&self) -> &[PresetChip] {
        &self.presets
    }

    /// Appends a chip and returns the id it was given.
    ///
    /// Ids are unique within the bar: the counter is pushed past any id a caller pinned, so a
    /// host-chosen id can never be handed out twice.
    pub fn add_preset(&mut self, label: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.presets.push(PresetChip::new(id, label));
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.base.request_redraw();
        id
    }

    /// Appends a chip with a caller-chosen id.
    ///
    /// Returns `IconItemError::ItemNotFound` when
    /// the id is already in use, and the same error for a zero id, which is the id contract
    /// `pie_menu`'s items follow: an id is an identity, and a duplicate makes "which chip did the
    /// user press" ambiguous.
    pub fn add_preset_with_id(
        &mut self,
        id: u64,
        label: impl Into<String>,
    ) -> Result<u64, IconItemError> {
        if id == 0 {
            return Err(IconItemError::ItemNotFound { id });
        }
        if self.presets.iter().any(|preset| preset.id == id) {
            return Err(IconItemError::AmbiguousItemId { id });
        }
        self.presets.push(PresetChip::new(id, label));
        self.next_id = self.next_id.max(id.saturating_add(1));
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.base.request_redraw();
        Ok(id)
    }

    /// Removes the chip with `id`.
    ///
    /// A removed selected chip leaves the bar with no selection rather than silently selecting a
    /// neighbour, which would report a choice the user never made.
    pub fn remove_preset(&mut self, id: u64) -> bool {
        let Some(index) = self.presets.iter().position(|preset| preset.id == id) else {
            return false;
        };
        self.presets.remove(index);
        self.layout_revision = self.layout_revision.wrapping_add(1);
        if self.selected_preset_id == Some(id) {
            self.selected_preset_id = None;
        }
        if self.hovered_id == Some(id) {
            self.hovered_id = None;
        }
        self.base.request_redraw();
        true
    }

    /// Removes every chip and clears the selection.
    pub fn clear_presets(&mut self) {
        if self.presets.is_empty() && self.selected_preset_id.is_none() {
            return;
        }
        self.presets.clear();
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.selected_preset_id = None;
        self.hovered_id = None;
        self.base.request_redraw();
    }

    /// Selects the chip with `id`, if it exists. Emits `preset_selected`.
    pub fn set_selected(&mut self, id: u64) -> bool {
        if !self.presets.iter().any(|preset| preset.id == id) {
            return false;
        }
        if self.selected_preset_id == Some(id) {
            return true;
        }
        self.selected_preset_id = Some(id);
        self.preset_selected.emit(id);
        self.base.request_redraw();
        true
    }

    /// The selected chip's id.
    pub fn selected_preset(&self) -> Option<u64> {
        self.selected_preset_id
    }

    /// The chip under `point`, if any.
    pub fn preset_at(&self, point: Point) -> Option<u64> {
        self.chip_rects().into_iter().find(|(_, rect)| rect.contains_point(point)).map(|(id, _)| id)
    }

    /// The bar's orientation.
    pub fn orientation(&self) -> ChipOrientation {
        self.orientation
    }

    /// Sets the orientation.
    pub fn set_orientation(&mut self, orientation: ChipOrientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        self.base.request_redraw();
    }

    /// The chip height in logical pixels.
    pub fn chip_height(&self) -> u32 {
        self.chip_height
    }

    /// Sets the chip height (floored at 8 px so a chip has a body).
    pub fn set_chip_height(&mut self, height: u32) {
        let height = height.max(8);
        if self.chip_height == height {
            return;
        }
        self.chip_height = height;
        self.base.request_redraw();
    }

    /// The horizontal padding inside a chip.
    pub fn chip_padding(&self) -> u32 {
        self.chip_padding
    }

    /// Sets the horizontal padding inside a chip.
    pub fn set_chip_padding(&mut self, padding: u32) {
        if self.chip_padding == padding {
            return;
        }
        self.chip_padding = padding;
        self.base.request_redraw();
    }

    /// The gap between chips.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Sets the gap between chips.
    pub fn set_spacing(&mut self, spacing: u32) {
        if self.spacing == spacing {
            return;
        }
        self.spacing = spacing;
        self.base.request_redraw();
    }

    /// Every chip's box, in bar order.
    ///
    /// The boxes depend on the bar geometry, the chip metrics, the font and the chip list, so the
    /// answer is cached under exactly those facts. A static frame replays the table instead of
    /// re-measuring every caption and re-walking the row (MISC-11).
    fn chip_rects(&self) -> Vec<(u64, Rect)> {
        let rect = self.base.geometry();
        let style = self.base.style();
        let font = role_font(style, self.chip_font_size());
        let key = ChipLayoutKey {
            rect,
            revision: self.layout_revision,
            orientation: self.orientation,
            chip_height: self.chip_height,
            chip_padding: self.chip_padding,
            spacing: self.spacing,
            font: font_key(&font),
        };
        {
            let mut cache = self.chip_layout.borrow_mut();
            if cache.key.as_ref() != Some(&key) {
                let mut rects = Vec::with_capacity(self.presets.len());
                let mut cursor = match self.orientation {
                    ChipOrientation::Horizontal => rect.x,
                    ChipOrientation::Vertical => rect.y,
                };
                for preset in &self.presets {
                    // The chip is as wide as its caption needs, bounded by the bar: an overflowing
                    // caption is elided by the draw rather than widening the chip past the bar.
                    let text =
                        crate::widget::metrics::estimate_text_width(&preset.label, &font, 1.0);
                    let width = match self.orientation {
                        ChipOrientation::Horizontal => {
                            (text + self.chip_padding * 2).min(rect.width.max(1))
                        }
                        ChipOrientation::Vertical => rect.width,
                    };
                    let chip = match self.orientation {
                        ChipOrientation::Horizontal => Rect::new(
                            cursor,
                            rect.y,
                            width,
                            self.chip_height.min(rect.height.max(1)),
                        ),
                        ChipOrientation::Vertical => Rect::new(
                            rect.x,
                            cursor,
                            rect.width,
                            self.chip_height.min(rect.height.max(1)),
                        ),
                    };
                    rects.push((preset.id, chip));
                    cursor += match self.orientation {
                        ChipOrientation::Horizontal => width as i32 + self.spacing as i32,
                        ChipOrientation::Vertical => self.chip_height as i32 + self.spacing as i32,
                    };
                }
                cache.rects = rects;
                cache.key = Some(key);
            }
        }
        self.chip_layout.borrow().rects.clone()
    }

    /// The font size a chip's caption is painted at.
    fn chip_font_size(&self) -> f32 {
        (self.chip_height as f32 * 0.45).clamp(9.0, 14.0)
    }

    /// Chooses the chip with `id`: selects it and reports whether it was already current.
    fn choose(&mut self, id: u64) {
        let already = self.selected_preset_id == Some(id);
        self.selected_preset_id = Some(id);
        self.base.request_redraw();
        if already {
            self.preset_reselected.emit();
        } else {
            self.preset_selected.emit(id);
        }
    }
}

impl Widget for PresetChipBar {
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
        Size::new(240, self.chip_height)
    }
    fn is_focusable(&self) -> bool {
        true
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl EventHandler for PresetChipBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let hovered = self.preset_at(*pos);
                if hovered != self.hovered_id {
                    self.hovered_id = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_id.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(id) = self.preset_at(*pos) {
                    self.choose(id);
                }
            }
            Event::KeyPress { key, .. } => match *key {
                crate::event::key_codes::LEFT | crate::event::key_codes::UP => {
                    self.step(-1);
                }
                crate::event::key_codes::RIGHT | crate::event::key_codes::DOWN => {
                    self.step(1);
                }
                crate::event::key_codes::ENTER | crate::event::key_codes::SPACE => {
                    if let Some(id) = self.selected_preset_id {
                        self.choose(id);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl PresetChipBar {
    /// Moves the keyboard selection by `delta`, wrapping inside the bar.
    fn step(&mut self, delta: i32) {
        if self.presets.is_empty() {
            return;
        }
        let count = self.presets.len() as i32;
        let current = self
            .selected_preset_id
            .and_then(|id| self.presets.iter().position(|preset| preset.id == id))
            .map_or(-delta, |index| index as i32);
        let next = (current + delta).rem_euclid(count) as usize;
        let id = self.presets[next].id;
        self.choose(id);
    }
}

impl Draw for PresetChipBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        let primary = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let ink = style.text_color.unwrap_or_else(|| {
            palette.as_ref().map(|colors| colors.foreground).unwrap_or(Color::BLACK)
        });
        // The chip body is one visible step off the window, the same derivation the drum and the
        // chip control use, so a bar mounted beside them matches.
        let body = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style.border_color.unwrap_or_else(|| body.blend(&ink, 0.35));
        let font = role_font(style, self.chip_font_size());
        let radius = (self.chip_height / 2).max(2);

        for (id, chip) in self.chip_rects() {
            let selected = self.selected_preset_id == Some(id);
            let hovered = self.hovered_id == Some(id);
            let fill = if selected {
                primary
            } else if hovered {
                body.blend(&ink, 0.10)
            } else {
                body
            };
            context.fill_rounded_rect(chip, radius, fill);
            context.draw_rounded_rect_stroke(chip, radius, border, 1);
            let label = self
                .presets
                .iter()
                .find(|preset| preset.id == id)
                .map(|preset| preset.label.as_str())
                .unwrap_or("");
            let text_color = if selected { primary.contrast_color() } else { ink };
            // The fitted primitive owns the truncation, so an over-long caption is ellipsised inside
            // its own chip instead of painting over the next one.
            let band = Rect::new(
                chip.x + self.chip_padding as i32,
                chip.y,
                chip.width.saturating_sub(self.chip_padding * 2),
                chip.height,
            );
            context.draw_text_line_cow(band, label, &font, text_color, HorizontalAlignment::Center);
        }
    }
}

impl WidgetProperties for PresetChipBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_preset" => Ok(match self.selected_preset_id {
                Some(id) => CapabilityValue::UInt(id),
                None => CapabilityValue::Null,
            }),
            "preset_count" => Ok(CapabilityValue::UInt(self.presets.len() as u64)),
            "orientation" => Ok(CapabilityValue::String(self.orientation.as_str().to_string())),
            "chip_height" => Ok(CapabilityValue::UInt(u64::from(self.chip_height))),
            "chip_padding" => Ok(CapabilityValue::UInt(u64::from(self.chip_padding))),
            "spacing" => Ok(CapabilityValue::UInt(u64::from(self.spacing))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_preset" => {
                let id = crate::widget::capability::coercion::expect_u32(value)? as u64;
                if self.set_selected(id) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "orientation" => {
                let token = expect_string(value)?;
                let orientation =
                    ChipOrientation::from_str(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_orientation(orientation);
                Ok(())
            }
            "chip_height" => {
                let height = crate::widget::capability::coercion::expect_u32(value)?;
                self.set_chip_height(height);
                Ok(())
            }
            "chip_padding" => {
                let padding = crate::widget::capability::coercion::expect_u32(value)?;
                self.set_chip_padding(padding);
                Ok(())
            }
            "spacing" => {
                let spacing = crate::widget::capability::coercion::expect_u32(value)?;
                self.set_spacing(spacing);
                Ok(())
            }
            "preset_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "selected_preset",
            "preset_count",
            "orientation",
            "chip_height",
            "chip_padding",
            "spacing",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> PresetChipBar {
        let mut bar = PresetChipBar::new(Rect::new(0, 0, 300, 24));
        bar.add_preset("Today");
        bar.add_preset("Last 7 days");
        bar
    }

    #[test]
    fn adding_a_preset_returns_a_unique_id() {
        let mut bar = PresetChipBar::new(Rect::new(0, 0, 300, 24));
        let first = bar.add_preset("A");
        let second = bar.add_preset("B");
        assert_ne!(first, second);
        assert_eq!(bar.presets().len(), 2);
    }

    #[test]
    fn a_pinned_id_is_never_handed_out_again() {
        let mut bar = PresetChipBar::new(Rect::new(0, 0, 300, 24));
        assert_eq!(bar.add_preset_with_id(9, "Pinned"), Ok(9));
        let next = bar.add_preset("Next");
        assert!(next > 9, "the counter must move past the pinned id");
        assert_eq!(bar.add_preset_with_id(9, "Dup"), Err(IconItemError::AmbiguousItemId { id: 9 }));
        assert_eq!(
            bar.add_preset_with_id(0, "Zero"),
            Err(IconItemError::ItemNotFound { id: 0 }),
            "zero is reserved as 'no chip'"
        );
    }

    #[test]
    fn choosing_reports_selection_and_reselection_separately() {
        use std::sync::{Arc, Mutex};
        let mut bar = bar();
        let selected = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&selected);
        bar.preset_selected.connect(move |id| sink.lock().unwrap().push(*id));
        let again = Arc::new(Mutex::new(0u32));
        let sink = Arc::clone(&again);
        bar.preset_reselected.connect(move || *sink.lock().unwrap() += 1);

        let id = bar.presets()[0].id;
        bar.choose(id);
        bar.choose(id);
        assert_eq!(selected.lock().unwrap().as_slice(), &[id]);
        assert_eq!(*again.lock().unwrap(), 1);
    }

    #[test]
    fn removing_the_selected_chip_clears_the_selection() {
        let mut bar = bar();
        let id = bar.presets()[0].id;
        assert!(bar.set_selected(id));
        assert!(bar.remove_preset(id));
        assert_eq!(bar.selected_preset(), None);
    }

    #[test]
    fn chips_are_hit_testable() {
        let bar = bar();
        let rects = bar.chip_rects();
        assert_eq!(rects.len(), 2);
        let (id, rect) = rects[1];
        assert_eq!(bar.preset_at(Point::new(rect.x + 2, rect.y + 2)), Some(id));
        assert_eq!(bar.preset_at(Point::new(-5, -5)), None);
    }

    /// The cached layout follows both the chip list and the chip metrics (MISC-11).
    #[test]
    fn the_layout_cache_follows_the_chips_and_geometry() {
        let mut bar = bar();
        assert_eq!(bar.chip_rects().len(), 2);
        let first_height = bar.chip_rects()[0].1.height;

        bar.add_preset("This month");
        assert_eq!(bar.chip_rects().len(), 3, "adding a chip invalidates the layout");

        bar.set_chip_height(40);
        // The bar's own height is 24, which caps a 40 px chip unless the bar grows with it.
        bar.set_geometry(Rect::new(0, 0, 300, 60));
        assert_eq!(bar.chip_rects()[0].1.height, 40);
        assert_ne!(first_height, 40, "the cached boxes follow the new metrics");

        bar.remove_preset(bar.presets()[0].id);
        assert_eq!(bar.chip_rects().len(), 2);
    }
}
