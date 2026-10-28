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

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::animation::CursorBlink;
use crate::widget::capability::coercion::{
    expect_bool, expect_number_list, expect_string, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::special_widgets::code_editor::CaretStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The number of output rows the control keeps before dropping the oldest, unless configured
/// through [`TerminalView::set_scrollback_lines`].
pub const DEFAULT_SCROLLBACK_LINES: usize = 5_000;

/// The prompt that precedes the input line and is echoed by [`TerminalView::submit`].
pub const DEFAULT_PROMPT: &str = "> ";

/// How a pointer drag selects the output buffer.
///
/// The two modes differ in what the copied text is, not merely in how it is highlighted: `Lines`
/// copies whole output rows, while `Characters` copies the exact character run between the drag
/// anchor and the pointer. A terminal needs both because a reader copying a command from a log
/// wants the line, and a reader copying a path out of it wants only the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputSelectionMode {
    /// Whole output rows between the anchor and the pointer.
    #[default]
    Lines,
    /// The character run between the anchor and the pointer.
    Characters,
}

impl OutputSelectionMode {
    /// The token spelling published as the `selection_mode` property.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lines => "lines",
            Self::Characters => "characters",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "lines" | "line" => Some(Self::Lines),
            "characters" | "character" | "chars" => Some(Self::Characters),
            _ => None,
        }
    }
}

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
    /// How many output rows the scrollback keeps; the oldest are dropped beyond this.
    scrollback_lines: usize,
    /// Whether an over-wide output row is soft-wrapped instead of truncated at the right edge.
    wrap_output: bool,
    /// Whether the output area paints a scrollbar track and thumb.
    show_scrollbar: bool,
    /// The selection over the output buffer, as an inclusive range of logical line indices.
    ///
    /// Empty selections are stored as `None` rather than as a zero-width range, so "nothing is
    /// selected" has exactly one representation.
    output_selection: Option<(usize, usize)>,
    /// The drag anchor of the output selection, as `(line, character)`.
    output_anchor: Option<(usize, usize)>,
    /// The drag head of the output selection, as `(line, character)`.
    output_head: Option<(usize, usize)>,
    /// Whether a pointer drag started in the output area is still extending the selection.
    selecting_output: bool,
    /// What a drag selects: whole rows or the character run between the two ends.
    selection_mode: OutputSelectionMode,
    /// The caret shape drawn on the input line.
    caret_style: CaretStyle,
    /// Whether the caret blinks. A steady caret is what a screen reader and a screenshot want.
    caret_blink_enabled: bool,
    /// The text preceding the input line, echoed by [`Self::submit`].
    prompt: String,
    /// Explicit sixteen-colour ANSI palette, or `None` for the appearance-derived default.
    ansi_palette: Option<[Color; 16]>,
    /// The last grid reported through `resized`, so a resize that changes nothing is silent.
    reported_grid: (u32, u32),
    /// Parsed and wrapped output rows, invalidated by [`Self::revision`] or a width change.
    output_cache: OutputLineCache,
    /// Measured glyph advances reused by the paint and hit-test paths.
    char_widths: CharWidthCache,
    /// Byte offset of the insertion point inside [`Self::preedit`].
    preedit_cursor: usize,
    /// Counts output mutations, so the parsed-row cache can tell a stale frame from a live one.
    revision: u64,
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
    /// The caret's rectangle on the last painted frame.
    ///
    /// Only the paint path can measure the prompt and every input glyph with the frame's own
    /// metrics, so the box is recorded there rather than re-derived by a second path that could
    /// disagree by a pixel. `None` before the first frame and whenever the terminal draws no
    /// caret.
    caret_rect: Option<Rect>,
    /// The declared character grid, as `(columns, rows)`; `None` derives it from the geometry.
    grid: Option<(u32, u32)>,
    /// The screen buffer a full-screen program has put the control into.
    ///
    /// The line buffer above is what a shell produces; a program that addresses the cursor or
    /// clears the screen needs a grid instead, because "append a line" cannot express "write at
    /// row 4, column 12". Both modes share the paint pipeline: the grid is materialised back into
    /// the line buffer after every write.
    screen: Option<ScreenBuffer>,
    /// Emitted when command is submitted.
    pub command_submitted: Signal1<String>,
    /// Emitted when the user asks to interrupt the running process (Ctrl+C).
    pub interrupt_requested: Signal1<String>,
    /// Emitted when the user asks for completion (Tab), with the current line.
    pub completion_requested: Signal1<String>,
    /// Emitted when an output line arrives, for audit trails and pipeline forwarding.
    pub output_received: Signal1<String>,
    /// Emitted when the output selection becomes non-empty (`true`) or is cleared (`false`).
    pub selection_changed: Signal1<bool>,
    /// Emitted when the character grid changes, as `(columns, rows)`.
    ///
    /// A host that owns the PTY needs this to resize the remote terminal; without it the control
    /// reflowed its own grid while the process behind it kept writing to the old width.
    pub resized: Signal1<(u32, u32)>,
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
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
            wrap_output: true,
            show_scrollbar: true,
            output_selection: None,
            output_anchor: None,
            output_head: None,
            selecting_output: false,
            selection_mode: OutputSelectionMode::default(),
            caret_style: CaretStyle::Line,
            caret_blink_enabled: true,
            prompt: DEFAULT_PROMPT.to_string(),
            ansi_palette: None,
            reported_grid: (0, 0),
            output_cache: OutputLineCache::default(),
            char_widths: CharWidthCache::default(),
            preedit_cursor: 0,
            revision: 0,
            scroll_offset: 0,
            visible_output_rows,
            cursor,
            input_cursor: 0,
            selection_anchor: None,
            preedit: String::new(),
            caret_rect: None,
            grid: None,
            screen: None,
            command_submitted: Signal1::new(),
            interrupt_requested: Signal1::new(),
            completion_requested: Signal1::new(),
            output_received: Signal1::new(),
            selection_changed: Signal1::new(),
            resized: Signal1::new(),
        }
    }

    /// Sets the character grid the view reports, or `None` to derive it from the geometry.
    pub fn set_grid(&mut self, grid: Option<(u32, u32)>) {
        let grid = grid.map(|(columns, rows)| (columns.max(1), rows.max(1)));
        if self.grid == grid {
            return;
        }
        self.grid = grid;
        // A declared grid is the size a host must set on its PTY, so it is reported exactly like
        // a geometry-derived one. Without this a host that pinned the grid never heard about the
        // change it had just asked for.
        self.announce_grid();
        self.base.request_redraw();
    }

    /// Reports the current grid through `resized` when it differs from the last report.
    fn announce_grid(&mut self) {
        let grid = self.grid();
        if grid != self.reported_grid {
            self.reported_grid = grid;
            self.visible_output_rows = Self::estimated_output_rows(self.geometry());
            self.resized.emit(grid);
        }
    }

    /// How many output rows the scrollback keeps before the oldest are dropped.
    pub fn scrollback_lines(&self) -> usize {
        self.scrollback_lines
    }

    /// Sets the scrollback depth, trimming any lines that no longer fit.
    pub fn set_scrollback_lines(&mut self, lines: usize) {
        // A depth of zero would make every append drop the line it just stored, so the smallest
        // meaningful scrollback is one row.
        let lines = lines.max(1);
        if self.scrollback_lines == lines {
            return;
        }
        self.scrollback_lines = lines;
        self.trim_scrollback();
        self.base.request_redraw();
    }

    /// Drops the oldest output rows beyond the configured scrollback.
    ///
    /// `scroll_offset` counts rows above the newest one, so a trim shifts which rows it points at.
    /// The offset is clamped rather than reset: a reader who was scrolling back keeps the window
    /// they were reading unless the trim consumed all of it.
    fn trim_scrollback(&mut self) {
        if self.lines.len() <= self.scrollback_lines {
            return;
        }
        let drop_count = self.lines.len() - self.scrollback_lines;
        self.lines.drain(0..drop_count);
        self.scroll_offset = self.scroll_offset.saturating_sub(drop_count);
        self.output_selection = None;
        self.output_anchor = None;
        self.output_head = None;
        self.revision = self.revision.wrapping_add(1);
        self.clamp_selection_offset();
    }

    /// Whether over-wide output rows are soft-wrapped.
    pub fn wrap_output(&self) -> bool {
        self.wrap_output
    }

    /// Enables or disables soft wrapping of over-wide output rows.
    pub fn set_wrap_output(&mut self, wrap: bool) {
        if self.wrap_output == wrap {
            return;
        }
        self.wrap_output = wrap;
        self.output_cache.invalidate();
        self.scroll_offset = 0;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the output area paints a scrollbar.
    pub fn show_scrollbar(&self) -> bool {
        self.show_scrollbar
    }

    /// Shows or hides the output scrollbar.
    pub fn set_show_scrollbar(&mut self, show: bool) {
        if self.show_scrollbar == show {
            return;
        }
        self.show_scrollbar = show;
        self.base.request_redraw();
    }

    /// The output selection as an inclusive range of logical line indices.
    pub fn output_selection(&self) -> Option<(usize, usize)> {
        self.output_selection
    }

    /// The selected output text, with ANSI escapes removed.
    ///
    /// In [`OutputSelectionMode::Lines`] this is the whole rows; in
    /// [`OutputSelectionMode::Characters`] it is trimmed to the drag's character range. ANSI
    /// sequences are stripped because a clipboard is plain text — copying `ESC[31m` into another
    /// application is never what a reader meant.
    pub fn selected_output(&self) -> Option<String> {
        let (start, end) = self.output_selection?;
        if start > end || end >= self.lines.len() {
            return None;
        }
        match self.selection_mode {
            OutputSelectionMode::Lines => {
                let rows: Vec<String> =
                    self.lines[start..=end].iter().map(|l| strip_ansi(l)).collect();
                Some(rows.join("\n"))
            }
            OutputSelectionMode::Characters => {
                let anchor = self.output_anchor.unwrap_or((start, 0));
                let head = self.output_head.unwrap_or_else(|| {
                    (end, self.lines.get(end).map(|l| strip_ansi(l).chars().count()).unwrap_or(0))
                });
                let (first, last) = if anchor <= head { (anchor, head) } else { (head, anchor) };
                let mut out = String::new();
                for line in first.0..=last.0 {
                    let Some(text) = self.lines.get(line) else { break };
                    let text = strip_ansi(text);
                    let chars: Vec<char> = text.chars().collect();
                    let from = if line == first.0 { first.1 } else { 0 };
                    let to = if line == last.0 { last.1 } else { chars.len() };
                    let from = from.min(chars.len());
                    let to = to.min(chars.len()).max(from);
                    if line != first.0 {
                        out.push('\n');
                    }
                    out.extend(chars[from..to].iter());
                }
                Some(out)
            }
        }
    }

    /// Copies the output selection to the clipboard, reporting whether anything was copied.
    pub fn copy_output(&mut self) -> bool {
        let Some(text) = self.selected_output() else {
            return false;
        };
        crate::clipboard::ClipboardManager::set_text(text)
    }

    /// Clears the output selection.
    pub fn clear_output_selection(&mut self) {
        self.output_anchor = None;
        self.output_head = None;
        self.selecting_output = false;
        self.apply_output_selection(None);
    }

    /// What a drag selects.
    pub fn selection_mode(&self) -> OutputSelectionMode {
        self.selection_mode
    }

    /// Sets what a drag selects.
    pub fn set_selection_mode(&mut self, mode: OutputSelectionMode) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        self.recompute_output_selection();
        self.base.request_redraw();
    }

    /// The caret shape drawn on the input line.
    pub fn caret_style(&self) -> CaretStyle {
        self.caret_style
    }

    /// Sets the caret shape.
    pub fn set_caret_style(&mut self, style: CaretStyle) {
        if self.caret_style == style {
            return;
        }
        self.caret_style = style;
        self.base.request_redraw();
    }

    /// Whether the caret blinks.
    pub fn caret_blink_enabled(&self) -> bool {
        self.caret_blink_enabled
    }

    /// Enables or disables the caret blink.
    ///
    /// A disabled blink leaves the caret visible and stops the frame bus: a control that asked
    /// for a frame it will never draw differently would animate forever.
    pub fn set_caret_blink_enabled(&mut self, enabled: bool) {
        if self.caret_blink_enabled == enabled {
            return;
        }
        self.caret_blink_enabled = enabled;
        if enabled && self.base.is_enabled() {
            self.cursor.start();
        } else {
            self.cursor.stop();
        }
        self.base.request_redraw();
    }

    /// The prompt preceding the input line.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Sets the prompt preceding the input line.
    pub fn set_prompt(&mut self, prompt: impl Into<String>) {
        let prompt = prompt.into();
        if self.prompt == prompt {
            return;
        }
        self.prompt = prompt;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The sixteen-colour ANSI palette in effect.
    ///
    /// An unset palette follows the active appearance: the bright half of a dark-background
    /// palette is unreadable on a light one, so "the default" is two palettes, not one.
    pub fn ansi_palette(&self) -> [Color; 16] {
        self.ansi_palette.unwrap_or_else(|| {
            let dark = crate::style::palette_shared()
                .map(|colors| colors.background.luminance() < 0.5)
                .unwrap_or(true);
            default_ansi_palette(dark)
        })
    }

    /// Sets an explicit sixteen-colour ANSI palette.
    pub fn set_ansi_palette(&mut self, palette: [Color; 16]) {
        self.ansi_palette = Some(palette);
        self.output_cache.invalidate();
        self.base.request_redraw();
    }

    /// Clears an explicit palette so the appearance-derived default applies again.
    pub fn clear_ansi_palette(&mut self) {
        if self.ansi_palette.take().is_some() {
            self.output_cache.invalidate();
            self.base.request_redraw();
        }
    }

    /// The published token spelling of the caret shape.
    fn caret_style_token(&self) -> &'static str {
        match self.caret_style {
            CaretStyle::Line => "line",
            CaretStyle::Block => "block",
            CaretStyle::Underline => "underline",
        }
    }

    /// The screen position of the IME composition caret, for a host placing its candidate window.
    ///
    /// Returns `None` when no composition is in progress: a position for text that is not on
    /// screen would put the candidate window somewhere the user is not typing.
    pub fn preedit_screen_point(&self) -> Option<Point> {
        if self.preedit.is_empty() {
            return None;
        }
        let rect = self.geometry();
        let prompt_row_height = OUTPUT_LINE_HEIGHT + 6;
        let y = (rect.y + rect.height as i32 - prompt_row_height + 3).max(rect.y);
        let font = effective_font(self.base.style());
        let prompt_width =
            crate::widget::metrics::estimate_text_width(&self.prompt, font, 1.0) as i32;
        let before: String = self.preedit.chars().take(self.preedit_cursor_chars()).collect();
        let x = rect.x
            + 8
            + prompt_width
            + self.prefix_width(font)
            + crate::widget::metrics::estimate_text_width(&before, font, 1.0) as i32;
        Some(Point::new(x, y))
    }

    /// The composition caret as a character offset inside the preedit text.
    fn preedit_cursor_chars(&self) -> usize {
        self.preedit.char_indices().take_while(|(offset, _)| *offset < self.preedit_cursor).count()
    }

    /// The measured advance of the input characters before the caret.
    fn prefix_width(&self, font: &Font) -> i32 {
        let chars: String = self.input_line.chars().take(self.input_cursor).collect();
        crate::widget::metrics::estimate_text_width(&chars, font, 1.0) as i32
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
        self.total_output_rows().saturating_sub(self.visible_output_rows.max(1))
    }

    /// How many painted output rows the buffer holds.
    ///
    /// With soft wrapping a logical line occupies one or more rows, and only the paint path has
    /// the measurements to say how many. The cached count is used while it is current; a stale
    /// cache (new output since the last frame) reports the logical line count, which is a lower
    /// bound, so scrolling clamps conservatively for at most one frame instead of pointing past
    /// the buffer.
    fn total_output_rows(&self) -> usize {
        if self.output_cache.revision == self.revision {
            self.output_cache.rows.len().max(self.lines.len())
        } else {
            self.lines.len()
        }
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

    /// Scrolls the output by whole visible pages.
    ///
    /// A positive `direction` moves toward older output, matching the sign convention of
    /// `Self::scroll_output`. Returns whether the viewport moved.
    pub fn scroll_page(&mut self, direction: i32) -> bool {
        let page = self.visible_output_rows.max(1) as i32;
        let moved = self.scroll_output(direction.signum() * page);
        if moved {
            self.base.request_redraw();
        }
        moved
    }

    /// Returns the output view to the newest row.
    pub fn scroll_to_end(&mut self) {
        if self.scroll_offset != 0 {
            self.scroll_offset = 0;
            self.base.request_redraw();
        }
    }

    /// The scrollbar track, or `None` when the bar is hidden or the buffer fits.
    fn scrollbar_track(&self) -> Option<Rect> {
        if !self.show_scrollbar {
            return None;
        }
        let total = self.total_output_rows();
        if total <= self.visible_output_rows.max(1) {
            return None;
        }
        let rect = self.geometry();
        let top = rect.y + 16;
        let bottom = rect.y + rect.height as i32 - OUTPUT_LINE_HEIGHT - 6;
        if bottom <= top {
            return None;
        }
        Some(Rect::new(rect.x + rect.width as i32 - 10, top, 6, (bottom - top) as u32))
    }

    /// The scrollbar thumb, sized and placed from the scroll state.
    fn scrollbar_thumb(&self) -> Option<Rect> {
        let track = self.scrollbar_track()?;
        let total = self.total_output_rows().max(1);
        let visible = self.visible_output_rows.max(1).min(total);
        let height = ((track.height as i64 * visible as i64) / total as i64).max(12) as u32;
        let height = height.min(track.height);
        let range = total.saturating_sub(visible);
        let travel = track.height.saturating_sub(height) as i64;
        // `scroll_offset` counts rows above the newest one, so the viewport's top row is
        // `total - visible - offset`; the thumb follows that row, not the offset itself.
        let top_row = range.saturating_sub(self.scroll_offset.min(range));
        let y =
            track.y + if range > 0 { (travel * top_row as i64 / range as i64) as i32 } else { 0 };
        Some(Rect::new(track.x, y, track.width, height))
    }

    /// Whether `pos` falls on the painted scrollbar track.
    fn scrollbar_track_contains(&self, pos: Point) -> bool {
        self.scrollbar_track().is_some_and(|track| {
            pos.x >= track.x
                && pos.x < track.x + track.width as i32
                && pos.y >= track.y
                && pos.y < track.y + track.height as i32
        })
    }

    /// Scrolls so the viewport's top row is where `y` points on the track.
    fn scroll_to_track(&mut self, y: i32) {
        let Some(track) = self.scrollbar_track() else {
            return;
        };
        let ratio = ((y - track.y) as f32 / track.height.max(1) as f32).clamp(0.0, 1.0);
        let max = self.max_scroll_offset();
        let top_row = (ratio * max as f32).round() as usize;
        let next = max.saturating_sub(top_row);
        if next != self.scroll_offset {
            self.scroll_offset = next;
            self.base.request_redraw();
        }
    }

    /// The logical output line under `pos`, or `None` when the pointer is outside the output area.
    fn output_line_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        let body_top = rect.y + 16;
        let prompt_top = rect.y + rect.height as i32 - OUTPUT_LINE_HEIGHT - 6;
        if pos.y < body_top || pos.y >= prompt_top {
            return None;
        }
        let row_height = OUTPUT_LINE_HEIGHT.max(1);
        let first_row = self
            .total_output_rows()
            .saturating_sub(self.visible_output_rows.max(1))
            .saturating_sub(self.scroll_offset.min(self.max_scroll_offset()));
        let row = first_row + ((pos.y - body_top) / row_height) as usize;
        if self.output_cache.revision == self.revision {
            self.output_cache.origins.get(row).copied()
        } else if row < self.lines.len() {
            Some(row)
        } else {
            None
        }
    }

    /// The character offset in `line` nearest to `x`.
    fn output_char_at(&self, line: usize, x: i32) -> usize {
        let cached = if self.output_cache.revision == self.revision {
            self.output_cache.plain.get(line).map(String::as_str)
        } else {
            None
        };
        // The stripped form is cached per revision, so a pointer move does not re-strip the line.
        let owned =
            cached.is_none().then(|| self.lines.get(line).map(|raw| strip_ansi(raw))).flatten();
        let Some(text) = cached.or(owned.as_deref()) else {
            return 0;
        };
        let font = effective_font(self.base.style());
        let mut offset = 0i32;
        let mut index = 0usize;
        for ch in text.chars() {
            let width = glyph_width(ch, font);
            if x < offset + width / 2 {
                break;
            }
            offset += width;
            index += 1;
        }
        index
    }

    /// Recomputes the line interval from the drag anchor and head.
    fn recompute_output_selection(&mut self) {
        let (Some(anchor), Some(head)) = (self.output_anchor, self.output_head) else {
            self.apply_output_selection(None);
            return;
        };
        let (first, last) = if anchor <= head { (anchor, head) } else { (head, anchor) };
        let next = match self.selection_mode {
            OutputSelectionMode::Lines => Some((first.0, last.0)),
            OutputSelectionMode::Characters => {
                // A character range inside one row is still a selection of that row; the copy
                // path trims it to the exact run.
                if anchor == head {
                    None
                } else {
                    Some((first.0, last.0))
                }
            }
        };
        self.apply_output_selection(next.filter(|(start, end)| start <= end));
    }

    /// Stores an output selection and reports the presence change.
    fn apply_output_selection(&mut self, next: Option<(usize, usize)>) {
        if self.output_selection == next {
            self.base.request_redraw();
            return;
        }
        self.output_selection = next;
        self.selection_changed.emit(next.is_some());
        self.base.request_redraw();
    }

    /// Clamps the stored scroll offset after the buffer shrank.
    fn clamp_selection_offset(&mut self) {
        let max = self.max_scroll_offset();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
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
            // repaint. Only the caret's own box changed, so that is the damage a
            // damage-tracking host is asked for.
            self.request_caret_repaint();
        }
        owes_frame
    }

    /// Submits one caret-blink frame as damage bounded by the caret's own rectangle.
    ///
    /// Mirrors the crate's other animated controls: a mounted control in a damage-tracking
    /// repaint mode asks for the rectangle it changed, while every other host keeps the
    /// whole-control request it already understood. The rectangle is the one the last frame
    /// drew; before the first frame there is none and the whole control is repainted.
    fn request_caret_repaint(&self) {
        #[cfg(not(alloc_frugal))]
        {
            if let (Some(bounds), Some(id)) =
                (self.caret_rect, crate::widget::runtime::registry_id_of(self.id()))
            {
                if crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full
                {
                    crate::widget::runtime::request_repaint_rect(id, bounds);
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// Returns output lines.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Returns the raw text of one scrollback row, or `None` when `row` is out of range.
    ///
    /// The text is what [`Self::append_output`] stored, so it may still carry SGR escape
    /// sequences. Use [`Self::plain_row_text`] for the visible characters.
    pub fn row_text(&self, row: usize) -> Option<&str> {
        self.lines.get(row).map(String::as_str)
    }

    /// Returns one scrollback row with every escape sequence removed.
    ///
    /// This is the text a search sees: an `"error"` painted red is the same five characters as
    /// one painted in the default ink.
    pub fn plain_row_text(&self, row: usize) -> Option<String> {
        self.lines.get(row).map(|line| strip_ansi(line))
    }

    /// Finds every occurrence of `needle` in the scrollback and returns `(row, column)` pairs.
    ///
    /// Rows are indices into [`Self::lines`]; matches come back top-to-bottom, and
    /// left-to-right inside a row. Columns count **characters**, not bytes, of the visible text,
    /// so a column can be used with `line.chars().nth(column)`.
    ///
    /// The scan covers the actual buffer rather than a copy: in screen mode the grid is
    /// materialised back into the line buffer after every write (`feed_screen`), so `lines`
    /// already holds both the scrollback and the visible screen. Escape sequences are stripped
    /// before comparison.
    ///
    /// # Regex
    ///
    /// `use_regex` is accepted so this API matches the other find bars, but **this build has no
    /// regular-expression engine** — the crate has no `regex` dependency and compiles none in.
    /// Matching is therefore always literal, and the flag is deliberately not allowed to change
    /// the result silently: [`TerminalFindBar::regex_supported`] reports `false` for this build,
    /// so a caller can disable the option rather than assume it works. An empty `needle` matches
    /// nothing.
    pub fn find_matches(
        &self,
        needle: &str,
        case_sensitive: bool,
        _use_regex: bool,
    ) -> Vec<(usize, usize)> {
        if needle.is_empty() {
            return Vec::new();
        }
        let needle_chars: Vec<char> = needle.chars().collect();
        let mut matches = Vec::new();
        let mut plain = String::new();
        for (row, line) in self.lines.iter().enumerate() {
            plain.clear();
            strip_ansi_into(line, &mut plain);
            let haystack: Vec<char> = plain.chars().collect();
            if needle_chars.len() > haystack.len() {
                continue;
            }
            for start in 0..=haystack.len() - needle_chars.len() {
                let hit = needle_chars.iter().zip(&haystack[start..]).all(|(query, ch)| {
                    if case_sensitive {
                        query == ch
                    } else {
                        query.eq_ignore_ascii_case(ch)
                    }
                });
                if hit {
                    matches.push((row, start));
                }
            }
        }
        matches
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
        // A program that addresses the cursor or clears the screen cannot be expressed as appended
        // lines, so the control switches to a grid for it. The switch is driven by the sequence
        // itself, which is the program's own declaration of what it needs.
        if self.screen.is_none() && contains_cursor_control(&line) {
            self.screen = Some(ScreenBuffer::new(self.grid().0.max(1) as usize));
        }
        if self.screen.is_some() {
            self.feed_screen(&line);
            self.output_received.emit(line);
            self.trim_scrollback();
            self.scroll_offset = 0;
            self.revision = self.revision.wrapping_add(1);
            self.base.request_layout();
            self.base.request_redraw();
            return;
        }
        if line.contains("\u{1b}[2J") {
            self.lines.clear();
            self.scroll_offset = 0;
        }
        // A carriage return without a line feed overwrites the current line, which is how a
        // progress indicator repaints itself.
        if let Some(rest) = line.strip_prefix('\r') {
            if let Some(last) = self.lines.last_mut() {
                *last = rest.to_string();
                self.revision = self.revision.wrapping_add(1);
                self.output_received.emit(rest.to_string());
                self.base.request_redraw();
                return;
            }
        }
        self.lines.push(line.clone());
        self.trim_scrollback();
        // New output returns the view to the tail. A reader who scrolled back would otherwise
        // have their offset silently point at different lines as the buffer shifts.
        self.scroll_offset = 0;
        self.revision = self.revision.wrapping_add(1);
        self.output_received.emit(line);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Feeds `line` to the screen grid and republishes the rows it changed.
    ///
    /// Only the rows the program actually touched are turned back into text. Materialising the
    /// whole grid on every chunk made per-write cost O(grid) instead of O(written cells).
    fn feed_screen(&mut self, line: &str) {
        let columns = self.grid().0.max(1) as usize;
        let Some(screen) = self.screen.as_mut() else {
            return;
        };
        screen.resize(columns);
        screen.feed(line);
        let Some((first, last)) = screen.take_dirty() else {
            return;
        };
        // The grid is authoritative in screen mode, so a line that is not a grid row (for example
        // one `submit` appended) is dropped, exactly as the full materialisation dropped it. Rows
        // that were trimmed as empty still exist in the grid and may carry text now, so the buffer
        // is then grown to the grid before any row is patched into it.
        self.lines.truncate(screen.cells.len());
        while self.lines.len() < screen.cells.len() {
            self.lines.push(String::new());
        }
        if first == 0 && last >= screen.cells.len() {
            self.lines = screen.render_lines();
            return;
        }
        for row in first..last.min(screen.cells.len()) {
            if let Some(text) = screen.row_text(row) {
                if let Some(slot) = self.lines.get_mut(row) {
                    *slot = text;
                }
            }
        }
        // Trailing empty rows are not part of the buffer, exactly as `render_lines` drops them.
        while self.lines.len() > 1 && self.lines.last().is_some_and(|line| line.is_empty()) {
            self.lines.pop();
        }
    }

    /// Appends several output lines with one layout pass.
    ///
    /// A process writes in bursts; asking for a layout per line made a 200-line burst queue 200
    /// requests for one visible change.
    pub fn append_output_batch(&mut self, lines: impl IntoIterator<Item = impl Into<String>>) {
        let mut changed = false;
        for line in lines {
            let line = line.into();
            if self.screen.is_none() && contains_cursor_control(&line) {
                self.screen = Some(ScreenBuffer::new(self.grid().0.max(1) as usize));
            }
            self.output_received.emit(line.clone());
            if self.screen.is_some() {
                self.feed_screen(&line);
            } else {
                if line.contains("\u{1b}[2J") {
                    self.lines.clear();
                    self.scroll_offset = 0;
                }
                self.lines.push(line);
            }
            changed = true;
        }
        if !changed {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        self.trim_scrollback();
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
        self.lines.push(format!("{}{cmd}", self.prompt));
        self.revision = self.revision.wrapping_add(1);
        self.trim_scrollback();
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
            "completion_requested" => Some(EventSignalRef::mapped(
                "completion_requested",
                &self.completion_requested,
                |v| crate::widget::capability::CapabilityValue::String(v.clone()),
            )),
            "interrupt_requested" => Some(EventSignalRef::mapped(
                "interrupt_requested",
                &self.interrupt_requested,
                |v| crate::widget::capability::CapabilityValue::String(v.clone()),
            )),
            "output_received" => {
                Some(EventSignalRef::mapped("output_received", &self.output_received, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "resized" => Some(EventSignalRef::mapped("resized", &self.resized, |v| {
                CapabilityValue::Tuple(vec![
                    CapabilityValue::UInt(v.0 as u64),
                    CapabilityValue::UInt(v.1 as u64),
                ])
            })),
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
            "scrollback_lines" => Ok(CapabilityValue::UInt(self.scrollback_lines as u64)),
            "wrap_output" => Ok(CapabilityValue::Bool(self.wrap_output)),
            "show_scrollbar" => Ok(CapabilityValue::Bool(self.show_scrollbar)),
            "caret_style" => Ok(CapabilityValue::String(self.caret_style_token().to_string())),
            "caret_blink_enabled" => Ok(CapabilityValue::Bool(self.caret_blink_enabled)),
            "prompt" => Ok(CapabilityValue::String(self.prompt.clone())),
            "selection_mode" => {
                Ok(CapabilityValue::String(self.selection_mode.as_str().to_string()))
            }
            "ansi_palette" => {
                let values = self
                    .ansi_palette()
                    .iter()
                    .map(|color| {
                        let packed =
                            ((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32;
                        CapabilityValue::Float(packed as f64)
                    })
                    .collect();
                Ok(CapabilityValue::Tuple(values))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "input_line" => {
                self.set_input_line(expect_string(value)?);
                Ok(())
            }
            "scrollback_lines" => {
                self.set_scrollback_lines(expect_usize(value)?);
                Ok(())
            }
            "wrap_output" => {
                self.set_wrap_output(expect_bool(value)?);
                Ok(())
            }
            "show_scrollbar" => {
                self.set_show_scrollbar(expect_bool(value)?);
                Ok(())
            }
            "caret_style" => {
                let token = expect_string(value)?;
                let style =
                    caret_style_from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_caret_style(style);
                Ok(())
            }
            "caret_blink_enabled" => {
                self.set_caret_blink_enabled(expect_bool(value)?);
                Ok(())
            }
            "prompt" => {
                self.set_prompt(expect_string(value)?);
                Ok(())
            }
            "selection_mode" => {
                let token = expect_string(value)?;
                let mode = OutputSelectionMode::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_selection_mode(mode);
                Ok(())
            }
            "ansi_palette" => {
                let values = expect_number_list(value)?;
                if values.len() != 16 {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                let mut palette = [Color::BLACK; 16];
                for (slot, entry) in palette.iter_mut().zip(values.iter()) {
                    let packed = *entry as i64;
                    if !(0..=0xFF_FFFF).contains(&packed) {
                        return Err(CapabilityAccessError::OutOfRange);
                    }
                    *slot = Color::rgb(
                        ((packed >> 16) & 0xFF) as u8,
                        ((packed >> 8) & 0xFF) as u8,
                        (packed & 0xFF) as u8,
                    );
                }
                self.set_ansi_palette(palette);
                Ok(())
            }
            // Derived from the output buffer. The old writer had no arm for it.
            "output_line_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TERMINAL_VIEW_PROPERTIES` plus the names added for this round; see the
        // capability-schema addition reported with the change.
        property_names_of![
            "output_line_count",
            "input_line",
            "scrollback_lines",
            "wrap_output",
            "show_scrollbar",
            "caret_style",
            "caret_blink_enabled",
            "prompt",
            "selection_mode",
            "ansi_palette",
            BASE_PROPERTY_NAMES
        ]
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
        if !self.caret_blink_enabled {
            // A steady caret is a deliberate appearance, not a paused animation: it must not
            // keep asking the frame bus for frames it will never draw differently.
            self.cursor.stop();
        } else if !self.cursor.is_running() {
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
                // the composition, which is where the candidate window is anchored. Dropping it
                // drew the composition at the end of the line even when the caret was mid-line.
                self.preedit = text.clone();
                self.preedit_cursor = (*cursor).min(text.len());
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } => {
                let pressed = crate::shortcut::Key::from_key_code(*key);
                let primary = crate::shortcut::event_carries_primary(*modifiers);
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT);
                if primary {
                    match pressed {
                        Some(crate::shortcut::Key::C) => {
                            // Ctrl+C: copy selected output first, then the input selection, and
                            // interrupt only when neither exists. A user who has just dragged
                            // over a log line meant to copy it, not to kill the process.
                            if self.output_selection.is_some() {
                                self.copy_output();
                            } else if let Some(selected) = self.selected_input() {
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
                            // Ctrl+L returns the control to its line buffer; a grid left behind
                            // would repopulate itself on the next full-screen write.
                            self.screen = None;
                            self.scroll_offset = 0;
                            self.revision = self.revision.wrapping_add(1);
                            self.clear_output_selection();
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
                    Some(crate::shortcut::Key::PageUp) => {
                        self.scroll_page(1);
                    }
                    Some(crate::shortcut::Key::PageDown) => {
                        self.scroll_page(-1);
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            Event::MousePress { pos, button: 1, modifiers, .. } => {
                let rect = self.geometry();
                let row_top = rect.y + rect.height as i32 - OUTPUT_LINE_HEIGHT - 6;
                if pos.y >= row_top {
                    let font = effective_font(self.base.style());
                    let prefix_width =
                        crate::widget::metrics::estimate_text_width(&self.prompt, font, 1.0) as i32;
                    let click_x = pos.x - rect.x - 8 - prefix_width;
                    let mut offset = 0i32;
                    self.input_cursor = 0;
                    for ch in self.input_line.chars() {
                        let width = glyph_width(ch, font);
                        if click_x < offset + width / 2 {
                            break;
                        }
                        offset += width;
                        self.input_cursor += 1;
                    }
                    self.input_cursor = self.input_cursor.min(self.input_line.chars().count());
                    self.selection_anchor = None;
                    self.clear_output_selection();
                    self.base.request_redraw();
                    return;
                }
                // A press on the scrollbar track jumps the viewport to that position instead of
                // selecting output underneath it.
                if self.show_scrollbar && self.scrollbar_track_contains(*pos) {
                    self.scroll_to_track(pos.y);
                    return;
                }
                if let Some(line) = self.output_line_at(*pos) {
                    let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                        .contains(crate::shortcut::Modifiers::SHIFT);
                    let character = if self.selection_mode == OutputSelectionMode::Characters {
                        self.output_char_at(line, pos.x - rect.x - 8)
                    } else {
                        0
                    };
                    if !shift || self.output_anchor.is_none() {
                        self.output_anchor = Some((line, character));
                    }
                    self.output_head = Some((line, character));
                    self.selecting_output = true;
                    self.recompute_output_selection();
                }
            }
            // Double-clicking an output row selects the whole row; dragging has no other way to
            // say "this row, not the one above it".
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(line) = self.output_line_at(*pos) {
                    let length =
                        self.lines.get(line).map(|l| strip_ansi(l).chars().count()).unwrap_or(0);
                    self.output_anchor = Some((line, 0));
                    self.output_head = Some((line, length));
                    self.selecting_output = false;
                    self.apply_output_selection(Some((line, line)));
                }
            }
            Event::MouseMove { pos } => {
                if self.selecting_output {
                    if let Some(line) = self.output_line_at(*pos) {
                        let rect = self.geometry();
                        let character = if self.selection_mode == OutputSelectionMode::Characters {
                            self.output_char_at(line, pos.x - rect.x - 8)
                        } else {
                            0
                        };
                        self.output_head = Some((line, character));
                        self.recompute_output_selection();
                    }
                }
            }
            Event::MouseRelease { .. } => {
                self.selecting_output = false;
            }
            Event::MouseLeave { .. } => {
                // A release outside the control is not delivered to it, so leaving is the last
                // signal that the drag ended; without this a later hover would keep extending
                // the selection.
                self.selecting_output = false;
            }
            Event::Resize { .. } => {
                // The event does not carry this control's own rectangle (it is the resized
                // container's size), so the grid is re-derived from the geometry the layout
                // has already applied and reported only when it actually changed.
                self.visible_output_rows = Self::estimated_output_rows(self.geometry());
                self.base.request_layout();
                self.announce_grid();
                self.clamp_selection_offset();
                self.base.request_redraw();
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
        let window_fill =
            crate::style::palette_shared().map(|colors| colors.background).unwrap_or(Color::WHITE);
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

        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

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

        // The scrollbar's column is reserved before wrapping, so a wrapped row never runs under
        // the thumb. Whether it is needed is derived from the logical line count first (wrapping
        // can only add rows, so a line overflow is a definite row overflow) and from the previous
        // frame's parsed row count otherwise.
        let need_scrollbar = self.show_scrollbar
            && (self.lines.len() > max_lines
                || (self.output_cache.revision == self.revision
                    && self.output_cache.rows.len() > max_lines));
        let content_width = if need_scrollbar {
            rect.width.saturating_sub(28)
        } else {
            rect.width.saturating_sub(16)
        } as i32;
        let palette =
            self.ansi_palette.unwrap_or_else(|| default_ansi_palette(background.luminance() < 0.5));
        ensure_output_rows(
            &mut self.output_cache,
            &self.lines,
            &mut self.char_widths,
            self.revision,
            self.wrap_output,
            context,
            prompt_font,
            content_width,
            &palette,
        );

        let total_rows = self.output_cache.rows.len();
        let visible = max_lines.max(1);
        let scroll = self.scroll_offset.min(total_rows.saturating_sub(visible));
        let start = total_rows.saturating_sub(visible).saturating_sub(scroll);
        let selection = self.output_selection;
        for (offset, runs) in self.output_cache.rows.iter().enumerate().skip(start).take(visible) {
            let y = rect.y + 16 + (offset - start) as i32 * prompt_line_height as i32;
            let origin = self.output_cache.origins.get(offset).copied().unwrap_or(offset);
            if selection.is_some_and(|(first, last)| origin >= first && origin <= last) {
                // The band is drawn per painted row; a character-mode copy is trimmed further
                // when the text is taken, which the band cannot show without per-glyph bookkeeping.
                context.fill_rect(
                    Rect::new(rect.x + 4, y, rect.width.saturating_sub(8), prompt_line_height),
                    crate::style::semantic_color(crate::style::SemanticColor::Info)
                        .unwrap_or(prompt_color)
                        .with_alpha(60),
                );
            }
            // SGR-styled output: each run is drawn with its colour, and the run widths place
            // the next run, so a coloured prompt line is not a second copy of the parser.
            let mut x = rect.x + 8;
            for run in runs {
                let base_ink = match run.color {
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
                let width = run.width;
                // SGR reverse swaps the glyph's ink and its cell's fill, so the run reads as a
                // selection or an error banner instead of ordinary text. An explicit background
                // (SGR 40-47, 100-107, 48;…) is the same cell fill without the swap.
                let (ink, cell_fill) = if run.reverse {
                    (run.bg.unwrap_or(background), Some(base_ink))
                } else {
                    (base_ink, run.bg)
                };
                if let Some(fill) = cell_fill {
                    context
                        .fill_rect(Rect::new(x, y, width.max(1) as u32, prompt_line_height), fill);
                }
                // SGR 3 selects the italic face. The run's advance was measured with the upright
                // face, which the backend shapes at the same width, so the saved width still
                // places the following run correctly.
                let glyph_font = if run.italic {
                    std::borrow::Cow::Owned(prompt_font.with_italic(true))
                } else {
                    std::borrow::Cow::Borrowed(prompt_font)
                };
                context.draw_text(
                    crate::core::Point::new(x, y),
                    &run.text,
                    &glyph_font,
                    ink,
                    HorizontalAlignment::Left,
                );
                if run.bold {
                    // The face has one weight, so bold is the crate's one-pixel overprint — the
                    // same trick `draw_markdown_line` uses. Dropping the SGR bold bit entirely is
                    // what made a bold prompt render as ordinary text.
                    context.draw_text(
                        crate::core::Point::new(x + 1, y),
                        &run.text,
                        &glyph_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
                if run.underline {
                    // A one-pixel rule on the line's baseline, the same weight as every other
                    // one-pixel rule this crate draws.
                    let baseline = y + prompt_line_height.saturating_sub(2) as i32;
                    context.draw_line(
                        crate::core::Point::new(x, baseline),
                        crate::core::Point::new(x + width.max(1), baseline),
                        ink,
                    );
                }
                x += width;
            }
        }

        if need_scrollbar {
            if let (Some(track), Some(thumb)) = (self.scrollbar_track(), self.scrollbar_thumb()) {
                context.fill_rect(track, background.blend(&output_color, 0.10));
                context.fill_rect(thumb, output_color.with_alpha(110));
            }
        }

        let prompt_y = (rect.y + rect.height as i32 - prompt_row_height as i32 + 3).max(rect.y);
        let prefix_width = context.measure_text(&self.prompt, prompt_font).width as i32;
        context.draw_text(
            crate::core::Point::new(rect.x + 8, prompt_y),
            &self.prompt,
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
            let width = self.char_widths.width(ch, prompt_font, context);
            if selection.is_some_and(|(start, end)| index >= start && index < end) {
                context.fill_rect(
                    Rect::new(pen_x, prompt_y, width.max(1) as u32, prompt_line_height),
                    prompt_color.with_alpha(70),
                );
            }
            if index == self.input_cursor {
                caret_x = pen_x;
            }
            let mut glyph = [0u8; 4];
            context.draw_text(
                crate::core::Point::new(pen_x, prompt_y),
                ch.encode_utf8(&mut glyph),
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
            // The composition is drawn where the caret is, not at the end of the line: an IME
            // user composing in the middle of a command was shown their text in the wrong place,
            // and the candidate window was anchored there too.
            context.draw_text(
                crate::core::Point::new(caret_x, prompt_y),
                &self.preedit,
                prompt_font,
                prompt_color.with_alpha(160),
                HorizontalAlignment::Left,
            );
        }

        // The caret sits at the input cursor, in the prompt's own ink — it is the same line,
        // continued. Its x is the measured advance of the characters before it, so it tracks
        // in-line movement rather than always sitting after the whole string.
        //
        // The box is resolved on every frame, visible or not, because the blink's damage
        // rectangle has to be the same box on both halves of the cycle.
        let caret_box = if self.base.is_enabled() {
            let caret_x = caret_x.min(rect.x + rect.width as i32 - 2).max(rect.x + 8);
            let cell = context.measure_text("M", prompt_font).width.max(1);
            Some(match self.caret_style {
                CaretStyle::Line => Rect::new(caret_x, prompt_y, 1, prompt_line_height.max(1)),
                CaretStyle::Block => Rect::new(caret_x, prompt_y, cell, prompt_line_height.max(1)),
                CaretStyle::Underline => Rect::new(
                    caret_x,
                    prompt_y + prompt_line_height.saturating_sub(2) as i32,
                    cell,
                    2,
                ),
            })
        } else {
            None
        };
        self.caret_rect = caret_box;
        if self.cursor.is_visible() || !self.caret_blink_enabled {
            if let Some(caret) = caret_box {
                let fill = match self.caret_style {
                    CaretStyle::Block => prompt_color.with_alpha(140),
                    CaretStyle::Line | CaretStyle::Underline => prompt_color,
                };
                context.fill_rect(caret, fill);
            }
        }
    }
}

/// One styled run of an ANSI-coloured output line.
#[derive(Debug, Clone)]
struct AnsiRun {
    text: String,
    color: Option<Color>,
    /// The run's background (SGR 40-47, 100-107, or 48;…), resolved against the palette.
    bg: Option<Color>,
    dim: bool,
    bold: bool,
    /// The run is italic (SGR 3).
    italic: bool,
    /// The run is underlined (SGR 4).
    underline: bool,
    /// The run swaps its foreground and background (SGR 7).
    reverse: bool,
    /// The run's measured advance, resolved when the row was parsed.
    ///
    /// The paint path measured every run on every frame, though a run's width cannot change while
    /// its text, font and row cache key stay the same.
    width: i32,
}

/// One SGR colour: an xterm palette index or a 24-bit truecolour triple.
///
/// The screen grid stores this rather than a resolved [`Color`] so a palette change still
/// recolours text that was written before it: the index is resolved on the line pipeline, exactly
/// as a shell's own escape sequences are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SgrColor {
    /// An xterm palette index, `0..=255`.
    Indexed(u8),
    /// A 24-bit truecolour triple.
    Rgb(u8, u8, u8),
}

/// The SGR state a screen cell is written with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SgrStyle {
    fg: Option<SgrColor>,
    bg: Option<SgrColor>,
    dim: bool,
    bold: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
}

/// The SGR state the line pipeline parses, with colours already resolved against the palette.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct RunStyle {
    color: Option<Color>,
    background: Option<Color>,
    dim: bool,
    bold: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
}

/// One cell of the cursor-addressed screen.
#[derive(Clone, Copy)]
struct ScreenCell {
    ch: char,
    style: SgrStyle,
}

impl Default for ScreenCell {
    fn default() -> Self {
        Self { ch: ' ', style: SgrStyle::default() }
    }
}

/// A cursor-addressed screen.
///
/// A line buffer cannot express "write at row 4, column 12" or "clear to the end of the line", so
/// a program that emits those sequences is given a grid instead. The grid is materialised back
/// into the line buffer after every write, which is what lets the existing wrap, paint, selection
/// and scrollback paths handle it unchanged.
struct ScreenBuffer {
    cells: Vec<Vec<ScreenCell>>,
    row: usize,
    column: usize,
    columns: usize,
    /// The SGR state new characters are written with.
    style: SgrStyle,
    /// The rows whose materialised text changed since the last [`Self::take_dirty`], as
    /// `(first, last_exclusive)`.
    ///
    /// A program writes at its cursor, so this is usually one row. Tracking the range is what lets
    /// the line buffer be patched instead of being re-materialised from the whole grid on every
    /// chunk of output.
    dirty: Option<(usize, usize)>,
    /// How many rows have been turned back into text; read by the tests that pin the incremental
    /// path, and compiled away otherwise.
    #[cfg(test)]
    materialized_rows: usize,
}

impl ScreenBuffer {
    /// Creates an empty screen `columns` wide.
    fn new(columns: usize) -> Self {
        Self {
            cells: vec![vec![ScreenCell::default(); columns.max(1)]],
            row: 0,
            column: 0,
            columns: columns.max(1),
            style: SgrStyle::default(),
            dirty: None,
            #[cfg(test)]
            materialized_rows: 0,
        }
    }

    /// Marks `row` as needing its text rebuilt.
    fn mark_dirty(&mut self, row: usize) {
        self.dirty = Some(match self.dirty {
            Some((first, last)) => (first.min(row), last.max(row + 1)),
            None => (row, row + 1),
        });
    }

    /// Marks every row as needing its text rebuilt.
    fn mark_all_dirty(&mut self) {
        self.dirty = Some((0, self.cells.len()));
    }

    /// Takes the pending row range, if any.
    fn take_dirty(&mut self) -> Option<(usize, usize)> {
        self.dirty.take()
    }

    /// Re-sizes the grid when the control's own column count changed.
    fn resize(&mut self, columns: usize) {
        let columns = columns.max(1);
        if self.columns == columns {
            return;
        }
        self.columns = columns;
        for row in &mut self.cells {
            row.resize(columns, ScreenCell::default());
        }
        self.column = self.column.min(columns - 1);
        // A narrower grid truncates cells, so every row's text can change.
        self.mark_all_dirty();
    }

    /// Ensures the cursor's row exists.
    fn ensure_row(&mut self) {
        while self.cells.len() <= self.row {
            self.cells.push(vec![ScreenCell::default(); self.columns]);
        }
    }

    /// Feeds one chunk of output through the grid.
    fn feed(&mut self, text: &str) {
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\u{1b}' => {
                    if chars.peek() != Some(&'[') {
                        continue;
                    }
                    chars.next();
                    let mut params = String::new();
                    let mut final_byte = None;
                    for next in chars.by_ref() {
                        if next.is_ascii_alphabetic() || next == '@' || next == '`' {
                            final_byte = Some(next);
                            break;
                        }
                        params.push(next);
                    }
                    if let Some(final_byte) = final_byte {
                        self.control(&params, final_byte);
                    }
                }
                '\n' => {
                    self.row += 1;
                    self.column = 0;
                    self.ensure_row();
                }
                '\r' => self.column = 0,
                '\t' => {
                    let next = (self.column / 8 + 1) * 8;
                    self.column = next.min(self.columns.saturating_sub(1));
                }
                '\u{8}' => self.column = self.column.saturating_sub(1),
                ch if !ch.is_control() => self.write(ch),
                _ => {}
            }
        }
    }

    /// Writes one printable character at the cursor and advances it.
    fn write(&mut self, ch: char) {
        if self.column >= self.columns {
            self.row += 1;
            self.column = 0;
        }
        self.ensure_row();
        if let Some(row) = self.cells.get_mut(self.row) {
            if let Some(cell) = row.get_mut(self.column) {
                *cell = ScreenCell { ch, style: self.style };
            }
        }
        self.mark_dirty(self.row);
        self.column += 1;
    }

    /// Applies one CSI sequence.
    fn control(&mut self, params: &str, final_byte: char) {
        let values: Vec<u32> =
            params.split(';').filter_map(|part| part.parse::<u32>().ok()).collect();
        let first = values.first().copied().unwrap_or(0) as usize;
        let second = values.get(1).copied().unwrap_or(0) as usize;
        match final_byte {
            'm' => self.apply_style(&values),
            'H' | 'f' => {
                self.row = first.max(1).saturating_sub(1);
                self.column = second.max(1).saturating_sub(1).min(self.columns - 1);
                self.ensure_row();
            }
            'A' => self.row = self.row.saturating_sub(first.max(1)),
            'B' => {
                self.row += first.max(1);
                self.ensure_row();
            }
            'C' => self.column = (self.column + first.max(1)).min(self.columns - 1),
            'D' => self.column = self.column.saturating_sub(first.max(1)),
            'G' => self.column = first.max(1).saturating_sub(1).min(self.columns - 1),
            'd' => {
                self.row = first.max(1).saturating_sub(1);
                self.ensure_row();
            }
            'J' => self.clear_screen(first),
            'K' => self.clear_line(first),
            _ => {}
        }
    }

    /// Applies an SGR parameter list to the write style.
    fn apply_style(&mut self, values: &[u32]) {
        if values.is_empty() {
            self.style = SgrStyle::default();
            return;
        }
        let mut index = 0usize;
        while index < values.len() {
            match values[index] {
                0 => self.style = SgrStyle::default(),
                1 => self.style.bold = true,
                2 => self.style.dim = true,
                3 => self.style.italic = true,
                4 => self.style.underline = true,
                7 => self.style.reverse = true,
                22 => {
                    self.style.dim = false;
                    self.style.bold = false;
                }
                23 => self.style.italic = false,
                24 => self.style.underline = false,
                27 => self.style.reverse = false,
                39 => self.style.fg = None,
                49 => self.style.bg = None,
                30..=37 => {
                    self.style.fg = Some(SgrColor::Indexed((values[index] - 30) as u8));
                }
                40..=47 => {
                    self.style.bg = Some(SgrColor::Indexed((values[index] - 40) as u8));
                }
                90..=97 => {
                    self.style.fg = Some(SgrColor::Indexed((values[index] - 90 + 8) as u8));
                }
                100..=107 => {
                    self.style.bg = Some(SgrColor::Indexed((values[index] - 100 + 8) as u8));
                }
                38 => {
                    if let Some((color, consumed)) = extended_sgr_color(values, index) {
                        self.style.fg = Some(color);
                        index += consumed;
                    }
                }
                48 => {
                    if let Some((color, consumed)) = extended_sgr_color(values, index) {
                        self.style.bg = Some(color);
                        index += consumed;
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }

    /// Clears the screen: mode 0 from the cursor down, 1 from the top to the cursor, 2 all.
    fn clear_screen(&mut self, mode: usize) {
        match mode {
            1 => {
                for row in 0..self.row {
                    self.clear_rows(row);
                }
                self.clear_line(1);
            }
            2 => {
                for row in 0..self.cells.len() {
                    self.clear_rows(row);
                }
                self.row = 0;
                self.column = 0;
            }
            _ => {
                self.clear_line(0);
                for row in (self.row + 1)..self.cells.len() {
                    self.clear_rows(row);
                }
            }
        }
    }

    /// Clears one whole row.
    fn clear_rows(&mut self, row: usize) {
        if let Some(cells) = self.cells.get_mut(row) {
            for cell in cells.iter_mut() {
                *cell = ScreenCell::default();
            }
        }
        self.mark_dirty(row);
    }

    /// Clears the cursor's line: mode 0 to the end, 1 from the start, 2 whole.
    fn clear_line(&mut self, mode: usize) {
        self.ensure_row();
        let Some(cells) = self.cells.get_mut(self.row) else {
            return;
        };
        match mode {
            1 => {
                let limit = (self.column + 1).min(cells.len());
                for cell in cells.iter_mut().take(limit) {
                    *cell = ScreenCell::default();
                }
            }
            2 => {
                for cell in cells.iter_mut() {
                    *cell = ScreenCell::default();
                }
            }
            _ => {
                for cell in cells.iter_mut().skip(self.column) {
                    *cell = ScreenCell::default();
                }
            }
        }
        self.mark_dirty(self.row);
    }

    /// One grid row as text with SGR sequences, trailing blanks trimmed.
    fn row_text(&mut self, row: usize) -> Option<String> {
        let cells = self.cells.get(row)?;
        let mut out = String::new();
        let mut current = SgrStyle::default();
        for cell in cells {
            if cell.style != current {
                emit_sgr(&mut out, cell.style);
                current = cell.style;
            }
            out.push(cell.ch);
        }
        while out.ends_with(' ') {
            out.pop();
        }
        #[cfg(test)]
        {
            self.materialized_rows += 1;
        }
        Some(out)
    }

    /// The grid as text lines with SGR sequences, ready for the line pipeline.
    ///
    /// Named `render_lines` rather than `to_lines` because it mutates the grid: it resets the
    /// per-row materialisation counter and `to_*` on a `&mut self` receiver reads as a cheap
    /// conversion, which this is not.
    fn render_lines(&mut self) -> Vec<String> {
        let mut lines: Vec<String> =
            (0..self.cells.len()).filter_map(|row| self.row_text(row)).collect();
        while lines.len() > 1 && lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
        lines
    }
}

/// Writes the SGR sequence that selects `style`.
fn emit_sgr(out: &mut String, style: SgrStyle) {
    out.push_str("\u{1b}[0");
    if style.dim {
        out.push_str(";2");
    }
    if style.bold {
        out.push_str(";1");
    }
    if style.italic {
        out.push_str(";3");
    }
    if style.underline {
        out.push_str(";4");
    }
    if style.reverse {
        out.push_str(";7");
    }
    emit_sgr_color(out, style.fg, false);
    emit_sgr_color(out, style.bg, true);
    out.push('m');
}

/// Appends the parameters selecting `color`, using the background ranges when asked.
fn emit_sgr_color(out: &mut String, color: Option<SgrColor>, background: bool) {
    match color {
        None => {}
        // The eight and bright-eight ranges have their own SGR numbers; only a wider index needs
        // the extended `38;5;n` form.
        Some(SgrColor::Indexed(index)) if index < 8 => {
            out.push(';');
            out.push_str(&(if background { 40 } else { 30 } + index as u32).to_string());
        }
        Some(SgrColor::Indexed(index)) if index < 16 => {
            out.push(';');
            out.push_str(&(if background { 100 } else { 90 } + (index as u32 - 8)).to_string());
        }
        Some(SgrColor::Indexed(index)) => {
            out.push_str(if background { ";48;5;" } else { ";38;5;" });
            out.push_str(&index.to_string());
        }
        Some(SgrColor::Rgb(r, g, b)) => {
            out.push_str(if background { ";48;2;" } else { ";38;2;" });
            out.push_str(&r.to_string());
            out.push(';');
            out.push_str(&g.to_string());
            out.push(';');
            out.push_str(&b.to_string());
        }
    }
}

/// Parses the `5;n` or `2;r;g;b` form that follows SGR 38/48.
///
/// Returns the colour and how many parameters beyond `index` it consumed, so the caller can skip
/// them instead of reading the same numbers as ordinary attributes.
fn extended_sgr_color(values: &[u32], index: usize) -> Option<(SgrColor, usize)> {
    match values.get(index + 1) {
        Some(5) => {
            let entry = *values.get(index + 2)?;
            Some((SgrColor::Indexed(entry.min(255) as u8), 2))
        }
        Some(2) => {
            let r = *values.get(index + 2)?;
            let g = *values.get(index + 3)?;
            let b = *values.get(index + 4)?;
            Some((SgrColor::Rgb(r.min(255) as u8, g.min(255) as u8, b.min(255) as u8), 4))
        }
        _ => None,
    }
}

/// Whether `text` carries a cursor-addressing or clearing sequence.
///
/// Only those sequences need a grid; SGR colouring is still a line-buffer concern, so a shell's
/// coloured output does not silently switch the control into screen mode.
fn contains_cursor_control(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' || chars.peek() != Some(&'[') {
            continue;
        }
        chars.next();
        for next in chars.by_ref() {
            if next.is_ascii_alphabetic() {
                if matches!(next, 'H' | 'f' | 'A' | 'B' | 'C' | 'D' | 'J' | 'K' | 'G' | 'd') {
                    return true;
                }
                break;
            }
        }
    }
    false
}

/// Parsed and wrapped output rows, keyed by the source lines they came from.
///
/// # Why the buffer is parsed once per change and not per frame
///
/// The paint path used to call [`ansi_runs`] for every visible line on every frame, allocating a
/// `String` per run, and measured every run's text again. Neither answer can change while the
/// line, its layout and its palette stay the same, so both are cached.
///
/// # Why an append is not a rebuild
///
/// A line's rows depend only on that line, so a change re-parses the lines from the first one
/// that differs instead of the whole buffer. A terminal that prints a line per second no longer
/// re-parses its whole scrollback on every line.
#[derive(Default)]
struct OutputLineCache {
    /// One entry per painted row; a soft-wrapped logical line contributes several.
    rows: Vec<Vec<AnsiRun>>,
    /// The logical output line each painted row came from, for hit-testing and selection.
    origins: Vec<usize>,
    /// Each logical line with its escape sequences removed, so character hit-testing does not
    /// re-strip the line on every pointer move.
    plain: Vec<String>,
    /// The source text of each logical line, so an unchanged line is not parsed again.
    ///
    /// A revision bump says *something* changed; comparing the text says which line it was.
    sources: Vec<String>,
    /// `row_starts[i]` is the index in [`Self::rows`] of logical line `i`'s first painted row.
    /// The final entry is the length of [`Self::rows`] itself.
    row_starts: Vec<usize>,
    /// The buffer revision the rows were parsed from.
    revision: u64,
    /// The layout key (wrap width, wrapping on/off, palette), or `None` after an invalidation.
    key: Option<(u32, bool, u64)>,
    /// How many logical lines have been parsed in total. Read by the tests that pin the
    /// incremental path and compiled away otherwise.
    #[cfg(test)]
    parsed_lines: usize,
}

impl OutputLineCache {
    /// Marks the parsed rows stale while keeping them until the next paint rebuilds them.
    fn invalidate(&mut self) {
        self.key = None;
        // The revision is invalidated too: the row count depends on the wrap width and the
        // palette, and until the next paint the honest fallback is the logical line count rather
        // than a count measured under the old settings.
        self.revision = u64::MAX;
    }
}

/// Measured glyph advances, reused by the paint and hit-test paths.
///
/// A terminal draws its input line one glyph at a time so the caret, the selection band and the
/// composition can sit on real glyph boundaries; measuring each glyph per frame was a shaping
/// pass per character per frame. Advances depend only on the face, so they are kept per size.
#[derive(Default)]
struct CharWidthCache {
    font_size: f32,
    entries: Vec<(char, i32)>,
}

impl CharWidthCache {
    /// The advance of `ch`, measured once per font size.
    fn width(&mut self, ch: char, font: &Font, context: &mut RenderContext) -> i32 {
        if (self.font_size - font.size()).abs() > f32::EPSILON {
            self.font_size = font.size();
            self.entries.clear();
        }
        if let Some((_, width)) = self.entries.iter().find(|(candidate, _)| *candidate == ch) {
            return *width;
        }
        let mut buffer = [0u8; 4];
        let glyph = ch.encode_utf8(&mut buffer);
        let width = context.measure_text(glyph, font).width as i32;
        self.entries.push((ch, width));
        width
    }
}

/// Parses and wraps the output buffer from the first line that changed.
#[allow(clippy::too_many_arguments)]
fn ensure_output_rows(
    cache: &mut OutputLineCache,
    lines: &[String],
    char_widths: &mut CharWidthCache,
    revision: u64,
    wrap: bool,
    context: &mut RenderContext,
    font: &Font,
    width: i32,
    palette: &[Color; 16],
) {
    let key = (width.max(0) as u32, wrap, palette_key(palette));
    let layout_changed = cache.key != Some(key);
    if !layout_changed && cache.revision == revision {
        return;
    }
    // A layout change invalidates every row; otherwise everything above the first differing
    // line is kept and only the tail is parsed.
    let split = if layout_changed {
        cache.rows.clear();
        cache.origins.clear();
        cache.plain.clear();
        cache.sources.clear();
        cache.row_starts.clear();
        cache.row_starts.push(0);
        0
    } else {
        let mut common = cache
            .sources
            .iter()
            .zip(lines.iter())
            .position(|(cached, live)| cached != live)
            .unwrap_or_else(|| cache.sources.len().min(lines.len()));
        // A scrollback trim drops lines from the front, shifting every survivor down: the first
        // live line is then not the cached first line, and without this the whole scrollback would
        // be re-parsed for each new line once the depth limit was reached. The overlap is verified
        // in full, so a repeated line cannot fake the shift and a mismatch falls back to a rebuild.
        if common == 0 {
            let shift = lines
                .first()
                .and_then(|first| cache.sources.iter().position(|cached| cached == first))
                .filter(|shift| {
                    *shift > 0
                        && cache
                            .sources
                            .get(*shift..)
                            .is_some_and(|tail| tail.iter().zip(lines.iter()).all(|(a, b)| a == b))
                });
            if let Some(shift) = shift {
                let row_start = cache.row_starts.get(shift).copied().unwrap_or(cache.rows.len());
                drop(cache.rows.drain(0..row_start));
                drop(cache.origins.drain(0..row_start));
                for origin in cache.origins.iter_mut() {
                    *origin = origin.saturating_sub(shift);
                }
                drop(cache.plain.drain(0..shift));
                drop(cache.sources.drain(0..shift));
                drop(cache.row_starts.drain(0..shift));
                for start in cache.row_starts.iter_mut() {
                    *start = start.saturating_sub(row_start);
                }
                common = cache.sources.len().min(lines.len());
            }
        }
        let row_start = cache.row_starts.get(common).copied().unwrap_or(cache.rows.len());
        cache.rows.truncate(row_start);
        cache.origins.truncate(row_start);
        cache.plain.truncate(common);
        cache.sources.truncate(common);
        cache.row_starts.truncate(common + 1);
        common
    };
    for (index, line) in lines.iter().enumerate().skip(split) {
        let (rows, plain) =
            parse_output_line(line, char_widths, context, font, width, wrap, palette);
        let painted = rows.len();
        cache.rows.extend(rows);
        cache.origins.extend(std::iter::repeat_n(index, painted));
        cache.plain.push(plain);
        cache.sources.push(line.clone());
        cache.row_starts.push(cache.rows.len());
        #[cfg(test)]
        {
            cache.parsed_lines += 1;
        }
    }
    cache.revision = revision;
    cache.key = Some(key);
}

/// Parses one logical output line and splits it into painted rows no wider than `width`.
///
/// Returns the rows and the line with its escape sequences removed. Wrapping is per *glyph*, so a
/// CJK character is never cut in half: the old path stopped the run when it reached the right
/// edge, which dropped the rest of the line and could stop in the middle of a glyph's advance.
fn parse_output_line(
    line: &str,
    char_widths: &mut CharWidthCache,
    context: &mut RenderContext,
    font: &Font,
    width: i32,
    wrap: bool,
    palette: &[Color; 16],
) -> (Vec<Vec<AnsiRun>>, String) {
    let plain = strip_ansi(line);
    let mut runs = ansi_runs(line, palette);
    // Widths are resolved here, once per change, rather than for every run on every frame.
    for run in &mut runs {
        run.width = context.measure_text(&run.text, font).width as i32;
    }
    if !wrap || width <= 0 {
        return (vec![runs], plain);
    }
    let mut rows: Vec<Vec<AnsiRun>> = Vec::new();
    let mut row: Vec<AnsiRun> = Vec::new();
    let mut x = 0i32;
    for run in runs {
        let mut chunk = String::new();
        for ch in run.text.chars() {
            let advance = char_widths.width(ch, font, context);
            if x > 0 && x + advance > width {
                if !chunk.is_empty() {
                    row.push(measured_run(core::mem::take(&mut chunk), &run, font, context));
                }
                rows.push(core::mem::take(&mut row));
                x = 0;
            }
            chunk.push(ch);
            x += advance;
        }
        if !chunk.is_empty() {
            row.push(measured_run(chunk, &run, font, context));
        }
    }
    rows.push(row);
    (rows, plain)
}

/// Builds one run of `text` carrying `source`'s style, measuring its advance once.
fn measured_run(
    text: String,
    source: &AnsiRun,
    font: &Font,
    context: &mut RenderContext,
) -> AnsiRun {
    let width = context.measure_text(&text, font).width as i32;
    AnsiRun {
        text,
        color: source.color,
        bg: source.bg,
        dim: source.dim,
        bold: source.bold,
        italic: source.italic,
        underline: source.underline,
        reverse: source.reverse,
        width,
    }
}

/// Appends `line` with every escape sequence removed to `out`.
///
/// The write-back form is what lets callers strip into a buffer they reuse; `strip_ansi` builds
/// one only because its callers want an owned value.
fn strip_ansi_into(line: &str, out: &mut String) {
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        if chars.peek() != Some(&'[') {
            continue;
        }
        chars.next();
        // A CSI sequence ends at its final byte, which in the SGR subset a shell emits is an
        // ASCII letter. Anything else is consumed so its bytes cannot leak into copied text.
        for next in chars.by_ref() {
            if next.is_ascii_alphabetic() {
                break;
            }
        }
    }
}

/// The text of one output line with every escape sequence removed.
fn strip_ansi(line: &str) -> String {
    let mut text = String::with_capacity(line.len());
    strip_ansi_into(line, &mut text);
    text
}

/// The advance of one glyph, measured without allocating a `String`.
///
/// The hit-test paths used to build `ch.to_string()` for every character of the line, which is one
/// heap allocation per character per pointer move.
fn glyph_width(ch: char, font: &Font) -> i32 {
    let mut buffer = [0u8; 4];
    crate::widget::metrics::estimate_text_width(ch.encode_utf8(&mut buffer), font, 1.0) as i32
}

/// Parses a published `caret_style` token.
fn caret_style_from_token(token: &str) -> Option<CaretStyle> {
    match token {
        "line" | "bar" => Some(CaretStyle::Line),
        "block" => Some(CaretStyle::Block),
        "underline" | "under_line" => Some(CaretStyle::Underline),
        _ => None,
    }
}

/// The sixteen-colour ANSI palette for the given appearance.
fn default_ansi_palette(dark: bool) -> [Color; 16] {
    if dark {
        [
            Color::rgb(0, 0, 0),
            Color::rgb(205, 49, 49),
            Color::rgb(13, 188, 121),
            Color::rgb(229, 229, 16),
            Color::rgb(36, 114, 200),
            Color::rgb(188, 63, 188),
            Color::rgb(17, 168, 205),
            Color::rgb(229, 229, 229),
            Color::rgb(102, 102, 102),
            Color::rgb(241, 76, 76),
            Color::rgb(35, 209, 139),
            Color::rgb(245, 245, 67),
            Color::rgb(59, 142, 234),
            Color::rgb(214, 112, 214),
            Color::rgb(41, 184, 219),
            Color::rgb(255, 255, 255),
        ]
    } else {
        // The bright half of the dark palette is near-white, which is invisible on a light
        // terminal body; this half is the same hue ramp pushed the other way.
        [
            Color::rgb(0, 0, 0),
            Color::rgb(176, 0, 32),
            Color::rgb(0, 128, 0),
            Color::rgb(128, 128, 0),
            Color::rgb(0, 0, 190),
            Color::rgb(128, 0, 128),
            Color::rgb(0, 128, 128),
            Color::rgb(90, 90, 90),
            Color::rgb(60, 60, 60),
            Color::rgb(205, 49, 49),
            Color::rgb(13, 150, 100),
            Color::rgb(170, 140, 0),
            Color::rgb(36, 114, 200),
            Color::rgb(188, 63, 188),
            Color::rgb(17, 140, 175),
            Color::rgb(30, 30, 30),
        ]
    }
}

/// A change-detection key for a palette.
fn palette_key(palette: &[Color; 16]) -> u64 {
    let mut key = 0xcbf2_9ce4_8422_2325u64;
    for color in palette {
        for channel in [color.r, color.g, color.b, color.a] {
            key ^= channel as u64;
            key = key.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    key
}

/// Splits one line into SGR-styled runs.
///
/// The parser understands the subset a shell and a progress program actually emit: reset,
/// bold/dim/italic/underline/reverse, the sixteen basic colours and their bright halves, 256-colour
/// and truecolour foregrounds and backgrounds. Any other control sequence is consumed and ignored,
/// so it cannot leak its escape bytes into the visible text.
fn ansi_runs(line: &str, palette: &[Color; 16]) -> Vec<AnsiRun> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut style = RunStyle::default();
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
                    runs.push(AnsiRun {
                        text: core::mem::take(&mut current),
                        color: style.color,
                        bg: style.background,
                        dim: style.dim,
                        bold: style.bold,
                        italic: style.italic,
                        underline: style.underline,
                        reverse: style.reverse,
                        width: 0,
                    });
                }
                if next == 'm' {
                    apply_sgr(&params, &mut style, palette);
                }
                break;
            }
            params.push(next);
        }
    }
    if !current.is_empty() {
        runs.push(AnsiRun {
            text: current,
            color: style.color,
            bg: style.background,
            dim: style.dim,
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            reverse: style.reverse,
            width: 0,
        });
    }
    runs
}

/// Resolves one parsed SGR colour against the sixteen-colour palette.
fn resolve_sgr_color(color: SgrColor, palette: &[Color; 16]) -> Color {
    match color {
        SgrColor::Indexed(index) => xterm_color(index as u32, palette),
        SgrColor::Rgb(r, g, b) => Color::rgb(r, g, b),
    }
}

/// Applies one SGR parameter list to the current run's style.
fn apply_sgr(params: &str, style: &mut RunStyle, palette: &[Color; 16]) {
    let values: Vec<u32> = params.split(';').filter_map(|part| part.parse::<u32>().ok()).collect();
    if values.is_empty() {
        *style = RunStyle::default();
        return;
    }
    let mut index = 0usize;
    while index < values.len() {
        match values[index] {
            0 => *style = RunStyle::default(),
            // SGR 1 is bold. It used to share the dim arm's result (`*dim = false`), which
            // silently discarded every bold run — a bold prompt rendered as ordinary text.
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            // 4 underline and 7 reverse are the two attributes a build log and an error banner
            // rely on; they were parsed away and never painted.
            4 => style.underline = true,
            7 => style.reverse = true,
            // 22 is "normal intensity": it clears *both* bold and dim.
            22 => {
                style.dim = false;
                style.bold = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            27 => style.reverse = false,
            39 => style.color = None,
            49 => style.background = None,
            30..=37 => style.color = Some(basic_ansi_color((values[index] - 30) as usize, palette)),
            40..=47 => {
                style.background = Some(basic_ansi_color((values[index] - 40) as usize, palette));
            }
            // The bright range addresses the upper half of the palette; mapping it onto the
            // lower half made `ESC[91m` and `ESC[31m` the same colour.
            90..=97 => {
                style.color = Some(basic_ansi_color((values[index] - 90 + 8) as usize, palette))
            }
            100..=107 => {
                style.background =
                    Some(basic_ansi_color((values[index] - 100 + 8) as usize, palette));
            }
            38 => {
                if let Some((color, consumed)) = extended_sgr_color(&values, index) {
                    style.color = Some(resolve_sgr_color(color, palette));
                    index += consumed;
                }
            }
            48 => {
                if let Some((color, consumed)) = extended_sgr_color(&values, index) {
                    style.background = Some(resolve_sgr_color(color, palette));
                    index += consumed;
                }
            }
            _ => {}
        }
        index += 1;
    }
}

/// One entry of the sixteen-colour palette.
fn basic_ansi_color(index: usize, palette: &[Color; 16]) -> Color {
    palette[index % 16]
}

/// The 256-colour palette's entry `index`.
fn xterm_color(index: u32, palette: &[Color; 16]) -> Color {
    match index {
        0..=15 => palette[index as usize],
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
        terminal.set_scrollback_lines(200);
        for i in 0..220 {
            terminal.append_output(format!("line {}", i));
        }

        assert_eq!(terminal.lines().len(), 200);
        assert_eq!(terminal.lines().first().map(|s| s.as_str()), Some("line 20"));
    }

    /// The scrollback depth is configurable and defaults to a build-log-sized buffer.
    ///
    /// It used to be the literal `200`, so a CI log of any length kept its last 200 lines and
    /// threw the failing command away.
    #[test]
    fn the_scrollback_depth_is_configurable() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        assert_eq!(terminal.scrollback_lines(), DEFAULT_SCROLLBACK_LINES);
        for i in 0..220 {
            terminal.append_output(format!("line {}", i));
        }
        assert_eq!(terminal.lines().len(), 220, "the default depth keeps the whole burst");

        terminal.set_scrollback_lines(50);
        assert_eq!(terminal.lines().len(), 50, "lowering the depth trims immediately");
        assert_eq!(terminal.lines().last().map(|s| s.as_str()), Some("line 219"));
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

    /// Page Up/Down walk the scrollback and `scroll_to_end` returns to the tail.
    #[test]
    fn page_keys_page_the_scrollback() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 400, 140));
        for n in 0..60 {
            terminal.append_output(format!("line{n}"));
        }
        let page = terminal.visible_output_rows.max(1);
        terminal.handle_event(&Event::key_press(33, 0));
        assert_eq!(terminal.scroll_offset, page, "PageUp reveals one page of older output");
        terminal.handle_event(&Event::key_press(34, 0));
        assert_eq!(terminal.scroll_offset, 0, "PageDown returns toward the tail");

        assert!(terminal.scroll_page(2));
        assert!(terminal.scroll_offset > 0);
        terminal.scroll_to_end();
        assert_eq!(terminal.scroll_offset, 0);
    }

    /// SGR bold is kept rather than silently dropped, and the bright range is a distinct half of
    /// the palette.
    #[test]
    fn bold_and_bright_sgr_survive_parsing() {
        let palette = default_ansi_palette(true);
        let bold = ansi_runs("\u{1b}[1mhi", &palette);
        assert!(bold.first().is_some_and(|run| run.bold), "SGR 1 must mark the run bold");

        let reset = ansi_runs("\u{1b}[1ma\u{1b}[22mb", &palette);
        assert!(reset[0].bold);
        assert!(!reset[1].bold, "SGR 22 clears bold along with dim");

        let bright = ansi_runs("\u{1b}[91mX", &palette);
        assert_eq!(bright[0].color, Some(palette[9]), "SGR 91 addresses the bright half");
        let normal = ansi_runs("\u{1b}[31mX", &palette);
        assert_eq!(normal[0].color, Some(palette[1]));
    }

    /// The prompt is configurable and the echoed command uses it.
    #[test]
    fn the_prompt_is_configurable() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        assert_eq!(terminal.prompt(), DEFAULT_PROMPT);
        terminal.set_prompt("$ ");
        terminal.set_input_line("ls");
        assert!(terminal.submit());
        assert_eq!(terminal.lines().last().map(|s| s.as_str()), Some("$ ls"));
    }

    /// Dragging over the output selects the rows it covers, and the copied text has no escapes.
    #[test]
    fn dragging_selects_output_rows() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.append_output("\u{1b}[31mred\u{1b}[0m first");
        terminal.append_output("second");
        terminal.append_output("third");

        // Row 0 starts at y = rect.y + 16 and each output row is `OUTPUT_LINE_HEIGHT` tall.
        terminal.handle_event(&Event::mouse_press(40, 16, 1));
        terminal.handle_event(&Event::mouse_move(40, 16 + OUTPUT_LINE_HEIGHT));
        assert_eq!(terminal.output_selection(), Some((0, 1)));
        assert_eq!(terminal.selected_output().as_deref(), Some("red first\nsecond"));
        assert!(!terminal.selected_output().unwrap().contains('\u{1b}'));

        terminal.handle_event(&Event::mouse_release(40, 16 + OUTPUT_LINE_HEIGHT, 1));
        assert!(terminal.copy_output());
    }

    /// A double-click selects one row, and clearing reports the presence change.
    #[test]
    fn double_click_selects_one_output_row() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.append_output("only");
        let seen = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = seen.clone();
        terminal.selection_changed.connect(move |present| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*present);
            }
        });

        terminal.handle_event(&Event::mouse_double_click(40, 16, 1));
        assert_eq!(terminal.output_selection(), Some((0, 0)));
        assert_eq!(terminal.selected_output().as_deref(), Some("only"));
        terminal.clear_output_selection();
        assert_eq!(terminal.output_selection(), None);

        let seen = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(seen, vec![true, false], "the signal reports appearance and disappearance");
    }

    /// The caret shape and blink are configurable, and a steady caret owes no frames.
    #[test]
    fn caret_style_and_blink_are_configurable() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 400, 200));
        assert_eq!(terminal.caret_style(), CaretStyle::Line);
        terminal.set_caret_style(CaretStyle::Block);
        assert_eq!(terminal.caret_style(), CaretStyle::Block);
        terminal.set_caret_style(CaretStyle::Underline);
        assert_eq!(terminal.caret_style(), CaretStyle::Underline);

        assert!(terminal.caret_blink_enabled());
        terminal.set_caret_blink_enabled(false);
        assert!(!terminal.caret_blink_enabled());
        assert!(!terminal.is_animating(), "a steady caret owes no frames");
        assert!(!terminal.tick(500));
        assert!(terminal.is_caret_visible(), "a steady caret stays visible");
    }

    /// The composition's candidate-window anchor follows the caret.
    #[test]
    fn the_ime_anchor_follows_the_caret() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("ab");
        terminal.handle_event(&Event::ime_preedit("你", 0));
        let at_end = terminal.preedit_screen_point().expect("a composition has an anchor");

        terminal.handle_event(&Event::key_press(37, 0));
        let mid_line = terminal.preedit_screen_point().expect("still composing");
        assert!(mid_line.x < at_end.x, "the anchor moved back with the caret");

        terminal.handle_event(&Event::ime_commit("你"));
        assert!(terminal.preedit_screen_point().is_none(), "a commit ends the composition");
    }

    /// A resize re-derives the grid and reports it only when it actually changed.
    #[test]
    fn a_resize_reports_the_new_grid() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        let grids = Arc::new(Mutex::new(Vec::<(u32, u32)>::new()));
        let sink = grids.clone();
        terminal.resized.connect(move |grid| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*grid);
            }
        });

        terminal.handle_event(&Event::resize(800, 600));
        let first = grids.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(first, vec![terminal.grid()], "the first resize reports the live grid");

        terminal.handle_event(&Event::resize(800, 600));
        let repeated = grids.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(repeated.len(), 1, "an unchanged grid is not reported twice");

        let mut declared = TerminalView::new(Rect::new(0, 0, 800, 600));
        declared.set_grid(Some((100, 30)));
        assert_eq!(declared.grid(), (100, 30));
    }

    /// Every appended line is published, so a host can forward or audit it.
    #[test]
    fn output_received_reports_each_line() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = lines.clone();
        terminal.output_received.connect(move |line| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(line.as_ref().clone());
            }
        });

        terminal.append_output("one");
        terminal.append_output_batch(["two", "three"]);
        let lines = lines.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(lines, vec!["one".to_string(), "two".to_string(), "three".to_string()]);
    }

    /// Soft wrapping splits an over-wide row into several painted rows.
    #[test]
    fn wrap_output_splits_an_over_wide_row() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 120, 200));
        terminal.append_output(
            "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ more text",
        );
        crate::widget::svg::render_to_svg(&mut terminal);
        assert!(
            terminal.output_cache.rows.len() > 1,
            "a 120 px row cannot hold that line, so it must wrap"
        );
        terminal.set_wrap_output(false);
        crate::widget::svg::render_to_svg(&mut terminal);
        assert_eq!(terminal.output_cache.rows.len(), 1, "without wrapping it stays one row");
    }
}

#[cfg(test)]
mod issue_2_8_tests {
    use super::*;

    /// SP-09: a cursor-addressing program writes into a grid instead of appending nonsense.
    #[test]
    fn cursor_addressing_and_clearing_drive_a_grid() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 320, 200));
        terminal.append_output("\u{1b}[2J\u{1b}[HHello");
        assert_eq!(terminal.lines()[0], "Hello");
        terminal.append_output("\u{1b}[2;3Hxy");
        assert_eq!(terminal.lines()[1], "  xy");
        // Clearing from the cursor removes the rest of that row, so the next write lands alone.
        terminal.append_output("\u{1b}[2;2H\u{1b}[K\u{1b}[2;2HZ");
        assert_eq!(terminal.lines()[1], " Z");
    }

    /// SP-10: underline and reverse survive the parser, so the painter can show them.
    #[test]
    fn sgr_underline_and_reverse_survive_parsing() {
        let palette = default_ansi_palette(true);
        let runs = ansi_runs("\u{1b}[4mu\u{1b}[0m\u{1b}[7mr\u{1b}[24;27mx", &palette);
        assert!(runs.iter().any(|run| run.underline));
        assert!(runs.iter().any(|run| run.reverse));
        assert!(runs.iter().all(|run| run.width >= 0));
        assert!(runs.last().is_some_and(|run| !run.underline && !run.reverse));
    }

    /// SP-P16: the stripped line is cached for character hit-testing.
    #[test]
    fn character_hit_testing_uses_the_cached_plain_line() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 320, 200));
        terminal.append_output("\u{1b}[31mabcdef\u{1b}[0m");
        // Two characters into the row is index 2; the escape run must not count.
        assert_eq!(terminal.output_char_at(0, 2), 0);
        assert!(terminal.output_char_at(0, 200) >= 6);
    }
}

#[cfg(test)]
mod issue_20261026_tests {
    use super::*;

    /// SP-009: the line parser understands truecolour, backgrounds and the italic bit.
    #[test]
    fn sgr_truecolour_background_and_italic_survive_parsing() {
        let palette = default_ansi_palette(true);
        let runs = ansi_runs("\u{1b}[3;38;2;1;2;3;48;5;200mX\u{1b}[0mY", &palette);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].color, Some(Color::rgb(1, 2, 3)));
        assert_eq!(runs[0].bg, Some(xterm_color(200, &palette)));
        assert!(runs[0].italic);
        assert_eq!(runs[1].color, None, "the reset clears the foreground");
        assert_eq!(runs[1].bg, None, "the reset clears the background");
        assert!(!runs[1].italic);
    }

    /// SP-009: a cursor-addressed grid keeps the whole colour range instead of clamping it.
    #[test]
    fn the_screen_grid_keeps_the_full_sgr_colour_range() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 320, 200));
        terminal.append_output("\u{1b}[2J\u{1b}[38;5;200m\u{1b}[48;2;10;20;30m\u{1b}[3mA");
        let palette = default_ansi_palette(true);
        let runs = ansi_runs(&terminal.lines()[0], &palette);
        assert_eq!(
            runs[0].color,
            Some(xterm_color(200, &palette)),
            "an entry past the first sixteen is not clamped to the palette"
        );
        assert_eq!(runs[0].bg, Some(Color::rgb(10, 20, 30)), "truecolour backgrounds survive");
        assert!(runs[0].italic, "the italic bit survives the grid");
    }

    /// SP-007: an append re-parses only the lines that changed.
    #[test]
    fn appending_output_parses_only_the_new_line() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.append_output("one");
        terminal.append_output("two");
        crate::widget::svg::render_to_svg(&mut terminal);
        let first = terminal.output_cache.parsed_lines;
        assert_eq!(first, 2, "both lines are parsed on the first frame");

        crate::widget::svg::render_to_svg(&mut terminal);
        assert_eq!(terminal.output_cache.parsed_lines, first, "an unchanged frame parses nothing");

        terminal.append_output("three");
        crate::widget::svg::render_to_svg(&mut terminal);
        assert_eq!(
            terminal.output_cache.parsed_lines,
            first + 1,
            "only the appended line is parsed again"
        );
        assert_eq!(terminal.output_cache.rows.len(), 3);
    }

    /// SP-008: a screen write re-materialises only the rows it touched.
    #[test]
    fn a_screen_write_materialises_only_the_row_it_changed() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 320, 200));
        terminal.append_output("\u{1b}[2J\u{1b}[1;1Hrow one");
        terminal.append_output("\u{1b}[2;1Hrow two");
        terminal.append_output("\u{1b}[3;1Hrow three");
        let before = terminal.screen.as_ref().map(|screen| screen.materialized_rows).unwrap_or(0);

        terminal.append_output("\u{1b}[2;2HX");
        let after = terminal.screen.as_ref().map(|screen| screen.materialized_rows).unwrap_or(0);
        assert_eq!(after - before, 1, "one row is rebuilt, not the whole grid");
        assert_eq!(terminal.lines().len(), 3);
        assert_eq!(terminal.lines()[1], "rXw two");
    }

    /// SP-007: a scrollback trim shifts the buffer instead of invalidating every row.
    #[test]
    fn a_scrollback_trim_keeps_the_surviving_rows_parsed() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_scrollback_lines(3);
        for line in ["one", "two", "three"] {
            terminal.append_output(line);
        }
        crate::widget::svg::render_to_svg(&mut terminal);
        let first = terminal.output_cache.parsed_lines;

        terminal.append_output("four");
        crate::widget::svg::render_to_svg(&mut terminal);
        assert_eq!(
            terminal.output_cache.parsed_lines,
            first + 1,
            "the surviving rows keep their entries"
        );
        assert_eq!(
            terminal.output_cache.plain,
            vec!["two".to_string(), "three".to_string(), "four".to_string()]
        );
    }

    /// SP-006: a blink frame in a damage-tracking mode damages only the caret's rectangle.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_blink_frame_damages_only_the_caret_rect() {
        use crate::widget::runtime;

        let mut terminal = TerminalView::new(Rect::new(0, 0, 320, 200));
        terminal.set_input_line("abc");
        crate::widget::svg::render_to_svg(&mut terminal);
        let caret = terminal.caret_rect.expect("the caret was painted");
        let id = runtime::register(crate::compat::Box::new(terminal)).expect("registry");
        assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Dirty));
        assert!(runtime::with_widget_mut(id, |widget| widget.tick(600)).unwrap_or(false));
        assert_eq!(runtime::dirty_rects(id), vec![caret], "only the caret box is repainted");
        runtime::unregister(id);
    }

    /// SP-006: the default repaint mode still takes the whole-control path.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_blink_frame_in_full_mode_keeps_the_whole_control_repaint() {
        use crate::widget::runtime;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let terminal = TerminalView::new(Rect::new(0, 0, 320, 200));
        let id = runtime::register(crate::compat::Box::new(terminal)).expect("registry");
        assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Full));
        let redraws = Arc::new(AtomicUsize::new(0));
        let counter = redraws.clone();
        let _ = runtime::with_widget(id, |widget| {
            widget.base().redraw_requested.connect(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            })
        });

        assert_eq!(runtime::with_widget_mut(id, |widget| widget.tick(600)), Some(true));
        assert_eq!(
            redraws.load(Ordering::Relaxed),
            1,
            "Full mode keeps the whole-control redraw request"
        );
        assert!(runtime::dirty_rects(id).is_empty(), "no damage rectangle is recorded");
        runtime::unregister(id);
    }
}

#[cfg(test)]
mod find_tests {
    use super::*;

    /// The scan reads the text the user sees, not the raw buffer: an SGR-coloured match is the
    /// same characters as an uncoloured one, and columns are character offsets.
    #[test]
    fn find_matches_scans_visible_text_and_reports_character_columns() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.append_output("\u{1b}[32mok\u{1b}[0m done");
        terminal.append_output("ok ok");

        assert_eq!(terminal.find_matches("ok", true, false), vec![(0, 0), (1, 0), (1, 3)]);
        assert_eq!(terminal.row_text(0), Some("\u{1b}[32mok\u{1b}[0m done"));
        assert_eq!(terminal.plain_row_text(0).as_deref(), Some("ok done"));
        assert_eq!(terminal.row_text(2), None);
        assert_eq!(terminal.plain_row_text(2), None);
    }

    /// A case-insensitive search folds ASCII case, exactly as the crate's web find does.
    #[test]
    fn find_matches_honours_case_sensitivity() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.append_output("Rust rust");

        assert_eq!(terminal.find_matches("Rust", true, false), vec![(0, 0)]);
        assert_eq!(terminal.find_matches("RUST", true, false), Vec::new());
        assert_eq!(terminal.find_matches("RUST", false, false), vec![(0, 0), (0, 5)]);
    }

    /// No regex engine is compiled in, so the flag must not change the result shape. The literal
    /// dot only matches a dot, which is what pins the documented literal behaviour.
    #[test]
    fn find_matches_treats_the_regex_flag_as_literal() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.append_output("abc a.c");

        assert_eq!(terminal.find_matches("a.c", true, true), vec![(0, 4)]);
        assert_eq!(terminal.find_matches("abc", true, true), vec![(0, 0)]);
    }

    /// An empty needle is not a match-everything wildcard.
    #[test]
    fn find_matches_ignores_an_empty_needle() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.append_output("anything");
        assert!(terminal.find_matches("", true, false).is_empty());
    }

    /// The scan covers the whole retained scrollback, not just the rows the viewport shows.
    #[test]
    fn find_matches_reaches_rows_outside_the_visible_window() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 60));
        for index in 0..80 {
            terminal.append_output(format!("line {index}"));
        }
        terminal.append_output("the failing command");

        assert_eq!(terminal.find_matches("failing", true, false), vec![(80, 4)]);
    }
}
