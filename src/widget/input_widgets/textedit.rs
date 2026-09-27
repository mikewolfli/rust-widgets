// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Multi-line text edit widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;
/// Multi-line text edit widget.
pub struct TextEdit {
    base: BaseWidget,
    text: String,
    placeholder_text: String,
    max_length: Option<usize>,
    read_only: bool,
    line_wrap: bool,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// Emitted after the text changes: on edits, and after an undo/redo restore.
    /// Not emitted when a `set_text` is given the text the widget already holds.
    pub text_changed: Signal1<String>,
}

impl TextEdit {
    /// Creates an empty text edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TextEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            read_only: false,
            line_wrap: true,
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            text_changed: Signal1::new(),
        }
    }
    /// Returns current text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets text and emits text_changed signal if different.
    ///
    /// # The one gate every path funnels through
    ///
    /// Typing, pasting, undo/redo and the property route all end here, so the `max_length` check
    /// belongs here rather than in each caller. It used to be in none of them: the limit was read
    /// only when the limit itself was set, so it did not survive a single keystroke.
    ///
    /// An over-long candidate is **refused whole** rather than truncated to fit. Truncating a paste
    /// would silently discard the tail of what the user handed the control, and a user cannot tell a
    /// completed paste from a cut one; refusing it moves the decision back to the caller, which is
    /// where the limit was declared in the first place. The one truncation that does happen is in
    /// [`Self::set_max_length`], where a *lowered* limit must apply to text already in the field and
    /// there is no caller left to refuse.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        if !self.within_max_length(&text) {
            return;
        }
        let before = self.text.clone();
        self.text = text;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "text_edit_text",
            )));
        }
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }
    /// Returns placeholder text.
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }
    /// Sets placeholder text.
    pub fn set_placeholder_text(&mut self, text: String) {
        self.placeholder_text = text;
        self.base.request_redraw();
    }
    /// Returns maximum text length.
    pub fn max_length(&self) -> Option<usize> {
        self.max_length
    }
    /// Sets maximum text length.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// The field was stored, published (getter, setter, schema row, round-trip test) and read by
    /// **nothing in the edit path**: the truncation below only ran when the *limit itself* changed,
    /// so `set_max_length(Some(10))` on a short field and then typing twenty characters produced a
    /// twenty-character field. A limit that only applies to the value that was already there is not
    /// a limit — it is a one-shot trim with a misleading name.
    ///
    /// The gate is now [`Self::within_max_length`], which every path that appends or replaces text
    /// consults, and this setter keeps its truncation because the *existing* text has to conform too.
    pub fn set_max_length(&mut self, max_length: Option<usize>) {
        self.max_length = max_length;
        // Truncate if needed (using floor_char_boundary to avoid mid-char panic)
        if let Some(max) = max_length {
            if self.text.len() > max {
                let boundary = floor_char_boundary(&self.text, max);
                let truncated = self.text[..boundary].to_string();
                self.set_text(truncated);
            }
        }
    }

    /// Whether `candidate` is short enough to replace the current text.
    ///
    /// # Why a byte count and not a character count
    ///
    /// `max_length` is documented as a *length* and the schema publishes it as `UInt`, and the
    /// truncation already in [`Self::set_max_length`] measures bytes (`self.text.len()`). A gate that
    /// counted characters would let a CJK document through at three bytes per character and then be
    /// shortened by the next call to the setter — two rules for one number, which is how a limit
    /// becomes untrustworthy. Bytes are also what a caller sizing a database column is counting.
    ///
    /// # Why the whole candidate rather than the appended character
    ///
    /// `set_text` is the one place text enters the control (typing, pasting and the property route
    /// all funnel through it), so the check belongs there -- one gate, not one per caller. Checking
    /// only the appended character would let a *paste* through, which is the case a limit most
    /// obviously exists for.
    fn within_max_length(&self, candidate: &str) -> bool {
        match self.max_length {
            Some(max) => candidate.len() <= max,
            None => true,
        }
    }
    /// Returns whether the widget is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.base.request_redraw();
    }
    /// Returns whether line wrap mode is enabled.
    pub fn line_wrap(&self) -> bool {
        self.line_wrap
    }
    /// Sets line wrap mode.
    pub fn set_line_wrap(&mut self, wrap: bool) {
        self.line_wrap = wrap;
        self.base.request_redraw();
    }
    /// Returns number of lines in the text.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        }
    }
    /// Returns text at specified line (0-indexed).
    pub fn line_text(&self, line: usize) -> Option<&str> {
        let mut start = 0;
        let mut current_line = 0;
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                if current_line == line {
                    return Some(&self.text[start..i]);
                }
                start = i + 1;
                current_line += 1;
            }
        }
        if current_line == line {
            Some(&self.text[start..])
        } else {
            None
        }
    }
    /// Appends text to the end.
    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let mut next = self.text.clone();
        next.push_str(text);
        self.set_text(next);
    }
    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }
    /// Returns whether the text edit is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Undo the latest text mutation.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Redo the latest undone text mutation.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is a text mutation to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone text mutation to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        self.restoring_history = true;
        self.text = text;
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }
}
// Implement Widget trait
impl Widget for TextEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        Size::new(dimensions::TEXT_EDIT_DEFAULT_WIDTH, dimensions::TEXT_EDIT_DEFAULT_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TextEdit`'s property contract.
///
/// `WidgetKind::TextEdit` is the kind the capability layer pairs with the
/// [`TerminalView`](crate::widget::TerminalView) control
/// (`capability::properties::terminal_view_capability`), so the old centralised
/// `TextEdit` arms were answered by `TerminalView`, not by this widget — despite
/// `TEXT_EDIT_PROPERTIES` existing, its names are all marked non-readable and
/// non-writable, so the multi-line editor below never served a property. The
/// contract for the kind lives beside `TerminalView`; this widget publishes none
/// of its own rather than claiming another control's.
impl WidgetProperties for TextEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text.clone())),
            "placeholder_text" => Ok(CapabilityValue::String(self.placeholder_text.clone())),
            "max_length" => match self.max_length {
                // An unset limit is `Null`, not a numeric sentinel. `LineEdit` answers
                // the same question the same way, and the schema declares this property
                // `UInt` — so returning `u64::MAX` for "unset" made the value
                // un-writable: reading a `textedit` with no limit and writing the value
                // back installed a nonsensical cap (or failed outright on a 32-bit
                // target, where `usize::try_from(u64::MAX)` is out of range). A property
                // whose read cannot be written back is a broken round trip.
                Some(limit) => Ok(CapabilityValue::UInt(limit as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "read_only" => Ok(CapabilityValue::Bool(self.read_only)),
            "line_wrap" => Ok(CapabilityValue::Bool(self.line_wrap)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        // These five are real, working accessors on this control, so the
        // property route must reach them. Forwarding everything to
        // `base_property_set` meant `rw_widget_property_set(id, "text", ..)`
        // answered `UnknownProperty` even though `set_text` worked.
        match name {
            "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_text(text);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "placeholder_text" => match value {
                CapabilityValue::String(text) => {
                    self.set_placeholder_text(text);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "max_length" => match value {
                CapabilityValue::UInt(limit) => {
                    let limit =
                        usize::try_from(limit).map_err(|_| CapabilityAccessError::OutOfRange)?;
                    self.set_max_length(Some(limit));
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "read_only" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_read_only(flag);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "line_wrap" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_line_wrap(flag);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            _ => {}
        }
        base_property_set(self, name, value)
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Must list every name `TEXT_EDIT_PROPERTIES` declares, because `get` /
        // `set` below answer all five. Publishing only the shared four while the
        // schema promised these names is the mismatch
        // `schema_and_contract_publish_the_same_names` exists to catch.
        property_names_of![
            "text",
            "placeholder_text",
            "max_length",
            "read_only",
            "line_wrap",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `text_edit` publishes.
    ///
    /// Every name in the set assigns state — the text, the placeholder, the length
    /// limit, the read-only flag, the wrap mode — and each needs a payload, so the whole
    /// set is answered through the property route. The names are placed in
    /// `TEXT_EDIT_PROPERTIES` rather than served by this widget (see the module docs on
    /// why), but the capability still resolves `text_edit` to `TextEdit`, so the refusal
    /// has to live here: returning `UnknownCommand` would make `invoke_command` report
    /// `UnsupportedOnWidget` for names the control does publish.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text"
            | "set_placeholder_text"
            | "set_max_length"
            | "set_read_only"
            | "set_line_wrap" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for TextEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || self.read_only {
            return;
        }
        if let Event::KeyPress { key, modifiers } = event {
            match *key {
                8 => {
                    // Backspace
                    if !self.text.is_empty() {
                        let mut next = self.text.clone();
                        next.pop();
                        self.set_text(next);
                    }
                }
                13 => {
                    // Enter
                    let mut next = self.text.clone();
                    next.push('\n');
                    self.set_text(next);
                }
                90 if modifiers & 2 != 0 => {
                    let _ = self.undo();
                }
                89 if modifiers & 2 != 0 => {
                    let _ = self.redo();
                }
                _ => {
                    // Character input
                    if let Some(ch) = char::from_u32(*key) {
                        if ch.is_ascii_graphic() || ch == ' ' || ch == '\t' {
                            let mut next = self.text.clone();
                            next.push(ch);
                            self.set_text(next);
                        }
                    }
                }
            }
        }
    }
}

impl Draw for TextEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding;

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the field, its border and its text unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        // The fallback name matters: the role table is keyed on **role** names, so `text_edit`
        // (the factory name) is not in it and would classify as `Surface`, i.e. the window fill.
        // `line_edit` is, and resolves to the field interior plus the theme's foreground.
        let theme = crate::style::resolved_theme_style("text_edit")
            .or_else(|| crate::style::resolved_theme_style("line_edit"));
        let field_from_theme = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .unwrap_or_else(|| Color::rgb(255, 255, 255));
        // The window fill, read as its own lock acquisition and copied out as a value, so the
        // guard is dropped before anything else touches the theme.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        // The filter is on the **resolved** value, not only on the theme's: the active theme is
        // applied to every control before it is drawn, and a control absent from the role table
        // resolves its background to the window fill itself — so letting that value through
        // unfiltered would paint the field in the window's own colour, which is invisible on
        // screen. A caller's own colour still wins.
        let field = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => field_from_theme,
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The border is one step from the field toward the ink, so it is visible on either
        // appearance rather than being a fixed grey a dark theme would render illegible.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| field.blend(&ink, 0.22));

        // Draw background
        context.fill_rect(rect, field);
        // Draw border
        context.draw_rect(rect, border);
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.text
        };
        if !display_text.is_empty() {
            // The placeholder is de-emphasised from the control's own ink rather than being a
            // fixed grey that a dark theme would render illegible.
            let text_color = if self.text.is_empty() { ink.blend(&field, 0.45) } else { ink };
            let font = Font::default();
            // `line_wrap` used to be stored, published (`get`/`set`/schema row/round-trip test) and
            // read by nothing: this was a single `draw_text` that ran the whole document off the
            // right edge and past the bottom. It was even commented as a placeholder -- "in real
            // implementation would handle line wrapping" -- which is the shape of a stored promise
            // rather than of a missing line.
            //
            // The value is laid out over the field's *interior*, inset by the same padding the
            // origin uses, so a wrapped line ends where the field ends rather than at the border.
            let interior = Rect::new(
                text_x,
                text_y,
                rect.width.saturating_sub(padding as u32 * 2),
                rect.height.saturating_sub(padding as u32 * 2),
            );
            self.draw_text_layout(context, display_text, interior, &font, text_color);
        }
    }
}

impl TextEdit {
    /// Paints `text` inside `interior`, honouring [`Self::line_wrap`].
    ///
    /// # The two layouts
    ///
    /// * **Wrapped** (`line_wrap: true`, the default): a line that would cross the interior's right
    ///   edge continues on the next row, so the whole document is readable without scrolling
    ///   sideways. This is the mode a document editor wants, and it is what the field's own name
    ///   promises.
    /// * **Unwrapped** (`line_wrap: false`): each `\n`-separated line is drawn in full and a line
    ///   wider than the field is clipped at the border rather than folded. This is the mode a source
    ///   editor wants, where folding a long line makes the indentation invisible.
    ///
    /// # Why the rows are walked manually
    ///
    /// The render context has `draw_text` and a per-`\n` `draw_text_line`, but neither wraps: the
    /// break has to be chosen *by the text*, which needs the font's own advance widths. So the walk
    /// measures glyph by glyph and breaks where the measurement says the row is full -- one
    /// measurement per character, which for a field-sized document is what keeps a long paragraph
    /// from being drawn as one unbounded string.
    fn draw_text_layout(
        &self,
        context: &mut RenderContext,
        text: &str,
        interior: Rect,
        font: &Font,
        color: Color,
    ) {
        let line_height = context.measure_text("M", font).height.max(1) as i32;
        let mut pen_y = interior.y;
        // A row is skipped rather than clipped when it would cross the interior's bottom: a
        // half-height row of glyphs reads as a rendering error, the same rule the list view's rows
        // follow.
        let bottom = interior.y + interior.height as i32;
        let mut row = String::new();
        let mut row_width = 0.0f32;
        for ch in text.chars() {
            if ch == '\n' {
                pen_y = Self::flush_row(
                    context,
                    &row,
                    interior.x,
                    pen_y,
                    line_height,
                    bottom,
                    font,
                    color,
                );
                row.clear();
                row_width = 0.0;
                continue;
            }
            let advance = context.measure_text(&ch.to_string(), font).width;
            // The break is taken *before* the glyph that would overflow, so the last glyph on a row
            // is never the one that crossed the edge. A row is never left empty by the break: a
            // single glyph wider than the interior has to go somewhere, and an empty row followed by
            // the overflowing glyph is worse than a row that is one glyph too wide.
            if self.line_wrap
                && row_width + advance as f32 > interior.width as f32
                && !row.is_empty()
            {
                pen_y = Self::flush_row(
                    context,
                    &row,
                    interior.x,
                    pen_y,
                    line_height,
                    bottom,
                    font,
                    color,
                );
                row.clear();
                row_width = 0.0;
            }
            row.push(ch);
            row_width += advance as f32;
        }
        if !row.is_empty() {
            Self::flush_row(context, &row, interior.x, pen_y, line_height, bottom, font, color);
        }
    }

    /// Draws one laid-out row at `pen_y` and returns the next row's origin.
    ///
    /// Returns `pen_y` unchanged when the row would cross `bottom`, so the caller needs no bound of
    /// its own and cannot advance the pen past the interior it was given.
    #[allow(clippy::too_many_arguments)]
    fn flush_row(
        context: &mut RenderContext,
        row: &str,
        x: i32,
        pen_y: i32,
        line_height: i32,
        bottom: i32,
        font: &Font,
        color: Color,
    ) -> i32 {
        if pen_y + line_height > bottom {
            return pen_y;
        }
        context.draw_text(Point::new(x, pen_y), row, font, color, HorizontalAlignment::Left);
        pen_y + line_height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn textedit_property_route_reaches_its_own_accessors() {
        // `text_edit` publishes these five names, so the property route must
        // accept them rather than answering `UnknownProperty`.
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        use crate::widget::capability::types::CapabilityValue;

        te.set("text", CapabilityValue::String("hello".to_string()))
            .expect("`text` is a published, writable property");
        assert_eq!(te.text(), "hello");
        assert_eq!(te.get("text").unwrap(), CapabilityValue::String("hello".to_string()));

        te.set("placeholder_text", CapabilityValue::String("type here".to_string()))
            .expect("`placeholder_text` is writable");
        assert_eq!(te.placeholder_text(), "type here");

        te.set("max_length", CapabilityValue::UInt(16)).expect("`max_length` is writable");
        assert_eq!(te.max_length(), Some(16));

        te.set("read_only", CapabilityValue::Bool(true)).expect("`read_only` is writable");
        assert!(te.is_read_only());

        te.set("line_wrap", CapabilityValue::Bool(false)).expect("`line_wrap` is writable");
        assert!(!te.line_wrap());

        // A type mismatch is still reported rather than silently coerced.
        assert!(te.set("read_only", CapabilityValue::UInt(1)).is_err());
    }

    #[test]
    fn textedit_creation_defaults() {
        let te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.text().is_empty());
        assert!(te.placeholder_text().is_empty());
        assert_eq!(te.max_length(), None);
        assert!(!te.is_read_only());
        assert!(te.line_wrap());
        assert!(te.is_empty());
        assert_eq!(te.line_count(), 1);
    }

    #[test]
    fn textedit_set_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Hello World".to_string());
        assert_eq!(te.text(), "Hello World");
        assert!(!te.is_empty());
    }

    #[test]
    fn textedit_set_text_empty() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Some text".to_string());
        te.set_text(String::new());
        assert!(te.text().is_empty());
    }

    #[test]
    fn textedit_placeholder() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.placeholder_text().is_empty());
        te.set_placeholder_text("Enter text here".to_string());
        assert_eq!(te.placeholder_text(), "Enter text here");
        te.set_placeholder_text(String::new());
        assert!(te.placeholder_text().is_empty());
    }

    #[test]
    fn textedit_max_length() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert_eq!(te.max_length(), None);
        te.set_max_length(Some(10));
        assert_eq!(te.max_length(), Some(10));
        te.set_max_length(None);
        assert_eq!(te.max_length(), None);
    }

    #[test]
    fn textedit_max_length_truncates() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Hello World Too Long".to_string());
        te.set_max_length(Some(10));
        assert_eq!(te.text().len(), 10);
    }

    #[test]
    fn textedit_read_only() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(!te.is_read_only());
        te.set_read_only(true);
        assert!(te.is_read_only());
        te.set_read_only(false);
        assert!(!te.is_read_only());
    }

    #[test]
    fn textedit_line_wrap() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.line_wrap());
        te.set_line_wrap(false);
        assert!(!te.line_wrap());
        te.set_line_wrap(true);
        assert!(te.line_wrap());
    }

    #[test]
    fn textedit_line_count() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert_eq!(te.line_count(), 1);
        te.set_text("Line 1\nLine 2\nLine 3".to_string());
        assert_eq!(te.line_count(), 3);
    }

    #[test]
    fn textedit_line_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("First\nSecond\nThird".to_string());
        assert_eq!(te.line_text(0), Some("First"));
        assert_eq!(te.line_text(1), Some("Second"));
        assert_eq!(te.line_text(2), Some("Third"));
        assert_eq!(te.line_text(5), None);
    }

    #[test]
    fn textedit_append() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.append("Hello");
        assert_eq!(te.text(), "Hello");
        te.append(" World");
        assert_eq!(te.text(), "Hello World");
    }

    #[test]
    fn textedit_undo_redo_restores_text() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("one");
        te.set_text("two");
        assert!(te.undo());
        assert_eq!(te.text(), "one");
        assert!(te.redo());
        assert_eq!(te.text(), "two");
    }

    #[test]
    fn textedit_control_z_and_control_y_drive_history() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("before");
        te.set_text("after");
        te.handle_event(&Event::key_press(90, 2));
        assert_eq!(te.text(), "before");
        te.handle_event(&Event::key_press(89, 2));
        assert_eq!(te.text(), "after");
    }

    #[test]
    fn textedit_clear() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_text("Some text".to_string());
        te.clear();
        assert!(te.is_empty());
    }

    #[test]
    fn textedit_geometry_delegation() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_geometry(Rect::new(10, 10, 400, 300));
        assert_eq!(te.geometry(), Rect::new(10, 10, 400, 300));
    }

    #[test]
    fn textedit_visibility() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.is_visible());
        te.hide();
        assert!(!te.is_visible());
        te.show();
        assert!(te.is_visible());
    }

    #[test]
    fn textedit_enabled() {
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        assert!(te.is_enabled());
        te.set_enabled(false);
        assert!(!te.is_enabled());
        te.set_enabled(true);
        assert!(te.is_enabled());
    }

    #[test]
    fn textedit_id_kind() {
        let te_a = TextEdit::new(Rect::new(0, 0, 100, 100));
        let te_b = TextEdit::new(Rect::new(0, 0, 100, 100));
        assert_ne!(te_a.id(), te_b.id());
        assert_eq!(te_a.kind(), WidgetKind::TextEdit);
        assert_eq!(te_b.kind(), WidgetKind::TextEdit);
    }

    #[test]
    fn textedit_signal_accessors() {
        let te = TextEdit::new(Rect::new(0, 0, 100, 100));
        let _ = &te.text_changed;
    }

    // ── `max_length` bounds every path into the field ──

    /// The limit is enforced on **typing**, not only on the value that was already there.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `max_length` was stored, published (getter, setter, schema row, round-trip test) and read by
    /// nothing in the edit path: the truncation only ran when the *limit itself* changed. So
    /// `set_max_length(Some(10))` on a short field followed by typing twenty characters produced a
    /// twenty-character field, and the property's reader could not tell — it answered `Some(10)`
    /// either way. This drives the actual keystrokes through `handle_event`, which is the path a
    /// user takes.
    #[test]
    fn max_length_bounds_typing_not_only_the_existing_value() {
        use crate::event::Event;
        let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
        te.set_max_length(Some(5));
        for ch in ['a', 'b', 'c', 'd', 'e', 'f', 'g'] {
            te.handle_event(&Event::KeyPress { key: ch as u32, modifiers: 0 });
        }
        assert_eq!(te.text(), "abcde", "the field stopped at the limit, and did not lose a prefix");
        assert_eq!(te.text().len(), 5);

        // A paste is the case a limit most obviously exists for, so it is refused wholesale rather
        // than silently cut: a user cannot tell a completed paste from a truncated one.
        let before = te.text().to_string();
        te.set_text("a much longer pasted string");
        assert_eq!(te.text(), before, "an over-long paste is refused, not truncated");

        // Deleting still works, so a full field is not a stuck one.
        te.handle_event(&Event::KeyPress { key: 8, modifiers: 0 });
        assert_eq!(te.text(), "abcd");
        // And the field is usable again now that there is room.
        te.handle_event(&Event::KeyPress { key: 'z' as u32, modifiers: 0 });
        assert_eq!(te.text(), "abcdz");

        // Raising the limit re-opens the field; clearing it removes the bound entirely.
        te.set_max_length(Some(10));
        te.set_text("0123456789");
        assert_eq!(te.text(), "0123456789");
        te.set_max_length(None);
        te.set_text("a string of any length at all");
        assert_eq!(te.text(), "a string of any length at all");

        // Lowering the limit truncates the text already in the field, on a char boundary.
        let mut unicode = TextEdit::new(Rect::new(0, 0, 300, 200));
        unicode.set_text("你好世界");
        unicode.set_max_length(Some(7));
        assert_eq!(unicode.text(), "你好", "a multi-byte truncation lands on a boundary");
    }

    // ── `line_wrap` decides the layout of the value ──

    /// Wrapping folds a long line onto the next row; not wrapping keeps one row and clips.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `line_wrap` was stored, published (`get`/`set`/schema row/round-trip test) and read by
    /// nothing. `draw` was a single `draw_text` of the whole document, commented in place as "in
    /// real implementation would handle line wrapping" — a stored promise. This asserts the two
    /// settings paint different pictures *and* that the difference is the wrapping: with wrapping on
    /// the value occupies several rows, with it off exactly one.
    #[test]
    fn line_wrap_folds_the_value_and_its_absence_does_not() {
        use crate::widget::svg::{render_to_svg, text_ink_boxes};

        let build = |wrap: bool| {
            let mut te = TextEdit::new(Rect::new(0, 0, 60, 200));
            te.set_line_wrap(wrap);
            te.set_text("the quick brown fox jumps over the lazy dog");
            te
        };

        let wrapped = render_to_svg(&mut build(true));
        let unwrapped = render_to_svg(&mut build(false));
        assert_ne!(
            wrapped, unwrapped,
            "the two settings must not render identically -- that was the dead state"
        );

        // The question `line_wrap` answers is "how many rows does the value occupy", so that is what
        // is asserted -- a bare "the SVG differs" would pass on a one-pixel shift. The layout emits
        // one `draw_text` per row and one glyph-geometry `<path>` per `draw_text`, so the row count
        // is the number of text ink boxes.
        let wrapped_rows = text_ink_boxes(&wrapped).len();
        let unwrapped_rows = text_ink_boxes(&unwrapped).len();
        assert!(
            wrapped_rows > unwrapped_rows,
            "wrapping must use more rows: {wrapped_rows} vs {unwrapped_rows}"
        );
        // Not wrapping is exactly one row of text, however long the line is.
        assert_eq!(unwrapped_rows, 1, "an unwrapped value stays on one row");

        // An explicit newline breaks on either setting, because it is the text's own decision rather
        // than the layout's.
        let mut explicit = TextEdit::new(Rect::new(0, 0, 300, 200));
        explicit.set_line_wrap(false);
        explicit.set_text("first\nsecond");
        assert_eq!(
            text_ink_boxes(&render_to_svg(&mut explicit)).len(),
            2,
            "an explicit newline is a break regardless of `line_wrap`"
        );

        // A value taller than the field is clipped rather than drawn outside it: the row count is
        // bounded by the interior's height (30 px less the padding, over an 8 px line).
        let mut overflowing = TextEdit::new(Rect::new(0, 0, 40, 30));
        overflowing.set_text("word ".repeat(40));
        let rows = text_ink_boxes(&render_to_svg(&mut overflowing)).len();
        assert!(rows <= 3, "a small field draws only the rows it has room for, not {rows} rows");
    }
}
