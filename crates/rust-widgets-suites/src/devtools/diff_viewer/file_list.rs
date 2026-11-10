// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The file selector beside a diff viewer: file status, rows and selection.

use rust_widgets::core::{Color, Font, HorizontalAlignment, Point, Rect};
use rust_widgets::event::wheel::WheelScroll;
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::coercion::expect_string;
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};
use std::fmt::Write as _;

/// How one file of a multi-file comparison changed.
///
/// This is the model-level status a file list displays; it is deliberately unrelated to
/// [`crate::devtools::diff_viewer::DiffKind`], which classifies a single *line*. A file can be `Modified` and every changed line
/// in it `Changed`, but the two answers are owned by different parts of the review view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileDiffStatus {
    /// The file exists only on the right-hand side.
    Added,
    /// The file exists only on the left-hand side.
    Removed,
    /// The file exists on both sides and differs.
    #[default]
    Modified,
    /// The file exists on both sides under different paths.
    Renamed,
}

impl FileDiffStatus {
    /// The token spelling published as part of the `files` property.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
            Self::Renamed => "renamed",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "added" | "a" | "+" => Some(Self::Added),
            "removed" | "deleted" | "d" | "-" => Some(Self::Removed),
            "modified" | "m" | "~" => Some(Self::Modified),
            "renamed" | "r" => Some(Self::Renamed),
            _ => None,
        }
    }

    /// The one-glyph marker drawn in a row's status cell.
    pub fn marker(self) -> &'static str {
        match self {
            Self::Added => "+",
            Self::Removed => "-",
            Self::Modified => "~",
            Self::Renamed => "R",
        }
    }
}

/// One file of a multi-file comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiffEntry {
    /// Repository-relative path of the file.
    pub path: String,
    /// How the file changed.
    pub status: FileDiffStatus,
    /// Added line count, as the host's own comparison reported it.
    pub additions: u32,
    /// Removed line count, as the host's own comparison reported it.
    pub deletions: u32,
}

impl FileDiffEntry {
    /// Creates an entry with the counts at zero.
    pub fn new(path: impl Into<String>, status: FileDiffStatus) -> Self {
        Self { path: path.into(), status, additions: 0, deletions: 0 }
    }

    /// Sets the added/removed counts.
    pub fn with_counts(mut self, additions: u32, deletions: u32) -> Self {
        self.additions = additions;
        self.deletions = deletions;
        self
    }

    /// The entry's published property spelling: `path|status|additions|deletions`.
    pub fn to_token(&self) -> String {
        format!("{}|{}|{}|{}", self.path, self.status.as_str(), self.additions, self.deletions)
    }

    /// Parses [`Self::to_token`]; an unknown status is a `TypeMismatch` for the caller.
    ///
    /// The fixed fields are read from the right: a path may legally contain `|` (it is a legal
    /// Unix file-name character), so anchoring the path at the front parsed `a|b.rs` as a status
    /// and made the property write fail for a path the control itself published.
    pub fn from_token(token: &str) -> Option<Self> {
        let mut parts = token.rsplitn(3, '|');
        let deletions = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
        let additions = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
        let (path, status) = parts.next()?.rsplit_once('|')?;
        let status = FileDiffStatus::from_token(status)?;
        Some(Self { path: path.to_string(), status, additions, deletions })
    }
}

/// Height of one file row and the gap above the first row.
const FILE_ROW_HEIGHT: i32 = 22;
const FILE_TOP: i32 = 6;

/// One file row's `+n -m` label and the width it was measured at.
///
/// The label is a pure function of the entry's two counts, so those form the key and a repaint
/// re-formats and re-measures only the rows whose counts moved. An empty label marks an entry
/// that has never been measured: a real label always carries both signs.
#[derive(Default)]
struct FileCountsCache {
    additions: u32,
    deletions: u32,
    text: String,
    width: i32,
}

/// A file selector that drives a multi-file [`DiffViewer`](crate::devtools::diff_viewer::DiffViewer).
///
/// `DiffViewer::set_texts` compares exactly one pair of snapshots, so a review of several files
/// has always needed a host-owned list beside it. This control is that list: it holds the file set
/// and its own selection, and reports which file the reader picked so the host can feed the
/// corresponding pair to a viewer.
///
/// It does **not** load or compare files — it has no source of file contents — so it never claims
/// a file was read. `file_selected` means "the selection moved to this path" and `file_activated`
/// means "the reader asked to open this path" (a double-click or Enter).
pub struct DiffFileList {
    base: BaseWidget,
    files: Vec<FileDiffEntry>,
    /// The selected rows, as indices into `files`.
    selected: Vec<usize>,
    /// The row an `Extended` Shift-extend grows from.
    ///
    /// Only meaningful in `SelectionMode::Extended`; every other mode leaves it `None`.
    selection_anchor: Option<usize>,
    /// Whether a pointer drag that began on a row is still growing an `Extended` selection.
    drag_selecting: bool,
    /// How many rows a press may add to the selection.
    selection_mode: rust_widgets::widget::view_widgets::list_view::SelectionMode,
    /// The first row the viewport paints.
    first_visible: usize,
    /// One counts label and measured width per file row, index-aligned with `files`.
    counts_cache: Vec<FileCountsCache>,
    /// The font the cached widths were measured with, so a font or size change re-measures them.
    counts_font: Option<Font>,
    /// Emitted when the selection moves, with the newly selected path.
    pub file_selected: Signal1<String>,
    /// Emitted when a file is activated by Enter or a double-click, with its path.
    pub file_activated: Signal1<String>,
}

impl DiffFileList {
    /// Creates an empty file list.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DiffFileList, geometry, "DiffFileList"),
            files: Vec::new(),
            selected: Vec::new(),
            selection_anchor: None,
            drag_selecting: false,
            selection_mode: rust_widgets::widget::view_widgets::list_view::SelectionMode::Single,
            first_visible: 0,
            counts_cache: Vec::new(),
            counts_font: None,
            file_selected: Signal1::new(),
            file_activated: Signal1::new(),
        }
    }

    /// The files in display order.
    pub fn files(&self) -> &[FileDiffEntry] {
        &self.files
    }

    /// Replaces the file set.
    ///
    /// A selection that still names a row is kept (the reader's place in a refreshed list is
    /// worth preserving), and a selection that no longer exists is dropped. No signal fires for a
    /// programmatic replacement: nothing was *selected* by the reader, and a subscriber that
    /// treated this as a pick would open a file the reader never chose.
    pub fn set_files(&mut self, files: Vec<FileDiffEntry>) {
        // Remember the selection by path, not by index: a refreshed list may be reordered, and an
        // index kept across it would point at a different file.
        let selected_paths: Vec<String> = self
            .selected
            .iter()
            .filter_map(|index| self.files.get(*index))
            .map(|file| file.path.clone())
            .collect();
        self.files = files;
        self.selected = selected_paths
            .into_iter()
            .filter_map(|path| self.files.iter().position(|file| file.path == path))
            .collect();
        // A refreshed list has different indices, so an anchor kept across it would extend from
        // an unrelated file. Re-anchor on the surviving selection instead.
        self.selection_anchor = if self.selection_mode
            == rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended
        {
            self.selected.first().copied()
        } else {
            None
        };
        // Keep the counts cache index-aligned with the new list; entries are keyed by the counts
        // themselves, so an entry that survives a reorder is reused only when it still applies.
        self.counts_cache.truncate(self.files.len());
        self.counts_cache.resize_with(self.files.len(), FileCountsCache::default);
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How many rows a press may select.
    pub fn selection_mode(&self) -> rust_widgets::widget::view_widgets::list_view::SelectionMode {
        self.selection_mode
    }

    /// Sets how many rows a press may select.
    pub fn set_selection_mode(
        &mut self,
        mode: rust_widgets::widget::view_widgets::list_view::SelectionMode,
    ) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        if !matches!(mode, rust_widgets::widget::view_widgets::list_view::SelectionMode::Multi) {
            self.selected.truncate(1);
        }
        if mode == rust_widgets::widget::view_widgets::list_view::SelectionMode::None {
            // `None` is documented as "the view accepts no selection at all", so entering it must
            // drop the row that was selected: leaving it made `selection_mode == "none"`
            // contradict `selected_paths()` and kept the highlight painted.
            self.selected.clear();
        }
        // Leaving `Extended` must not leave an anchor behind for a later switch back to extend
        // from; entering it starts from whatever row survived the mode change.
        self.selection_anchor =
            if mode == rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended {
                self.selected.first().copied()
            } else {
                None
            };
        self.base.request_redraw();
    }

    /// The selected paths, in display order.
    pub fn selected_paths(&self) -> Vec<&str> {
        self.selected
            .iter()
            .filter_map(|index| self.files.get(*index))
            .map(|file| file.path.as_str())
            .collect()
    }

    /// The first selected path, which is the one a viewer is expected to show.
    pub fn selected_path(&self) -> Option<&str> {
        self.selected_paths().first().copied()
    }

    /// Selects `path`, reporting the move.
    ///
    /// Returns `false` — and emits nothing — when no file has that path, because a subscriber told
    /// about a file that is not in the list would ask the viewer for contents that are not there.
    pub fn select_file(&mut self, path: &str) -> bool {
        let Some(index) = self.files.iter().position(|file| file.path == path) else {
            return false;
        };
        // A `None` view selects nothing, so this call cannot have selected anything and must not
        // report that it did.
        if self.selection_mode == rust_widgets::widget::view_widgets::list_view::SelectionMode::None
        {
            return false;
        }
        // `select_index` toggles in `Multi` mode (a click on a selected row removes it), but this
        // method's contract is to *select*: calling it for an already-selected path must not
        // deselect the file the caller asked for, nor report it as newly selected.
        if self.selection_mode
            == rust_widgets::widget::view_widgets::list_view::SelectionMode::Multi
        {
            if !self.selected.contains(&index) {
                self.selected.push(index);
                self.selected.sort_unstable();
                self.reveal(index);
                self.file_selected.emit(path.to_string());
                self.base.request_redraw();
            }
            return true;
        }
        self.select_index(index);
        true
    }

    /// Selects the row at `index`, honouring the selection mode.
    pub fn select_index(&mut self, index: usize) {
        let Some(file) = self.files.get(index) else { return };
        let path = file.path.clone();
        // Whether the path is now selected, so a Multi-mode click that removes a row does not
        // report it as the newly selected file.
        let mut announce = true;
        let changed = match self.selection_mode {
            rust_widgets::widget::view_widgets::list_view::SelectionMode::None => {
                self.selection_anchor = None;
                false
            }
            rust_widgets::widget::view_widgets::list_view::SelectionMode::Multi => {
                self.selection_anchor = None;
                if let Some(position) = self.selected.iter().position(|row| *row == index) {
                    self.selected.remove(position);
                    announce = false;
                } else {
                    self.selected.push(index);
                    self.selected.sort_unstable();
                }
                true
            }
            rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended => {
                // A plain click starts a new extension from the clicked row.
                self.selection_anchor = Some(index);
                if self.selected == [index] {
                    false
                } else {
                    self.selected = vec![index];
                    true
                }
            }
            rust_widgets::widget::view_widgets::list_view::SelectionMode::Single => {
                self.selection_anchor = None;
                if self.selected == [index] {
                    false
                } else {
                    self.selected = vec![index];
                    true
                }
            }
        };
        if changed {
            self.reveal(index);
            if announce {
                self.file_selected.emit(path);
            }
            self.base.request_redraw();
        }
    }

    /// Extends an `Extended` selection from its anchor to `index`, reporting the head.
    ///
    /// The selected rows are the inclusive display-order range, so a backward Shift-click and a
    /// forward one describe the same set. Without this the published `extended` mode behaved
    /// exactly like `single`.
    fn extend_selection_to(&mut self, index: usize) {
        let Some(file) = self.files.get(index) else {
            return;
        };
        let path = file.path.clone();
        let Some(anchor) = self.selection_anchor.filter(|anchor| *anchor < self.files.len()) else {
            self.select_index(index);
            return;
        };
        let (start, end) = if anchor <= index { (anchor, index) } else { (index, anchor) };
        let next: Vec<usize> = (start..=end).collect();
        if self.selected == next {
            return;
        }
        self.selected = next;
        self.reveal(index);
        self.file_selected.emit(path);
        self.base.request_redraw();
    }

    /// The end of an `Extended` selection that a Shift-arrow moves.
    ///
    /// When the range grew downward the head is its last row; when it grew upward the head is its
    /// first; a one-row selection uses the anchor itself.
    fn selection_head(&self) -> Option<usize> {
        let anchor = self.selection_anchor?;
        let start = self.selected.first().copied()?;
        let end = self.selected.last().copied()?;
        Some(if end > anchor {
            end
        } else if start < anchor {
            start
        } else {
            anchor
        })
    }

    /// Whether a Shift modifier bit is held.
    fn shift_held(modifiers: u32) -> bool {
        use rust_widgets::shortcut::Modifiers;
        Modifiers::from_event_bits(modifiers).contains(Modifiers::SHIFT)
    }

    /// The row a drag points at, clamped to the painted rows.
    ///
    /// A drag keeps extending even when the pointer leaves the control horizontally or moves past
    /// the first/last painted row: the nearest painted row is the one the reader means. Unlike
    /// `row_at`, the horizontal position is irrelevant here.
    fn drag_row_at(&self, pos: Point) -> Option<usize> {
        if self.files.is_empty() {
            return None;
        }
        let rect = self.geometry();
        let offset = pos.y - rect.y - FILE_TOP;
        let row = if offset <= 0 { 0 } else { (offset / Self::row_height()) as usize };
        let last_painted = self.visible_rows().saturating_sub(1);
        let index = self.first_visible + row.min(last_painted);
        (index < self.files.len()).then_some(index)
    }

    /// Scrolls the list when a drag reaches past the top or bottom edge.
    ///
    /// Returns whether the viewport moved. [`Self::drag_row_at`] clamps to the painted rows, so
    /// without this the range could only grow as far as the rows already on screen; the arrow-key
    /// path already reveals as it extends, the pointer path had nothing.
    fn autoscroll_drag(&mut self, pos: Point) -> bool {
        let rect = self.geometry();
        let top = rect.y + FILE_TOP;
        let bottom = top + self.visible_rows() as i32 * Self::row_height();
        let delta = if pos.y < top {
            -1
        } else if pos.y >= bottom {
            1
        } else {
            0
        };
        delta != 0 && self.scroll_by(delta)
    }

    /// Activates the row at `index`, reporting it.
    pub fn activate_index(&mut self, index: usize) {
        if let Some(file) = self.files.get(index) {
            let path = file.path.clone();
            self.file_activated.emit(path);
        }
    }

    /// The height one row occupies in the painted list.
    fn row_height() -> i32 {
        FILE_ROW_HEIGHT
    }

    /// Rows the viewport can paint.
    fn visible_rows(&self) -> usize {
        ((self.geometry().height as i32 - FILE_TOP).max(0) / Self::row_height()).max(1) as usize
    }

    /// Keeps `first_visible` inside the scrollable range.
    fn clamp_scroll(&mut self) {
        let max_first = self.files.len().saturating_sub(self.visible_rows());
        self.first_visible = self.first_visible.min(max_first);
    }

    /// Scrolls the list so `index` is visible.
    fn reveal(&mut self, index: usize) {
        let rows = self.visible_rows();
        if index < self.first_visible {
            self.first_visible = index;
        } else if index >= self.first_visible + rows {
            self.first_visible = index + 1 - rows;
        }
        self.clamp_scroll();
    }

    /// The row index under `pos`, when the point is on a painted file row.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let offset = pos.y - rect.y - FILE_TOP;
        // The gap above the first row is part of the control but not of any row; truncating a
        // small negative offset towards zero would otherwise make it row 0.
        if offset < 0 {
            return None;
        }
        let row = (offset / Self::row_height()) as usize;
        // Only the rows the viewport paints are hit targets: a press below the last painted row
        // must not select a file the reader cannot see.
        if row >= self.visible_rows() {
            return None;
        }
        let index = self.first_visible + row;
        (index < self.files.len()).then_some(index)
    }

    /// The painted box of row `index`, for hit tests and tests of the layout.
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        // Only the rows the viewport paints have a box: `draw` stops at `visible_rows()` and
        // `row_at` rejects anything past it, so a row below the window has no painted box to
        // report.
        if index < self.first_visible
            || index >= self.files.len()
            || index >= self.first_visible + self.visible_rows()
        {
            return None;
        }
        let row = (index - self.first_visible) as i32;
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            rect.y + FILE_TOP + row * Self::row_height(),
            rect.width,
            Self::row_height() as u32,
        ))
    }

    /// Scrolls by `rows`, returning whether the position moved.
    pub fn scroll_by(&mut self, rows: i32) -> bool {
        let before = self.first_visible;
        let next = self.first_visible as i32 + rows;
        self.first_visible = next.max(0) as usize;
        self.clamp_scroll();
        let moved = self.first_visible != before;
        if moved {
            self.base.request_redraw();
        }
        moved
    }
}

impl Widget for DiffFileList {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("diff_file_list")
    }
    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(240, 280)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the two names this control emits; see `DiffViewer`'s own note for why the arms
    /// are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "file_selected" => {
                Some(EventSignalRef::mapped("file_selected", &self.file_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "file_activated" => {
                Some(EventSignalRef::mapped("file_activated", &self.file_activated, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for DiffFileList {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "files" => Ok(CapabilityValue::Tuple(
                self.files.iter().map(|file| CapabilityValue::String(file.to_token())).collect(),
            )),
            "file_count" => Ok(CapabilityValue::UInt(self.files.len() as u64)),
            "selected_path" => Ok(match self.selected_path() {
                Some(path) => CapabilityValue::String(path.to_string()),
                None => CapabilityValue::Null,
            }),
            "selection_mode" => Ok(CapabilityValue::String(
                match self.selection_mode {
                    rust_widgets::widget::view_widgets::list_view::SelectionMode::Single => {
                        "single"
                    }
                    rust_widgets::widget::view_widgets::list_view::SelectionMode::Multi => "multi",
                    rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended => {
                        "extended"
                    }
                    rust_widgets::widget::view_widgets::list_view::SelectionMode::None => "none",
                }
                .to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "files" => {
                let items = rust_widgets::widget::capability::coercion::expect_string_list(value)?;
                let mut files = Vec::with_capacity(items.len());
                for item in items {
                    let entry = FileDiffEntry::from_token(&item)
                        .ok_or(CapabilityAccessError::TypeMismatch)?;
                    files.push(entry);
                }
                self.set_files(files);
                Ok(())
            }
            "file_count" | "selected_path" => Err(CapabilityAccessError::ReadOnlyProperty),
            "selection_mode" => {
                let token = expect_string(value)?;
                let mode = match token.as_str() {
                    "single" => {
                        rust_widgets::widget::view_widgets::list_view::SelectionMode::Single
                    }
                    "multi" => rust_widgets::widget::view_widgets::list_view::SelectionMode::Multi,
                    "extended" => {
                        rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended
                    }
                    "none" => rust_widgets::widget::view_widgets::list_view::SelectionMode::None,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_selection_mode(mode);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "files",
            "file_count",
            "selected_path",
            "selection_mode",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for DiffFileList {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // An unfocused file list must not steal the arrow keys from the focused control.
        if matches!(event, Event::KeyPress { .. }) && self.base.focus_reason().is_none() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, modifiers, .. } => {
                if let Some(index) = self.row_at(*pos) {
                    let extended = self.selection_mode
                        == rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended;
                    if extended && Self::shift_held(*modifiers) {
                        self.extend_selection_to(index);
                    } else {
                        self.select_index(index);
                    }
                    // A press on a row starts a drag that grows the range as the pointer moves.
                    self.drag_selecting = extended;
                }
            }
            Event::MouseMove { pos } => {
                if self.drag_selecting
                    && self.selection_mode
                        == rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended
                {
                    // Edge auto-scroll runs before the row lookup, so the range grows against the
                    // viewport the scroll just produced.
                    self.autoscroll_drag(*pos);
                    if let Some(index) = self.drag_row_at(*pos) {
                        self.extend_selection_to(index);
                    }
                }
            }
            Event::MouseRelease { .. } => self.drag_selecting = false,
            Event::MouseLeave { .. } => {
                // A release outside the control is not delivered to it, so leaving is the last
                // signal that the drag ended.
                self.drag_selecting = false;
            }
            Event::FocusLost => self.drag_selecting = false,
            Event::MouseDoubleClick { pos, button: 1, .. } => {
                if let Some(index) = self.row_at(*pos) {
                    self.activate_index(index);
                }
            }
            Event::KeyPress { key, modifiers, .. } => {
                let extend = Self::shift_held(*modifiers)
                    && self.selection_mode
                        == rust_widgets::widget::view_widgets::list_view::SelectionMode::Extended;
                // A Shift-arrow with no anchor starts the extension at the first row instead of
                // jumping two rows (the arrow step would otherwise apply to a row just selected).
                if extend && matches!(*key, 38 | 40) && self.selection_head().is_none() {
                    if !self.files.is_empty() {
                        self.select_index(0);
                    }
                    return;
                }
                match *key {
                    38 => {
                        if extend {
                            if let Some(head) = self.selection_head() {
                                if head > 0 {
                                    self.extend_selection_to(head - 1);
                                }
                            }
                        } else {
                            let next =
                                self.selected.first().map_or(0, |index| index.saturating_sub(1));
                            if !self.files.is_empty() {
                                self.select_index(next);
                            }
                        }
                    }
                    40 => {
                        if extend {
                            if let Some(head) = self.selection_head() {
                                let last = self.files.len().saturating_sub(1);
                                if head < last {
                                    self.extend_selection_to(head + 1);
                                }
                            }
                        } else if !self.files.is_empty() {
                            let next = self
                                .selected
                                .first()
                                .map_or(0, |index| (index + 1).min(self.files.len() - 1));
                            self.select_index(next);
                        }
                    }
                    36 => {
                        if extend && !self.files.is_empty() {
                            if self.selection_head().is_none() {
                                self.select_index(0);
                            }
                            self.extend_selection_to(0);
                        } else if !self.files.is_empty() {
                            // A plain Home moves the selection to the first row. It used to do
                            // nothing at all unless Shift was held, which left the documented
                            // navigation keys half-implemented.
                            self.select_index(0);
                        }
                    }
                    35 => {
                        if extend && !self.files.is_empty() {
                            if self.selection_head().is_none() {
                                self.select_index(0);
                            }
                            self.extend_selection_to(self.files.len() - 1);
                        } else if !self.files.is_empty() {
                            self.select_index(self.files.len() - 1);
                        }
                    }
                    13 => {
                        if let Some(index) = self.selected.first().copied() {
                            self.activate_index(index);
                        }
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                self.scroll_by(-scroll.lines_y);
            }
            _ => {}
        }
    }
}

impl Draw for DiffFileList {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let theme = rust_widgets::style::resolved_theme_style_shared("diff_viewer");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| rust_widgets::core::Color::border_of(background));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let selection =
            rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Info)
                .map(|token| token.blend(&background, 0.75))
                .unwrap_or_else(|| background.blend(&text_color, 0.25));
        let font = effective_font(style);

        // A translucent accent wash so a selected row keeps its text readable.
        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        rust_widgets::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        let rows = self.visible_rows();
        // One counts buffer for every row: a cache hit builds no text at all, and a miss reuses
        // this allocation instead of one `String` per visible row per frame.
        let mut counts = String::new();
        // A width belongs to the font it was measured with, so a font or size change drops the
        // cached measurements rather than budgeting against stale advances.
        if self.counts_font.as_ref() != Some(font) {
            self.counts_cache.clear();
            self.counts_cache.resize_with(self.files.len(), FileCountsCache::default);
            self.counts_font = Some(font.clone());
        }
        // The character budget follows the rendered advance of one representative glyph instead
        // of a fixed divisor, so an elided path stays inside its column at any font size.
        let char_advance = context.measure_text("M", font).width.max(1) as i32;
        for offset in 0..rows {
            let index = self.first_visible + offset;
            let Some(file) = self.files.get(index) else {
                break;
            };
            let Some(row) = self.row_rect(index) else {
                break;
            };
            // The cache stays index-aligned with `files`, so a painted row always has an entry;
            // the guard mirrors the two lookups above rather than assuming the invariant.
            let Some(entry) = self.counts_cache.get_mut(index) else {
                break;
            };
            if entry.text.is_empty()
                || entry.additions != file.additions
                || entry.deletions != file.deletions
            {
                entry.additions = file.additions;
                entry.deletions = file.deletions;
                counts.clear();
                let _ = write!(counts, "+{} -{}", file.additions, file.deletions);
                entry.text.clear();
                entry.text.push_str(&counts);
                entry.width = context.measure_text(&entry.text, font).width as i32;
            }
            if self.selected.contains(&index) {
                context.fill_rect(row, selection);
            }
            let status_color = match file.status {
                FileDiffStatus::Added => {
                    rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Success)
                }
                FileDiffStatus::Removed => {
                    rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Error)
                }
                FileDiffStatus::Modified => {
                    rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Warning)
                }
                FileDiffStatus::Renamed => {
                    rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Info)
                }
            }
            .unwrap_or(text_color);
            context.draw_text(
                Point::new(row.x + 6, row.y + 5),
                file.status.marker(),
                font,
                status_color,
                HorizontalAlignment::Left,
            );
            let path_band = (row.width as i32 - 12 - entry.width - 8).max(24);
            let path = rust_widgets::widget::text_utils::elide_end(
                &file.path,
                (path_band / char_advance).max(1) as usize,
            );
            context.draw_text(
                Point::new(row.x + 22, row.y + 5),
                &path,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
            context.draw_text(
                Point::new(row.x + row.width as i32 - entry.width - 6, row.y + 5),
                &entry.text,
                font,
                text_color.blend(&background, 0.45),
                HorizontalAlignment::Left,
            );
            context.draw_line(
                Point::new(row.x, row.y + row.height as i32 - 1),
                Point::new(row.x + row.width as i32, row.y + row.height as i32 - 1),
                background.blend(&text_color, 0.10),
            );
        }
    }
}
