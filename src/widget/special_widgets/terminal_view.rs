// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TerminalView widget — a pseudo-terminal view with local line editing.
//!
//! # Scope
//!
//! This is a **pseudo-terminal**: it owns an output buffer, an editable input line, command
//! history, in-line cursor movement, selection, scrollback, and a minimal ANSI SGR parser. It
//! does not allocate a PTY and does not spawn a process. A host that owns a process feeds its
//! output through [`TerminalView::append_output`] and executes the strings published by
//! `command_submitted` (or by `interrupt_requested` for a signal request).

#[cfg(test)]
use crate::core::Point;
use crate::core::{Color, HorizontalAlignment, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::animation::CursorBlink;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The blink period and the output row estimate both need a line height; the draw path measures
/// the real face, while the input path needs a constant that is available without a render
/// context. They agree at the default body font size.
const OUTPUT_LINE_HEIGHT: i32 = 14;

/// Terminal-like text view with input line and command history.
pub struct TerminalView {
    base: BaseWidget,
    lines: Vec<String>,
    input_line: String,
    history: Vec<String>,
    history_index: Option<usize>,
    /// How many output rows are scrolled above the newest one.
    ///
    /// Zero follows the tail, which is the state a terminal spends most of its time in. A wheel
    /// notch moves this away from the tail so older output can be read without the next line of
    /// output yanking the view back down; new output resets it to zero.
    scroll_offset: usize,
    /// How many output rows the paint path could fit on the last frame.
    ///
    /// Kept so the wheel clamp uses the viewport the user actually sees rather than a second,
    /// independently derived row count. The draw path refreshes it; the constructor seeds it from
    /// the geometry so scrolling works before the first frame.
    visible_output_rows: usize,
    /// The caret at the end of the input line, which blinks while the terminal is enabled.
    ///
    /// # Why a terminal needs one more than most fields
    ///
    /// A terminal's whole appearance is *text that has stopped arriving*. Without a caret there
    /// is nothing on screen that distinguishes "the shell is waiting for you" from "the shell
    /// has hung", and a reader's first instinct on a frozen prompt is to kill the process. This
    /// control drew no caret at all — no field, no blink, no `tick` — so it was the one control
    /// in the crate whose animation a user is most likely to be *waiting on*. The blink is the
    /// crate's shared [`CursorBlink`], the same primitive `line_edit` and `code_editor` use.
    cursor: CursorBlink,
    /// The caret's position in the input line, in characters.
    input_cursor: usize,
    /// The other end of the selected run in the input line, when a selection exists.
    selection_anchor: Option<usize>,
    /// Uncommitted IME composition text, drawn at the caret.
    preedit: String,
    /// The declared character grid, as `(columns, rows)`; `None` derives it from the geometry.
    grid: Option<(u32, u32)>,
    /// Emitted when command is submitted.
    pub command_submitted: Signal1<String>,
    /// Emitted when the user asks to interrupt the running process (Ctrl+C).
    pub interrupt_requested: Signal1<String>,
    /// Emitted when the user asks for completion (Tab), with the current line.
    pub completion_requested: Signal1<String>,
}

impl TerminalView {
    /// Creates terminal view.
    pub fn new(geometry: Rect) -> Self {
        let mut cursor = CursorBlink::new();
        // Started at construction rather than on `FocusGained`: a terminal is an *input device*
        // by definition, so a freshly mounted one is already waiting for a command. Waiting for a
        // focus event that a host may never send would put the caret back where this control was.
        cursor.start();
        let visible_output_rows = Self::estimated_output_rows(geometry);
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TerminalView"),
            lines: Vec::new(),
            input_line: String::new(),
            history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
            visible_output_rows,
            cursor,
            input_cursor: 0,
            selection_anchor: None,
            preedit: String::new(),
            grid: None,
            command_submitted: Signal1::new(),
            interrupt_requested: Signal1::new(),
            completion_requested: Signal1::new(),
        }
    }

    /// Sets the character grid the view reports, or `None` to derive it from the geometry.
    pub fn set_grid(&mut self, grid: Option<(u32, u32)>) {
        let grid = grid.map(|(columns, rows)| (columns.max(1), rows.max(1)));
        if self.grid == grid {
            return;
        }
        self.grid = grid;
        self.base.request_redraw();
    }

    /// The character grid: the declared size, or a geometry-derived default.
    pub fn grid(&self) -> (u32, u32) {
        self.grid.unwrap_or_else(|| {
            let rect = self.geometry();
            let columns = (rect.width / 8).max(1);
            let rows = Self::estimated_output_rows(rect) as u32;
            (columns, rows)
        })
    }

    /// The caret's character index in the input line.
    pub fn input_cursor(&self) -> usize {
        self.input_cursor
    }

    /// Returns the selected input text, if a selection exists.
    pub fn selected_input(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        Some(self.input_line.chars().skip(start).take(end - start).collect())
    }

    /// The ordered selection range in characters.
    fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let start = anchor.min(self.input_cursor);
        let end = anchor.max(self.input_cursor);
        (start != end).then_some((start, end))
    }

    /// Clamps the caret and selection to the input line's current length.
    fn clamp_input_positions(&mut self) {
        let length = self.input_line.chars().count();
        self.input_cursor = self.input_cursor.min(length);
        if let Some(anchor) = self.selection_anchor {
            self.selection_anchor = Some(anchor.min(length));
        }
    }

    /// Inserts `text` at the caret, replacing any selection.
    fn insert_input(&mut self, text: &str) {
        self.clamp_input_positions();
        let chars: Vec<char> = self.input_line.chars().collect();
        let (start, end) = self.selection_range().unwrap_or((self.input_cursor, self.input_cursor));
        let mut next: String = chars[..start].iter().collect();
        next.push_str(text);
        next.extend(chars[end..].iter());
        self.input_line = next;
        self.input_cursor = start + text.chars().count();
        self.selection_anchor = None;
        self.history_index = None;
        self.base.request_redraw();
    }

    /// Deletes the selected run, or `count` characters before the caret.
    fn delete_input(&mut self, count: usize) {
        self.clamp_input_positions();
        let chars: Vec<char> = self.input_line.chars().collect();
        if let Some((start, end)) = self.selection_range() {
            self.input_line = chars[..start].iter().chain(chars[end..].iter()).collect();
            self.input_cursor = start;
            self.selection_anchor = None;
        } else if count > 0 && self.input_cursor > 0 {
            let start = self.input_cursor.saturating_sub(count);
            self.input_line = chars[..start]
                .iter()
                .chain(chars[self.input_cursor.min(chars.len())..].iter())
                .collect();
            self.input_cursor = start;
        }
        self.history_index = None;
        self.base.request_redraw();
    }

    /// How many output rows a body of this geometry shows, before the first frame has measured the
    /// font. Mirrors the paint path's reservation: one prompt row plus a 16 px top inset.
    fn estimated_output_rows(geometry: Rect) -> usize {
        let prompt_row_height = OUTPUT_LINE_HEIGHT + 6;
        let body_height = (geometry.height as i32 - prompt_row_height).max(0);
        let usable = (body_height - 16).max(0);
        ((usable / OUTPUT_LINE_HEIGHT).max(1)) as usize
    }

    /// The largest scroll offset that still leaves a full window of output on screen.
    fn max_scroll_offset(&self) -> usize {
        self.lines.len().saturating_sub(self.visible_output_rows.max(1))
    }

    /// Moves the viewport by `lines` output rows relative to the tail.
    ///
    /// Positive values reveal older output and negative values move back toward the newest line.
    /// Returns whether the offset changed, so the caller repaints only on a real move.
    fn scroll_output(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let next = (self.scroll_offset as i64 + lines as i64)
            .clamp(0, self.max_scroll_offset() as i64) as usize;
        if next == self.scroll_offset {
            return false;
        }
        self.scroll_offset = next;
        true
    }

    /// Whether the caret is drawn on the current frame.
    ///
    /// Exposed so the blink can be asserted without sleeping: the value is a function of the
    /// ticks the control was given, not of the wall clock.
    pub fn is_caret_visible(&self) -> bool {
        self.cursor.is_visible()
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is owed.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let owes_frame = self.cursor.tick(delta_ms);
        if owes_frame {
            // The caret's two halves are different pictures, so the frame that flips it must
            // repaint. Without this the blink would advance and never be drawn.
            self.base.request_redraw();
        }
        owes_frame
    }

    /// Returns output lines.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Returns current input line.
    pub fn input_line(&self) -> &str {
        &self.input_line
    }

    /// Sets current input line.
    pub fn set_input_line(&mut self, line: impl Into<String>) {
        let line = line.into();
        if self.input_line == line {
            return;
        }
        self.input_line = line;
        // A programmatic line is placed whole, so the caret follows its end: leaving the caret
        // where the previous line's cursor was made a later Backspace delete from the middle.
        self.input_cursor = self.input_line.chars().count();
        self.selection_anchor = None;
        self.base.request_redraw();
    }

    /// Appends output line.
    ///
    /// A line that contains the clear-screen sequence (`ESC[2J`) empties the buffer before the
    /// text after it is appended, which is how a full-screen program tells a terminal to start
    /// over. SGR colour sequences are interpreted at paint time; everything else is text.
    pub fn append_output(&mut self, line: impl Into<String>) {
        let line = line.into();
        if line.contains("\u{1b}[2J") {
            self.lines.clear();
            self.scroll_offset = 0;
        }
        // A carriage return without a line feed overwrites the current line, which is how a
        // progress indicator repaints itself.
        if let Some(rest) = line.strip_prefix('\r') {
            if let Some(last) = self.lines.last_mut() {
                *last = rest.to_string();
                self.base.request_redraw();
                return;
            }
        }
        self.lines.push(line);
        if self.lines.len() > 200 {
            let drop_count = self.lines.len() - 200;
            self.lines.drain(0..drop_count);
        }
        // New output returns the view to the tail. A reader who scrolled back would otherwise
        // have their offset silently point at different lines as the buffer shifts.
        self.scroll_offset = 0;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends several output lines with one layout pass.
    ///
    /// A process writes in bursts; asking for a layout per line made a 200-line burst queue 200
    /// requests for one visible change.
    pub fn append_output_batch(&mut self, lines: impl IntoIterator<Item = impl Into<String>>) {
        let mut changed = false;
        for line in lines {
            let line = line.into();
            if line.contains("\u{1b}[2J") {
                self.lines.clear();
                self.scroll_offset = 0;
            }
            self.lines.push(line);
            changed = true;
        }
        if !changed {
            return;
        }
        if self.lines.len() > 200 {
            let drop_count = self.lines.len() - 200;
            self.lines.drain(0..drop_count);
        }
        self.scroll_offset = 0;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Submits current command line.
    pub fn submit(&mut self) -> bool {
        let cmd = self.input_line.trim().to_string();
        if cmd.is_empty() {
            return false;
        }
        self.history.push(cmd.clone());
        self.history_index = None;
        self.lines.push(format!("> {cmd}"));
        self.command_submitted.emit(cmd);
        self.input_line.clear();
        self.input_cursor = 0;
        self.selection_anchor = None;
        self.scroll_offset = 0;
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    fn recall_history(&mut self, up: bool) {
        if self.history.is_empty() {
            return;
        }

        let next_index = match (self.history_index, up) {
            (None, true) => Some(self.history.len() - 1),
            (None, false) => None,
            (Some(index), true) => Some(index.saturating_sub(1)),
            (Some(index), false) if index + 1 < self.history.len() => Some(index + 1),
            (Some(_), false) => None,
        };

        self.history_index = next_index;
        self.input_line =
            next_index.and_then(|index| self.history.get(index).cloned()).unwrap_or_default();
        self.input_cursor = self.input_line.chars().count();
        self.selection_anchor = None;
        self.base.request_redraw();
    }
}

impl Widget for TerminalView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::TERMINAL_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::TERMINAL_DEFAULT_HEIGHT,
        )
    }

    /// One frame of the caret's blink. The frame bus calls this; nothing else does.
    fn tick(&mut self, delta_ms: u32) -> bool {
        TerminalView::tick(self, delta_ms)
    }

    /// A blinking caret always owes frames while it runs; a disabled terminal's steady caret
    /// does not, which is what `CursorBlink::stop` is for.
    fn is_animating(&self) -> bool {
        self.cursor.is_running()
    }

    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "command_submitted" => {
                Some(EventSignalRef::mapped("command_submitted", &self.command_submitted, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `TerminalView`'s property contract, published under the `TextEdit` kind.
///
/// The capability layer makes `WidgetKind::TextEdit` the kind of this control
/// (`terminal_view_capability`), and this impl reproduces exactly what the old
/// `read_input_props` / `write_input_props` arms answered for that kind.
impl WidgetProperties for TerminalView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "output_line_count" => Ok(CapabilityValue::UInt(self.lines().len() as u64)),
            "input_line" => Ok(CapabilityValue::String(self.input_line().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "input_line" => {
                self.set_input_line(expect_string(value)?);
                Ok(())
            }
            // Derived from the output buffer. The old writer had no arm for it.
            "output_line_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TERMINAL_VIEW_PROPERTIES`.
        property_names_of!["output_line_count", "input_line", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `terminal_view` publishes.
    ///
    /// `submit` maps onto the widget's real `submit`, which reports `false` for
    /// an empty input line; that is reported as needing a payload rather than as
    /// a submitted command. `append_output` carries the line to append, so a bare
    /// invocation is answered the same way rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "submit" => {
                if self.submit() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "append_output" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for TerminalView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // A disabled terminal holds a steady caret (the shared rule for a control that cannot be
        // typed into), and an enabled one blinks. This is the only place the two states differ.
        if !self.base.is_enabled() {
            self.cursor.stop();
            return;
        }
        if !self.cursor.is_running() {
            self.cursor.start();
        }

        match event {
            // Platform text — a printable string or an IME commit — appends to the same line.
            //
            // Both variants describe one user action: text the platform has already resolved.
            // Matching only `TextInput` dropped every IME commit, so a user typing through an
            // input method saw nothing arrive at the prompt.
            Event::TextInput { text } | Event::ImeCommit { text } => {
                self.preedit.clear();
                self.insert_input(text);
            }
            Event::ImePreedit { text, cursor } => {
                // The composition is shown without committing it; `cursor` is the caret inside
                // the composition and is retained for the host's candidate window.
                let _ = cursor;
                self.preedit = text.clone();
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } => {
                let pressed = crate::shortcut::Key::from_key_code(*key);
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let primary = modifiers.contains(crate::shortcut::Modifiers::PRIMARY);
                let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT);
                if primary {
                    match pressed {
                        Some(crate::shortcut::Key::C) => {
                            // Ctrl+C: copy the selection, or interrupt when nothing is selected.
                            if let Some(selected) = self.selected_input() {
                                let _ = crate::clipboard::ClipboardManager::set_text(selected);
                            } else {
                                self.interrupt_requested.emit(self.input_line.clone());
                            }
                            return;
                        }
                        Some(crate::shortcut::Key::V) => {
                            let text = crate::clipboard::ClipboardManager::text();
                            if !text.is_empty() {
                                self.insert_input(&text);
                            }
                            return;
                        }
                        Some(crate::shortcut::Key::L) => {
                            self.lines.clear();
                            self.scroll_offset = 0;
                            self.base.request_redraw();
                            return;
                        }
                        Some(crate::shortcut::Key::A) => {
                            self.selection_anchor = Some(0);
                            self.input_cursor = self.input_line.chars().count();
                            self.base.request_redraw();
                            return;
                        }
                        Some(crate::shortcut::Key::Left) => {
                            self.input_cursor = self.input_cursor.saturating_sub(1);
                            if !shift {
                                self.selection_anchor = None;
                            }
                            self.base.request_redraw();
                            return;
                        }
                        Some(crate::shortcut::Key::Right) => {
                            let length = self.input_line.chars().count();
                            self.input_cursor = (self.input_cursor + 1).min(length);
                            if !shift {
                                self.selection_anchor = None;
                            }
                            self.base.request_redraw();
                            return;
                        }
                        _ => {}
                    }
                }
                match pressed {
                    Some(crate::shortcut::Key::Enter) => {
                        let _ = self.submit();
                    }
                    Some(crate::shortcut::Key::Backspace) => self.delete_input(1),
                    Some(crate::shortcut::Key::Delete) => {
                        let length = self.input_line.chars().count();
                        if self.input_cursor < length {
                            self.input_cursor += 1;
                            self.delete_input(1);
                        }
                    }
                    Some(crate::shortcut::Key::Left) => {
                        self.input_cursor = self.input_cursor.saturating_sub(1);
                        if shift {
                            self.selection_anchor.get_or_insert(self.input_cursor + 1);
                        } else {
                            self.selection_anchor = None;
                        }
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::Right) => {
                        let length = self.input_line.chars().count();
                        self.input_cursor = (self.input_cursor + 1).min(length);
                        if shift {
                            self.selection_anchor
                                .get_or_insert(self.input_cursor.saturating_sub(1));
                        } else {
                            self.selection_anchor = None;
                        }
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::Home) => {
                        self.input_cursor = 0;
                        self.selection_anchor = None;
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::End) => {
                        self.input_cursor = self.input_line.chars().count();
                        self.selection_anchor = None;
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::Up) => self.recall_history(true),
                    Some(crate::shortcut::Key::Down) => self.recall_history(false),
                    Some(crate::shortcut::Key::Tab) => {
                        self.completion_requested.emit(self.input_line.clone());
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                // A click on the prompt row places the caret; a click in the output area is not
                // an input position.
                let rect = self.geometry();
                let row_top = rect.y + rect.height as i32 - OUTPUT_LINE_HEIGHT - 6;
                if pos.y >= row_top {
                    let prefix = "> ";
                    let font = effective_font(self.base.style());
                    let prefix_width =
                        crate::widget::metrics::estimate_text_width(prefix, font, 1.0) as i32;
                    let click_x = pos.x - rect.x - 8 - prefix_width;
                    let mut offset = 0i32;
                    self.input_cursor = 0;
                    for ch in self.input_line.chars() {
                        let width =
                            crate::widget::metrics::estimate_text_width(&ch.to_string(), font, 1.0)
                                as i32;
                        if click_x < offset + width / 2 {
                            break;
                        }
                        offset += width;
                        self.input_cursor += 1;
                    }
                    self.input_cursor = self.input_cursor.min(self.input_line.chars().count());
                    self.selection_anchor = None;
                    self.base.request_redraw();
                }
            }
            // Wheel: positive `delta.y` moves the content down, which in a terminal means
            // revealing older output above the tail. Three lines per notch is the desktop
            // default; the tail itself clamps so neither direction can leave the buffer.
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                if self.scroll_output(scroll.lines_y) {
                    self.base.request_redraw();
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for TerminalView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("terminal_view")
            .or_else(|| crate::style::resolved_theme_style_shared("text_edit"));
        // `terminal_view` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical to the
        // window behind it. That is why the field is derived rather than taken from the
        // resolved style when the two coincide:
        //
        // The previous form stepped the resolved fill 8 % toward the ink, where the
        // resolved fill had itself come from `text_edit` (`rgb(180,180,180)` on the light
        // appearance). Two mid-grey steps of 8 % land at `rgb(166,166,166)` — a dark slab
        // on a light theme, and the 1.13:1 background the success-green prompt was then
        // drawn onto. The worst text ratio in the snapshot set was therefore not a text
        // defect at all: the *surface* was wrong, and the text was a symptom.
        //
        // A terminal body is a field, so it is derived the same way every other text
        // field in the crate derives one: from the window fill, stepped toward the ink.
        // The step is large enough to read as a distinct surface (a terminal is *not* the
        // window) and is applied to the window colour rather than to a value that may
        // already be a step. A caller's own colour still wins (rule #21).
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let field_from_theme = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .filter(|resolved| *resolved != window_fill)
            .unwrap_or_else(|| window_fill.blend(&Color::BLACK, 0.14));
        let resolved = style.background_color.unwrap_or(field_from_theme);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.2));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(217, 224, 236));
        let background = resolved;
        // Output text is the *theme's* ink or, failing that, whatever contrasts with the
        // terminal body itself — not the resolved fill's own contrast, because the terminal
        // body is a deliberate step away from the window and the two must stay legible
        // together.
        let output_color = if style.text_color.is_some() || theme.is_some() {
            text_color
        } else {
            background.contrast_color()
        };
        // The prompt line is a *success* state — the shell is ready for input — so it reads
        // the theme's success token rather than a literal green. The token is then pushed
        // away from the terminal body until it clears the legibility floor: at full
        // saturation the green was 1.13:1 on a mid-grey body, which is a colour that exists
        // in the file and not on the screen.
        let prompt_color = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .map(|token| nudge_apart(token, background))
            .unwrap_or_else(|| background.blend(&text_color, 0.6));

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        // The prompt row is anchored to the control's bottom edge, but it has to be a
        // *whole* row: the origin is the glyph's top-left, so `y = height - 10` placed the
        // line's top 10 px above the edge and let the glyphs paint 4 px below it. Reserving
        // one line height plus a margin keeps the prompt inside the frame, and it also
        // subtracts that row from the output area below so the two cannot overlap.
        let prompt_font = effective_font(style);
        let prompt_line_height = context.measure_text("M", prompt_font).height.max(1);
        let prompt_row_height = prompt_line_height + 6;
        let body_height = rect.height.saturating_sub(prompt_row_height);

        let max_lines = (body_height.saturating_sub(16) / prompt_line_height) as usize;
        // Record the measured window so the wheel input path clamps against the rows the user
        // actually sees rather than the constructor's estimate.
        self.visible_output_rows = max_lines.max(1);
        let scroll = self.scroll_offset.min(self.max_scroll_offset());
        let start = self.lines.len().saturating_sub(max_lines).saturating_sub(scroll);
        for (idx, line) in self.lines.iter().skip(start).enumerate() {
            let y = rect.y + 16 + idx as i32 * prompt_line_height as i32;
            let line_bounds =
                Rect::new(rect.x + 8, y, rect.width.saturating_sub(16), prompt_line_height);
            // SGR-styled output: each run is drawn with its colour, and the run widths place
            // the next run, so a coloured prompt line is not a second copy of the parser.
            let mut x = line_bounds.x;
            let right = line_bounds.x + line_bounds.width as i32;
            for run in ansi_runs(line) {
                if x >= right {
                    break;
                }
                let color = match run.color {
                    Some(color) => {
                        if run.dim {
                            color.blend(&background, 0.4)
                        } else {
                            color
                        }
                    }
                    None if run.dim => output_color.blend(&background, 0.4),
                    None => output_color,
                };
                let width = context.measure_text(&run.text, prompt_font).width as i32;
                context.draw_text(
                    crate::core::Point::new(x, y),
                    &run.text,
                    prompt_font,
                    color,
                    HorizontalAlignment::Left,
                );
                x += width;
            }
        }

        let prompt_y = (rect.y + rect.height as i32 - prompt_row_height as i32 + 3).max(rect.y);
        let prefix = "> ";
        let prefix_width = context.measure_text(prefix, prompt_font).width as i32;
        context.draw_text(
            crate::core::Point::new(rect.x + 8, prompt_y),
            prefix,
            prompt_font,
            prompt_color,
            HorizontalAlignment::Left,
        );

        // The input line is drawn character by character from the caret's own metrics: that is
        // the only way the selection band, the caret and the IME composition can all sit where
        // the glyphs are. A single fitted run put the caret at the end of the *string*, which
        // made in-line editing invisible.
        let input_x = rect.x + 8 + prefix_width;
        let mut caret_x = input_x;
        let selection = self.selection_range();
        let mut pen_x = input_x;
        for (index, ch) in self.input_line.chars().enumerate() {
            let glyph = ch.to_string();
            let width = context.measure_text(&glyph, prompt_font).width as i32;
            if selection.is_some_and(|(start, end)| index >= start && index < end) {
                context.fill_rect(
                    Rect::new(pen_x, prompt_y, width.max(1) as u32, prompt_line_height),
                    prompt_color.with_alpha(70),
                );
            }
            if index == self.input_cursor {
                caret_x = pen_x;
            }
            context.draw_text(
                crate::core::Point::new(pen_x, prompt_y),
                &glyph,
                prompt_font,
                prompt_color,
                HorizontalAlignment::Left,
            );
            pen_x += width;
        }
        if self.input_cursor >= self.input_line.chars().count() {
            caret_x = pen_x;
        }
        if !self.preedit.is_empty() {
            context.draw_text(
                crate::core::Point::new(pen_x, prompt_y),
                &self.preedit,
                prompt_font,
                prompt_color.with_alpha(160),
                HorizontalAlignment::Left,
            );
        }

        // The caret sits at the input cursor, in the prompt's own ink — it is the same line,
        // continued. Its x is the measured advance of the characters before it, so it tracks
        // in-line movement rather than always sitting after the whole string.
        if self.base.is_enabled() && self.cursor.is_visible() {
            let caret_x = caret_x.min(rect.x + rect.width as i32 - 2).max(rect.x + 8);
            // A caret is a thin, full-height block rather than a stroke: it is the position the
            // *next* character will occupy, so it is as tall as the line and as wide as one
            // pixel column of it.
            context.fill_rect(
                Rect::new(caret_x, prompt_y, 1, prompt_line_height.max(1)),
                prompt_color,
            );
        }
    }
}

/// One styled run of an ANSI-coloured output line.
struct AnsiRun {
    text: String,
    color: Option<Color>,
    dim: bool,
}

/// Splits one line into SGR-styled runs.
///
/// The parser understands the subset a shell and a progress program actually emit: reset,
/// bold/dim, the sixteen basic colours, and 256-colour foregrounds. Any other control sequence
/// is consumed and ignored, so it cannot leak its escape bytes into the visible text.
fn ansi_runs(line: &str) -> Vec<AnsiRun> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut color = None;
    let mut dim = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            current.push(ch);
            continue;
        }
        if chars.peek() != Some(&'[') {
            continue;
        }
        chars.next();
        let mut params = String::new();
        for next in chars.by_ref() {
            if next.is_ascii_alphabetic() {
                if !current.is_empty() {
                    runs.push(AnsiRun { text: core::mem::take(&mut current), color, dim });
                }
                if next == 'm' {
                    apply_sgr(&params, &mut color, &mut dim);
                }
                break;
            }
            params.push(next);
        }
    }
    if !current.is_empty() {
        runs.push(AnsiRun { text: current, color, dim });
    }
    runs
}

/// Applies one SGR parameter list to the current run's style.
fn apply_sgr(params: &str, color: &mut Option<Color>, dim: &mut bool) {
    let values: Vec<u32> = params.split(';').filter_map(|part| part.parse::<u32>().ok()).collect();
    if values.is_empty() {
        *color = None;
        *dim = false;
        return;
    }
    let mut index = 0usize;
    while index < values.len() {
        match values[index] {
            0 => {
                *color = None;
                *dim = false;
            }
            1 => *dim = false,
            2 => *dim = true,
            22 => *dim = false,
            39 => *color = None,
            30..=37 => *color = Some(basic_ansi_color(values[index] - 30)),
            90..=97 => *color = Some(basic_ansi_color(values[index] - 90)),
            38 => {
                if let (Some(&5), Some(entry)) = (values.get(index + 1), values.get(index + 2)) {
                    *color = Some(xterm_color(*entry));
                    index += 2;
                }
            }
            _ => {}
        }
        index += 1;
    }
}

/// The sixteen basic ANSI colours.
fn basic_ansi_color(index: u32) -> Color {
    const COLORS: [Color; 8] = [
        Color::rgb(0, 0, 0),
        Color::rgb(205, 49, 49),
        Color::rgb(13, 188, 121),
        Color::rgb(229, 229, 16),
        Color::rgb(36, 114, 200),
        Color::rgb(188, 63, 188),
        Color::rgb(17, 168, 205),
        Color::rgb(229, 229, 229),
    ];
    COLORS[(index % 8) as usize]
}

/// The 256-colour palette's entry `index`.
fn xterm_color(index: u32) -> Color {
    match index {
        0..=15 => basic_ansi_color(index % 8),
        16..=231 => {
            let value = index - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            Color::rgb(
                steps[(value / 36) as usize % 6],
                steps[(value / 6) as usize % 6],
                steps[value as usize % 6],
            )
        }
        _ => {
            let level = 8 + (index.saturating_sub(232) as u8).min(23) * 10;
            Color::rgb(level, level, level)
        }
    }
}

/// Pushes `ink` away from `surface` until it is legible on it, preserving the hue.
///
/// Retained as a named call site for the prompt colour: the rule itself lives on
/// [`Color::legible_on`] so the terminal, the calendar and the text editors share one
/// implementation rather than three drifting copies.
fn nudge_apart(ink: Color, surface: Color) -> Color {
    ink.legible_on(surface, 4.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn submit_emits_and_appends_command() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("ls -la");

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        terminal.command_submitted.connect(move |cmd| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(cmd.as_ref().clone());
            }
        });

        assert!(terminal.submit());
        assert_eq!(terminal.lines().last().map(|s| s.as_str()), Some("> ls -la"));

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["ls -la".to_string()]);
    }

    #[test]
    fn history_recall_works_with_arrow_keys() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("first");
        assert!(terminal.submit());
        terminal.set_input_line("second");
        assert!(terminal.submit());

        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "second");

        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "first");

        terminal.handle_event(&Event::key_press(40, 0));
        assert_eq!(terminal.input_line(), "second");
    }

    #[test]
    fn append_output_keeps_recent_window() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        for i in 0..220 {
            terminal.append_output(format!("line {}", i));
        }

        assert_eq!(terminal.lines().len(), 200);
        assert_eq!(terminal.lines().first().map(|s| s.as_str()), Some("line 20"));
    }

    #[test]
    fn default_state() {
        let terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        assert!(terminal.lines().is_empty());
        assert_eq!(terminal.input_line(), "");
    }

    #[test]
    fn append_output_adds_lines() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        terminal.append_output("first line");
        terminal.append_output("second line");

        assert_eq!(terminal.lines().len(), 2);
        assert_eq!(terminal.lines()[0], "first line");
        assert_eq!(terminal.lines()[1], "second line");
    }

    #[test]
    fn empty_submit_returns_false() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        // Empty input line
        assert!(!terminal.submit());

        // Whitespace-only input
        terminal.set_input_line("   ");
        assert!(!terminal.submit());

        // Still no lines appended
        assert!(terminal.lines().is_empty());
    }

    #[test]
    fn clear_input_after_submit() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        terminal.set_input_line("command");
        assert!(terminal.submit());
        assert_eq!(terminal.input_line(), "", "input must be cleared after submit");
    }

    #[test]
    fn input_line_set_get() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        terminal.set_input_line("custom input");
        assert_eq!(terminal.input_line(), "custom input");

        terminal.set_input_line("");
        assert_eq!(terminal.input_line(), "");
    }

    #[test]
    fn history_recall_bounds() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        // No history yet
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "");

        terminal.set_input_line("cmd1");
        assert!(terminal.submit());
        terminal.set_input_line("cmd2");
        assert!(terminal.submit());

        // Up twice: cmd2 -> cmd1
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd2");
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd1");
        // Up again stays at oldest
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd1");

        // Down goes back
        terminal.handle_event(&Event::key_press(40, 0));
        assert_eq!(terminal.input_line(), "cmd2");
        // Down again clears
        terminal.handle_event(&Event::key_press(40, 0));
        assert_eq!(terminal.input_line(), "");
    }

    #[test]
    fn text_input_appends_unicode_to_the_line() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.handle_event(&Event::text_input("你好"));
        terminal.handle_event(&Event::text_input(" ls"));
        assert_eq!(terminal.input_line(), "你好 ls");
    }

    /// An IME commit is committed text and reaches the prompt through the same path.
    ///
    /// The handler used to match only `TextInput`, so a desktop input method's final string was
    /// dropped even though the platform had already resolved it.
    #[test]
    fn ime_commit_appends_to_the_input_line() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.handle_event(&Event::ime_commit("你好"));
        assert_eq!(terminal.input_line(), "你好", "an IME commit must reach the prompt");
        terminal.handle_event(&Event::ime_commit(" ls"));
        assert_eq!(terminal.input_line(), "你好 ls");
    }

    #[test]
    fn backspace_deletes_one_character_not_one_byte() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("ab");
        terminal.handle_event(&Event::key_press(8, 0));
        assert_eq!(terminal.input_line(), "a");

        // A multi-byte glyph is removed whole.
        terminal.set_input_line("你");
        terminal.handle_event(&Event::key_press(8, 0));
        assert_eq!(terminal.input_line(), "");
    }

    #[test]
    fn enter_submits_text_entered_via_text_input() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.handle_event(&Event::text_input("ls -la"));
        terminal.handle_event(&Event::key_press(13, 0));
        assert_eq!(terminal.input_line(), "");
        assert_eq!(terminal.lines().last().map(|s| s.as_str()), Some("> ls -la"));
    }

    #[test]
    fn history_recall_then_editing_starts_a_fresh_line() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("cmd1");
        assert!(terminal.submit());
        terminal.set_input_line("cmd2");
        assert!(terminal.submit());

        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd2");
        terminal.handle_event(&Event::text_input(" extra"));
        assert_eq!(terminal.input_line(), "cmd2 extra");
        // Editing reset the history cursor, so Up recalls from the newest again.
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd2");
    }

    #[test]
    fn a_disabled_terminal_ignores_text_input_and_backspace() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("ab");
        terminal.set_enabled(false);
        terminal.handle_event(&Event::text_input("x"));
        terminal.handle_event(&Event::key_press(8, 0));
        assert_eq!(terminal.input_line(), "ab", "a disabled terminal must not edit the line");
    }
    /// The caret blinks on the ticks it is given, and stops when the terminal is disabled.
    ///
    /// Regression (this plan's A.8.1b P0): the control drew no caret at all — no field, no blink,
    /// no `tick` — so a terminal whose output had stopped was indistinguishable from one that had
    /// hung. The blink is asserted through `is_caret_visible` rather than a pixel, because the
    /// whole contract is that one boolean flipping on `delta_ms`.
    #[test]
    fn the_caret_blinks_on_the_ticks_it_is_given() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 400, 200));
        assert!(terminal.is_animating(), "a fresh terminal waits for input, so it blinks");
        assert!(terminal.is_caret_visible(), "a blink starts visible");

        // A quarter period does not flip it; crossing the half-period does.
        assert!(terminal.tick(100), "a running caret owes another frame");
        assert!(terminal.is_caret_visible(), "100 ms is inside the visible half");
        terminal.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
        assert!(!terminal.is_caret_visible(), "one half-period later it is hidden");
        terminal.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
        assert!(terminal.is_caret_visible(), "and the next half brings it back");

        // A disabled terminal holds a steady caret and stops owing frames, so an idle disabled
        // control does not repaint forever.
        terminal.handle_event(&Event::key_press(65, 0));
        terminal.set_enabled(false);
        terminal.handle_event(&Event::key_press(65, 0));
        assert!(!terminal.is_animating(), "a disabled terminal's caret is steady");
        assert!(terminal.is_caret_visible(), "a steady caret is visible, not hidden");
        assert!(!terminal.tick(100), "and it owes no frames");
    }

    /// The wheel walks the output scrollback and clamps at both ends.
    #[test]
    fn terminal_wheel_scrolls_scrollback_and_clamps() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 400, 140));
        for n in 0..40 {
            terminal.append_output(format!("line{n}"));
        }
        assert_eq!(terminal.scroll_offset, 0, "the tail is the resting state");

        // At the tail, scrolling toward newer output cannot go any further.
        terminal.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(terminal.scroll_offset, 0, "the offset clamps at the tail");

        // One notch the other way reveals three older rows.
        terminal.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(terminal.scroll_offset, 3, "a notch reveals three output rows");

        // Scrolling on stops at the oldest full window.
        let max_offset = terminal.max_scroll_offset();
        assert!(max_offset > 3, "the fixture keeps older output to reach");
        for _ in 0..40 {
            terminal.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        }
        assert_eq!(terminal.scroll_offset, max_offset, "the offset clamps at the oldest window");
    }
}
