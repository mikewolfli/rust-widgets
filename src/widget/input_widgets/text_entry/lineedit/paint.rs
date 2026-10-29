// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Everything [`LineEdit`] measures or paints: geometry, caches, carets and `impl Draw`.

use super::model::*;

use crate::compat::String;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::platform::EchoMode;
use crate::render::RenderContext;

use crate::widget::decorations::{DecorationLayout, DecorationMetrics, DECORATION_GAP};
use crate::widget::metrics::{
    active_profile, dimensions, effective_font, estimate_text_width, profile_metrics,
    ControlMetrics,
};
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::widget_trait::InputMethodQuery;
use crate::widget::{Draw, Widget};

impl LineEdit {
    /// The advance of `text[from..to]`, memoised for the current value/echo/font/scale.
    ///
    /// The borrows are `try_borrow`/`try_borrow_mut` so a re-entrant paint (a backend that asks a
    /// widget to measure while it is already inside this method) falls back to measuring instead
    /// of panicking.
    pub(super) fn cached_advance(
        &self,
        context: &RenderContext,
        source: AdvanceSource,
        text: &str,
        font: &Font,
        from: usize,
        to: usize,
    ) -> i32 {
        let dpi_bits = context.dpi_scale().to_bits();
        if let Ok(cache) = self.text_advances.try_borrow() {
            if cache.revision == self.text_revision
                && cache.echo_mode == Some(self.echo_mode)
                && cache.dpi_bits == dpi_bits
                && cache.font.as_ref() == Some(font)
            {
                let entries = match source {
                    AdvanceSource::Display => &cache.display,
                    AdvanceSource::Value => &cache.value,
                };
                if let Some((_, _, advance)) =
                    entries.iter().find(|(start, end, _)| *start == from && *end == to)
                {
                    return *advance;
                }
            }
        }
        let advance =
            if from < to { context.measure_text(&text[from..to], font).width as i32 } else { 0 };
        if let Ok(mut cache) = self.text_advances.try_borrow_mut() {
            if cache.revision != self.text_revision
                || cache.echo_mode != Some(self.echo_mode)
                || cache.dpi_bits != dpi_bits
                || cache.font.as_ref() != Some(font)
            {
                cache.revision = self.text_revision;
                cache.echo_mode = Some(self.echo_mode);
                cache.dpi_bits = dpi_bits;
                cache.font = Some(font.clone());
                cache.display.clear();
                cache.value.clear();
            }
            let entries = match source {
                AdvanceSource::Display => &mut cache.display,
                AdvanceSource::Value => &mut cache.value,
            };
            // A moved caret or selection adds one entry per distinct offset within a revision, so
            // the list is capped. The oldest entry is evicted rather than the whole list dropped:
            // clearing it threw away the value's own advance on every 33rd caret move, which is
            // the one entry every frame needs (I-P13).
            if entries.len() >= 32 {
                entries.remove(0);
            }
            entries.push((from, to, advance));
        }
        advance
    }
}

impl LineEdit {
    /// Returns whether a press at `pos` lands on the clear button's hit area.
    pub(super) fn clear_button_hit(&self, pos: Point) -> bool {
        if !self.clear_button_enabled || self.text.is_empty() || self.read_only {
            return false;
        }
        self.clear_button_rect().contains_point(pos)
    }

    /// The clear button's box, in the field's trailing content corner.
    pub(super) fn clear_button_rect(&self) -> Rect {
        let field = self.field_rect();
        let padding = dimensions::TEXT_FIELD_PADDING_H as i32;
        let size = CLEAR_BUTTON_SIZE as i32;
        Rect::new(
            (field.x + field.width as i32 - padding - size).max(field.x),
            field.y + ((field.height as i32 - size) / 2).max(0),
            size as u32,
            size as u32,
        )
    }

    /// The widest offset the value can scroll, in pixels.
    ///
    /// Derived from the same measurement the paint uses, so a value that fits reports zero and
    /// [`Self::set_scroll_x`] cannot park the value in empty space.
    pub(super) fn max_scroll_x(&self) -> i32 {
        if self.text.is_empty() {
            return 0;
        }
        let font = effective_font(self.base.style());
        let (_, width) = self.value_box_geometry();
        // The same table the hit test reads, so the scrolled origin, the pointer's view of it and
        // the paint all describe one ruler (I-P13).
        let advance = self.with_prefix_advances(font, |advances| advances.total());
        (advance + LINE_EDIT_CARET_ROOM as i32 - width).max(0)
    }

    /// The value box's left edge and width, measured without a paint context.
    pub(super) fn value_box_geometry(&self) -> (i32, i32) {
        let field = self.field_rect();
        let font = effective_font(self.base.style());
        let padding = dimensions::TEXT_FIELD_PADDING_H as i32;
        let gap = DECORATION_GAP as i32;
        let inner_left = field.x + padding;
        let inner_right = field.x + field.width as i32 - padding;
        let prefix_width = if self.decorations.prefix.is_empty() {
            0
        } else {
            estimate_text_width(&self.decorations.prefix, font, 1.0) as i32
        };
        let suffix_width = if self.decorations.suffix.is_empty() {
            0
        } else {
            estimate_text_width(&self.decorations.suffix, font, 1.0) as i32
        };
        let left = if prefix_width > 0 { inner_left + prefix_width + gap } else { inner_left };
        let clear_room = if self.clear_button_enabled { CLEAR_BUTTON_SIZE as i32 + gap } else { 0 };
        let right = if suffix_width > 0 {
            inner_right - suffix_width - gap - clear_room
        } else {
            inner_right - clear_room
        };
        (left, (right - left).max(0))
    }

    /// Scrolls the value so the caret is inside the visible box.
    ///
    /// Called after every edit and caret move. A value narrower than the field keeps `scroll_x`
    /// at zero, which is what preserves the existing pixels for every non-overflowing value.
    pub(super) fn ensure_caret_visible(&mut self) {
        if self.text.is_empty() {
            self.scroll_x = 0;
            return;
        }
        let font = effective_font(self.base.style());
        let (_, width) = self.value_box_geometry();
        let caret = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
        // The caret's prefix width comes from the same table the hit test and `draw`'s scroll
        // bounds use, instead of a second, differently-rounded estimate (I-P13).
        let caret_advance = self.with_prefix_advances(font, |advances| {
            advances.advance_at(caret).unwrap_or_else(|| advances.total())
        });
        let caret_room = LINE_EDIT_CARET_ROOM as i32;
        // Reveal the caret with one caret of slack before the trailing edge, which is the same
        // slack `size_hint` reserves so the marker never sits on the border.
        if caret_advance - self.scroll_x > width - caret_room {
            self.scroll_x = (caret_advance - width + caret_room).max(0);
        } else if caret_advance < self.scroll_x {
            self.scroll_x = caret_advance;
        }
        self.scroll_x = self.scroll_x.clamp(0, self.max_scroll_x());
    }
}

impl LineEdit {
    /// The byte offset a horizontal pixel `x` points at.
    ///
    /// # How the pixel is turned into an index
    ///
    /// The value's advances are read from [`PrefixAdvances`] — the same context-free ruler the
    /// scroll bounds use — and the caret goes to the boundary nearest the click (I-P03). This used
    /// to construct a zero-sized paint backend and shape the whole value on every press and every
    /// drag step, i.e. a cold start of the whole measurement stack for the field's most frequent
    /// pointer operation. The cache is rebuilt only when the text revision or the font changes, so
    /// consecutive clicks and a drag read one table.
    ///
    /// The value is clipped to the same [`Self::value_box_geometry`] box the paint and the press
    /// hit-test use, and the result is snapped to a character boundary, which is what keeps
    /// `&self.text[..caret]` total when the value contains multi-byte characters.
    pub(super) fn byte_index_at_x(&self, x: i32) -> usize {
        if self.text.is_empty() {
            return 0;
        }
        let font = effective_font(self.base.style());
        let (left, width) = self.value_box_geometry();
        // The pointer's distance from the value's origin, clamped to the value's own box so a
        // press on the field's padding (or past the end of a short value) reads as "the nearest
        // end" rather than as a negative index.
        let dx = (x - left + self.scroll_x).clamp(0, width);
        self.with_prefix_advances(font, |advances| {
            // The pointer's offset is turned into the nearest **measured** boundary. The table
            // holds a real advance per character, so a proportional face cannot put the caret on
            // the wrong glyph (I-004).
            let boundary = advances.boundary_at_advance(dx);
            advances.byte_at(boundary)
        })
    }
}

impl LineEdit {
    /// The field the control actually paints.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A text field is a *fixed-height band*: the active profile's text-field height is the
    /// content floor every field in this crate shares — Material and Cupertino's 48 px, or
    /// macOS's compact 22 px. Painting `rect` made a 240x120 census cell a 240x120 white
    /// slab — a rectangle pretending to be a field — and it also made `size_hint`'s 24 disagree
    /// with the ink by a factor of five. [`ControlMetrics::full_width_band`] keeps the full
    /// width, takes the profile's height and centres it, which is exactly what stops a field
    /// from drawing as a 120 px panel.
    ///
    /// Everything the control paints **and everything it hit-tests** is placed from this
    /// one box, so the clickable area cannot drift away from the ink.
    pub(super) fn field_rect(&self) -> Rect {
        ControlMetrics::full_width_band(
            self.geometry(),
            profile_metrics(active_profile()).text_field_min_height,
        )
    }
}

impl LineEdit {
    /// The boxes the field's five regions occupy, measured for the current font.
    ///
    /// # Why the layout is computed from `field_rect()` and not from `geometry()`
    ///
    /// The decoration slots belong to the **field**, which on a tall control is a band centred inside
    /// it. Measuring from the control's rectangle would put the helper row under the control instead of
    /// under the field, and the support row would be as wide as the cell rather than as the input.
    pub(super) fn decoration_layout(&self, context: &mut RenderContext) -> DecorationLayout {
        let field = self.field_rect();
        let style = self.base.style();
        // The effective font, taken through the shared accessor so this layout, `byte_index_at_x`
        // and `draw` all measure with one font (D09-STYLE-02).
        let font = effective_font(style);
        let dpi_bits = context.dpi_scale().to_bits();
        // The layout depends on the field box, the font, the DPI and the five decoration slots
        // (plus the counter, which follows the value and the limit). All of those advance
        // `text_revision`, so one revision comparison covers the whole input (I-P13).
        if let Ok(cache) = self.layout_cache.try_borrow() {
            if let Some(entry) = cache.as_ref() {
                if entry.field == field
                    && entry.revision == self.text_revision
                    && entry.dpi_bits == dpi_bits
                    && entry.font == *font
                {
                    return entry.layout;
                }
            }
        }
        let metrics = DecorationMetrics::measure(&self.decorations, |text| {
            context.measure_text(text, font).width
        });
        let counter_width =
            self.counter_text().map(|text| context.measure_text(&text, font).width).unwrap_or(0);
        let line_height = font.effective_line_height().max(1.0) as u32;
        let layout = DecorationLayout::compute(
            field,
            dimensions::TEXT_FIELD_PADDING_H,
            line_height,
            DECORATION_GAP,
            metrics,
            counter_width,
            &self.decorations,
        );
        if let Ok(mut cache) = self.layout_cache.try_borrow_mut() {
            *cache = Some(LayoutCacheEntry {
                field,
                revision: self.text_revision,
                dpi_bits,
                font: font.clone(),
                layout,
            });
        }
        layout
    }

    /// The caret's window rectangle and byte offset, as a native IME asks for them.
    ///
    /// # Why this is measured without a `RenderContext`
    ///
    /// The IME query is answered on a `&self` borrow between frames, when no paint context is
    /// available. [`estimate_text_width`] is the crate's context-free model of the renderer's
    /// ruler, and [`crate::render::fitted_origin`] is the same origin function `draw` lays the
    /// value out with, so the reported caret rides with the glyphs instead of drifting from them.
    /// The box arithmetic mirrors [`DecorationLayout::compute`]: the prefix and suffix slots keep
    /// their own edges, and the value — and therefore the caret — is placed between them.
    pub(super) fn caret_query(&self) -> InputMethodQuery {
        let field = self.field_rect();
        let font = effective_font(self.base.style());
        let padding = dimensions::TEXT_FIELD_PADDING_H as i32;
        let gap = DECORATION_GAP as i32;
        let inner_left = field.x + padding;
        let inner_right = field.x + field.width as i32 - padding;
        let prefix_width = if self.decorations.prefix.is_empty() {
            0
        } else {
            estimate_text_width(&self.decorations.prefix, font, 1.0) as i32
        };
        let suffix_width = if self.decorations.suffix.is_empty() {
            0
        } else {
            estimate_text_width(&self.decorations.suffix, font, 1.0) as i32
        };
        let value_left =
            if prefix_width > 0 { inner_left + prefix_width + gap } else { inner_left };
        let value_right =
            if suffix_width > 0 { inner_right - suffix_width - gap } else { inner_right };
        let value_width = (value_right - value_left).max(0) as u32;

        // The same value box and alignment `draw` uses, so a centred or right-aligned value keeps
        // its caret on its own glyphs rather than at the field's leading edge.
        let fit = crate::render::TEXT_FIT_MARGIN as i32;
        let value_box = Rect::new(
            value_left - fit,
            field.y,
            value_width.saturating_add((fit * 2) as u32),
            field.height,
        );
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        // The text the field paints, which is what the value's advance is measured from. The
        // caret's prefix is measured from `text` itself, exactly as `draw` does, so a masked
        // field still reports the position of the caret in its own value.
        let masked: core::cell::Ref<'_, String>;
        let display_text: &str = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            match self.echo_mode {
                EchoMode::Normal => &self.text,
                EchoMode::NoEcho => "",
                EchoMode::Password => {
                    masked = self.mask_text_cached();
                    masked.as_str()
                }
            }
        };
        let value_advance = estimate_text_width(display_text, font, 1.0) as i32;
        let value_origin = crate::render::fitted_origin(value_box, value_advance, value_align);
        let caret_byte = floor_char_boundary(self.text.as_str(), self.cursor_position);
        // The caret's prefix advance comes from the same table the pointer hit test and the scroll
        // bounds read, so the IME window, the pointer and the paint agree (I-P13).
        let prefix_advance = self.with_prefix_advances(font, |advances| {
            advances.advance_at(caret_byte).unwrap_or_else(|| advances.total())
        });
        let caret_x = (value_origin.x + prefix_advance - self.scroll_x)
            .min(value_left + value_width as i32)
            .max(value_left);

        // The caret spans the field's content band, inset so it does not touch the border — the
        // same two-pixel inset `draw` uses.
        let caret_top = field.y + 2;
        let caret_height = field.height.saturating_sub(4).max(1);
        InputMethodQuery {
            caret_rect: Rect::new(caret_x, caret_top, 1, caret_height),
            caret_byte: Some(caret_byte),
        }
    }
}

impl Draw for LineEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        //
        // The **field**, not the control's rectangle: see `field_rect`. Every measurement
        // below — the fill, the border, the text's line box and the caret's top and bottom
        // — is taken from this one box, so they cannot disagree about where the field is.
        let rect = self.field_rect();
        let style = self.style();
        // ── The decorated layout ──
        //
        // Every box below comes from **one** derivation, measured against the renderer's own font. The
        // value's origin, the caret, the two slots and the support row cannot disagree, because they
        // are the same answer read for different purposes. The value's origin used to be
        // `rect.x + padding` regardless of any slot, so a `$` would have been drawn *over* the value
        // it marks; and the caret measured from that same fixed inset, so it sat at the wrong end of
        // a centred or right-aligned field.
        let layout = self.decoration_layout(context);
        // Draw background
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.face_with_gradient_corners(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        // Draw border
        //
        // The **border** answers to the meaning channel, the fill above to the interaction one.
        // A refusal therefore keeps its outline while the pointer is over the field, which is the
        // one moment the outline is what the user is looking for. When the control carries no
        // meaning the border falls back to the interaction style's own colour, so a control with
        // `semantic_state() == None` draws exactly as it did before the two channels were split.
        let semantic_border =
            crate::style::resolved_semantic_border("line_edit", self.semantic_state());
        if let Some(border_color) = semantic_border.or(style.border_color) {
            let bw = style.border_width.unwrap_or(0);
            // STY-01: the outline follows the fill's corner radius. A square rule around a
            // rounded field left the four corners showing the backdrop.
            crate::widget::draw::stroke_surface_border(
                context,
                Rect::new(rect.x, rect.y, rect.width, rect.height),
                style.resolved_corner_radii().to_array(),
                border_color,
                bw,
            );
        }
        // Draw text or placeholder
        //
        // The value is **borrowed** when the echo mode allows it. `display_text()` returns an owned
        // `String`, so the ordinary case — normal echo, painting the field's own text — cloned the
        // whole value once per frame on the crate's highest-traffic control. Only the password mask
        // genuinely derives a string, and it is held in `masked` so the borrow can outlive the match.
        let masked: core::cell::Ref<'_, String>;
        let revealed = self.reveals_value();
        let display_text: &str = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            match self.echo_mode {
                EchoMode::Normal => &self.text,
                EchoMode::NoEcho => "",
                EchoMode::Password if revealed => &self.text,
                EchoMode::Password => {
                    masked = self.mask_text_cached();
                    masked.as_str()
                }
            }
        };
        let font = effective_font(style);
        // Whether the painted string is the field's own value (normal echo, or a revealed
        // password) rather than a derived one. The caret measures its prefix from that same
        // string when it can, so the two share one memoised advance instead of measuring two
        // copies of it.
        let display_is_value = !(self.text.is_empty() && !self.placeholder_text.is_empty())
            && match self.echo_mode {
                EchoMode::Normal => true,
                EchoMode::NoEcho => false,
                EchoMode::Password => revealed,
            };
        let value_line = context.text_line(rect, font);
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        // The box the value is laid out in, and the alignment it uses within that box.
        //
        // Resolved **once**, outside the `if` that draws the string, because the caret below needs
        // the exact same two facts: it rides with the glyphs, so it must be placed from the origin
        // the glyphs were placed from. Computing them in two places is how the caret and the value
        // drifted apart in the first place.
        //
        // `draw_text_fitted` insets its box by `TEXT_FIT_MARGIN` at each end, which is right for
        // a label that must not touch its frame but wrong for a field value: left-aligned ink
        // would then begin `TEXT_FIT_MARGIN` past the field's own padding. Handing it a box
        // widened by that same inset on both sides puts the left edge back on the padding, so
        // `Left` is byte-identical to the pre-alignment `draw_text` and the other two alignments
        // measure from the same true box.
        let fit = crate::render::TEXT_FIT_MARGIN as i32;
        let value_box = Rect::new(
            layout.value.x - fit,
            value_line.y,
            layout.value.width + (fit * 2) as u32,
            value_line.height,
        );
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        let value_advance = self.cached_advance(
            context,
            AdvanceSource::Display,
            display_text,
            font,
            0,
            display_text.len(),
        );
        let value_origin = crate::render::fitted_origin(value_box, value_advance, value_align);
        // ── Selection band ──
        //
        // Painted under the glyphs so the value stays readable while selected. The two offsets are
        // measured through the renderer's shaper, the same ruler the caret uses, and clipped to the
        // value's own box so a selection cannot spill over the prefix, the suffix or the border.
        if let Some((start, end)) = self.selection_range() {
            let start = start.min(display_text.len());
            let end = end.min(display_text.len());
            if start < end {
                let selection_color = text_color.blend(&bg, 0.3).blend(&bg, 0.25);
                let left_edge = layout.value.x;
                let right_edge = layout.value.x + layout.value.width as i32;
                let start_advance = self.cached_advance(
                    context,
                    AdvanceSource::Display,
                    display_text,
                    font,
                    0,
                    start,
                );
                let end_advance = self.cached_advance(
                    context,
                    AdvanceSource::Display,
                    display_text,
                    font,
                    0,
                    end,
                );
                let x0 =
                    (value_origin.x + start_advance - self.scroll_x).clamp(left_edge, right_edge);
                let x1 =
                    (value_origin.x + end_advance - self.scroll_x).clamp(left_edge, right_edge);
                if x1 > x0 {
                    context.draw_rect(
                        Rect::new(x0, value_line.y, (x1 - x0) as u32, value_line.height.max(1)),
                        selection_color,
                    );
                }
            }
        }
        if !display_text.is_empty() {
            if self.scroll_x == 0 {
                // The field's own line box. A glyph origin is the box's top-left edge, so the
                // previous `rect.y + rect.height / 2` placed that edge on the field's middle line
                // and drew the value half a line low.
                context.draw_text_fitted_cow(
                    value_box,
                    display_text,
                    font,
                    text_color,
                    value_align,
                );
            } else {
                // Scrolled: the value's origin moves left with the caret and the paint is clipped
                // to the value box, so the leading characters leave the field instead of being
                // elided. A value that fits never takes this path (`scroll_x` stays zero).
                context.push_clip(
                    layout.value.x,
                    value_line.y,
                    layout.value.width,
                    value_line.height,
                );
                context.draw_text(
                    Point::new(value_origin.x - self.scroll_x, value_line.y),
                    display_text,
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
                context.pop_clip();
            }
        }

        // ── The in-field slots ──
        //
        // Drawn in their **own** boxes, in a muted tone of the field's ink so they read as unit marks
        // rather than as content. They are deliberately not folded into `display_text`: the caret below
        // is measured against `self.text` alone, so a prefix in the string would put `cursor_position
        // == 0` after the `$` and make the start of the value unreachable.
        let slot_color = text_color.blend(&bg, 0.35);
        if let Some(prefix_box) = layout.prefix {
            context.draw_text(
                Point::new(prefix_box.x, value_line.y),
                &self.decorations.prefix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }
        if let Some(suffix_box) = layout.suffix {
            context.draw_text(
                Point::new(suffix_box.x, value_line.y),
                &self.decorations.suffix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }

        // ── The support row ──
        //
        // Message on the leading edge, counter on the trailing one. The error takes the theme's error
        // colour, because a refusal that is painted in the same ink as a hint is a refusal the user has
        // to read to notice.
        if let Some(row) = layout.support {
            // The validation colour is the theme's error token, not a literal red: a theme with a
            // different error hue (or a high-contrast palette) must see a consistent error state.
            let error_color = crate::style::semantic_color(crate::style::SemanticColor::Error)
                .unwrap_or(Color::rgb(200, 40, 40));
            let message = self.decorations.support_message();
            if !message.is_empty() {
                let message_color =
                    if self.decorations.has_error() { error_color } else { slot_color };
                let row_line = context.text_line(row, font);
                // Fitted to the band before the counter: a long support message used to paint
                // straight over the trailing `n/max` reading.
                let message_width = layout
                    .counter
                    .map_or(row.width, |counter| (counter.x - row.x - 4).max(0) as u32);
                context.draw_text_fitted_cow(
                    Rect::new(row.x, row_line.y, message_width, row_line.height.max(1)),
                    message,
                    font,
                    message_color,
                    HorizontalAlignment::Left,
                );
            }
            if let Some(counter_box) = layout.counter {
                let counter_line = context.text_line(counter_box, font);
                // The counter is a *budget* reading, so it turns to the error colour once the value
                // exceeds the limit — the one moment it has something to warn about.
                let counter_color = if self.is_over_limit() { error_color } else { slot_color };
                if let Some(counter) = self.counter_text() {
                    context.draw_text(
                        Point::new(counter_box.x, counter_line.y),
                        &counter,
                        font,
                        counter_color,
                        HorizontalAlignment::Right,
                    );
                }
            }
        }
        // Draw the caret for whichever field owns keyboard focus.
        if self.focused && !self.read_only && self.cursor_blink.is_visible() {
            // The caret sits at **`cursor_position`**, not at the end of the value.
            //
            // This measured the whole string, so the marker was always drawn after the last
            // glyph however the control was positioned: with `text == "Sample"` and
            // `cursor_position == 0`, the caret rendered past the `e`. The field answers
            // `cursor_position`, `set_cursor_position` clamps it, and `backspace`/`delete`
            // both act on it — so the cursor was the one thing in the field that ignored the
            // very field that defines it, and every editing affordance appeared to work at the
            // wrong end of the text.
            //
            // The slice is taken through `floor_char_boundary` because `cursor_position` indexes
            // bytes and the value may be multi-byte; slicing mid-character would panic, which is
            // why the import for it was already present in this file.
            //
            // # Why the origin is the same `fitted_origin` the value was drawn with
            //
            // A centred or right-aligned value does **not** start at `text_x`: the fit path lays it
            // out from `fitted_origin(value_box, advance, alignment)`. Measuring the caret from
            // `text_x` therefore put the marker at the left edge of a right-aligned field while its
            // own text sat at the right edge. Rather than duplicate that arithmetic here — the two
            // would drift exactly as the value's origin and the caret's once did — the whole value's
            // advance is measured and run through the same origin function, so the caret rides with
            // the glyphs it belongs to.
            let caret_byte = floor_char_boundary(self.text.as_str(), self.cursor_position);
            // The value's own advance and its fitted origin were resolved once above, for the
            // painted value; the caret rides on the same two facts instead of re-deriving them.
            // The prefix is measured from the same string the caret belongs to: the painted one
            // when it is the field's value, otherwise the value itself.
            let prefix_advance = if display_is_value {
                self.cached_advance(
                    context,
                    AdvanceSource::Display,
                    display_text,
                    font,
                    0,
                    caret_byte,
                )
            } else {
                self.cached_advance(
                    context,
                    AdvanceSource::Value,
                    self.text.as_str(),
                    font,
                    0,
                    caret_byte,
                )
            };
            let caret_x = value_origin.x + prefix_advance - self.scroll_x;
            // The marker is clipped to the **value's** box. A caret beyond the visible text (a value
            // wider than the room it has) belongs at the last pixel a user can see, not outside the
            // control and not under the suffix. Clipping to the whole field instead would let the caret
            // sit on top of a `%` and read as if the unit were part of the value.
            let caret_x = caret_x.min(layout.value.x + layout.value.width as i32);
            let caret_x = caret_x.max(layout.value.x);
            // The caret spans the field's own content band, inset so it does not touch the
            // border. The band is the field's, not the control's: with the field centred in
            // a tall cell, `rect` alone would have drawn a caret taller than the field it
            // belongs to.
            let caret_top = rect.y + 2;
            let caret_bottom = rect.y + rect.height as i32 - 2;
            let caret_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
            context.draw_line(
                Point::new(caret_x, caret_top),
                Point::new(caret_x, caret_bottom),
                caret_color,
            );

            // The uncommitted composition is drawn **after the caret**, where it will land when it is
            // committed, with an underline marking it as preedit.
            //
            // # Why the field draws this and not only `ImePreedit`
            //
            // `ImePreedit` is an overlay: a host that mounts one gets a composition string with an
            // underline, in a box of its own. But a user typing into a field expects the composition
            // **inline**, at the caret, growing as they type — that is what makes an input method
            // legible. Drawing it here is possible only because the preedit is now part of the
            // field's model (`composition`); the overlay could never place it, because it does not
            // know where this field's caret is.
            //
            // The advance is measured from the **same** `prefix` the caret uses, so the underline
            // begins exactly at the caret and cannot drift from it.
            if let Some(preedit) = self.composition.as_deref() {
                let preedit_x = caret_x;
                let preedit_width = context.measure_text(preedit, font).width as i32;
                context.push_clip(
                    layout.value.x,
                    value_line.y,
                    layout.value.width,
                    value_line.height,
                );
                context.draw_text(
                    Point::new(preedit_x, value_line.y),
                    preedit,
                    font,
                    caret_color,
                    crate::core::HorizontalAlignment::Left,
                );
                // The underline is one pixel below the glyph box, over the preedit's own width.
                // The line box above already carries the font's measured height (it is derived
                // from the same `"M"` measurement), so it is read instead of measured again.
                let underline_y = value_line.y + value_line.height as i32 + 1;
                context.draw_line(
                    Point::new(preedit_x, underline_y),
                    Point::new(preedit_x + preedit_width, underline_y),
                    caret_color,
                );
                context.pop_clip();
            }
        }

        // ── The opt-in clear button ──
        //
        // Drawn only when the field holds text, so an empty field keeps its plain corner. The
        // glyph is a cross built from two lines, which needs no icon registry and reads at any
        // scale the field's own font takes.
        if self.clear_button_enabled && !self.text.is_empty() {
            let button = self.clear_button_rect();
            let ink = slot_color;
            let inset = 4;
            let cx = button.x + button.width as i32 / 2;
            let cy = button.y + button.height as i32 / 2;
            let arm = (button.width as i32 - inset * 2).max(2) / 2;
            context.draw_line(Point::new(cx - arm, cy - arm), Point::new(cx + arm, cy + arm), ink);
            context.draw_line(Point::new(cx - arm, cy + arm), Point::new(cx + arm, cy - arm), ink);
        }

        // ── Focus ring ──
        //
        // Only a keyboard focus draws it — a pointer press focuses without claiming visual
        // focus, which is the rule every other control in the crate follows.
        if self.base.draws_focus_ring() {
            let ring = crate::widget::metrics::FocusRing::for_control(
                rect,
                style.border_radius.unwrap_or(0),
            );
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    crate::widget::metrics::focus_ring_color(bg.contrast_color()),
                    crate::widget::metrics::FOCUS_RING_WIDTH,
                );
            }
        }
    }
}
