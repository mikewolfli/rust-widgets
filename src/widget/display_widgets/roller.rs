// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Roller widget — scroll-wheel style selector (BLUE13 R2.3).
//!
//! Displays a scroll-wheel list of options where one item is highlighted in the
//! center. Supports mouse wheel scrolling and click-to-select interaction.

use crate::compat::{String, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::{expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Roller widget for selecting from a list via scroll-wheel interaction.
pub struct Roller {
    base: BaseWidget,
    /// List of option strings.
    options: Vec<String>,
    /// Currently selected index.
    selected_index: usize,
    /// Number of visible items (should be odd for symmetry).
    visible_count: u32,
    /// Font size for items.
    font_size: f32,
    /// Signal emitted when selection changes.
    pub changed_signal: GenericSignal,
}

impl Roller {
    /// Creates a new `Roller` with the given options and geometry.
    ///
    /// The initial selection is at index 0 and `visible_count` defaults to 5
    /// (clamped to an odd value so the selected item sits in the center).
    pub fn new(options: Vec<String>, rect: Rect) -> Self {
        let visible_count = 5u32 | 1; // ensure odd, at least 3
        Self {
            base: BaseWidget::new(WidgetKind::Roller, rect, "Roller"),
            options,
            selected_index: 0,
            visible_count,
            font_size: 16.0,
            changed_signal: GenericSignal::new(),
        }
    }

    /// Returns all option strings.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Replaces the option list and clamps the selected index to the new range.
    pub fn set_options(&mut self, options: Vec<String>) {
        self.options = options;
        if !self.options.is_empty() {
            self.selected_index = self.selected_index.min(self.options.len() - 1);
        } else {
            self.selected_index = 0;
        }
        self.changed_signal.emit();
        self.base.request_redraw();
    }

    /// Returns the currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Sets the selected index, clamped to the valid range.
    ///
    /// Emits `changed_signal` only when the value actually changes.
    pub fn set_selected_index(&mut self, index: usize) {
        let clamped = if self.options.is_empty() { 0 } else { index.min(self.options.len() - 1) };
        if clamped != self.selected_index {
            self.selected_index = clamped;
            self.changed_signal.emit();
            self.base.request_redraw();
        }
    }

    /// Returns the text of the currently selected option, or `None` if the
    /// options list is empty.
    pub fn selected_text(&self) -> Option<&str> {
        if self.selected_index < self.options.len() {
            Some(self.options[self.selected_index].as_str())
        } else {
            None
        }
    }

    /// Returns the number of visible items in the wheel.
    pub fn visible_count(&self) -> u32 {
        self.visible_count
    }

    /// Sets the number of visible items. The value is rounded up to the next
    /// odd number (minimum 3) so that the selected item appears centered.
    pub fn set_visible_count(&mut self, count: u32) {
        let clamped = count.max(3);
        let odd = if clamped.is_multiple_of(2) { clamped + 1 } else { clamped };
        if odd != self.visible_count {
            self.visible_count = odd;
            self.base.request_redraw();
        }
    }

    /// Sets the font size used to render option text.
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.max(4.0);
        self.base.request_redraw();
    }

    /// Returns the font size used to render option text.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// The height of one row in the wheel.
    ///
    /// # Why this is not a function of the control's rectangle
    ///
    /// A row is a **fixed row**, like a picker's drum: the same control shows the same
    /// row height in a short box and a tall one. Deriving it from `rect` — combined with
    /// sizing the wheel from the same rectangle — made the census cell a 120 px surface
    /// with one 24 px selection band in it, and made a wheel in a list slot a different
    /// object from a wheel here. The base is [`dimensions::ROLLER_ROW_HEIGHT`], which is
    /// the row the default 16 pt face needs; `font_size` still scales it, but as a
    /// proportion of the type the control was told to draw, never of `rect`.
    fn item_height(&self) -> u32 {
        // The control's own default face, which is the type size `ROLLER_ROW_HEIGHT` was
        // chosen for. Scaling from the *table's* base font instead made the default 16 pt
        // face round 28 up to 32, so the drawn row disagreed with the named one on the one
        // font size every default roller uses.
        const DEFAULT_FONT_SIZE: f32 = 16.0;
        if self.font_size <= DEFAULT_FONT_SIZE {
            return dimensions::ROLLER_ROW_HEIGHT;
        }
        // A caller with a larger font gets a proportionally larger row. The base is the
        // named row at the named font, and never `rect`: the two are unrelated facts.
        // `font_size` is clamped to `>= 4.0` by the setter, so the divisor is never zero.
        let scaled = (self.font_size / DEFAULT_FONT_SIZE * dimensions::ROLLER_ROW_HEIGHT as f32)
            .round() as u32;
        scaled.max(dimensions::ROLLER_ROW_HEIGHT)
    }

    /// The total content height for all visible items.
    ///
    /// Fixed at [`dimensions::ROLLER_WHEEL_HEIGHT`] for the default wheel — five rows —
    /// rather than derived from the *current* row height, because the wheel's extent is
    /// a chrome fact: a caller who changes the font changes the row's density inside the
    /// wheel, not how tall the wheel is. Deriving it from `item_height()` coupled the
    /// control's box to the type size, which is the same drift that made the drawn wheel
    /// disagree with the height `size_hint` reported.
    fn content_height(&self) -> u32 {
        dimensions::ROLLER_WHEEL_HEIGHT * self.visible_count / dimensions::ROLLER_VISIBLE_ROWS
    }

    /// The band the wheel actually occupies: full width, its own height, centred.
    ///
    /// # Why the wheel has its own height
    ///
    /// A roller paints a **wheel of rows**, not a surface. The control used to fill its
    /// whole rectangle (`fill_rect(rect, bg_color)`) and lay its items out around the
    /// rectangle's middle, so the 240x120 census cell drew a 240x120 fill with a single
    /// 24 px selection band at y 48 — a panel with one row in it rather than a wheel.
    /// [`dimensions::ROLLER_WHEEL_HEIGHT`] is the wheel's own extent —
    /// [`dimensions::ROLLER_VISIBLE_ROWS`] rows — and the shared
    /// [`ControlMetrics::full_width_band`] centres it, so a wheel in a list slot and a
    /// wheel in a census cell are the same object. When the caller's rectangle is
    /// *shorter* than the wheel the band is clamped to it rather than painted outside,
    /// which is the one case where the control legitimately fills its rectangle.
    fn wheel_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::ROLLER_WHEEL_HEIGHT)
    }
}

impl Widget for Roller {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Estimate width from the longest option string.
        let char_width = self.font_size * 0.6;
        let max_len = self.options.iter().map(|s| s.len()).max().unwrap_or(10);
        let width = (max_len as f32 * char_width).ceil().max(80.0) as u32;
        // The height is the wheel's own band, which `wheel_band` draws from, so the two
        // cannot describe different controls: this is the `content_height` the control
        // has always reported, now also the height it paints at.
        Size::new(width, self.content_height())
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Roller`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch.
impl WidgetProperties for Roller {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => Ok(CapabilityValue::UInt(self.selected_index() as u64)),
            "visible_count" => Ok(CapabilityValue::UInt(self.visible_count() as u64)),
            "item_count" => Ok(CapabilityValue::UInt(self.options().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected_index(expect_usize(value)?);
                Ok(())
            }
            "visible_count" => {
                self.set_visible_count(expect_u32(value)?);
                Ok(())
            }
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_index", "visible_count", "item_count", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `roller` publishes.
    ///
    /// All three assign state — the option list, the selected row and the number of
    /// rows on screen — and each needs a payload, so the whole set is answered through
    /// the property route rather than performed here.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_options" | "set_selected_index" | "set_visible_count" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Roller {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || self.options.is_empty() {
            return;
        }
        match event {
            // Wheel / scroll: positive delta.y scrolls forward (next item),
            // negative delta.y scrolls backward (previous item).
            Event::Wheel { delta, modifiers: _ } => {
                if delta.y > 0 && self.selected_index + 1 < self.options.len() {
                    self.set_selected_index(self.selected_index + 1);
                } else if delta.y < 0 && self.selected_index > 0 {
                    self.set_selected_index(self.selected_index - 1);
                }
            }
            // Mouse press: determine which item was clicked relative to the
            // center of the visible wheel.
            Event::MousePress { pos, button: _ } => {
                let band = self.wheel_band();
                let item_h = self.item_height() as i32;
                let center_y = band.y + (band.height as i32) / 2;
                let clicked_offset = pos.y - center_y;
                // A press outside the wheel band selects nothing: the hit test is the
                // **drawn** band, so the clickable rows and the painted rows are the same
                // object. Reading the control's own rectangle here (as the old form did)
                // made the empty space above and below the wheel live.
                if pos.y < band.y || pos.y >= band.y + band.height as i32 {
                    return;
                }
                let half_visible = (self.visible_count / 2) as i32;
                // Clamp the row offset to the visible range.
                let row_offset = (clicked_offset / item_h).clamp(-half_visible, half_visible);
                if row_offset == 0 {
                    // Clicked the center item — fire clicked signal.
                    self.base.clicked.emit();
                } else {
                    let target = self.selected_index as i32 + row_offset;
                    let target =
                        target.clamp(0, self.options.len().saturating_sub(1) as i32) as usize;
                    self.set_selected_index(target);
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos: _ } => {
                self.base.clicked.emit();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for Roller {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 || self.options.is_empty() {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("roller")
            .or_else(|| crate::style::resolved_theme_style("list_box"));
        // `roller` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The wheel's own fill is therefore a step toward the
        // foreground, so it reads as a recessed surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(240, 240, 240));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(50, 50, 50));
        let bg_color = resolved.blend(&text_color, 0.08);
        // The centre item is a *selection*, which is chrome rather than data, so it
        // reads the theme's primary token instead of the previous literal blue.
        let selected_bg = crate::style::resolved_theme_style("button")
            .and_then(|button| button.background_color)
            .unwrap_or_else(|| bg_color.blend(&text_color, 0.6));
        // Its label must be legible **on the selection band itself**, so it takes whichever
        // of black/white contrasts with that band. The previous form blended the label 92% of
        // the way toward the wheel's own surface, which is a light-theme assumption baked into
        // arithmetic: on the light theme the band resolves to the theme's primary
        // `rgb(33,150,243)` while the surface is `rgb(221,221,221)`, so the label came out
        // `rgb(206,215,223)` — 2.14:1 against the band it sits on (below the 3:1 large-text
        // floor, let alone the 4.5:1 body floor). Same rule as the theme's `Primary` role.
        let selected_text_color = selected_bg.contrast_color();
        let muted_color = text_color.blend(&bg_color, 0.47);

        let font =
            self.style().font.clone().unwrap_or_else(|| Font::simple("sans-serif", self.font_size));

        // ── The wheel actually painted ──
        //
        // `rect` is the area the control was *given*; a wheel's own chrome is its rows,
        // `visible_count` of them at `ROLLER_ROW_HEIGHT` each. The band is the single
        // derivation the surface fill, the selection band, every row and the hit test are
        // placed from, so none of them can disagree about where the wheel is.
        let band = self.wheel_band();
        let item_h = self.item_height() as i32;
        let half_visible = (self.visible_count / 2) as usize;
        let center_x = band.x + (band.width as i32) / 2;
        let center_y = band.y + (band.height as i32) / 2;

        // Draw a background fill for the wheel's own band, not for the whole control.
        context.fill_rect(band, bg_color);

        // Clipping region to ensure items don't spill outside the wheel.
        context.push_clip(band.x, band.y, band.width, band.height);

        // Draw each visible item around the center.
        for offset in 0..=half_visible {
            for &sign in &[1i32, -1i32] {
                if offset == 0 && sign == -1 {
                    continue; // center item only once
                }
                let idx = self.selected_index as i32 + sign * offset as i32;
                if idx < 0 || idx >= self.options.len() as i32 {
                    continue;
                }
                let idx = idx as usize;
                let y = center_y + sign * offset as i32 * item_h;

                let item_rect = Rect::new(band.x, y - item_h / 2, band.width, item_h as u32);

                let is_selected = offset == 0;
                if is_selected {
                    // Highlight the selected item in the center.
                    context.fill_rect(item_rect, selected_bg);
                }

                // Draw the option text, measuring to center horizontally.
                let metrics = context.measure_text(&self.options[idx], &font);
                let text_x = center_x - (metrics.width as i32) / 2;
                // Vertically center the text in the row through the shared primitive: a
                // glyph origin is the box's **top** edge, so `y - metrics.height / 2` put
                // that edge on the row's middle line and drew every label half a line low.
                let row_line = context.text_line(item_rect, &font);
                let text_y = row_line.y;

                let color = if is_selected {
                    selected_text_color
                } else {
                    // Items farther from center are more muted.
                    let fade = 1.0 - (offset as f32 / (half_visible.max(1)) as f32) * 0.5;
                    Color::rgba(
                        (muted_color.r as f32 * fade) as u8,
                        (muted_color.g as f32 * fade) as u8,
                        (muted_color.b as f32 * fade) as u8,
                        muted_color.a,
                    )
                };

                context.draw_text(
                    Point::new(text_x, text_y),
                    &self.options[idx],
                    &font,
                    color,
                    HorizontalAlignment::Left,
                );
            }
        }

        context.pop_clip();
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::compat::MiniToString;
    use crate::core::{Point, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn make_roller() -> Roller {
        let options = vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Cherry".to_string(),
            "Dragonfruit".to_string(),
            "Elderberry".to_string(),
        ];
        Roller::new(options, Rect::new(0, 0, 160, 120))
    }

    #[test]
    fn roller_creation_defaults() {
        let roller = make_roller();
        assert_eq!(roller.options.len(), 5);
        assert_eq!(roller.selected_index(), 0);
        assert_eq!(roller.selected_text(), Some("Apple"));
        assert!(roller.visible_count() >= 3);
        assert!(roller.visible_count() % 2 == 1);
        assert_eq!(roller.kind(), WidgetKind::Roller);
    }

    #[test]
    fn roller_set_selected_index() {
        let mut roller = make_roller();
        assert_eq!(roller.selected_index(), 0);

        roller.set_selected_index(2);
        assert_eq!(roller.selected_index(), 2);
        assert_eq!(roller.selected_text(), Some("Cherry"));

        // Clamp above max.
        roller.set_selected_index(100);
        assert_eq!(roller.selected_index(), 4);
        assert_eq!(roller.selected_text(), Some("Elderberry"));
    }

    #[test]
    fn roller_selected_text() {
        let mut roller = make_roller();
        assert_eq!(roller.selected_text(), Some("Apple"));

        roller.set_selected_index(3);
        assert_eq!(roller.selected_text(), Some("Dragonfruit"));

        // Empty options → no selected text.
        let empty: Roller = Roller::new(Vec::new(), Rect::new(0, 0, 100, 100));
        assert!(empty.selected_text().is_none());
    }

    #[test]
    fn roller_mouse_wheel_changes_selection() {
        let mut roller = make_roller();
        assert_eq!(roller.selected_index(), 0);

        // Scroll down (positive delta) should move to next item.
        roller.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 1);

        // Scroll down again.
        roller.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 2);

        // Scroll up (negative delta) should move to previous item.
        roller.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 1);

        // Scroll up past start clamps to 0.
        roller.set_selected_index(0);
        roller.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 0);

        // Scroll down past end clamps to last.
        roller.set_selected_index(roller.options.len() - 1);
        roller.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(roller.selected_index(), roller.options.len() - 1);
    }

    #[test]
    fn roller_draw_does_not_panic() {
        let mut roller = make_roller();
        // Create a minimal render context for testing.
        let mut backend = SoftwarePaintBackend::new(Size::new(160, 120), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        roller.draw(&mut ctx);
        backend.end_frame();
        // Also test with empty options.
        let mut empty: Roller = Roller::new(Vec::new(), Rect::new(0, 0, 100, 100));
        let mut backend2 = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend2.begin_frame(Color::WHITE);
        let mut ctx2 = RenderContext::new(&mut backend2);
        empty.draw(&mut ctx2);
        backend2.end_frame();
    }

    #[test]
    fn roller_set_options_replaces_and_clamps() {
        let mut roller = make_roller();
        roller.set_selected_index(3);
        assert_eq!(roller.selected_text(), Some("Dragonfruit"));

        // Replace with fewer options; index should clamp.
        roller.set_options(vec!["X".to_string(), "Y".to_string()]);
        assert_eq!(roller.options.len(), 2);
        assert_eq!(roller.selected_index(), 1);
        assert_eq!(roller.selected_text(), Some("Y"));

        // Replace with empty list.
        roller.set_options(Vec::new());
        assert!(roller.options.is_empty());
        assert_eq!(roller.selected_index(), 0);
        assert!(roller.selected_text().is_none());
    }

    #[test]
    fn roller_visible_count_is_always_odd() {
        let roller = make_roller();
        assert!(roller.visible_count() % 2 == 1);

        let mut roller = roller;
        roller.set_visible_count(4);
        assert!(roller.visible_count() % 2 == 1);
        assert!(roller.visible_count() >= 3);

        roller.set_visible_count(7);
        assert_eq!(roller.visible_count(), 7);
    }

    #[test]
    fn roller_font_size_accessors() {
        let mut roller = make_roller();
        assert!((roller.font_size() - 16.0).abs() < 0.01);

        roller.set_font_size(20.0);
        assert!((roller.font_size() - 20.0).abs() < 0.01);

        // Clamp to minimum.
        roller.set_font_size(0.0);
        assert!((roller.font_size() - 4.0).abs() < 0.01);
    }

    #[test]
    fn roller_mouse_press_selects_item() {
        let mut roller = make_roller();
        // The hit test is the **drawn** wheel band, not the control's rectangle. The
        // wheel is `ROLLER_WHEEL_HEIGHT` (140 px) in this 120 px control, so the band is
        // clamped to the control and its middle is the control's middle; each press is
        // placed one *row* from that middle, using the row the painter uses.
        let band = roller.wheel_band();
        let item_h = roller.item_height() as i32;
        assert!(item_h > 0, "a row is a drawable height");
        let center_y = band.y + (band.height as i32) / 2;

        // Click one item above center.
        roller.handle_event(&Event::MousePress {
            pos: Point::new(band.x + 10, center_y - item_h),
            button: 1,
        });
        // Should have moved one index up (if available).
        assert_eq!(roller.selected_index(), 0); // already at 0, can't go up

        roller.set_selected_index(2);
        // Click one item below center → index 3.
        roller.handle_event(&Event::MousePress {
            pos: Point::new(band.x + 10, center_y + item_h),
            button: 1,
        });
        assert_eq!(roller.selected_index(), 3);
    }

    /// The wheel's rows are a fixed height, so the same control is the same object in
    /// any rectangle.
    ///
    /// This pins the defect the fix removes: `item_height` was `font_size * 1.5` and
    /// the wheel's band was the control's own `rect`, so the census cell drew a 120 px
    /// panel with one 24 px selection band in it while a 96 px list slot drew a
    /// different wheel. The band is now the wheel's own extent — `visible_count` rows —
    /// centred in the control and clamped only when the control is shorter than it.
    #[test]
    fn the_wheel_keeps_its_own_extent_in_any_rectangle() {
        for height in [160u32, 240, 400] {
            let roller = Roller::new(
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                Rect::new(0, 0, 200, height),
            );
            let band = roller.wheel_band();
            // The band is the wheel's own height, not the caller's.
            assert_eq!(band.height, roller.content_height(), "at control height {height}");
            assert_eq!(
                band.height,
                dimensions::ROLLER_WHEEL_HEIGHT,
                "a five-row wheel, whatever the control's height is"
            );
            // The row scales with the *type*, never with the box: at the default 16 pt
            // face it is `ROLLER_ROW_HEIGHT`, and a taller control does not change it.
            assert_eq!(
                roller.item_height(),
                dimensions::ROLLER_ROW_HEIGHT,
                "a taller control must not stretch a row (at {height})"
            );
            assert_eq!(band.width, 200, "the wheel spans the control's width");
            // Centred, so the wheel sits on the control's middle line.
            assert_eq!(band.y, (height - band.height) as i32 / 2, "at control height {height}");
        }
        // A control shorter than the wheel clamps it rather than painting outside: this
        // is the one case where the control legitimately fills its rectangle, and it is
        // what the 240x120 census cell does.
        let short = Roller::new(vec!["a".to_string()], Rect::new(0, 0, 200, 40));
        assert_eq!(short.wheel_band().height, 40);
        assert_eq!(short.wheel_band().y, 0, "a clamped band starts at the control's edge");
        // And the census cell itself, which is shorter than the wheel.
        let census = Roller::new(vec!["a".to_string()], crate::widget::census::CENSUS_RECT);
        assert_eq!(census.wheel_band().height, 120);
    }

    /// A press outside the drawn wheel selects nothing.
    ///
    /// The old handler tested the control's rectangle, so a click in the empty space
    /// above or below the wheel moved the selection — a hit area larger than the ink.
    #[test]
    fn a_press_outside_the_wheel_band_changes_nothing() {
        // A tall control with a much shorter wheel: the rows occupy only the middle.
        let mut roller = Roller::new(
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            Rect::new(0, 0, 200, 400),
        );
        roller.set_selected_index(1);
        let band = roller.wheel_band();
        assert!(band.y > 0, "the wheel is centred, so there is empty space above it");
        roller.handle_event(&Event::MousePress { pos: Point::new(100, band.y - 10), button: 1 });
        assert_eq!(roller.selected_index(), 1, "a press above the wheel must not select");
        roller.handle_event(&Event::MousePress {
            pos: Point::new(100, band.y + band.height as i32 + 10),
            button: 1,
        });
        assert_eq!(roller.selected_index(), 1, "a press below the wheel must not select");
    }

    /// The emitted fill is the wheel band, not the control's whole rectangle.
    #[test]
    fn the_roller_paints_a_wheel_rather_than_a_panel() {
        // A 400 px control with a 140 px wheel: the fill must be the wheel, so the
        // band's own rectangle is what the snapshot starts from rather than the cell.
        let mut roller =
            Roller::new(vec!["Apple".to_string(), "Banana".to_string()], Rect::new(0, 0, 240, 400));
        let svg = crate::widget::svg::render_to_svg(&mut roller);
        let band = roller.wheel_band();
        assert_eq!(band.height, dimensions::ROLLER_WHEEL_HEIGHT);
        assert!(band.y > 0, "the wheel is centred, not pinned to the control's top edge");
        assert!(
            svg.contains(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                band.x, band.y, band.width, band.height
            )),
            "the wheel's own band is what is painted: {svg}"
        );
        // The defect's signature: a fill spanning the control's whole height. The wheel
        // is `visible_count` rows, so the control's own fill is the band — never the
        // control's full height. (The `<rect x="0" y="0" … height="400">` above is the
        // *exporter's* backdrop, painted before the control draws, which is why this
        // asserts on the band's own rectangle rather than on the absence of a number.)
        assert!(
            !svg.contains(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                band.x, band.y, band.width, 400
            )),
            "the control's own height would mean a panel fill: {svg}"
        );
        // The selection band is one row tall, not the whole wheel.
        assert!(
            svg.contains(&format!("height=\"{}\"", roller.item_height())),
            "the centre row is a row, not the wheel: {svg}"
        );
    }
}
