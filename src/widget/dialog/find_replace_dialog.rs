//! FindReplaceDialog widget — a find/replace dialog (like VS Code/IDE find).
//!
//! Provides a two-row dialog with find and replace input fields, toggle
//! buttons for match case, whole word, regex, and highlight all, plus
//! action buttons for find next, find previous, replace, replace all,
//! and close. Emits typed signals when actions are triggered.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
}

impl Widget for FindReplaceDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for FindReplaceDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible {
            return;
        }

        let geom = self.geometry();

        // ── Background ──
        context.fill_rect(geom, Color::BACKGROUND);
        context.draw_rect_stroke(geom, Color::BORDER, 1);

        let (find_row, replace_row) = self.compute_layout();
        self.find_row_rect = find_row;
        self.replace_row_rect = replace_row;

        // ── Find row: label + input + toggles + buttons ──
        let mut x = find_row.x;

        // "Find:" label
        let label_width = 40u32;
        let label_rect = Rect::new(x, find_row.y, label_width, find_row.height);
        let font = crate::core::Font::simple("sans-serif", 12.0);
        context.draw_text(
            Point::new(label_rect.x + 2, label_rect.y + label_rect.height as i32 / 2 + 4),
            "Find:",
            &font,
            Color::FOREGROUND,
        );
        x += label_width as i32 + GAP;

        // Find text input background
        let input_width =
            ((find_row.width as i32 - label_width as i32 - GAP * 6 - BTN_SIZE as i32 * 4 - 40)
                as u32)
                .max(60);
        let input_rect = Rect::new(x, find_row.y, input_width, find_row.height);
        context.fill_rect(input_rect, Color::WHITE);
        context.draw_rect_stroke(input_rect, Color::BORDER, 1);
        let display_text = if self.find_text.is_empty() { "" } else { &self.find_text };
        context.draw_text(
            Point::new(input_rect.x + 2, input_rect.y + input_rect.height as i32 / 2 + 4),
            display_text,
            &font,
            Color::BLACK,
        );
        x = input_rect.x + input_rect.width as i32 + GAP;

        // Match Case toggle
        let mc_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let mc_color = if self.match_case { Color::PRIMARY } else { Color::LIGHT_GRAY };
        context.fill_rect(mc_rect, mc_color);
        context.draw_text(
            Point::new(mc_rect.x + 2, mc_rect.y + mc_rect.height as i32 / 2 + 4),
            "Aa",
            &font,
            if self.match_case { Color::WHITE } else { Color::DARK_GRAY },
        );
        x += BTN_SIZE as i32 + GAP;

        // Whole Word toggle
        let ww_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let ww_color = if self.whole_word { Color::PRIMARY } else { Color::LIGHT_GRAY };
        context.fill_rect(ww_rect, ww_color);
        context.draw_text(
            Point::new(ww_rect.x + 1, ww_rect.y + ww_rect.height as i32 / 2 + 4),
            "W",
            &font,
            if self.whole_word { Color::WHITE } else { Color::DARK_GRAY },
        );
        x += BTN_SIZE as i32 + GAP;

        // Regex toggle
        let rx_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let rx_color = if self.use_regex { Color::PRIMARY } else { Color::LIGHT_GRAY };
        context.fill_rect(rx_rect, rx_color);
        context.draw_text(
            Point::new(rx_rect.x + 1, rx_rect.y + rx_rect.height as i32 / 2 + 4),
            ".*",
            &font,
            if self.use_regex { Color::WHITE } else { Color::DARK_GRAY },
        );
        x += BTN_SIZE as i32 + GAP;

        // Highlight All toggle
        let ha_rect = Rect::new(x, find_row.y, BTN_SIZE, find_row.height);
        let ha_color = if self.highlight_all { Color::PRIMARY } else { Color::LIGHT_GRAY };
        context.fill_rect(ha_rect, ha_color);
        context.draw_text(
            Point::new(ha_rect.x + 1, ha_rect.y + ha_rect.height as i32 / 2 + 4),
            "H",
            &font,
            if self.highlight_all { Color::WHITE } else { Color::DARK_GRAY },
        );
        x += BTN_SIZE as i32 + GAP;

        // Find Previous button
        let fp_rect = Rect::new(x, find_row.y, 24, find_row.height);
        context.fill_rect(fp_rect, Color::SECONDARY);
        context.draw_text(
            Point::new(fp_rect.x + 2, fp_rect.y + fp_rect.height as i32 / 2 + 4),
            "\u{25B2}",
            &font,
            Color::WHITE,
        );
        x += 24 + GAP;

        // Find Next button
        let fn_rect = Rect::new(x, find_row.y, 24, find_row.height);
        context.fill_rect(fn_rect, Color::PRIMARY);
        context.draw_text(
            Point::new(fn_rect.x + 2, fn_rect.y + fn_rect.height as i32 / 2 + 4),
            "\u{25BC}",
            &font,
            Color::WHITE,
        );

        // ── Replace row: label + input + buttons ──
        let mut x2 = replace_row.x;

        // "Replace:" label
        let rl_rect = Rect::new(x2, replace_row.y, label_width, replace_row.height);
        context.draw_text(
            Point::new(rl_rect.x + 2, rl_rect.y + rl_rect.height as i32 / 2 + 4),
            "Rpl:",
            &font,
            Color::FOREGROUND,
        );
        x2 += label_width as i32 + GAP;

        // Replace text input background
        let r_input_width =
            ((replace_row.width as i32 - label_width as i32 - GAP * 3 - 48) as u32).max(60);
        let r_input_rect = Rect::new(x2, replace_row.y, r_input_width, replace_row.height);
        context.fill_rect(r_input_rect, Color::WHITE);
        context.draw_rect_stroke(r_input_rect, Color::BORDER, 1);
        let r_text = if self.replace_text.is_empty() { "" } else { &self.replace_text };
        context.draw_text(
            Point::new(r_input_rect.x + 2, r_input_rect.y + r_input_rect.height as i32 / 2 + 4),
            r_text,
            &font,
            Color::BLACK,
        );
        x2 = r_input_rect.x + r_input_rect.width as i32 + GAP;

        // Replace button
        let rep_rect = Rect::new(x2, replace_row.y, 24, replace_row.height);
        context.fill_rect(rep_rect, Color::SECONDARY);
        context.draw_text(
            Point::new(rep_rect.x + 1, rep_rect.y + rep_rect.height as i32 / 2 + 4),
            "R",
            &font,
            Color::WHITE,
        );
        x2 += 28;

        // Replace All button
        let ra_rect = Rect::new(x2, replace_row.y, 24, replace_row.height);
        context.fill_rect(ra_rect, Color::SECONDARY);
        context.draw_text(
            Point::new(ra_rect.x + 1, ra_rect.y + ra_rect.height as i32 / 2 + 4),
            "RA",
            &font,
            Color::WHITE,
        );
    }
}

impl EventHandler for FindReplaceDialog {
    fn handle_event(&mut self, event: &Event) {
        if !self.visible {
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
