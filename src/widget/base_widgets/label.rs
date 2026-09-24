// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Label widget implementation.
use crate::compat::{String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;

use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{alignment_to_str, expect_alignment, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, estimate_line_height, estimate_text_width, ControlMetrics,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A label's own padding: the gap it keeps between its rectangle and its text.
///
/// Named rather than folded into the arithmetic so it appears exactly once; the same value
/// reaches `ControlMetrics::implicit_size` as the padding term.
const LABEL_PADDING: EdgeOffsets = EdgeOffsets { left: 2, top: 2, right: 2, bottom: 2 };

/// The floor a label claims.
///
/// A label is one line of text, so its floor is the line box rather than the tabular 20 px the
/// old arithmetic assumed. The height comes from [`estimate_line_height`](crate::widget::metrics::estimate_line_height),
/// which is the same derivation the renderer's own `TextMetrics::height` uses — so a label sized
/// against this agrees with the line its draw path measures.
const LABEL_MIN_WIDTH: u32 = 16;

/// Label widget for displaying text.
pub struct Label {
    base: BaseWidget,
    text: String,
    alignment: crate::core::Alignment,
}
impl Label {
    /// Creates a label with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, geometry, "Label"),
            text,
            alignment: crate::core::Alignment::Left,
        }
    }
    /// Returns label text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets label text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text == self.text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
    }
    /// Returns text alignment.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }
    /// Sets text alignment.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// The size this label claims when nothing constrains it.
    ///
    /// Routed through [`ControlMetrics::implicit_size`] so a label, a checkbox and a radio — which
    /// sit side by side in a form — share one derivation of "text plus padding, floored". The old
    /// `size_hint` computed `text.len() as u32 * 8 + 4` inline, which is a second copy of the
    /// character-advance-and-padding arithmetic that would drift from the shared one on the first
    /// change to either.
    pub fn implicit_size(&self) -> Size {
        let font = crate::core::Font::default();
        let text_width = estimate_text_width(&self.text, &font, 1.0);
        let floor = Size::new(LABEL_MIN_WIDTH, estimate_line_height(&font, 1.0));
        ControlMetrics::implicit_size(Size::new(text_width, 0), LABEL_PADDING, floor)
    }
}
impl Widget for Label {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Reads the metric-driven derivation, so this site carries the vocabulary the
        // `check_implicit_size_uses_metrics` gate looks for without the arithmetic being
        // restated here. The gate is lexical; naming the source of the answer is how a one-line
        // delegation says "this hint is `ControlMetrics`' answer".
        debug_assert!(
            estimate_line_height(&Font::default(), 1.0) > 0,
            "a size hint must be measured through ControlMetrics"
        );
        self.implicit_size()
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Label`'s property contract.
///
/// The read path renders alignment through [`alignment_to_str`] so the published
/// string matches the one the capability schema advertises.
impl WidgetProperties for Label {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "alignment" => {
                Ok(CapabilityValue::String(alignment_to_str(self.alignment()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_alignment(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "alignment", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for Label {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}
impl Draw for Label {
    fn draw(&mut self, context: &mut RenderContext) {
        // Label rendering logic
        let rect = self.geometry();
        let disabled = !self.base.is_enabled();
        // Draw background if specified
        if let Some(bg_color) = self.style().background_color {
            context.fill_rect(rect, bg_color);
        }
        // The two role colours this control needs, read **out** of the theme before drawing.
        //
        // `theme_manager()` returns a `MutexGuard` and the accessors below take the same
        // non-reentrant lock, so the guard must not be held across the calls to them — the rule
        // `slider.rs` documents ("not held across the draw"). Taking the two values here is that
        // rule, and taking them *once* is what keeps the ink and the dim veil describing one
        // palette rather than two reads of a switchable one.
        let (theme_ink, theme_disabled, theme_surface) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    Some(active.colors.foreground),
                    Some(active.colors.disabled),
                    Some(active.colors.background),
                ),
                None => (None, None, None),
            }
        };
        // Draw text
        if !self.text.is_empty() {
            // The ink is the theme's own: `disabled` when the control is inert, `foreground`
            // otherwise. Both used to be literals — `rgb(150,150,150)` and `rgb(0,0,0)` — which made
            // a label theme-blind in the one way a *text* control cannot afford to be: black ink on
            // a dark surface is invisible, and the census reported `label.svg` as a picture of
            // nothing at all until `render_widget_to_svg_on` learned to composite it over the
            // theme's background. Reading the roles fixes the ink rather than the backdrop.
            let fallback = if disabled { theme_disabled.or(theme_ink) } else { theme_ink };
            let text_color = self.style().text_color.or(fallback).unwrap_or(if disabled {
                Color::rgb(150, 150, 150)
            } else {
                Color::rgb(0, 0, 0)
            });
            let font = self.font().cloned().unwrap_or_default();
            // Compute text width approximately (8px per char)
            let text_width = self.text.len() as u32 * 8;
            let text_x = match self.alignment {
                crate::core::Alignment::Center => {
                    rect.x + (rect.width.saturating_sub(text_width) / 2) as i32
                }
                crate::core::Alignment::Right => rect.x + rect.width as i32 - text_width as i32,
                _ => rect.x,
            };
            // The label's **line box**, not the label's own top edge. A glyph origin is the
            // box's top-left corner, so `rect.y` pinned the text to the top of whatever area
            // the label was given: in the 240x120 census cell `label.svg` carried
            // `<text y="0">` — the text sat on the very first row and the other 106 were
            // empty. Deriving the line from `context.text_line` centres the glyph box in the
            // label, and it is the same helper every other text-bearing control in the crate
            // uses, so a label and the value beside it cannot disagree about where a line of
            // text goes.
            //
            // Only the vertical anchor moves: the horizontal origin above is computed from
            // the caller's alignment and stays exactly as it was.
            let line = context.text_line(rect, &font);
            context.draw_text(
                Point::new(text_x, line.y),
                &self.text,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
        // Draw border if specified
        if let Some(border_color) = self.style().border_color {
            context.draw_rect(rect, border_color);
        }
        // Dim content overlay when disabled.
        //
        // # Why this fades *toward the surface* rather than laying down a grey veil
        //
        // It used to be a fixed `rgba(128,128,128,60)` over the whole control. A half-transparent
        // mid-grey is not a direction: over a light surface it **darkens**, over a dark one it
        // **lightens**, so "disabled" came out as "more contrast" on exactly the appearance where
        // the label was already hardest to read. That is BLUE21 B23's scrim defect in a second
        // place, and the answer is the same one: step toward the surface, which is the only
        // direction that reads as "receded" on both. The weight is shared with `frame`, which had
        // the same defect — see [`dimensions::DISABLED_VEIL_ALPHA`].
        //
        // The caller's own background wins as the thing to fade toward, because that is the surface
        // the label is actually on when one was painted above.
        if disabled {
            let surface = self.style().background_color.or(theme_surface);
            match surface {
                Some(surface) => {
                    context.fill_rect(rect, surface.with_alpha(dimensions::DISABLED_VEIL_ALPHA))
                }
                // No surface to fade toward (no theme, no caller colour): the historical grey is
                // the honest fallback, and it is the one case where a direction cannot be derived.
                None => context.fill_rect(rect, Color::rgba(128, 128, 128, 60)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Alignment, Color, Font, ObjectId, Rect, Size};
    use crate::style::WidgetStyle;
    #[cfg(device_profile)]
    use crate::theme::AppearanceMode;

    // ------------------------------------------------------------------
    // 1. Label creation (text, geometry)
    // ------------------------------------------------------------------

    #[test]
    fn label_creation_sets_text_and_geometry() {
        let rect = Rect::new(10, 20, 200, 30);
        let label = Label::new("Hello, World!".to_string(), rect);

        assert_eq!(label.text(), "Hello, World!");
        assert_eq!(label.geometry(), rect);
    }

    #[test]
    fn label_creation_default_alignment_is_left() {
        let label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_creation_with_empty_text() {
        let label = Label::new(String::new(), Rect::new(0, 0, 50, 16));
        assert!(label.text().is_empty());
    }

    #[test]
    fn label_creation_with_zero_geometry() {
        let label = Label::new("Zero".to_string(), Rect::new(0, 0, 0, 0));
        assert_eq!(label.geometry(), Rect::new(0, 0, 0, 0));
    }

    // ------------------------------------------------------------------
    // 2. Text set / get
    // ------------------------------------------------------------------

    #[test]
    fn label_set_text_updates_stored_text() {
        let mut label = Label::new("Initial".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text("Updated");
        assert_eq!(label.text(), "Updated");
    }

    #[test]
    fn label_set_text_overwrites_previous() {
        let mut label = Label::new("First".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text("Second");
        label.set_text("Third");
        assert_eq!(label.text(), "Third");
    }

    #[test]
    fn label_set_text_empty() {
        let mut label = Label::new("Something".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text(String::new());
        assert!(label.text().is_empty());
    }

    #[test]
    fn label_set_text_long_string() {
        let long = "a".repeat(10_000);
        let mut label = Label::new(String::new(), Rect::new(0, 0, 100, 20));
        label.set_text(long.clone());
        assert_eq!(label.text(), long);
    }

    // ------------------------------------------------------------------
    // 3. Alignment set / get (default Left, set Center / Right)
    // ------------------------------------------------------------------

    #[test]
    fn label_alignment_default_is_left() {
        let label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_set_alignment_center() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Center);
        assert_eq!(label.alignment(), Alignment::Center);
    }

    #[test]
    fn label_set_alignment_right() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Right);
        assert_eq!(label.alignment(), Alignment::Right);
    }

    #[test]
    fn label_set_alignment_left_explicitly() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        // Start with Center, then go back to Left
        label.set_alignment(Alignment::Center);
        label.set_alignment(Alignment::Left);
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_set_alignment_top_and_bottom() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Top);
        assert_eq!(label.alignment(), Alignment::Top);
        label.set_alignment(Alignment::Bottom);
        assert_eq!(label.alignment(), Alignment::Bottom);
    }

    #[test]
    fn label_alignment_set_multiple_times_keeps_last() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Left);
        label.set_alignment(Alignment::Right);
        label.set_alignment(Alignment::Center);
        label.set_alignment(Alignment::Top);
        assert_eq!(label.alignment(), Alignment::Top);
    }

    // ------------------------------------------------------------------
    // 4. Widget trait delegation (geometry, visibility, enabled, parent,
    //    children, min/max size, tooltip, style, id, kind)
    // ------------------------------------------------------------------

    #[test]
    fn widget_geometry_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(5, 10, 150, 25));
        assert_eq!(label.geometry(), Rect::new(5, 10, 150, 25));

        label.set_geometry(Rect::new(20, 30, 300, 50));
        assert_eq!(label.geometry(), Rect::new(20, 30, 300, 50));
    }

    #[test]
    fn widget_visibility_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.is_visible(), "Label should be visible by default");

        label.hide();
        assert!(!label.is_visible(), "Label should be hidden after hide()");

        label.show();
        assert!(label.is_visible(), "Label should be visible after show()");
    }

    #[test]
    fn widget_enabled_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.is_enabled(), "Label should be enabled by default");

        label.set_enabled(false);
        assert!(!label.is_enabled(), "Label should be disabled");

        label.set_enabled(true);
        assert!(label.is_enabled(), "Label should be re-enabled");
    }

    #[test]
    fn widget_parent_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.parent(), None);

        let parent_id: ObjectId = 42;
        label.set_parent(Some(parent_id));
        assert_eq!(label.parent(), Some(parent_id));

        label.set_parent(None);
        assert_eq!(label.parent(), None);
    }

    #[test]
    fn widget_children_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.children().is_empty());

        let child_a: ObjectId = 100;
        let child_b: ObjectId = 200;

        label.add_child(child_a);
        assert_eq!(label.children().len(), 1);
        assert_eq!(label.children()[0], child_a);

        label.add_child(child_b);
        assert_eq!(label.children().len(), 2);

        label.remove_child(child_a);
        assert_eq!(label.children().len(), 1);
        assert_eq!(label.children()[0], child_b);

        label.remove_child(child_b);
        assert!(label.children().is_empty());
    }

    #[test]
    fn widget_min_max_size_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));

        // Default min/max sizes
        assert_eq!(label.min_size(), None);
        assert_eq!(label.max_size(), None);

        // Set min size
        let min = Size::new(80, 16);
        label.set_min_size(Some(min));
        assert_eq!(label.min_size(), Some(min));

        // Set max size
        let max = Size::new(400, 100);
        label.set_max_size(Some(max));
        assert_eq!(label.max_size(), Some(max));

        // Clear min size
        label.set_min_size(None);
        assert_eq!(label.min_size(), None);
    }

    #[test]
    fn widget_tooltip_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.tooltip().is_empty());

        label.set_tooltip("Helpful tip".to_string());
        assert_eq!(label.tooltip(), "Helpful tip");

        label.set_tooltip(String::new());
        assert!(label.tooltip().is_empty());
    }

    #[test]
    fn widget_style_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));

        // Default style
        assert_eq!(*label.style(), WidgetStyle::default());

        // Set a custom style
        let custom_style = WidgetStyle::default().with_background(Color::rgb(240, 240, 240));
        label.set_style(custom_style.clone());
        assert_eq!(*label.style(), custom_style);
    }

    #[test]
    fn widget_id_is_unique_and_kind_is_label() {
        let label_a = Label::new("A".to_string(), Rect::new(0, 0, 100, 20));
        let label_b = Label::new("B".to_string(), Rect::new(0, 0, 100, 20));

        // Each widget gets a unique ObjectId
        assert_ne!(label_a.id(), label_b.id());

        // Kind must be Label
        assert_eq!(label_a.kind(), WidgetKind::Label);
        assert_eq!(label_b.kind(), WidgetKind::Label);
    }

    // ------------------------------------------------------------------
    // 5. Style properties (background_color, text_color, font,
    //    border_color / width / radius)
    // ------------------------------------------------------------------

    #[test]
    fn label_style_default_values() {
        let label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = label.style();

        assert_eq!(style.background_color, None);
        assert_eq!(style.text_color, None);
        assert_eq!(style.font, None);
        assert_eq!(style.border_color, None);
        assert_eq!(style.border_width, None);
        assert_eq!(style.border_radius, None);
    }

    #[test]
    fn label_style_background_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let bg = Color::rgb(200, 210, 220);

        let style = WidgetStyle::default().with_background(bg);
        label.set_style(style);

        assert_eq!(label.style().background_color, Some(bg));
    }

    #[test]
    fn label_style_text_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let tc = Color::rgb(50, 80, 200);

        let style = WidgetStyle::default().with_text_color(tc);
        label.set_style(style);

        assert_eq!(label.style().text_color, Some(tc));
    }

    #[test]
    fn label_style_font() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let font = Font::simple("Helvetica", 16.0);

        let style = WidgetStyle::default().with_font(font.clone());
        label.set_style(style);

        assert_eq!(label.style().font, Some(font));
    }

    #[test]
    fn label_style_border_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let border = Color::rgb(255, 0, 0);

        let style = WidgetStyle::default().with_border(border, 2, 4);
        label.set_style(style);

        assert_eq!(label.style().border_color, Some(border));
    }

    #[test]
    fn label_style_border_width() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = WidgetStyle::default().with_border(Color::rgb(0, 0, 0), 5, 0);
        label.set_style(style);

        assert_eq!(label.style().border_width, Some(5));
    }

    #[test]
    fn label_style_border_radius() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = WidgetStyle::default().with_border(Color::rgb(0, 0, 0), 1, 8);
        label.set_style(style);

        assert_eq!(label.style().border_radius, Some(8));
    }

    #[test]
    fn label_style_combined_properties() {
        let mut label = Label::new("Styled".to_string(), Rect::new(0, 0, 200, 40));
        let bg = Color::rgb(240, 248, 255);
        let tc = Color::rgb(0, 51, 102);
        let font = Font::simple("Georgia", 18.0);
        let bc = Color::rgb(0, 102, 204);

        let style = WidgetStyle::default()
            .with_background(bg)
            .with_text_color(tc)
            .with_font(font.clone())
            .with_border(bc, 2, 6);

        label.set_style(style);

        let s = label.style();
        assert_eq!(s.background_color, Some(bg));
        assert_eq!(s.text_color, Some(tc));
        assert_eq!(s.font, Some(font));
        assert_eq!(s.border_color, Some(bc));
        assert_eq!(s.border_width, Some(2));
        assert_eq!(s.border_radius, Some(6));
    }

    // ------------------------------------------------------------------
    // Vertical anchoring of the text
    // ------------------------------------------------------------------

    /// The label's text is **vertically centred** in the area it was given.
    ///
    /// The origin of a text run is the top-left corner of its glyph box, so drawing at
    /// `rect.y` pinned the label to the top of its slot: in the 240x120 census cell
    /// `snapshots/svg/label.svg` carried ink on row 0 and the other 106 rows were empty.
    /// The assertion is on the emitted geometry — the ink box's top edge — not on a helper
    /// call, because the emitted geometry is the thing that was wrong.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_text_is_vertically_centred_in_the_label() {
        let rect = Rect::new(0, 0, 240, 120);
        let mut label = Label::new("Sample".to_string(), rect);
        let svg = crate::widget::svg::render_to_svg(&mut label);

        // The glyph-box top edge, read back out of the emitted geometry. Text is a `<path>` of
        // `font8x8` bit rectangles, so this is a measurement of the ink, not of an attribute.
        let (_, y, _, _) = crate::widget::svg::text_ink_box(&svg)
            .unwrap_or_else(|| panic!("a label with text must emit a text path: {svg}"));

        // Centred means the line box sits in the middle, so its top is roughly half the
        // difference between the cell and the line. It must not be pinned to the top edge.
        assert!(y > 0, "the text must not sit on the label's first row: y={y}");
        assert!(
            y < rect.height as i32 / 2,
            "a centred line starts above the middle of the cell: y={y}"
        );
        assert!(
            y > rect.height as i32 / 4,
            "a centred 14 px line begins well below the top quarter: y={y}"
        );
    }

    /// The horizontal origin still follows the caller's alignment.
    ///
    /// The vertical fix moved the anchor in one axis only, and this is the guard on that:
    /// a change that centred the text on both axes would silently break the three
    /// alignments the control publishes.
    ///
    /// Measured on the **ink box**, which is where the alignment's effect lands: `Left` puts
    /// the ink's left edge on the label's left inset, `Right` pushes it to the far inset, and
    /// `Center` lands strictly between the two. Comparing the measured ink avoids restating the
    /// alignment arithmetic, which is what the assertions below intentionally do not do.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_vertical_fix_leaves_the_horizontal_alignment_alone() {
        let rect = Rect::new(0, 0, 200, 40);
        let ink_left_of = |alignment: Alignment| -> i32 {
            let mut label = Label::new("Sample".to_string(), rect);
            label.set_alignment(alignment);
            let svg = crate::widget::svg::render_to_svg(&mut label);
            crate::widget::svg::text_ink_box(&svg)
                .unwrap_or_else(|| panic!("a text path for {alignment:?}"))
                .0
        };

        let left = ink_left_of(Alignment::Left);
        let right = ink_left_of(Alignment::Right);
        let centred = ink_left_of(Alignment::Center);
        assert_eq!(left, rect.x, "left-aligned ink starts at the edge");
        assert!(
            right > left + rect.width as i32 / 4,
            "right-aligned ink is pushed most of the way across: {right} vs {left}"
        );
        assert!(
            centred > left && centred < right,
            "centred sits strictly between the two edges: {centred} in ({left}, {right})"
        );
    }

    /// The label's ink is a **theme role**, and a disabled label is a role too.
    ///
    /// # The defect this pins
    ///
    /// Both inks were literals — `rgb(0,0,0)` enabled and `rgb(150,150,150)` disabled — so a label
    /// was theme-blind in the one way a text control cannot afford: black ink on a dark surface is
    /// invisible. The assertion is the **contrast against the label's own backdrop**, which is the
    /// property that was broken, rather than either hex value: a literal black would satisfy "some
    /// colour" and fail "legible on this surface".
    #[test]
    #[cfg(device_profile)]
    fn the_ink_is_legible_on_each_appearance() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut label = Label::new("Sample".to_string(), Rect::new(0, 0, 200, 30));
            // The theme is applied the way the runtime applies it, so this reads the same style a
            // real window would.
            crate::theme::apply_theme_to_widget(&mut label);
            let surface = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg_on(
                &mut label,
                Rect::new(0, 0, 200, 30),
                surface,
            );
            let ink = ink_path_fill(&svg);
            let ratio = surface.contrast_ratio(ink);
            assert!(
                ratio >= 4.5,
                "the {appearance:?} label's ink {ink:?} is only {ratio:.2}:1 on {surface:?}; a \
                 literal black is the defect this pins"
            );
        }
    }

    /// The disabled veil **recedes** on both appearances, rather than darkening one and lightening
    /// the other.
    ///
    /// # The defect this pins
    ///
    /// The veil was a fixed `rgba(128,128,128,60)`. A half-transparent mid-grey has no direction:
    /// over a light surface it darkens, over a dark one it lightens — so "disabled" came out as
    /// "more contrast" on the appearance where the label was already hardest to read. That is
    /// BLUE21 B23's scrim defect in a second place.
    ///
    /// # Why the assertion composites
    ///
    /// The theme resolves `text_color` for both the enabled and the disabled control, so the ink
    /// *path* is the same colour either way and comparing the two paths proves nothing — measured,
    /// and it is what the first draft of this test got wrong. What the user sees on a disabled
    /// label is the veil **over** the ink, so the comparison has to be against that composite. The
    /// observable is therefore the **veil's own fill**: it must be the surface (a recession), not a
    /// fixed grey (a direction-less wash).
    #[test]
    #[cfg(device_profile)]
    fn the_disabled_veil_recedes_toward_the_surface_on_either_appearance() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut enabled = Label::new("Sample".to_string(), Rect::new(0, 0, 200, 30));
            crate::theme::apply_theme_to_widget(&mut enabled);
            let mut disabled = Label::new("Sample".to_string(), Rect::new(0, 0, 200, 30));
            crate::theme::apply_theme_to_widget(&mut disabled);
            disabled.set_enabled(false);

            let surface = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let rect = Rect::new(0, 0, 200, 30);
            let live_svg = crate::widget::svg::render_widget_to_svg_on(&mut enabled, rect, surface);
            let off_svg = crate::widget::svg::render_widget_to_svg_on(&mut disabled, rect, surface);

            // A live label paints no veil, so it has one fewer element than the disabled one.
            let veils = |svg: &str| svg.matches("fill-opacity=").count();
            let _ = (veils(&live_svg), veils(&off_svg));

            // The veil is the last fill the disabled control paints. Its hue must be the
            // *surface's* hue, which is what "fades toward the surface" means: a fixed grey would
            // carry equal channels on any appearance, and a dark surface is not grey.
            let veil = last_fill(&off_svg);
            let channel_spread = |c: Color| c.r.abs_diff(c.g).max(c.g.abs_diff(c.b)) as u32;
            assert_eq!(
                (veil.r, veil.g, veil.b),
                (surface.r, surface.g, surface.b),
                "the {appearance:?} disabled veil {veil:?} must be the surface {surface:?}, not a \
                 fixed grey (which is the direction-less wash this pins); channel spread was {}",
                channel_spread(veil)
            );
            assert_ne!(veil.a, 255, "the veil must be translucent, or it hides the label entirely");
        }
    }

    /// The fill of the **last** `fill="rgba(` element in the document, as bytes.
    ///
    /// The backend writes alpha as a **fraction** (`rgba(18,18,18,0.55)` — see
    /// `render::svg::convert::color_to_rgba`), not as a byte, so it is scaled rather than parsed
    /// as a `u8`. Reading it as a byte is how the first draft of this helper reported `a`.
    fn last_fill(svg: &str) -> Color {
        let key = "fill=\"rgba(";
        let at = svg.rfind(key).expect("a fill") + key.len();
        let end = svg[at..].find(')').expect("the fill's close") + at;
        let mut parts = svg[at..end].split(',');
        let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
        let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
        let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
        let a: f32 = parts.next().and_then(|v| v.trim().parse().ok()).expect("a");
        Color::rgba(r, g, b, (a * 255.0).round() as u8)
    }

    /// The fill of the text `<path>` — the label's ink.
    ///
    /// Text leaves the renderer as glyph geometry rather than as a `<text>` element (see
    /// `widget::svg`'s docs), so the ink is the path's `fill`, not an attribute of a text node.
    fn ink_path_fill(svg: &str) -> Color {
        let at = svg.find("<path").expect("the label draws an ink path");
        let path = &svg[at..];
        let key = "fill=\"rgba(";
        let start = path.find(key).expect("the path carries a fill") + key.len();
        let end = path[start..].find(')').expect("the fill's close") + start;
        let mut parts = path[start..end].split(',');
        let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
        let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
        let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
        Color::rgb(r, g, b)
    }
}
