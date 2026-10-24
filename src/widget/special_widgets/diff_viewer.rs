// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DiffViewer widget.

/// The top of the first diff row below the column headings.
const ROW_TOP: i32 = 34;
/// The pitch between diff rows.
const ROW_HEIGHT: i32 = 16;
/// How much room below a row's top its height still needs.
const ROW_BOTTOM_INSET: i32 = 14;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Diff line state.
///
/// Classifies how the two sides relate for one compared line. Which variant a
/// line receives is decided by the diff algorithm, not by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// The line is present and identical on both sides.
    Equal,
    /// The line exists only on the right-hand side.
    Added,
    /// The line exists only on the left-hand side.
    Removed,
    /// The line exists on both sides but its content differs.
    Changed,
}

/// One compared line entry.
///
/// `left` and `right` are the two sides of the same row; they are `None` when
/// that side has no line at this position, which is what distinguishes
/// [`DiffKind::Added`] (left is `None`) from [`DiffKind::Removed`] (right is
/// `None`). Nothing enforces that invariant, so a hand-built entry can express
/// combinations the diff algorithm never produces, such as `Changed` with one
/// side `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Content shown in the left column, or `None` for a line that only exists
    /// on the right.
    pub left: Option<String>,
    /// Content shown in the right column, or `None` for a line that only exists
    /// on the left.
    pub right: Option<String>,
    /// How the two sides relate; see [`DiffKind`].
    pub kind: DiffKind,
}

/// Side-by-side diff viewer for text snapshots.
pub struct DiffViewer {
    base: BaseWidget,
    left: String,
    right: String,
    lines: Vec<DiffLine>,
    selected_index: Option<usize>,
    /// The first diff row the viewport paints.
    ///
    /// The paint path used to stop at a fixed twelve rows, so a longer comparison was not
    /// reachable. This is the scroll state the wheel drives.
    first_visible_line: usize,
    /// Emitted after compare. Payload is changed-lines count.
    pub compared: Signal1<usize>,
}

impl DiffViewer {
    /// Creates empty diff viewer.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "DiffViewer"),
            left: String::new(),
            right: String::new(),
            lines: Vec::new(),
            selected_index: None,
            first_visible_line: 0,
            compared: Signal1::new(),
        }
    }

    /// Sets both text snapshots and recomputes diff.
    pub fn set_texts(&mut self, left: impl Into<String>, right: impl Into<String>) {
        self.left = left.into();
        self.right = right.into();
        self.first_visible_line = 0;
        self.recompute();
    }

    /// Returns diff lines.
    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    /// Returns selected line index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Selects line index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.lines.len() {
            return false;
        }
        self.selected_index = Some(index);
        self.base.request_redraw();
        true
    }

    /// Returns count of non-equal lines.
    pub fn change_count(&self) -> usize {
        self.lines.iter().filter(|line| line.kind != DiffKind::Equal).count()
    }

    /// How many diff rows fit below the column headings.
    ///
    /// At least one, so a short viewer still paints a row instead of scrolling everything away.
    fn visible_line_count(&self) -> usize {
        let usable = self.geometry().height as i32 - ROW_TOP - ROW_BOTTOM_INSET;
        if usable < 0 {
            return 1;
        }
        (usable / ROW_HEIGHT + 1) as usize
    }

    /// The first row the viewport actually starts at, clamped to the last full window.
    fn visible_start(&self) -> usize {
        let max_first = self.lines.len().saturating_sub(self.visible_line_count());
        self.first_visible_line.min(max_first)
    }

    /// Scrolls the viewport by `lines` rows toward the end of the comparison.
    ///
    /// Positive values reveal later rows, negative values reveal earlier ones. Returns whether the
    /// viewport moved, so the caller repaints only on a change.
    fn scroll_lines(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let max_first = self.lines.len().saturating_sub(self.visible_line_count());
        let next =
            (self.first_visible_line as i64 + lines as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_line {
            return false;
        }
        self.first_visible_line = next;
        true
    }

    fn recompute(&mut self) {
        let left_lines: Vec<&str> = self.left.lines().collect();
        let right_lines: Vec<&str> = self.right.lines().collect();

        self.lines.clear();

        // # Size guard
        //
        // The Myers trace keeps one `V` snapshot per edit distance `D`, so its memory is
        // O((N + M) * D) and a pair of large, entirely different documents would spend it
        // without bound. When the line grid exceeds this budget the previous positional
        // comparison is kept: it misreports insertions as changes, but it is linear, never
        // allocates a trace, and still produces a usable row list. The full-clone of both
        // inputs below is a separate, pre-existing cost and is deliberately not addressed
        // here.
        if (left_lines.len() + 1).saturating_mul(right_lines.len() + 1) > 4_000_000 {
            let max_len = left_lines.len().max(right_lines.len());
            for idx in 0..max_len {
                let l = left_lines.get(idx).copied();
                let r = right_lines.get(idx).copied();
                let kind = match (l, r) {
                    (Some(a), Some(b)) if a == b => DiffKind::Equal,
                    (Some(_), Some(_)) => DiffKind::Changed,
                    (Some(_), None) => DiffKind::Removed,
                    (None, Some(_)) => DiffKind::Added,
                    (None, None) => DiffKind::Equal,
                };
                self.lines.push(DiffLine {
                    left: l.map(|s| s.to_string()),
                    right: r.map(|s| s.to_string()),
                    kind,
                });
            }
        } else {
            self.lines = build_diff_lines(&left_lines, &right_lines);
        }

        self.selected_index = if self.lines.is_empty() { None } else { Some(0) };
        self.compared.emit(self.change_count());
        self.base.request_layout();
        self.base.request_redraw();
    }
}

/// One primitive step of an edit script.
///
/// The indexes are line indexes into the left (`Equal`/`Delete`) and right
/// (`Equal`/`Insert`) snapshots, so the script is a lossless description of how
/// the left document turns into the right one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditOp {
    /// Left line `l` and right line `r` are the same line.
    Equal(usize, usize),
    /// Left line `l` has no counterpart on the right.
    Delete(usize),
    /// Right line `r` has no counterpart on the left.
    Insert(usize),
}

/// Computes a shortest edit script with Myers' O(ND) greedy algorithm.
///
/// `V[k]` tracks the furthest `x` reached on diagonal `k = x - y`; one snapshot of
/// `V` is saved per round `d`, which is what the linear backtrack needs to recover
/// the steps. The script is assembled backwards from the end and then reversed.
fn myers_diff(a: &[&str], b: &[&str]) -> Vec<EditOp> {
    let n = a.len();
    let m = b.len();
    let max = n + m;

    // `V` is indexed by `k` shifted by `offset`. The two spare slots cover the
    // `k - 1` / `k + 1` probes at the widest round and keep the empty-input case
    // (`max == 0`) in bounds.
    let offset = max + 1;
    let mut v = vec![0isize; 2 * max + 3];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut reached = false;

    for d in 0..=max {
        trace.push(v.clone());
        let d = d as isize;
        let mut k = -d;
        while k <= d {
            let down = (k + offset as isize + 1) as usize;
            let right = (k + offset as isize - 1) as usize;
            let mut x =
                if k == -d || (k != d && v[right] < v[down]) { v[down] } else { v[right] + 1 };
            let mut y = x - k;
            while x < n as isize && y < m as isize && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(k + offset as isize) as usize] = x;
            if x >= n as isize && y >= m as isize {
                reached = true;
            }
            k += 2;
        }
        if reached {
            break;
        }
    }

    let mut ops = Vec::new();
    let mut x = n as isize;
    let mut y = m as isize;
    let mut d = (trace.len() - 1) as isize;
    while d >= 0 {
        let v = &trace[d as usize];
        let k = x - y;
        let prev_k = if k == -d
            || (k != d
                && v[(k - 1 + offset as isize) as usize] < v[(k + 1 + offset as isize) as usize])
        {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[(prev_k + offset as isize) as usize];
        let prev_y = prev_x - prev_k;
        // Walk back along the snake (the equal lines the forward pass skipped).
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            ops.push(EditOp::Equal(x as usize, y as usize));
        }
        if d > 0 {
            if x == prev_x {
                y -= 1;
                ops.push(EditOp::Insert(y as usize));
            } else {
                x -= 1;
                ops.push(EditOp::Delete(x as usize));
            }
        }
        x = prev_x;
        y = prev_y;
        d -= 1;
    }
    ops.reverse();
    ops
}

/// Turns an edit script into display rows.
///
/// A maximal run of `Delete`/`Insert` steps between two `Equal` steps is one change
/// region. Its deletions and insertions are paired positionally into `Changed` rows
/// (the existing side-by-side meaning) and any leftovers become `Removed` / `Added`
/// rows, which is exactly the `left`/`right: Option<String>` shape of [`DiffLine`].
fn build_diff_lines(a: &[&str], b: &[&str]) -> Vec<DiffLine> {
    let ops = myers_diff(a, b);
    let mut lines = Vec::new();
    let mut idx = 0usize;
    while idx < ops.len() {
        match ops[idx] {
            EditOp::Equal(l, r) => {
                lines.push(DiffLine {
                    left: Some(a[l].to_string()),
                    right: Some(b[r].to_string()),
                    kind: DiffKind::Equal,
                });
                idx += 1;
            }
            EditOp::Delete(_) | EditOp::Insert(_) => {
                let mut deletes = Vec::new();
                let mut inserts = Vec::new();
                while idx < ops.len() {
                    match ops[idx] {
                        EditOp::Equal(_, _) => break,
                        EditOp::Delete(l) => deletes.push(l),
                        EditOp::Insert(r) => inserts.push(r),
                    }
                    idx += 1;
                }
                let paired = deletes.len().min(inserts.len());
                for (&l, &r) in deletes.iter().zip(inserts.iter()).take(paired) {
                    lines.push(DiffLine {
                        left: Some(a[l].to_string()),
                        right: Some(b[r].to_string()),
                        kind: DiffKind::Changed,
                    });
                }
                for &l in &deletes[paired..] {
                    lines.push(DiffLine {
                        left: Some(a[l].to_string()),
                        right: None,
                        kind: DiffKind::Removed,
                    });
                }
                for &r in &inserts[paired..] {
                    lines.push(DiffLine {
                        left: None,
                        right: Some(b[r].to_string()),
                        kind: DiffKind::Added,
                    });
                }
            }
        }
    }
    lines
}

impl Widget for DiffViewer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(500, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "compared" => Some(EventSignalRef::mapped("compared", &self.compared, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            _ => None,
        }
    }
}

/// `DiffViewer`'s property contract.
///
/// Both texts are writable, but each name must be written through
/// `set_texts` — which recomputes the diff and emits `compared` — rather than
/// stored directly, so a reader never sees stale `line_count` / `change_count`
/// after a text write.
impl WidgetProperties for DiffViewer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "left_text" => Ok(CapabilityValue::String(self.left.clone())),
            "right_text" => Ok(CapabilityValue::String(self.right.clone())),
            "line_count" => Ok(CapabilityValue::UInt(self.lines().len() as u64)),
            "change_count" => Ok(CapabilityValue::UInt(self.change_count() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "left_text" => match value {
                CapabilityValue::String(text) => {
                    let right = self.right.clone();
                    self.set_texts(text, right);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "right_text" => match value {
                CapabilityValue::String(text) => {
                    let left = self.left.clone();
                    self.set_texts(left, text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "line_count" | "change_count" | "selected_index" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "left_text",
            "right_text",
            "line_count",
            "change_count",
            "selected_index",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `diff_viewer` publishes.
    ///
    /// `select_index` names which diff line to select, and the control has no
    /// defensible default — line 0 of a two-sided diff is not "the interesting one",
    /// it is just the first — so a bare invocation is refused as
    /// [`CapabilityAccessError::OutOfRange`]: the name is valid and the index is what
    /// is missing. The property layer treats `selected_index` as read-only on purpose
    /// (see the note on `TimelineWidget` for the same rule), which is why the command
    /// is the only route and why it must not invent an argument. `set_texts` carries
    /// both snapshots.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "select_index" | "set_texts" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for DiffViewer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::KeyPress { key, modifiers: _ } = event {
            match *key {
                38 => {
                    if let Some(index) = self.selected_index {
                        if index > 0 {
                            let _ = self.select_index(index - 1);
                        }
                    }
                }
                40 => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.lines.len() {
                            let _ = self.select_index(index + 1);
                        }
                    }
                }
                _ => { /* Other keys are not relevant */ }
            }
        }
        // Wheel: positive `delta.y` moves the content down, so the viewport moves toward the
        // start. Three lines per notch is the desktop default shared with the text controls.
        if let Event::Wheel { delta, modifiers, .. } = event {
            let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
            if self.scroll_lines(-scroll.lines_y) {
                self.base.request_redraw();
            }
        }
    }
}

impl Draw for DiffViewer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes the appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("diff_viewer");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(background));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The split divider and the selection outline are secondary chrome, derived
        // from the resolved colours so they follow the appearance too.
        let divider = background.blend(&text_color, 0.15);
        // # Why the row rule reads `outline_variant` and not the blend above
        //
        // A row rule separates two *rows* of content; the focus ring is a different role. M6's
        // criterion is "the row line's colour is not the focus ring's", and the `outline_variant`
        // token is exactly the row-divider role the theme declares for it — the same one
        // `table_widget`, `virtual_table`, `data_table` and `tree_table` now read. A local blend
        // of the surface is a colour no theme can tune independently; the role is.
        let row_rule = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.outline_variant)
            .unwrap_or(divider);
        let selection = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .unwrap_or_else(|| background.blend(&text_color, 0.5));
        // Added / Removed / Changed are *states*, so they read the theme's semantic
        // tokens and are tinted over the resolved surface to stay legible in both
        // appearances.
        let added = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .map(|token| token.blend(&background, 0.85));
        let removed = crate::style::semantic_color(crate::style::SemanticColor::Error)
            .map(|token| token.blend(&background, 0.85));
        let changed = crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .map(|token| token.blend(&background, 0.85));

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        let mid_x = rect.x + (rect.width as i32 / 2);
        context.draw_line(
            Point::new(mid_x, rect.y),
            Point::new(mid_x, rect.y + rect.height as i32),
            divider,
        );

        // The LEFT/RIGHT column headings and the diff line bodies take the **effective font**
        // (D09-STYLE-01), so the theme body font and the user's text scale reach them.
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 16),
            "LEFT",
            effective_font(style),
            text_color,
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(mid_x + 8, rect.y + 16),
            "RIGHT",
            effective_font(style),
            text_color,
            HorizontalAlignment::Left,
        );

        for (row, line) in
            self.lines.iter().skip(self.visible_start()).take(self.visible_line_count()).enumerate()
        {
            let idx = self.visible_start() + row;
            let y = rect.y + ROW_TOP + (row as i32) * ROW_HEIGHT;
            // Rows below the control are not painted: a 120 px viewer used to draw its eleventh
            // row about 90 px below itself, over the neighbouring widget.
            if y + ROW_BOTTOM_INSET > rect.y + rect.height as i32 {
                break;
            }
            let bg = match line.kind {
                DiffKind::Equal => None,
                DiffKind::Added => added.or_else(|| Some(background.blend(&Color::GREEN, 0.12))),
                DiffKind::Removed => removed.or_else(|| Some(background.blend(&Color::RED, 0.12))),
                DiffKind::Changed => {
                    changed.or_else(|| Some(background.blend(&Color::YELLOW, 0.12)))
                }
            };
            if let Some(color) = bg {
                context.fill_rect(
                    Rect::new(rect.x + 2, y - 10, rect.width.saturating_sub(4), 14),
                    color,
                );
            }
            if self.selected_index == Some(idx) {
                context.draw_rect(
                    Rect::new(rect.x + 2, y - 11, rect.width.saturating_sub(4), 16),
                    selection,
                );
            }

            if let Some(text) = &line.left {
                context.draw_text(
                    Point::new(rect.x + 8, y),
                    text,
                    effective_font(style),
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
            if let Some(text) = &line.right {
                context.draw_text(
                    Point::new(mid_x + 8, y),
                    text,
                    effective_font(style),
                    text_color,
                    HorizontalAlignment::Left,
                );
            }

            // # The rule this control was missing
            //
            // No row separator was drawn at all: rows were distinguished only by their fill, so an
            // `Equal` row (which has none) ran straight into the next one and a reader counting
            // lines in a long hunk had nothing to count against. The rule is drawn **after** the
            // text so it sits under the glyphs rather than being overpainted by the next row's
            // fill, and it spans the same inset as the fill so the table reads as one column.
            context.draw_line(
                Point::new(rect.x + 2, y + 7),
                Point::new(rect.x + rect.width as i32 - 2, y + 7),
                row_rule,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn recompute_detects_changes() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nb\nc", "a\nB\nc\nd");

        assert_eq!(viewer.lines().len(), 4);
        assert_eq!(viewer.change_count(), 2);
        assert_eq!(viewer.lines()[1].kind, DiffKind::Changed);
        assert_eq!(viewer.lines()[3].kind, DiffKind::Added);
    }

    #[test]
    fn compared_signal_emits_change_count() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        let counts = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = counts.clone();
        viewer.compared.connect(move |count| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*count);
            }
        });

        viewer.set_texts("x", "y");

        let got = counts.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![1]);
    }

    #[test]
    fn arrow_keys_move_selected_line() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nb", "a\nb\nc");
        assert_eq!(viewer.selected_index(), Some(0));

        viewer.handle_event(&Event::key_press(40, 0));
        assert_eq!(viewer.selected_index(), Some(1));

        viewer.handle_event(&Event::key_press(40, 0));
        assert_eq!(viewer.selected_index(), Some(2));

        viewer.handle_event(&Event::key_press(38, 0));
        assert_eq!(viewer.selected_index(), Some(1));
    }

    #[test]
    fn leading_insertion_leaves_the_rest_equal() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("A\nB\nC", "X\nA\nB\nC");

        let lines = viewer.lines();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].kind, DiffKind::Added);
        assert_eq!(lines[0].left, None);
        assert_eq!(lines[0].right.as_deref(), Some("X"));
        assert!(lines[1..].iter().all(|line| line.kind == DiffKind::Equal));
        assert_eq!(lines[1].left.as_deref(), Some("A"));
        assert_eq!(viewer.change_count(), 1);
    }

    #[test]
    fn identical_inputs_are_all_equal() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("alpha\nbeta\ngamma", "alpha\nbeta\ngamma");

        assert_eq!(viewer.lines().len(), 3);
        assert!(viewer.lines().iter().all(|line| line.kind == DiffKind::Equal));
        assert_eq!(viewer.change_count(), 0);
    }

    #[test]
    fn change_count_matches_non_equal_rows() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("one\ntwo\nthree\nfour", "one\n2\nthree\nfour\nfive");

        let non_equal = viewer.lines().iter().filter(|line| line.kind != DiffKind::Equal).count();
        assert_eq!(viewer.change_count(), non_equal);
        assert!(non_equal > 0);
    }

    /// The wheel scrolls the visible diff window and clamps at both ends.
    ///
    /// # The defect this pins
    ///
    /// The paint path stopped at a fixed twelve rows, so a longer comparison could not be
    /// reached. This drives the window the paint path now starts from.
    #[test]
    fn diff_viewer_wheel_scrolls_and_clamps() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let joined = text.join("\n");
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 100));
        viewer.set_texts(joined.clone(), joined);
        assert_eq!(viewer.lines().len(), 20);
        assert_eq!(viewer.first_visible_line, 0);

        // One notch toward the end reveals three rows.
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(viewer.first_visible_line, 3, "a downward notch moves the viewport down");

        // The other direction returns to the top and clamps there.
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(viewer.first_visible_line, 0);
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(viewer.first_visible_line, 0, "the viewport clamps at the first row");

        // Scrolling on stops at the last full window.
        let max_first = viewer.lines().len() - viewer.visible_line_count();
        for _ in 0..20 {
            viewer.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        }
        assert_eq!(viewer.first_visible_line, max_first, "the viewport clamps at the last window");
    }
}
