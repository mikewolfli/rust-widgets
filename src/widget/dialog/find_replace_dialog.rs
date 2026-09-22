// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! FindReplaceDialog widget — a find/replace dialog (like VS Code/IDE find).
//!
//! Provides a two-row dialog with find and replace input fields, toggle
//! buttons for match case, whole word, regex, and highlight all, plus
//! action buttons for find next, find previous, replace, replace all,
//! and close. Emits typed signals when actions are triggered.

use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Padding inside the dialog.
const PADDING: i32 = 8;
/// Row height (input field area).
const ROW_HEIGHT: u32 = 28;
/// Small button size for toggle/action buttons.
const BTN_SIZE: u32 = 24;
/// Gap between adjacent widgets on a row.
const GAP: i32 = 6;

/// FindReplaceDialog widget — a find/replace panel for text search and replace.
pub struct FindReplaceDialog {
    base: BaseWidget,
    /// Current find (search) text.
    find_text: String,
    /// Current replace text.
    replace_text: String,
    /// Match case toggle state.
    match_case: bool,
    /// Whole word toggle state.
    whole_word: bool,
    /// Use regex toggle state.
    use_regex: bool,
    /// Highlight all matches toggle state.
    highlight_all: bool,
    /// Whether a search that reaches the end of the document continues from the top.
    wrap_around: bool,
    /// Whether the dialog is visible.
    visible: bool,

    // -- Signals --
    /// Emitted when find next is requested, provides the find text.
    pub find_next_signal: Signal1<String>,
    /// Emitted when find previous is requested, provides the find text.
    pub find_previous_signal: Signal1<String>,
    /// Emitted when replace is requested, provides the replace text.
    pub replace_signal: Signal1<String>,
    /// Emitted when replace all is requested, provides (find_text, replace_text).
    pub replace_all_signal: Signal1<(String, String)>,
    /// Emitted when the dialog is closed.
    pub close_signal: Signal1<()>,

    // Internal state tracking
    /// Which text field has focus: 0 = find, 1 = replace
    focus_field: u8,
    /// Cached row rectangles for hit-testing.
    find_row_rect: Rect,
    replace_row_rect: Rect,
}

impl FindReplaceDialog {
    /// Creates a new FindReplaceDialog with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FindReplaceDialog, geometry, "FindReplaceDialog"),
            find_text: String::new(),
            replace_text: String::new(),
            match_case: false,
            whole_word: false,
            use_regex: false,
            highlight_all: false,
            wrap_around: true,
            visible: false,
            find_next_signal: Signal1::new(),
            find_previous_signal: Signal1::new(),
            replace_signal: Signal1::new(),
            replace_all_signal: Signal1::new(),
            close_signal: Signal1::new(),
            focus_field: 0,
            find_row_rect: Rect::default(),
            replace_row_rect: Rect::default(),
        }
    }

    // ── Find text accessors ──

    /// Returns the current find text.
    pub fn find_text(&self) -> &str {
        &self.find_text
    }

    /// Sets the find text.
    pub fn set_find_text(&mut self, text: &str) {
        self.find_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current replace text.
    pub fn replace_text(&self) -> &str {
        &self.replace_text
    }

    /// Sets the replace text.
    pub fn set_replace_text(&mut self, text: &str) {
        self.replace_text = text.to_string();
        self.base.request_redraw();
    }

    // ── Match case ──

    /// Returns whether match case is enabled.
    pub fn is_match_case(&self) -> bool {
        self.match_case
    }

    /// Sets the match case toggle.
    pub fn set_match_case(&mut self, value: bool) {
        self.match_case = value;
        self.base.request_redraw();
    }

    // ── Whole word ──

    /// Returns whether whole word matching is enabled.
    pub fn is_whole_word(&self) -> bool {
        self.whole_word
    }

    /// Sets the whole word toggle.
    pub fn set_whole_word(&mut self, value: bool) {
        self.whole_word = value;
        self.base.request_redraw();
    }

    // ── Use regex ──

    /// Returns whether regex mode is enabled.
    pub fn is_use_regex(&self) -> bool {
        self.use_regex
    }

    /// Sets the regex toggle.
    pub fn set_use_regex(&mut self, value: bool) {
        self.use_regex = value;
        self.base.request_redraw();
    }

    // ── Highlight all ──

    /// Returns whether highlight all matches is enabled.
    pub fn is_highlight_all(&self) -> bool {
        self.highlight_all
    }

    /// Sets the highlight all toggle.
    pub fn set_highlight_all(&mut self, value: bool) {
        self.highlight_all = value;
        self.base.request_redraw();
    }

    // ── Wrap around ──

    /// Returns whether a search wraps around to the start of the document.
    pub fn is_wrap_around(&self) -> bool {
        self.wrap_around
    }

    /// Sets the wrap-around toggle.
    pub fn set_wrap_around(&mut self, value: bool) {
        self.wrap_around = value;
        self.base.request_redraw();
    }

    // ── Visibility ──

    /// Shows the dialog.
    pub fn show(&mut self) {
        self.visible = true;
        self.base.request_redraw();
    }

    /// Hides the dialog.
    pub fn hide(&mut self) {
        self.visible = false;
        self.base.request_redraw();
    }

    /// Returns whether the dialog is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // ── Actions ──

    /// Emits `find_next_signal` with the current find text.
    pub fn find_next(&mut self) {
        if !self.find_text.is_empty() {
            self.find_next_signal.emit(self.find_text.clone());
        }
    }

    /// Emits `find_previous_signal` with the current find text.
    pub fn find_previous(&mut self) {
        if !self.find_text.is_empty() {
            self.find_previous_signal.emit(self.find_text.clone());
        }
    }

    /// Emits `replace_signal` with the current replace text.
    pub fn replace(&mut self) {
        if !self.find_text.is_empty() {
            self.replace_signal.emit(self.replace_text.clone());
        }
    }

    /// Emits `replace_all_signal` with (find_text, replace_text).
    pub fn replace_all(&mut self) {
        if !self.find_text.is_empty() {
            self.replace_all_signal.emit((self.find_text.clone(), self.replace_text.clone()));
        }
    }

    /// Appends a character to the focused text field.
    fn append_to_focused(&mut self, ch: char) {
        match self.focus_field {
            0 => {
                self.find_text.push(ch);
                self.base.request_redraw();
            }
            1 => {
                self.replace_text.push(ch);
                self.base.request_redraw();
            }
            _ => {}
        }
    }

    /// Deletes the last character from the focused text field.
    fn backspace_focused(&mut self) {
        match self.focus_field {
            0 => {
                self.find_text.pop();
                self.base.request_redraw();
            }
            1 => {
                self.replace_text.pop();
                self.base.request_redraw();
            }
            _ => {}
        }
    }

    /// Computes layout rectangles for the two rows.
    fn compute_layout(&self) -> (Rect, Rect) {
        let geom = self.geometry();
        let row1 = Rect::new(
            geom.x + PADDING,
            geom.y + PADDING,
            geom.width.saturating_sub((PADDING as u32) * 2),
            ROW_HEIGHT,
        );
        let row2 = Rect::new(
            geom.x + PADDING,
            row1.y + row1.height as i32 + GAP,
            geom.width.saturating_sub((PADDING as u32) * 2),
            ROW_HEIGHT,
        );
        (row1, row2)
    }

    /// The line box, vertically centred in `cell`, that a label in `font` should occupy.
    ///
    /// Both rows of this panel draw each label from a hand-computed `y + height / 2 + 4`
    /// origin. The `+ 4` is not a centring term: the renderer's origin is the glyph's
    /// **top-left**, so the box painted downward from a point already below the cell's own
    /// middle and the bottom half of every label hung out of the row. Centring the measured
    /// line inside the cell is the correct placement, and naming it once keeps the entry
    /// fields, the toggles and the buttons aligned to the same rule.
    ///
    /// The arithmetic now lives in [`crate::render::text_line`], which this delegates to: the
    /// rule was correct here first and has since become the crate-wide primitive for it, so
    /// keeping a second copy would be the drift this file's own comment warns about.
    fn text_line(&self, cell: Rect, font: &crate::core::Font, context: &RenderContext) -> Rect {
        context.text_line(cell, font)
    }
}

impl Widget for FindReplaceDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(350, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `FindReplaceDialog`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for FindReplaceDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "find_text" => Ok(CapabilityValue::String(self.find_text().to_string())),
            "replace_text" => Ok(CapabilityValue::String(self.replace_text().to_string())),
            "match_case" => Ok(CapabilityValue::Bool(self.is_match_case())),
            "wrap_around" => Ok(CapabilityValue::Bool(self.is_wrap_around())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "find_text" => {
                self.set_find_text(&expect_string(value)?);
                Ok(())
            }
            "replace_text" => {
                self.set_replace_text(&expect_string(value)?);
                Ok(())
            }
            "match_case" => {
                self.set_match_case(expect_bool(value)?);
                Ok(())
            }
            "wrap_around" => {
                self.set_wrap_around(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "find_text",
            "replace_text",
            "match_case",
            "wrap_around",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for FindReplaceDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible {
            return;
        }

        let geom = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Every colour below used
        // to be one of the `Color::BACKGROUND` / `FOREGROUND` / `PRIMARY` constants, which
        // do not move when the appearance switches — the rendering census reported the
        // control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("find_replace_dialog");
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        let ink = style.text_color.or(themed_ink).unwrap_or(Color::FOREGROUND);
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::BACKGROUND);
        let border = style
            .border_color
            .or(themed_border)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.25));
        // The entry fields and the accent are read as their own lock acquisition and copied
        // out as values, so the guard is dropped before anything else touches the theme.
        let (window_fill, accent, muted) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.primary, active.colors.secondary)
                }
                None => (Color::BACKGROUND, Color::PRIMARY, Color::SECONDARY),
            }
        };
        // The two entry fields are editable interiors: one step *away* from the bar's own
        // surface, so on a light bar they are lighter and on a dark one darker, rather
        // than the forced white they used to be.
        let field = if surface.is_dark() {
            surface.blend(&Color::WHITE, 0.06)
        } else {
            surface.blend(&Color::BLACK, 0.04)
        };
        let accent_ink = accent.contrast_color();
        let muted_ink = muted.contrast_color();

        // ── Background ──
        // The bar floats over the surface it searches, so its fill is derived from its own
        // resolved surface and nudged away from the window fill.
        let bar = if surface == window_fill { surface.blend(&ink, 0.06) } else { surface };
        context.fill_rect(geom, bar);
        context.draw_rect_stroke(geom, border, 1);

        let (find_row, replace_row) = self.compute_layout();
        self.find_row_rect = find_row;
        self.replace_row_rect = replace_row;

        // ── Find row: label + input + toggles + buttons ──
        //
        // The row's fixed budget — a 40 px label, an input, four 24 px toggles and two 24 px
        // arrows, each separated by `GAP` — comes to 40 + 6*6 + 96 + 48 + 6 = 226 px. A
        // 240 px control leaves only the input room to absorb the remainder, so the arrows
        // used to be placed at x=240 and x=270, entirely outside the dialog. The input is
        // therefore clamped to the space that is actually left before the buttons: each
        // fixed-width control is laid out from the row's right edge inwards, so the input
        // takes up the slack instead of the last two buttons leaving the frame.
        //
        // `trailing` is every column to the right of the input — four toggles, two arrows
        // and the six gaps between them — and it is subtracted from the row while the input
        // keeps only what is left. There is deliberately **no minimum width on the input**: a
        // floor of 40 px was enough to push the two arrow buttons 10 px and 40 px past the
        // row's right edge at the census rectangle, so the floor lets the last fixed columns
        // leave the frame. The input is the one flexible column here and it is the one that
        // absorbs a narrow dialog; the arrows are the affordances, so they keep their width.
        //
        // The subtraction is floored at zero *before* the cast: casting a negative remainder
        // to `u32` wraps it to ~4.29e9, which is how this row reported a right edge of
        // `4294967296` in the SVG.
        let label_width = 40u32;
        let trailing = GAP * 6 + BTN_SIZE as i32 * 4 + 24 + GAP + 24;
        let input_width = (find_row.width as i32 - label_width as i32 - trailing).max(0) as u32;
        let mut x = find_row.x;

        // "Find:" label. Bounded by the label column, so it truncates there rather than
        // running into the input it names.
        let label_rect = Rect::new(x, find_row.y, label_width, find_row.height);
        let font = crate::core::Font::simple("sans-serif", 12.0);
        let label_line = self.text_line(label_rect, &font, context);
        context.draw_text_fitted(label_line, "Find:", &font, ink, HorizontalAlignment::Left);
        x += label_width as i32 + GAP;

        // Find text input background
        let input_rect = Rect::new(x, find_row.y, input_width, find_row.height);
        context.fill_rect(input_rect, field);
        context.draw_rect_stroke(input_rect, border, 1);
        let display_text = if self.find_text.is_empty() { "" } else { &self.find_text };
        let input_line = self.text_line(input_rect, &font, context);
        context.draw_text_fitted(input_line, display_text, &font, ink, HorizontalAlignment::Left);
        x = input_rect.x + input_rect.width as i32 + GAP;

        // Match Case toggle
        let mc_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let mc_color = if self.match_case { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(mc_rect, mc_color);
        let mc_line = self.text_line(mc_rect, &font, context);
        context.draw_text_fitted(
            mc_line,
            "Aa",
            &font,
            if self.match_case { accent_ink } else { ink },
            HorizontalAlignment::Left,
        );
        x += BTN_SIZE as i32 + GAP;

        // Whole Word toggle
        let ww_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let ww_color = if self.whole_word { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(ww_rect, ww_color);
        let ww_line = self.text_line(ww_rect, &font, context);
        context.draw_text_fitted(
            ww_line,
            "W",
            &font,
            if self.whole_word { accent_ink } else { ink },
            HorizontalAlignment::Left,
        );
        x += BTN_SIZE as i32 + GAP;

        // Regex toggle
        let rx_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let rx_color = if self.use_regex { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(rx_rect, rx_color);
        let rx_line = self.text_line(rx_rect, &font, context);
        context.draw_text_fitted(
            rx_line,
            ".*",
            &font,
            if self.use_regex { accent_ink } else { ink },
            HorizontalAlignment::Left,
        );
        x += BTN_SIZE as i32 + GAP;

        // Highlight All toggle
        let ha_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let ha_color = if self.highlight_all { accent } else { field.blend(&ink, 0.15) };
        context.fill_rect(ha_rect, ha_color);
        let ha_line = self.text_line(ha_rect, &font, context);
        context.draw_text_fitted(
            ha_line,
            "H",
            &font,
            if self.highlight_all { accent_ink } else { ink },
            HorizontalAlignment::Left,
        );
        x += BTN_SIZE as i32 + GAP;

        // Find Previous button
        let fp_rect = Rect::new(x, find_row.y, 24, find_row.height);
        context.fill_rect(fp_rect, muted);
        let fp_line = self.text_line(fp_rect, &font, context);
        context.draw_text_fitted(fp_line, "\u{25B2}", &font, muted_ink, HorizontalAlignment::Left);
        x += 24 + GAP;

        // Find Next button
        let fn_rect = Rect::new(x, find_row.y, 24, find_row.height);
        context.fill_rect(fn_rect, accent);
        let fn_line = self.text_line(fn_rect, &font, context);
        context.draw_text_fitted(fn_line, "\u{25BC}", &font, accent_ink, HorizontalAlignment::Left);

        // ── Replace row: label + input + buttons ──
        // The replace row carried the same underflow and the same absent minimum: the
        // remainder here is small but still positive at 240 px, so the visible defect was the
        // Replace All button 30 px past the right edge rather than a wrapped width. Flooring
        // in `i32` keeps a narrower row from producing the same ~4.29e9 px input the find row
        // did, and the input is allowed to reach zero so the two trailing buttons stay inside.
        let r_input_width =
            (replace_row.width as i32 - label_width as i32 - GAP * 3 - 28 - 24).max(0) as u32;
        let mut x2 = replace_row.x;

        // "Replace:" label
        let rl_rect = Rect::new(x2, replace_row.y, label_width, replace_row.height);
        let rl_line = self.text_line(rl_rect, &font, context);
        context.draw_text_fitted(rl_line, "Rpl:", &font, ink, HorizontalAlignment::Left);
        x2 += label_width as i32 + GAP;

        // Replace text input background
        let r_input_rect = Rect::new(x2, replace_row.y, r_input_width, replace_row.height);
        context.fill_rect(r_input_rect, field);
        context.draw_rect_stroke(r_input_rect, border, 1);
        let r_text = if self.replace_text.is_empty() { "" } else { &self.replace_text };
        let r_input_line = self.text_line(r_input_rect, &font, context);
        context.draw_text_fitted(r_input_line, r_text, &font, ink, HorizontalAlignment::Left);
        x2 = r_input_rect.x + r_input_rect.width as i32 + GAP;

        // Replace button
        let rep_rect = Rect::new(x2, replace_row.y, 24, replace_row.height);
        context.fill_rect(rep_rect, muted);
        let rep_line = self.text_line(rep_rect, &font, context);
        context.draw_text_fitted(rep_line, "R", &font, muted_ink, HorizontalAlignment::Left);
        x2 += 28;

        // Replace All button
        let ra_rect = Rect::new(x2, replace_row.y, 24, replace_row.height);
        context.fill_rect(ra_rect, muted);
        let ra_line = self.text_line(ra_rect, &font, context);
        context.draw_text_fitted(ra_line, "RA", &font, muted_ink, HorizontalAlignment::Left);
    }
}

impl EventHandler for FindReplaceDialog {
    fn handle_event(&mut self, event: &Event) {
        if !self.visible {
            self.base.handle_event(event);
            return;
        }

        // A disabled dialog must not act on input: no focus changes, no find/replace
        // actions, no close. Only `visible` was checked here, so `set_enabled(false)`
        // left the whole dialog live.
        if !self.base.is_enabled() {
            self.base.handle_event(event);
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    // Check clicks on find row buttons
                    let (find_row, replace_row) = self.compute_layout();

                    // Focus find field on click
                    let find_input_w =
                        ((find_row.width as i32 - 46 - GAP * 6 - BTN_SIZE as i32 * 4 - 40) as u32)
                            .max(60);
                    let find_input_region = Rect::new(
                        find_row.x + 46, // after "Find:" label
                        find_row.y,
                        find_input_w,
                        find_row.height,
                    );
                    if find_input_region.contains_point(*pos) {
                        self.focus_field = 0;
                        self.base.request_redraw();
                        return;
                    }

                    // Click on replace input
                    let r_input_width =
                        ((replace_row.width as i32 - 46 - GAP * 3 - 48) as u32).max(60);
                    let r_input_region = Rect::new(
                        replace_row.x + 46, // after "Rpl:" label
                        replace_row.y,
                        r_input_width,
                        replace_row.height,
                    );
                    if r_input_region.contains_point(*pos) {
                        self.focus_field = 1;
                        self.base.request_redraw();
                        return;
                    }

                    // Toggle buttons on find row
                    let find_input_width =
                        ((find_row.width as i32 - 46 - GAP * 6 - BTN_SIZE as i32 * 4 - 40) as u32)
                            .max(60) as i32;
                    let button_start_x = find_row.x + 46 + find_input_width + GAP;

                    // Match Case
                    let mc_rect = Rect::new(button_start_x, find_row.y, BTN_SIZE, find_row.height);
                    if mc_rect.contains_point(*pos) {
                        self.match_case = !self.match_case;
                        self.base.request_redraw();
                        return;
                    }

                    // Whole Word
                    let ww_rect = Rect::new(
                        button_start_x + BTN_SIZE as i32 + GAP,
                        find_row.y,
                        BTN_SIZE,
                        find_row.height,
                    );
                    if ww_rect.contains_point(*pos) {
                        self.whole_word = !self.whole_word;
                        self.base.request_redraw();
                        return;
                    }

                    // Regex
                    let rx_rect = Rect::new(
                        button_start_x + (BTN_SIZE as i32 + GAP) * 2,
                        find_row.y,
                        BTN_SIZE,
                        find_row.height,
                    );
                    if rx_rect.contains_point(*pos) {
                        self.use_regex = !self.use_regex;
                        self.base.request_redraw();
                        return;
                    }

                    // Highlight All
                    let ha_rect = Rect::new(
                        button_start_x + (BTN_SIZE as i32 + GAP) * 3,
                        find_row.y,
                        BTN_SIZE,
                        find_row.height,
                    );
                    if ha_rect.contains_point(*pos) {
                        self.highlight_all = !self.highlight_all;
                        self.base.request_redraw();
                        return;
                    }

                    // Find Previous button
                    let fp_x = button_start_x + (BTN_SIZE as i32 + GAP) * 4;
                    let fp_rect = Rect::new(fp_x, find_row.y, 24, find_row.height);
                    if fp_rect.contains_point(*pos) {
                        self.find_previous();
                        return;
                    }

                    // Find Next button
                    let fn_rect = Rect::new(fp_x + 24 + GAP, find_row.y, 24, find_row.height);
                    if fn_rect.contains_point(*pos) {
                        self.find_next();
                        return;
                    }

                    // Replace button
                    let r_btn_x = replace_row.x
                        + 46
                        + (replace_row.width as i32 - 46 - GAP * 3 - 48).max(60)
                        + GAP;
                    let rep_rect = Rect::new(r_btn_x, replace_row.y, 24, replace_row.height);
                    if rep_rect.contains_point(*pos) {
                        self.replace();
                        return;
                    }

                    // Replace All button
                    let ra_rect = Rect::new(r_btn_x + 28, replace_row.y, 24, replace_row.height);
                    if ra_rect.contains_point(*pos) {
                        self.replace_all();
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    27 => {
                        // Escape
                        self.visible = false;
                        self.close_signal.emit(());
                        self.base.request_redraw();
                    }
                    13 => {
                        // Enter — trigger find next
                        self.find_next();
                    }
                    8 => {
                        // Backspace
                        self.backspace_focused();
                    }
                    9 => {
                        // Tab — switch focus between find and replace
                        self.focus_field = 1 - self.focus_field;
                        self.base.request_redraw();
                    }
                    _ => {
                        // Printable character range (rough check)
                        if *key >= 32 && *key <= 126 {
                            if let Some(ch) = char::from_u32(*key) {
                                self.append_to_focused(ch);
                            }
                        }
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn find_replace_dialog_default_creation() {
        let dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        assert_eq!(dialog.kind(), WidgetKind::FindReplaceDialog);
        assert!(!dialog.is_visible());
        assert!(dialog.find_text().is_empty());
        assert!(dialog.replace_text().is_empty());
        assert!(!dialog.is_match_case());
        assert!(!dialog.is_whole_word());
        assert!(!dialog.is_use_regex());
        assert!(!dialog.is_highlight_all());
    }

    #[test]
    fn find_replace_dialog_set_get_text() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("hello");
        assert_eq!(dialog.find_text(), "hello");

        dialog.set_replace_text("world");
        assert_eq!(dialog.replace_text(), "world");

        dialog.set_find_text("");
        assert!(dialog.find_text().is_empty());
    }

    #[test]
    fn find_replace_dialog_toggle_options() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));

        assert!(!dialog.is_match_case());
        dialog.set_match_case(true);
        assert!(dialog.is_match_case());
        dialog.set_match_case(false);
        assert!(!dialog.is_match_case());

        assert!(!dialog.is_whole_word());
        dialog.set_whole_word(true);
        assert!(dialog.is_whole_word());

        assert!(!dialog.is_use_regex());
        dialog.set_use_regex(true);
        assert!(dialog.is_use_regex());

        assert!(!dialog.is_highlight_all());
        dialog.set_highlight_all(true);
        assert!(dialog.is_highlight_all());
    }

    #[test]
    fn find_replace_dialog_show_hide() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        assert!(!dialog.is_visible());

        dialog.show();
        assert!(dialog.is_visible());

        dialog.hide();
        assert!(!dialog.is_visible());
    }

    #[test]
    fn find_replace_dialog_signals_find_next() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("search");
        dialog.show();

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.find_next_signal.connect(move |text: Arc<String>| {
            if *text == "search" {
                fired_clone.store(true, Ordering::SeqCst);
            }
        });

        dialog.find_next();
        assert!(fired.load(Ordering::SeqCst), "find_next_signal should fire with the find text");
    }

    #[test]
    fn find_replace_dialog_signals_replace_all() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("foo");
        dialog.set_replace_text("bar");
        dialog.show();

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.replace_all_signal.connect(move |pair: Arc<(String, String)>| {
            if pair.0 == "foo" && pair.1 == "bar" {
                fired_clone.store(true, Ordering::SeqCst);
            }
        });

        dialog.replace_all();
        assert!(
            fired.load(Ordering::SeqCst),
            "replace_all_signal should fire with (find, replace)"
        );
    }

    /// A **shown** find bar paints differently in the two appearances.
    ///
    /// The rendering census measures this control hidden, which is correct — a find bar
    /// is opened on demand, so `visible: false` is its designed initial state and the
    /// census therefore counts zero ink. That makes the census silent about whether the
    /// bar responds to a theme switch, so the property is asserted here, through the real
    /// `Draw` path, by rendering a shown instance in each appearance and comparing the
    /// output.
    ///
    /// # Why the guard covers the whole test, not just each render
    ///
    /// The appearance is process-wide, and `theme_test_guard` is the mutex that serialises
    /// tests against it. This test used to take the guard *inside* its `frame` helper, release
    /// it, and then restore the appearance after the assertion — outside any guard. The window
    /// between the helper returning and that final write was unguarded, so a test in another
    /// module that reads the global theme while rendering (for instance `meter`'s render
    /// assertions) could observe a half-switched state and fail intermittently. Holding one
    /// guard for the whole test is what makes "restore what I changed" atomic with respect to
    /// every other reader.
    #[test]
    fn a_shown_find_bar_renders_differently_in_light_and_dark() {
        let _guard = crate::theme::theme_test_guard();
        {
            let mut manager = crate::style::theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.register_theme(crate::theme::Theme::dark());
        }

        fn frame(appearance: crate::style::AppearanceMode) -> String {
            crate::style::theme_manager().set_appearance(appearance);
            let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 240, 120));
            dialog.set_find_text("Sample");
            dialog.show();
            crate::theme::apply_active_theme(&mut dialog);
            crate::widget::svg::render_to_svg(&mut dialog)
        }

        let light = frame(crate::style::AppearanceMode::Light);
        let dark = frame(crate::style::AppearanceMode::Dark);
        assert_ne!(
            light, dark,
            "a theme switch must change what a shown find bar paints; identical output \
             means the chrome is hardcoded"
        );
        // Restored while the guard is still held, so no other reader can observe the light
        // appearance this test selected.
        crate::style::theme_manager().set_appearance(crate::style::AppearanceMode::Light);
    }

    #[test]
    fn find_replace_dialog_close_on_escape() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.show();
        assert!(dialog.is_visible());

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.close_signal.connect(move |_: Arc<()>| {
            fired_clone.store(true, Ordering::SeqCst);
        });

        dialog.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!dialog.is_visible());
        assert!(fired.load(Ordering::SeqCst), "close_signal should fire on Escape");
    }

    /// A disabled dialog must neither close nor act on typed input.
    ///
    /// `handle_event` gated on `visible` only, so `set_enabled(false)` left the dialog
    /// fully interactive — a suspended dialog could still be closed with Escape and
    /// still ran find/replace on Enter.
    #[test]
    fn find_replace_dialog_disabled_ignores_input() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.show();
        dialog.set_enabled(false);

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        dialog.close_signal.connect(move |_: Arc<()>| {
            fired_clone.store(true, Ordering::SeqCst);
        });

        dialog.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(dialog.is_visible(), "a disabled dialog must not close on Escape");
        assert!(!fired.load(Ordering::SeqCst), "a disabled dialog must not emit close_signal");

        // The find action must not run either.
        let find_fired = Arc::new(AtomicBool::new(false));
        let ff = Arc::clone(&find_fired);
        dialog.find_next_signal.connect(move |_: Arc<String>| {
            ff.store(true, Ordering::SeqCst);
        });
        dialog.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(!find_fired.load(Ordering::SeqCst), "a disabled dialog must not run find");

        // Re-enabling restores the behaviour, so the gate suspends rather than locks.
        dialog.set_enabled(true);
        dialog.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!dialog.is_visible(), "re-enabling must restore Escape-to-close");
    }

    #[test]
    fn find_replace_dialog_svg_output() {
        let mut dialog = FindReplaceDialog::new(Rect::new(0, 0, 400, 80));
        dialog.set_find_text("find_me");
        dialog.set_replace_text("replace_with");
        dialog.set_match_case(true);
        dialog.show();

        let svg = render_to_svg(&mut dialog);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }
}
