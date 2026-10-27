// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cascader — a multi-level chooser that walks one path down a tree of options.
//!
//! # Why this is a control of its own
//!
//! `Dropdown`'s data model is a flat list of strings plus one index
//! (`dropdown.rs:34-39`: `items: Vec<String>`, `selected_index: usize`). A
//! cascader's value is a **path** — a sequence of indices, one per level — and the
//! thing being chosen is a leaf reached by walking that path. The two are not the
//! same shape:
//!
//! | | `Dropdown` | `ComboBox` / `EditableComboBox` | `Cascader` |
//! |---|---|---|---|
//! | holds | `Vec<String>` + one index | `Vec<String>` + one index | **a tree + a path** |
//! | selection | `usize` | `usize` | **`Vec<usize>`, one per level** |
//! | display | the selected string | the selected string | **the path joined by `/`** |
//! | opening a level | — | — | **shows the next level beside it** |
//!
//! Forcing this into `Dropdown` would mean giving one control two incompatible
//! meanings for `items` and `selected_index`, which is rule #78's test for "this is
//! a new control, not an extension".
//!
//! # What it shares rather than reimplements
//!
//! The expand/collapse state, the click-to-open interaction and the "selected item
//! is highlighted" drawing follow `Dropdown`'s conventions, so a caller switching
//! between the two does not have to relearn them.

//! # Reachability
//!
//! Registered in the widget factory as `cascader` (aliases `cascade`,
//! `multi_level_select`), so it is reachable by name from the declarative JSON path
//! (`"cascader"`), from a CSS selector (`Cascader`), and through the typed
//! `create_cascader` on `ControlBackend`.

use crate::compat::{Instant, RefCell, String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The height of one row in an open level.
const ROW_HEIGHT: u32 = 26;

/// The width of one popped-out level.
const LEVEL_WIDTH: u32 = 160;

/// How far each level is offset horizontally from the one before it.
const LEVEL_INDENT: i32 = LEVEL_WIDTH as i32;

// ── Column-style overlay drawing ─────────────────────────────────────────────
//
// The open state is drawn as columns rather than as one long list because that is
// what makes a cascade readable: level 2 appears *beside* level 1, so the user can
// see which parent the visible children belong to. A single indented list loses
// that relationship the moment a level is long enough to scroll.

impl Cascader {
    /// The deepest level currently drawn (0-based).
    ///
    /// The semantics are "how many columns are on screen", which is **one more**
    /// than the number of nodes the browsed path has walked: choosing at level 0
    /// adds a node to the path *and* reveals level 1. A path of length `n` therefore
    /// draws levels `0..=n`.
    ///
    /// An earlier version returned `expanded_path.len()`, which drew one column too
    /// few and made a freshly-opened overlay show no rows at all — the bug that
    /// `cascader_arrow_keys_move_within_the_visible_rows` and its siblings caught.
    fn open_depth(&self) -> usize {
        self.expanded_path.len()
    }

    /// The number of columns currently drawn, which is `open_depth() + 1`.
    ///
    /// Named separately from `open_depth` so the two cannot be confused again:
    /// one is the deepest *index*, the other is a *count*.
    fn visible_level_count(&self) -> usize {
        self.open_depth() + 1
    }
}

/// Whether an option's children are known, being fetched, or failed to arrive.
///
/// A cascader that can only be built from a fully-synchronous tree cannot describe the
/// common case of a directory picker, an address book, or any taxonomy large enough that
/// fetching every level up front is wasted work. The declared shape is therefore that a
/// branch may be **empty but not terminal**: [`CascaderOption::loading`] marks it as
/// "children are coming", which is a different statement from "this is a leaf".
///
/// The distinction matters because the two look identical in the data (`children` is
/// empty either way) and behave oppositely to a user: a leaf is a selectable answer, while
/// a pending branch is a place to wait. Collapsing them would let a user "select" a
/// category that has no value yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CascaderLoadState {
    /// The option's children are known and complete.
    #[default]
    Ready,
    /// The option's children are being fetched; the level shows a placeholder row.
    Loading,
    /// The fetch finished with no children, so the option is unusable rather than empty.
    ///
    /// Kept distinct from `Ready` with no children because that combination is a mistake in
    /// the data (an option that is neither a value nor a branch), while a failed load is a
    /// real outcome an application must be able to display.
    Failed,
}

/// A single option in a cascader tree.
///
/// A leaf is a node with no children. There is deliberately no separate `CascaderLeaf`
/// type: a node that happens to have no children *is* a leaf, and a second type would
/// allow the contradictory state "leaf with children".
///
/// `children` being empty means either "this is a leaf" or "children are still loading" —
/// see [`load_state`](CascaderOption::load_state), which is what distinguishes them.
pub struct CascaderOption {
    /// Stable identifier, unique among its siblings.
    pub id: String,
    /// Text drawn in the level's row.
    pub label: String,
    /// Nested options, in display order.
    pub children: Vec<CascaderOption>,
    /// Whether this option is a pending/ready/failed branch.
    pub load_state: CascaderLoadState,
    /// Whether the row can be chosen. A disabled row is drawn dimmed and skipped by the
    /// keyboard, but its children remain reachable through a synchronous tree.
    pub enabled: bool,
}

impl CascaderOption {
    /// Creates a childless option (a leaf).
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            load_state: CascaderLoadState::Ready,
            enabled: true,
        }
    }

    /// Creates an option with children.
    pub fn branch(
        id: impl Into<String>,
        label: impl Into<String>,
        children: Vec<CascaderOption>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children,
            load_state: CascaderLoadState::Ready,
            enabled: true,
        }
    }

    /// Creates a branch whose children will arrive from an asynchronous loader.
    ///
    /// The option is created with no children and marked [`CascaderLoadState::Loading`],
    /// so the level renders a placeholder row instead of looking like an empty directory.
    /// Supply the children later with [`Cascader::complete_load`].
    pub fn loading(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            load_state: CascaderLoadState::Loading,
            enabled: true,
        }
    }

    /// Adds one child, returning `self` for chaining.
    ///
    /// Adding a child to a pending branch is allowed and **clears the pending flag**: a
    /// branch that has children is by definition not still waiting for them, so leaving the
    /// flag set would make the level draw a spinner above rows that already exist.
    pub fn child(mut self, option: CascaderOption) -> Self {
        self.children.push(option);
        self.load_state = CascaderLoadState::Ready;
        self
    }

    /// Whether this option has no children and can therefore be chosen as a value.
    ///
    /// A pending or failed branch is **not** a leaf even though it has no children: it is
    /// not a value the user can commit, and reporting it as one would let a selection
    /// resolve to a node that holds nothing.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty() && self.load_state == CascaderLoadState::Ready
    }

    /// Whether the level below this option is still being fetched.
    pub fn is_loading(&self) -> bool {
        self.load_state == CascaderLoadState::Loading
    }

    /// Whether the row can be chosen.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Marks the row as disabled, returning `self` for chaining.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Cascader — walks a path down a tree of options.
pub struct Cascader {
    base: BaseWidget,
    /// The full option tree.
    options: Vec<CascaderOption>,
    /// The committed selection, as one index per level.
    ///
    /// A path is only a valid selection when it ends on a leaf; a partial path
    /// (`0, 1` where the node at `1` still has children) is a *navigation* position,
    /// not a value, and is kept in [`Self::expanded_path`] instead.
    selected_path: Vec<usize>,
    /// Whether a value is a *set* of leaf paths rather than exactly one path (I-122).
    ///
    /// Off by default, so an existing caller keeps the original single-select control:
    /// activating a leaf replaces [`Self::selected_path`] and closes the overlay. On,
    /// activating a leaf toggles its membership in [`Self::selected_paths`] and touches
    /// neither the committed path nor the overlay.
    multi_select: bool,
    /// The paths toggled on in multi-select mode, in the order they were added (I-122).
    ///
    /// Every entry names an enabled leaf at the time it was written. The set is expressed
    /// in the same index coordinates as [`Self::selected_path`], so replacing the tree
    /// re-validates it the same way the single path is re-validated.
    selected_paths: Vec<Vec<usize>>,
    /// The path currently being browsed while the picker is open.
    ///
    /// Separate from `selected_path` so that closing the overlay without choosing
    /// leaves the committed value untouched — cancelling must not be a selection.
    expanded_path: Vec<usize>,
    /// Whether the overlay is showing.
    expanded: bool,
    /// Separator used when rendering the selected path as text.
    separator: String,
    /// Whether typing filters the current level's rows.
    filterable: bool,
    /// The filter text, when filtering is on.
    filter: String,
    /// Emitted when the committed selection changes, with the selected path.
    ///
    /// Its payload and its emission points are unchanged by multi-select mode: a toggle
    /// only touches [`Self::selected_paths`], so a single-select subscriber never sees a
    /// payload it did not ask for.
    pub selection_changed: Signal1<Vec<usize>>,
    /// Emitted when the multi-select set changes, with every selected path (I-122).
    ///
    /// Deliberately a second signal rather than a change to `selection_changed`: the two
    /// carry different shapes, and widening the existing payload would break every
    /// single-select subscriber.
    pub selection_set_changed: Signal1<Vec<Vec<usize>>>,
    /// Emitted with the branch path when the user opens a level whose children are
    /// loaded on demand (I-121).
    pub load_requested: Signal1<Vec<usize>>,
    /// The rendered selection text, memoised until the path, the tree or the separator
    /// changes.
    ///
    /// The draw path needs a `&str`, and `display_text()` builds a `Vec<String>` of every
    /// label on the path before joining it. That was one allocation per label per frame
    /// (WDGT-06), for a value that only changes when the user picks something.
    display_cache: RefCell<Option<String>>,
    /// Maximum rows painted per open level. `None` shows them all.
    max_visible: Option<usize>,
    /// First painted row per open level.
    scroll_offsets: Vec<usize>,
    /// Reveal progress of the overlay, `0.0..=1.0`, advanced by [`Cascader::tick`].
    reveal_progress: f32,
    /// The text painted while nothing is selected (I-11).
    ///
    /// Defaults to the `"Select…"` the field used to hardcode, so an existing caller paints exactly
    /// what it always did.
    placeholder: String,
    /// Prefix typed since the last navigation, for first-letter search (I-14).
    type_ahead: String,
    /// When the current type-ahead prefix began, for the shared 800 ms rule (I-P06 / I-14).
    type_ahead_started: Option<Instant>,
    /// Bumped whenever any input of the overlay's per-level row lists changes (I-P12).
    levels_revision: u64,
    /// Per-level row indices of the open overlay, rebuilt once per revision (I-P12).
    level_rows: RefCell<LevelRows>,
}

/// The open overlay's row indices, per level (I-P12).
///
/// The paint, the hit test and the geometry used to filter and window a level independently —
/// `filtered_indices` allocated a fresh index `Vec` each time, and the draw called it once per
/// level per frame while the hit path called it again per click. This cache is built once per
/// revision and read by all three.
#[derive(Default)]
struct LevelRows {
    revision: u64,
    /// Per level: how many rows the level has, before filtering.
    counts: Vec<usize>,
    /// Per level: the filtered indices, empty when the level is unfiltered (identity order).
    filtered: Vec<Vec<usize>>,
    /// Per level: the rows inside the painted window.
    windowed: Vec<Vec<usize>>,
}

impl Cascader {
    /// Creates an empty cascader.
    ///
    /// Defaults: no options, no selection, closed, `/` separator, not filterable.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Cascader, geometry, "Cascader"),
            options: Vec::new(),
            selected_path: Vec::new(),
            multi_select: false,
            selected_paths: Vec::new(),
            expanded_path: Vec::new(),
            expanded: false,
            separator: "/".to_string(),
            filterable: false,
            filter: String::new(),
            selection_changed: Signal1::new(),
            selection_set_changed: Signal1::new(),
            load_requested: Signal1::new(),
            display_cache: RefCell::new(None),
            max_visible: None,
            scroll_offsets: Vec::new(),
            reveal_progress: 1.0,
            placeholder: String::from("Select…"),
            type_ahead: String::new(),
            type_ahead_started: None,
            levels_revision: 0,
            level_rows: RefCell::new(LevelRows::default()),
        }
    }

    /// The text painted while nothing is selected (I-11).
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the text painted while nothing is selected (I-11).
    pub fn set_placeholder(&mut self, text: &str) {
        if self.placeholder == text {
            return;
        }
        self.placeholder = text.to_string();
        self.base.request_redraw();
    }

    /// Invalidates the overlay's per-level row cache (I-P12).
    fn invalidate_level_rows(&mut self) {
        self.levels_revision = self.levels_revision.wrapping_add(1);
    }

    /// Rebuilds the overlay's per-level row cache when one of its inputs changed (I-P12).
    fn ensure_level_rows(&self) {
        let levels = self.visible_level_count();
        if let Ok(cache) = self.level_rows.try_borrow() {
            if cache.revision == self.levels_revision && cache.counts.len() == levels {
                return;
            }
        }
        let mut counts = Vec::with_capacity(levels);
        let mut filtered = Vec::with_capacity(levels);
        let mut windowed = Vec::with_capacity(levels);
        for level in 0..levels {
            let Some(siblings) = self.siblings_at(level) else {
                counts.push(0);
                filtered.push(Vec::new());
                windowed.push(Vec::new());
                continue;
            };
            let total = siblings.len();
            counts.push(total);
            // An unfiltered level keeps the identity order: there is nothing to collect, so the
            // rows are generated from the count when they are read.
            let rows: Vec<usize> = if self.filter.is_empty() || level != self.expanded_path.len() {
                Vec::new()
            } else {
                let needle = crate::widget::input_widgets::text_match::fold(&self.filter);
                siblings
                    .iter()
                    .enumerate()
                    .filter(|(_, option)| {
                        crate::widget::input_widgets::text_match::contains(&option.label, &needle)
                    })
                    .map(|(index, _)| index)
                    .collect()
            };
            let row_count = if rows.is_empty() { total } else { rows.len() };
            let offset = self.scroll_offset(level).min(row_count);
            let page = self.page_size(row_count);
            let window: Vec<usize> = if rows.is_empty() {
                (offset..(offset + page).min(total)).collect()
            } else {
                rows.iter().skip(offset).take(page).copied().collect()
            };
            filtered.push(rows);
            windowed.push(window);
        }
        if let Ok(mut cache) = self.level_rows.try_borrow_mut() {
            *cache = LevelRows { revision: self.levels_revision, counts, filtered, windowed };
        }
    }

    /// The rows the level has, before filtering (I-P12).
    fn level_row_count(&self, level: usize) -> usize {
        self.ensure_level_rows();
        if let Ok(cache) = self.level_rows.try_borrow() {
            return cache.counts.get(level).copied().unwrap_or(0);
        }
        0
    }

    /// Runs `f` with the rows of `level` inside the painted window (I-P12).
    fn with_level_window<R>(&self, level: usize, f: impl FnOnce(&[usize]) -> R) -> R {
        self.ensure_level_rows();
        if let Ok(cache) = self.level_rows.try_borrow() {
            if let Some(rows) = cache.windowed.get(level) {
                return f(rows);
            }
        }
        f(&[])
    }

    /// Runs `f` with the rows the filter keeps at `level`, and the level's unfiltered count
    /// (I-P12).
    ///
    /// An **unfiltered** level has nothing to collect, so `rows` is empty and the caller reads the
    /// identity order from `count` — which is what keeps an unfiltered level free of allocation.
    fn with_level_filtered<R>(&self, level: usize, f: impl FnOnce(&[usize], usize) -> R) -> R {
        self.ensure_level_rows();
        if let Ok(cache) = self.level_rows.try_borrow() {
            let rows = cache.filtered.get(level).map(|rows| rows.as_slice()).unwrap_or(&[]);
            return f(rows, cache.counts.get(level).copied().unwrap_or(0));
        }
        f(&[], 0)
    }

    /// Replaces the whole option tree, clearing any selection that no longer
    /// resolves.
    ///
    /// # Why the selection is re-validated rather than kept
    ///
    /// A path is expressed as indices, so a new tree with fewer children at some
    /// level makes the old indices address a *different* node — silently keeping
    /// them would report a selection the user never made. Re-resolving and dropping
    /// what no longer exists is the only answer that cannot lie.
    pub fn set_options(&mut self, options: Vec<CascaderOption>) {
        self.options = options;
        self.expanded_path.clear();
        self.filter.clear();
        *self.display_cache.borrow_mut() = None;
        self.invalidate_level_rows();
        if !self.selected_path.is_empty() && self.resolve(&self.selected_path).is_none() {
            self.selected_path.clear();
        }
        // The set is expressed in the same index coordinates, so a new tree can make an entry
        // address a different node (or turn a former leaf into a branch). Re-resolve every entry
        // and drop the ones that no longer name an enabled leaf, which is the same answer the
        // single path gets above.
        let previously_selected = self.selected_paths.len();
        let selected = core::mem::take(&mut self.selected_paths);
        let mut kept = Vec::with_capacity(selected.len());
        for path in selected {
            if self.resolve(&path).is_some_and(|option| option.is_leaf() && option.is_enabled()) {
                kept.push(path);
            }
        }
        self.selected_paths = kept;
        if self.selected_paths.len() != previously_selected {
            // A subscriber that still held the old set would otherwise be stale: the caller
            // replaced the tree, but the *selection* is what this signal reports.
            self.selection_set_changed.emit(self.selected_paths.clone());
        }
        self.base.request_redraw();
    }

    /// Returns the option tree.
    pub fn options(&self) -> &[CascaderOption] {
        &self.options
    }

    /// The committed selection, as one index per level.
    pub fn selected_path(&self) -> &[usize] {
        &self.selected_path
    }

    /// Whether activating a leaf toggles it in [`Self::selected_paths`] instead of
    /// replacing [`Self::selected_path`] (I-122).
    pub fn is_multi_select(&self) -> bool {
        self.multi_select
    }

    /// Turns the opt-in multi-select mode on or off (I-122).
    ///
    /// `false` is the default and reproduces the original single-path behaviour exactly.
    /// The set is **kept** when the mode is turned off, so switching modes back and forth
    /// does not discard picks the user made; a caller that wants a clean slate writes an
    /// empty list through [`Self::set_selected_paths`].
    ///
    /// The committed single path and the set are independent: entering the mode does not
    /// seed the set from the current value, and leaving it does not restore the set into
    /// the value. That keeps a mode switch from silently rewriting either one.
    pub fn set_multi_select(&mut self, multi_select: bool) {
        if self.multi_select == multi_select {
            return;
        }
        self.multi_select = multi_select;
        self.base.request_redraw();
    }

    /// The paths toggled on in multi-select mode, in the order they were added (I-122).
    pub fn selected_paths(&self) -> &[Vec<usize>] {
        &self.selected_paths
    }

    /// Replaces the whole multi-select set (I-122), returning whether every path was accepted.
    ///
    /// A write is **atomic**: if any entry is empty, does not resolve, or does not name an
    /// enabled leaf, nothing is changed and `false` is returned. Committing a branch as a
    /// value is the one thing a cascader must not do, and a partial write would leave the
    /// caller unsure which entries landed.
    ///
    /// Duplicate paths collapse to their first occurrence, because a set cannot hold the
    /// same value twice and the caller's order is the only meaningful one.
    ///
    /// Emits [`Self::selection_set_changed`] once when the resulting set differs from the
    /// current one; rewriting the same set is not a change.
    pub fn set_selected_paths(&mut self, paths: Vec<Vec<usize>>) -> bool {
        let mut accepted: Vec<Vec<usize>> = Vec::with_capacity(paths.len());
        for path in paths {
            let Some(option) = self.resolve(&path) else {
                return false;
            };
            // A disabled leaf is not selectable by any route, including a programmatic one.
            if !option.is_leaf() || !option.is_enabled() {
                return false;
            }
            if !accepted.contains(&path) {
                accepted.push(path);
            }
        }
        if accepted == self.selected_paths {
            return true;
        }
        self.selected_paths = accepted;
        self.selection_set_changed.emit(self.selected_paths.clone());
        self.base.request_redraw();
        true
    }

    /// Adds or removes one leaf path in the multi-select set (I-122).
    ///
    /// The single emission point for a user-driven toggle, which is what keeps the signal
    /// at exactly one per activation however the row was reached (click, Enter or Right).
    fn toggle_selected_path(&mut self, path: Vec<usize>) {
        if !self.is_leaf_path(&path) {
            return;
        }
        if self.resolve(&path).is_some_and(|option| !option.is_enabled()) {
            return;
        }
        let existing = self.selected_paths.iter().position(|selected| *selected == path);
        if let Some(position) = existing {
            self.selected_paths.remove(position);
        } else {
            self.selected_paths.push(path);
        }
        self.selection_set_changed.emit(self.selected_paths.clone());
        self.base.request_redraw();
    }

    /// Sets the separator used when the path is rendered as text.
    ///
    /// An empty separator is accepted: it means "concatenate the labels", which is
    /// the right spelling for languages whose breadcrumb convention has no glyph
    /// between parts.
    pub fn set_separator(&mut self, separator: &str) {
        if self.separator == separator {
            return;
        }
        self.separator = separator.to_string();
        *self.display_cache.borrow_mut() = None;
        self.base.request_redraw();
    }

    /// Returns the separator.
    pub fn separator(&self) -> &str {
        &self.separator
    }

    /// Whether the overlay is showing.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Opens the overlay, starting from the committed selection.
    ///
    /// Opening at the selected path means the user resumes where they left off
    /// rather than at the root of a large tree.
    pub fn expand(&mut self) {
        if self.expanded {
            return;
        }
        self.expanded_path = self.selected_path.clone();
        self.expanded = true;
        // The overlay owns the press that dismisses it (I-127).
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.base.id());
        self.filter.clear();
        self.scroll_offsets.clear();
        self.reveal_progress = 0.0;
        self.invalidate_level_rows();
        self.base.request_redraw();
    }

    /// Closes the overlay without committing anything.
    ///
    /// Discards the browsed path, which is what makes cancel a cancel: the value
    /// stays whatever the last `set_selected_path` or committed click produced.
    pub fn collapse(&mut self) {
        if !self.expanded {
            return;
        }
        self.expanded = false;
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.base.id());
        self.expanded_path.clear();
        self.filter.clear();
        self.invalidate_level_rows();
        self.base.request_redraw();
    }

    /// Advances the overlay's fade. Returns whether another frame is due.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if self.reveal_progress >= 1.0 {
            return false;
        }
        self.reveal_progress = (self.reveal_progress + delta_ms as f32 / 120.0).min(1.0);
        self.base.request_redraw();
        self.reveal_progress < 1.0
    }

    /// The maximum number of rows painted per open level.
    pub fn max_visible(&self) -> Option<usize> {
        self.max_visible
    }

    /// Caps the rows painted per open level.
    ///
    /// `None` restores full expansion. A cap of zero is treated as one row.
    pub fn set_max_visible(&mut self, max_visible: Option<usize>) {
        let max_visible = max_visible.map(|value| value.max(1));
        if self.max_visible == max_visible {
            return;
        }
        self.max_visible = max_visible;
        self.invalidate_level_rows();
        self.base.request_redraw();
    }

    /// The first row painted at `level`.
    pub fn scroll_offset(&self, level: usize) -> usize {
        self.scroll_offsets.get(level).copied().unwrap_or(0)
    }

    /// Sets the first row painted at `level`, clamped to the level's rows.
    pub fn set_scroll_offset(&mut self, level: usize, offset: usize) {
        let rows = self.filtered_indices(level).len();
        let page = self.page_size(rows);
        let max_offset = rows.saturating_sub(page.min(rows));
        let offset = offset.min(max_offset);
        if self.scroll_offsets.len() <= level {
            self.scroll_offsets.resize(level + 1, 0);
        }
        if self.scroll_offsets[level] == offset {
            return;
        }
        self.scroll_offsets[level] = offset;
        self.invalidate_level_rows();
        self.base.request_redraw();
    }

    /// Number of rows painted at once for a level with `rows` rows.
    pub fn page_size(&self, rows: usize) -> usize {
        self.max_visible.unwrap_or(rows).max(1)
    }

    /// Whether tying filters the visible rows.
    pub fn is_filterable(&self) -> bool {
        self.filterable
    }

    /// Sets whether typing filters the visible rows.
    pub fn set_filterable(&mut self, filterable: bool) {
        // Only a real change resets the filter: the filter exists to narrow filterable options,
        // so re-applying the current flag must not silently discard what the user typed (WDGT-15).
        if self.filterable == filterable {
            return;
        }
        self.filterable = filterable;
        self.filter.clear();
        self.invalidate_level_rows();
        self.base.request_redraw();
    }

    /// The current filter text.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The options at `level`, honouring the filter when one is set.
    ///
    /// Returns an empty slice for a level that cannot exist, which is what a caller
    /// should see for a path that walks off the tree.
    pub fn visible_options_at(&self, level: usize) -> &[CascaderOption] {
        let Some(siblings) = self.siblings_at(level) else {
            return &[];
        };
        if self.filter.is_empty() {
            return siblings;
        }
        // Filtering is only meaningful on the level being browsed: applying it to
        // every level would hide the rows that lead to a match.
        if level != self.expanded_path.len() {
            return siblings;
        }
        // A `Vec` cannot be returned by reference from a filtered view, so the filter is not
        // applied here: this accessor answers "what rows exist at this level", and
        // [`Self::filtered_indices`] is the one that narrows them. They share the
        // `level == expanded_path.len()` guard above, so the two cannot disagree about which
        // level is browsable.
        //
        // This used to compute `self.filter.to_lowercase()` and discard it (`let _ = needle;`),
        // which read as a filter that had been started and forgotten. It filtered nothing, so it
        // is removed rather than left to imply otherwise.
        siblings
    }

    /// The indices of the rows the filter keeps at `level`.
    ///
    /// Always ascending, so the drawing and hit-test paths iterate the same order
    /// and a row's screen position is a function of its rank in this list.
    pub fn filtered_indices(&self, level: usize) -> Vec<usize> {
        let Some(siblings) = self.siblings_at(level) else {
            return Vec::new();
        };
        if self.filter.is_empty() || level != self.expanded_path.len() {
            return (0..siblings.len()).collect();
        }
        let needle = self.filter.to_lowercase();
        siblings
            .iter()
            .enumerate()
            // Shared ASCII fast path: the previous `label.to_lowercase().contains(..)`
            // allocated one `String` per option per keystroke.
            .filter(|(_, option)| {
                crate::widget::text_utils::contains_ignore_ascii_case(&option.label, &needle)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// The siblings a level indexes into, given the browsed path.
    fn siblings_at(&self, level: usize) -> Option<&[CascaderOption]> {
        if level > self.expanded_path.len() {
            return None;
        }
        let mut current: &[CascaderOption] = &self.options;
        for depth in 0..level {
            let index = *self.expanded_path.get(depth)?;
            current = &current.get(index)?.children;
        }
        Some(current)
    }

    /// Resolves a path to the option it names, when every index is in range.
    pub fn resolve(&self, path: &[usize]) -> Option<&CascaderOption> {
        let mut current: &[CascaderOption] = &self.options;
        let mut found: Option<&CascaderOption> = None;
        for index in path {
            let option = current.get(*index)?;
            found = Some(option);
            current = &option.children;
        }
        found
    }

    /// Whether `path` names a selectable value, i.e. resolves to a leaf.
    ///
    /// A branch is not a value: choosing one would leave the control reporting a
    /// selection the user has not finished making.
    pub fn is_leaf_path(&self, path: &[usize]) -> bool {
        self.resolve(path).is_some_and(CascaderOption::is_leaf)
    }

    // ── Asynchronous level loading ─────────────────────────

    /// Supply the children of the branch at `path`, completing a pending load.
    ///
    /// `path` addresses the branch the way every other method does. Returns `false` and
    /// changes nothing when the path resolves to a leaf, or to a branch that already has
    /// children — silently accepting those would let a completed fetch overwrite a subtree
    /// the caller may still be holding indices into.
    ///
    /// An empty `children` resolves the branch as [`CascaderLoadState::Failed`] rather than
    /// as a leaf: a fetch that returned nothing has not turned the option into a value, and
    /// treating it as one would let the user commit an empty category.
    pub fn complete_load(&mut self, path: &[usize], children: Vec<CascaderOption>) -> bool {
        // Decide on the immutable view first, so the borrow checker sees the read and the
        // write as separate steps; it also keeps the two rejection reasons adjacent.
        let Some(existing) = self.resolve(path) else {
            return false;
        };
        if !existing.children.is_empty() {
            return false;
        }
        if existing.load_state == CascaderLoadState::Ready {
            // A ready branch with no children is a leaf, and a leaf's children cannot be
            // supplied after the fact without changing it from a value into a branch.
            return false;
        }
        let Some(option) = self.option_at_path_mut(path) else {
            return false;
        };
        if children.is_empty() {
            option.load_state = CascaderLoadState::Failed;
        } else {
            option.children = children;
            option.load_state = CascaderLoadState::Ready;
        }
        self.base.request_redraw();
        true
    }

    /// Marks the branch at `path` as failed to load.
    ///
    /// Exists separately from `complete_load(path, vec![])` so a caller that has an error to
    /// report does not have to express it as "loaded nothing" — the two produce the same
    /// state, but only one of them reads as intentional at the call site.
    pub fn fail_load(&mut self, path: &[usize]) -> bool {
        self.complete_load(path, Vec::new())
    }

    /// Whether the branch at `path` is still waiting for its children.
    pub fn is_loading_path(&self, path: &[usize]) -> bool {
        self.resolve(path).is_some_and(|option| option.is_loading())
    }

    /// Chooses the option at `option_index` on `level`, as a click would.
    ///
    /// Exposed so tests can drive the selection path without synthesising pointer
    /// coordinates for a specific column layout — the geometry is asserted separately, and
    /// coupling these assertions to it would make them fail for unrelated layout changes.
    pub fn choose_at(&mut self, level: usize, option_index: usize) -> bool {
        if self.siblings_at(level).and_then(|s| s.get(option_index)).is_none() {
            return false;
        }
        self.choose(level, option_index);
        true
    }

    /// Resolve `path` to a mutable option.
    ///
    /// The mutable counterpart to [`resolve`](Self::resolve); both walk the same chain so
    /// they cannot disagree about what a path names.
    fn option_at_path_mut(&mut self, path: &[usize]) -> Option<&mut CascaderOption> {
        let mut index = path.first().copied()?;
        let mut current = self.options.get_mut(index)?;
        for &next in &path[1..] {
            index = next;
            current = current.children.get_mut(index)?;
        }
        Some(current)
    }

    /// Sets the committed selection.
    ///
    /// Returns `false` and changes nothing when `path` does not name a leaf, so a
    /// caller cannot commit a half-made choice.
    pub fn set_selected_path(&mut self, path: Vec<usize>) -> bool {
        if !self.is_leaf_path(&path) {
            return false;
        }
        // A disabled leaf is not selectable by any route, including a programmatic one.
        if self.resolve(&path).is_some_and(|option| !option.enabled) {
            return false;
        }
        if self.selected_path == path {
            return true;
        }
        self.selected_path = path;
        *self.display_cache.borrow_mut() = None;
        self.selection_changed.emit(self.selected_path.clone());
        self.base.request_redraw();
        true
    }

    /// Clears the committed selection.
    ///
    /// In multi-select mode this also empties the set, because the set *is* the control's
    /// value there; a caller that left it behind would see picks on screen after asking for
    /// "none". Each signal fires only for the state that actually changed, so single-select
    /// behaviour (and the single-select signal) is unchanged.
    pub fn clear_selected(&mut self) {
        let mut changed = false;
        if !self.selected_path.is_empty() {
            self.selected_path.clear();
            *self.display_cache.borrow_mut() = None;
            self.selection_changed.emit(Vec::new());
            changed = true;
        }
        if self.multi_select && !self.selected_paths.is_empty() {
            self.selected_paths.clear();
            self.selection_set_changed.emit(Vec::new());
            changed = true;
        }
        if changed {
            self.base.request_redraw();
        }
    }

    /// The labels along the selected path, outermost first.
    pub fn selected_labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        let mut current: &[CascaderOption] = &self.options;
        for index in &self.selected_path {
            let Some(option) = current.get(*index) else {
                break;
            };
            labels.push(option.label.clone());
            current = &option.children;
        }
        labels
    }

    /// The selected path rendered as text, joined by the separator.
    pub fn display_text(&self) -> String {
        self.cached_display_text().clone()
    }

    /// The rendered selection, borrowed from the memo.
    ///
    /// Recomputing is O(path length) allocations; the draw path runs once per frame and
    /// reads the same value every time until the selection changes.
    fn cached_display_text(&self) -> core::cell::Ref<'_, String> {
        if self.display_cache.borrow().is_none() {
            let text = self.selected_labels().join(&self.separator);
            *self.display_cache.borrow_mut() = Some(text);
        }
        core::cell::Ref::map(self.display_cache.borrow(), |cached| {
            cached.as_ref().expect("just filled above")
        })
    }

    /// The rectangle of the closed control's own box.
    fn field_rect(&self) -> Rect {
        self.geometry()
    }

    /// The rectangle of the open overlay's level at `level`.
    ///
    /// Levels are laid out left to right from the control's left edge, which is the
    /// column convention: each open level sits beside its parent.
    ///
    /// The height is the **content** height (one row per visible option), not the
    /// field's height. Sizing it from the field made the hit-test and the drawing
    /// disagree, so a click on the second row of a level fell outside the rectangle
    /// that the drawing had painted — which is the bug
    /// `cascader_choosing_from_an_earlier_level_discards_the_deeper_path` caught.
    /// `draw_overlay` and `row_at` both call this, so they cannot disagree again.
    fn level_rect(&self, level: usize) -> Rect {
        // The row count comes from the frame's own row cache, so a hit test allocates nothing
        // (I-P12).
        let rows = self.with_level_window(level, |window| window.len());
        self.level_rect_for(level, rows.max(1))
    }

    /// [`Self::level_rect`] for a caller that already has the level's row count.
    ///
    /// `level_rect` derives the count by filtering the siblings, which allocates an index `Vec`.
    /// `draw_overlay` filters once per level to paint the rows, so asking for the rectangle
    /// through `level_rect` filtered the same level a second time — two allocations per open
    /// level per frame, on a list that can be thousands of options long. The geometry is a pure
    /// function of the count, so it belongs in a form both callers can share.
    fn level_rect_for(&self, level: usize, rows: usize) -> Rect {
        let rect = self.geometry();
        let height = (rows as u32 * ROW_HEIGHT).max(ROW_HEIGHT);
        Rect::new(
            rect.x + (level as i32) * LEVEL_INDENT,
            rect.y + rect.height as i32,
            LEVEL_WIDTH,
            height,
        )
    }

    /// The row index at `level` under `pos`, when the pointer is over one.
    fn row_at(&self, level: usize, pos: Point) -> Option<usize> {
        let level_rect = self.level_rect(level);
        if !level_rect.contains_point(pos) {
            return None;
        }
        let offset = pos.y - level_rect.y;
        if offset < 0 {
            return None;
        }
        let row = (offset as u32 / ROW_HEIGHT) as usize;
        // The window comes from the cached row list rather than a fresh filter (I-P12).
        self.with_level_window(level, |window| window.get(row).copied())
    }

    /// The level under `pos`, when the pointer is over an open level.
    fn level_at(&self, pos: Point) -> Option<usize> {
        // Searched deepest-first so that overlapping columns (they never do today,
        // but a future indent wider than `LEVEL_WIDTH` would make them) resolve to
        // the one painted on top.
        (0..self.visible_level_count())
            .rev()
            .find(|level| self.level_rect(*level).contains_point(pos))
    }

    /// Chooses the row at `level`, index `option_index`.
    ///
    /// A branch extends the browsed path and reveals the next level. A leaf commits the path
    /// and closes in single-select mode, and toggles its membership in the set without
    /// closing in multi-select mode. That single rule is what makes one click mean both
    /// "go deeper" and "pick this", depending on what was clicked.
    fn choose(&mut self, level: usize, option_index: usize) {
        let Some(option) = self.siblings_at(level).and_then(|s| s.get(option_index)) else {
            return;
        };
        if !option.enabled {
            return;
        }
        let is_loading = option.is_loading();
        let is_leaf = option.is_leaf();
        if is_leaf && self.multi_select {
            // Only leaves differ in multi-select mode. The browsed path stays where it is (a
            // leaf has no next level to reveal) and the overlay stays open, so several leaves
            // of the same level can be toggled in one visit.
            let mut path = self.expanded_path.clone();
            path.truncate(level);
            path.push(option_index);
            self.toggle_selected_path(path);
            return;
        }
        // Truncate to this level, then push, so choosing from an earlier level
        // discards the deeper path rather than appending to it.
        self.expanded_path.truncate(level);
        self.expanded_path.push(option_index);
        self.filter.clear();
        self.invalidate_level_rows();

        if is_leaf {
            let path = self.expanded_path.clone();
            self.collapse();
            self.set_selected_path(path);
        } else {
            // A branch still marked `Loading` has no children yet; the host is asked to
            // supply them for exactly this path (I-121). A branch only ever navigates, in
            // both modes.
            if is_loading {
                self.load_requested.emit(self.expanded_path.clone());
            }
            self.base.request_redraw();
        }
    }
}

impl Widget for Cascader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(dimensions::CASCADER_DEFAULT_WIDTH, dimensions::CASCADER_DEFAULT_HEIGHT)
    }

    /// The overlay fade owes frames until it is complete.
    fn is_animating(&self) -> bool {
        self.expanded && self.reveal_progress < 1.0
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
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    let mut items = crate::compat::Vec::with_capacity(v.len());
                    for index in v.iter() {
                        items.push(CapabilityValue::UInt(*index as u64));
                    }
                    CapabilityValue::Tuple(items)
                }))
            }
            "load_requested" => {
                Some(EventSignalRef::mapped("load_requested", &self.load_requested, |v| {
                    // The declared shape is `ListScalar` of `UInt`, so the requested path is
                    // delivered as the index sequence rather than its `Debug` spelling.
                    let mut items = crate::compat::Vec::with_capacity(v.len());
                    for index in v.iter() {
                        items.push(CapabilityValue::UInt(*index as u64));
                    }
                    CapabilityValue::Tuple(items)
                }))
            }
            "selection_set_changed" => Some(EventSignalRef::mapped(
                "selection_set_changed",
                &self.selection_set_changed,
                |paths| {
                    // A list of index paths; each path is a `Tuple` of `UInt`, and the whole
                    // selection is a `Tuple` of those. `Mixed` is the honest shape because no
                    // tuple-of-lists shape exists.
                    let mut selected = crate::compat::Vec::with_capacity(paths.len());
                    for path in paths.iter() {
                        let mut items = crate::compat::Vec::with_capacity(path.len());
                        for index in path.iter() {
                            items.push(CapabilityValue::UInt(*index as u64));
                        }
                        selected.push(CapabilityValue::Tuple(items));
                    }
                    CapabilityValue::Tuple(selected)
                },
            )),
            _ => None,
        }
    }
}

/// `Cascader`'s property contract.
///
/// The option tree is written through `set_options`; the property layer reports the
/// path as a `separator`-joined string plus the derived depth, following the same
/// "list state is set through methods, scalars are properties" convention `Meter`,
/// `RadarChart` and `KanbanBoard` use.
impl WidgetProperties for Cascader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The committed path as indices (BLUE26 B26.09): `set_selected_path` takes a sequence
            // the bare command channel cannot carry, so the sequence is the property and
            // `set_selected_path` is its published command form.
            "selected_path" => Ok(CapabilityValue::Tuple(
                self.selected_path()
                    .iter()
                    .map(|index| CapabilityValue::UInt(*index as u64))
                    .collect(),
            )),
            "value" => Ok(CapabilityValue::String(self.display_text())),
            "depth" => Ok(CapabilityValue::UInt(self.selected_path.len() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            "separator" => Ok(CapabilityValue::String(self.separator().to_string())),
            "filterable" => Ok(CapabilityValue::Bool(self.is_filterable())),
            "filter" => Ok(CapabilityValue::String(self.filter().to_string())),
            "max_visible" => match self.max_visible() {
                Some(max) => Ok(CapabilityValue::UInt(max as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "expanded" => {
                if expect_bool(value)? {
                    self.expand();
                } else {
                    self.collapse();
                }
                Ok(())
            }
            "separator" => {
                let separator = expect_string(value)?;
                self.set_separator(&separator);
                Ok(())
            }
            "filterable" => {
                self.set_filterable(expect_bool(value)?);
                Ok(())
            }
            // The value is a path, which is a sequence; it is written through
            // `set_selected_path` so an invalid path is refused rather than encoded.
            "value" | "depth" => Err(CapabilityAccessError::ReadOnlyProperty),
            "selected_path" => {
                let CapabilityValue::Tuple(items) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let path = items
                    .iter()
                    .map(|item| match item {
                        CapabilityValue::UInt(index) => {
                            usize::try_from(*index).map_err(|_| CapabilityAccessError::OutOfRange)
                        }
                        CapabilityValue::Int(index) if *index >= 0 => Ok(*index as usize),
                        _ => Err(CapabilityAccessError::TypeMismatch),
                    })
                    .collect::<Result<Vec<_>, CapabilityAccessError>>()?;
                // A path that names no leaf is refused, not silently clamped: committing a
                // half-made choice is the one thing a cascader must not do. An **empty** path is
                // the declared default (nothing selected), so it clears the selection rather than
                // being refused.
                if path.is_empty() {
                    self.clear_selected();
                    return Ok(());
                }
                if self.set_selected_path(path) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("selected_path"),
                        reason: crate::compat::String::from("path does not name a leaf"),
                    })
                }
            }
            "filter" => Err(CapabilityAccessError::ReadOnlyProperty),
            "max_visible" => {
                match value {
                    CapabilityValue::Null => self.set_max_visible(None),
                    other => self.set_max_visible(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "placeholder" => {
                let text = expect_string(value)?;
                self.set_placeholder(&text);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `CASCADER_PROPERTIES`.
        property_names_of![
            "selected_path",
            "value",
            "depth",
            "expanded",
            "separator",
            "filterable",
            "filter",
            "max_visible",
            "placeholder",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `cascader` publishes.
    ///
    /// `expand` and `collapse` are real actions on this control, so they go through its
    /// own methods rather than the `expanded` property: the methods open at the selected
    /// path and discard the browsed path respectively, and both short-circuit on the
    /// state they are already in. `set_options` and `set_selected_path` assign state —
    /// a tree and an index path — and need a payload, so they are answered through the
    /// property route. The tree itself has no property: an option carries `children`, and a
    /// record field cannot carry a nested record list (`value_matches_kind` refuses it), so
    /// `set_options` stays unpublished and the control's own API is the tree's write path.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "expand" => {
                self.expand();
                Ok(())
            }
            "collapse" => {
                self.collapse();
                Ok(())
            }
            "set_options" | "set_selected_path" => {
                // `set_selected_path` carries its sequence through the `selected_path` property
                // (B26.03); `set_options` has no property route (the tree is nested).
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

/// The overlay's resolved colours, read once per frame (I-P07).
#[derive(Clone, Copy)]
struct OverlayPalette {
    column_surface: Color,
    highlight_surface: Color,
    separator: Color,
    on_highlight: Color,
    branch_ink: Color,
}

impl OverlayPalette {
    /// Reads the palette the open list is painted from.
    fn resolve() -> Self {
        let manager = crate::style::theme_manager();
        match manager.current_theme() {
            Some(active) => Self {
                column_surface: active.colors.surface_container,
                highlight_surface: active.colors.primary,
                separator: active.colors.outline_variant,
                on_highlight: active.colors.primary.contrast_color(),
                branch_ink: active.colors.secondary,
            },
            None => Self {
                column_surface: Color::WHITE,
                highlight_surface: Color::rgb(232, 240, 254),
                separator: Color::rgb(190, 192, 198),
                on_highlight: Color::rgb(40, 44, 52),
                branch_ink: Color::rgb(140, 144, 152),
            },
        }
    }
}

impl Draw for Cascader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        self.draw_field(context, rect);
        if self.expanded {
            self.draw_overlay(context);
        }
    }
}

impl Cascader {
    /// Draws the closed field: the selected path as text, plus an affordance.
    fn draw_field(&self, context: &mut RenderContext, rect: Rect) {
        let style = self.style();
        let background = style.background_color.unwrap_or(Color::WHITE);
        let border = style.border_color.unwrap_or(Color::rgb(190, 192, 198));
        let text_color = style.text_color.unwrap_or(Color::rgb(40, 44, 52));

        context.fill_rounded_rect(rect, 4, background);
        context.draw_rounded_rect_stroke(rect, 4, border, 1);

        let text = self.cached_display_text();
        let is_empty = text.is_empty();
        // The hint is the host's string, which defaults to the `"Select…"` the field used to
        // hardcode (I-11).
        let display: &str = if is_empty { self.placeholder.as_str() } else { text.as_str() };
        // A placeholder is dimmer than a real value, so an empty field cannot be
        // mistaken for a field whose value happens to be that word. It is the *resolved
        // ink* dimmed toward the field, not a fixed grey: the literal `150,154,162` did not
        // move with the appearance at all, so it measured 2.48:1 on the light field and sat
        // at the wrong end of the ramp on the dark one.
        let color = if is_empty {
            // A placeholder reads as a hint rather than a value: the resolved ink dimmed
            // toward the field, but still above the 4.5:1 floor, since a hint the user cannot
            // read is not a hint. (The literal it replaced was a fixed grey that did not move
            // with the appearance at all, and measured 2.48:1 on the light field.)
            text_color.blend(&background, 0.35).legible_on(background, 4.5)
        } else {
            text_color
        };
        // The origin is the glyph box's top edge, so the label is centred from the measured
        // line box rather than from the fixed `21` that only fitted one font size. It is
        // also fitted to the space left of the chevron: an unbounded label ran into the
        // disclosure arrow (and past the field on a narrow layout).
        let font = crate::widget::metrics::role_font(self.style(), 12.0);
        let text_h = context.measure_text(display, &font).height as i32;
        let label_width = ((rect.width as i32) - 8 - 22).max(0) as u32;
        context.draw_text_fitted_cow(
            Rect::new(
                rect.x + 8,
                rect.y + (rect.height as i32 - text_h) / 2,
                label_width,
                text_h.max(1) as u32,
            ),
            display,
            &font,
            color,
            HorizontalAlignment::Left,
        );

        // A chevron pointing down when closed and up when open, which is the one
        // piece of state a user needs from the field itself.
        let cx = rect.x + rect.width as i32 - 14;
        let cy = rect.y + rect.height as i32 / 2;
        let (from_y, to_y) = if self.expanded { (cy + 3, cy - 3) } else { (cy - 3, cy + 3) };
        context.draw_line_stroke(Point::new(cx - 4, to_y), Point::new(cx, from_y), text_color, 1);
        context.draw_line_stroke(Point::new(cx, from_y), Point::new(cx + 4, to_y), text_color, 1);
    }

    /// Draws every open level as a column, deepest last so it paints on top.
    fn draw_overlay(&self, context: &mut RenderContext) {
        // One palette read for the whole overlay (I-P07): the column colours used to take the
        // theme manager's lock once per open level per frame.
        let palette = OverlayPalette::resolve();
        // One row-index build for the whole overlay (I-P12): the paint, the hit test and the
        // geometry below all read this cache.
        self.ensure_level_rows();
        for level in 0..self.visible_level_count() {
            self.with_level_window(level, |visible| {
                let column = self.level_rect_for(level, visible.len());
                self.draw_level(context, level, column, visible, &palette);
            });
        }
    }

    /// Draws one column of the overlay.
    ///
    /// # Why the column reads the theme and the field's fallbacks do not
    ///
    /// The closed field's literals are *fallbacks*: `apply_active_theme` fills
    /// `style.background_color` / `border_color` / `text_color` before this runs, so on a themed
    /// build the `unwrap_or` arms are unreachable. The overlay is different — it painted
    /// `Color::WHITE` and three fixed greys unconditionally, so nothing the theme said reached the
    /// list a dark build opens. That is the same defect `dropdown` had, and it is fixed the same
    /// way: a popup is one step above the page, so the column is `surface_container`; a highlighted
    /// row is a *selection*, so it is the accent pair rather than a paler grey; and the branch
    /// chevron is the weak separator ink rather than a literal.
    fn draw_level(
        &self,
        context: &mut RenderContext,
        level: usize,
        column: Rect,
        visible: &[usize],
        palette: &OverlayPalette,
    ) {
        // The palette was read once for the whole overlay (I-P07); these are its copies.
        let (column_surface, highlight_surface, separator, on_highlight, branch_ink) = (
            palette.column_surface,
            palette.highlight_surface,
            palette.separator,
            palette.on_highlight,
            palette.branch_ink,
        );
        let row_ink = self.style().text_color.unwrap_or_else(|| column_surface.contrast_color());
        // One role font for every row in this column: it used to be rebuilt per row, and on a
        // loading row twice.
        let row_font = crate::widget::metrics::role_font(self.style(), 12.0);
        // The overlay fades in rather than appearing on one frame. Only the fills and strokes
        // take the alpha; the text stays fully opaque so it is never unreadably faint.
        let reveal_alpha = (self.reveal_progress.clamp(0.0, 1.0) * 255.0).round() as u8;

        context.fill_rounded_rect(column, 4, column_surface.with_alpha(reveal_alpha));
        context.draw_rounded_rect_stroke(column, 4, separator.with_alpha(reveal_alpha), 1);

        let Some(siblings) = self.siblings_at(level) else {
            return;
        };
        // The index the path currently points at on this level, if any.
        let current = self.expanded_path.get(level).copied();
        // The index the **committed** path points at on this level, if any: reopening the list used
        // to lose the branch the user had already chosen (I-12).
        let committed = self.selected_path.get(level).copied();
        // The prefix every row of this level shares in multi-select set coordinates.
        let level_prefix: &[usize] = self.expanded_path.get(..level).unwrap_or(&[]);

        for (row, &option_index) in visible.iter().enumerate() {
            let Some(option) = siblings.get(option_index) else {
                continue;
            };
            let row_rect = Rect::new(
                column.x,
                column.y + (row as i32) * ROW_HEIGHT as i32,
                column.width,
                ROW_HEIGHT,
            );
            let highlighted = option_index == current.unwrap_or(usize::MAX);
            // Single-select marks the row the committed path names. Multi-select marks every
            // row whose full path is in the set, which is the visible feedback a toggle has.
            // The comparison borrows both sides, so marking adds no per-frame allocation.
            let chosen = if self.multi_select {
                self.selected_paths.iter().any(|selected| {
                    selected.len() == level + 1
                        && selected[level] == option_index
                        && selected.get(..level) == Some(level_prefix)
                })
            } else {
                committed == Some(option_index)
            };
            if highlighted {
                // The row the path points at is highlighted, which is how the user
                // sees which branch the next column belongs to. The fill is the accent, so the
                // row reads as a **selection** rather than as a paler grey; its ink is the
                // accent's contrast colour, which is legible on that fill by construction.
                context.fill_rect(row_rect, highlight_surface.with_alpha(reveal_alpha));
            } else if chosen {
                // The committed branch keeps a faint wash, so a reopened list shows the branch the
                // value came from without competing with the row the browsed path is on (I-12).
                context.fill_rect(row_rect, branch_ink.with_alpha(reveal_alpha / 8));
            }
            let row_line = context.text_line(row_rect, &row_font);
            // Fitted to the space left of the branch chevron / pending ellipsis zone (the
            // rightmost 20px of the row): an unbounded label ran underneath it.
            let label_width = ((row_rect.width as i32) - 8 - 20).max(0) as u32;
            let label_ink = if !option.enabled {
                // A disabled row is dimmed on every highlight, so "cannot be chosen" is
                // visible rather than only enforced.
                row_ink.with_alpha(110)
            } else if highlighted {
                on_highlight
            } else {
                row_ink
            };
            context.draw_text_fitted_cow(
                Rect::new(row_rect.x + 8, row_line.y, label_width, row_line.height.max(1)),
                &option.label,
                &row_font,
                label_ink,
                HorizontalAlignment::Left,
            );
            if option.is_loading() {
                // A pending branch shows an ellipsis where the chevron would be: it is not
                // "a subtree you can open" yet, and drawing the chevron would invite a
                // click that has nothing to open. The row stays in place so the tree does
                // not jump when the children arrive.
                context.draw_text(
                    Point::new(row_rect.x + row_rect.width as i32 - 18, row_line.y),
                    "…",
                    &row_font,
                    branch_ink,
                    HorizontalAlignment::Left,
                );
            } else if !option.is_leaf() {
                // A branch is marked so the user knows a further level exists —
                // otherwise clicking a branch would look like nothing happened.
                let ax = row_rect.x + row_rect.width as i32 - 12;
                let ay = row_rect.y + ROW_HEIGHT as i32 / 2;
                context.draw_line_stroke(
                    Point::new(ax - 2, ay - 4),
                    Point::new(ax + 2, ay),
                    branch_ink,
                    1,
                );
                context.draw_line_stroke(
                    Point::new(ax + 2, ay),
                    Point::new(ax - 2, ay + 4),
                    branch_ink,
                    1,
                );
            }
            // A row on the committed path carries a check mark at its trailing edge, drawn in two
            // strokes the same way `list_box` draws its check box (I-12). It sits to the left of
            // the branch chevron's cell so the two marks cannot overlap.
            if chosen {
                let cx = row_rect.x + row_rect.width as i32 - 30;
                let cy = row_rect.y + ROW_HEIGHT as i32 / 2;
                context.draw_line_stroke(
                    Point::new(cx, cy + 1),
                    Point::new(cx + 3, cy + 4),
                    on_highlight,
                    2,
                );
                context.draw_line_stroke(
                    Point::new(cx + 3, cy + 4),
                    Point::new(cx + 9, cy - 4),
                    on_highlight,
                    2,
                );
            }
        }

        // A filter that matches nothing draws an empty panel; the sibling dropdowns in this family
        // both say "(No matches)", and so does this one (I-13).
        if visible.is_empty() && !self.filter.is_empty() && level == self.expanded_path.len() {
            let row_rect = Rect::new(column.x, column.y, column.width, ROW_HEIGHT);
            let row_line = context.text_line(row_rect, &row_font);
            context.draw_text_fitted_cow(
                Rect::new(
                    row_rect.x + 8,
                    row_line.y,
                    (row_rect.width as i32 - 16).max(1) as u32,
                    row_line.height.max(1),
                ),
                "(No matches)",
                &row_font,
                branch_ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for Cascader {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let field = self.field_rect();
                if field.contains_point(*pos) {
                    if self.expanded {
                        self.collapse();
                    } else {
                        self.expand();
                    }
                    return;
                }
                if self.expanded {
                    if let Some(level) = self.level_at(*pos) {
                        if let Some(option_index) = self.row_at(level, *pos) {
                            self.choose(level, option_index);
                        } else {
                            // A click on the column but past its rows dismisses, which
                            // is the behaviour every popup list has.
                            self.collapse();
                        }
                    } else {
                        // A click outside the field and outside every column closes
                        // without choosing — the standard way to abandon a picker.
                        self.collapse();
                    }
                }
            }
            // Backspace edits the filter before the general key arm sees it, so
            // filtering and navigation do not compete for the same key codes.
            Event::KeyDown((8, _)) if self.filterable && self.expanded => {
                // On an empty filter there is nothing left to delete; leaving it
                // alone keeps the key from closing the overlay unexpectedly.
                if !self.filter.is_empty() {
                    self.filter.pop();
                    self.invalidate_level_rows();
                }
                self.base.request_redraw();
            }
            Event::KeyDown((key, _)) | Event::KeyPress { key, .. } => match *key {
                27 => self.collapse(), // Escape
                40 => self.move_browsed(1),
                38 => self.move_browsed(-1),
                39 => {
                    // Right walks into the next unbrowsed level: the highlighted branch of
                    // this level, or the first enabled row when the next level has not been
                    // entered yet. Re-choosing the deepest walked node would be idempotent and
                    // left the keyboard unable to descend at all.
                    let level = self.expanded_path.len();
                    if let Some(&current) = self.expanded_path.get(level) {
                        self.choose(level, current);
                    } else {
                        let visible = self.filtered_indices(level);
                        let target = visible.iter().copied().find(|&index| {
                            self.siblings_at(level)
                                .and_then(|siblings| siblings.get(index))
                                .is_some_and(|option| option.enabled)
                        });
                        if let Some(index) = target {
                            self.choose(level, index);
                        }
                    }
                }
                37 => {
                    // Left retreats one level, keeping the shallower path browsed.
                    if !self.expanded_path.is_empty() {
                        self.expanded_path.pop();
                        self.invalidate_level_rows();
                        self.base.request_redraw();
                    } else {
                        self.collapse();
                    }
                }
                13 => {
                    // Enter activates the browsed path when it ends on a leaf: it commits and
                    // closes in single-select mode, and toggles the path in the set without
                    // closing in multi-select mode — the same rule a leaf click follows.
                    let path = self.expanded_path.clone();
                    if !path.is_empty() && self.is_leaf_path(&path) {
                        if self.multi_select {
                            self.toggle_selected_path(path);
                        } else if self.set_selected_path(path) {
                            self.collapse();
                        }
                    }
                }
                36 => self.move_browsed(
                    -(self.visible_options_at(self.expanded_path.len().saturating_sub(1)).len()
                        as i32),
                ),
                35 => self.move_browsed(
                    self.visible_options_at(self.expanded_path.len().saturating_sub(1)).len()
                        as i32,
                ),
                // Page Up / Page Down move the browsed row by one painted window, the same
                // semantic `list_box`, `combo_box` and `dropdown` give the two keys (I-14).
                33 => {
                    let level = self.expanded_path.len().saturating_sub(1);
                    let page = self.page_size(self.level_row_count(level)) as i32;
                    self.move_browsed(-page);
                }
                34 => {
                    let level = self.expanded_path.len().saturating_sub(1);
                    let page = self.page_size(self.level_row_count(level)) as i32;
                    self.move_browsed(page);
                }
                // First-letter search inside the current level's filtered rows, the same
                // type-ahead the rest of the family has (I-14).
                32..=126 => {
                    if let Some(ch) = char::from_u32(*key) {
                        let level = self.expanded_path.len().saturating_sub(1);
                        let now = Instant::now();
                        let expired = self.type_ahead_started.is_none_or(|started| {
                            now.duration_since(started).as_millis()
                                > u128::from(
                                    crate::widget::input_widgets::text_match::TYPE_AHEAD_TIMEOUT_MS,
                                )
                        });
                        if expired {
                            self.type_ahead.clear();
                        }
                        self.type_ahead_started = Some(now);
                        self.type_ahead.push(ch);
                        let needle =
                            crate::widget::input_widgets::text_match::fold(&self.type_ahead);
                        // The candidate rows come from the frame's own cache, so a keystroke
                        // allocates no index list (I-P12 / I-14).
                        let target = self.with_level_filtered(level, |rows, total| {
                            let len = if rows.is_empty() { total } else { rows.len() };
                            if len == 0 {
                                return None;
                            }
                            let row_at = |position: usize| {
                                if rows.is_empty() {
                                    position
                                } else {
                                    rows[position]
                                }
                            };
                            let start = self
                                .expanded_path
                                .get(level)
                                .and_then(|index| (0..len).find(|position| row_at(*position) == *index))
                                .map_or(0, |position| (position + 1) % len);
                            (0..len).map(|offset| row_at((start + offset) % len)).find(|&index| {
                                self.siblings_at(level)
                                    .and_then(|siblings| siblings.get(index))
                                    .is_some_and(|option| {
                                        option.enabled
                                            && crate::widget::input_widgets::text_match::starts_with(
                                                &option.label,
                                                &needle,
                                            )
                                    })
                            })
                        });
                        if let Some(index) = target {
                            self.expanded_path.truncate(level);
                            self.expanded_path.push(index);
                            self.invalidate_level_rows();
                            self.base.request_redraw();
                        }
                    }
                }
                _ => {}
            },
            Event::Wheel { delta, modifiers } => {
                if !self.expanded {
                    return;
                }
                // The wheel scrolls the deepest open column, which is the one the user is
                // looking at.
                let level = self.visible_level_count().saturating_sub(1);
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y != 0 {
                    let offset =
                        (self.scroll_offset(level) as i32 + scroll.lines_y).max(0) as usize;
                    self.set_scroll_offset(level, offset);
                }
            }
            // Typed characters filter the browsed level when filtering is on. Control
            // characters are excluded in the pattern so the guard and the body agree
            // about what is acceptable. An IME commit is the same text arriving after the
            // input method resolved it, so it takes the same branch and the same guard.
            Event::TextInput { text } | Event::ImeCommit { text }
                if self.filterable && self.expanded && text.chars().all(|ch| !ch.is_control()) =>
            {
                self.filter.push_str(text);
                self.invalidate_level_rows();
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}

impl Cascader {
    /// Moves the browsed path's last index by `delta`, clamped to the level.
    ///
    /// Operates on the **browsed** path, so keyboard navigation is preview-only
    /// until the user confirms a leaf by clicking it. Arrow keys on a closed
    /// cascader do nothing, because there is no visible level to move within.
    fn move_browsed(&mut self, delta: i32) {
        if !self.expanded {
            return;
        }
        // Arrow keys act on the level that already has a walked node — the column
        // the user is actively choosing in. Falling back to level 0 means the first
        // press enters the first column.
        //
        // Using `open_depth()` alone was wrong: after the first press walked level 0,
        // `open_depth()` became 1, so the *second* press acted on a column that had
        // not been chosen into yet. Keying on "the deepest walked node" keeps every
        // press on the column the user is deciding in.
        let level = self.expanded_path.len().saturating_sub(1);
        let visible = self.filtered_indices(level);
        if visible.is_empty() {
            return;
        }
        let current = self
            .expanded_path
            .get(level)
            .and_then(|index| visible.iter().position(|visible| visible == index));
        let next = match current {
            Some(position) => {
                let mut cursor = position as i32;
                loop {
                    cursor += delta.signum();
                    if cursor < 0 || cursor >= visible.len() as i32 {
                        return;
                    }
                    let candidate = visible[cursor as usize];
                    if self
                        .siblings_at(level)
                        .and_then(|siblings| siblings.get(candidate))
                        .is_some_and(|option| option.enabled)
                    {
                        break cursor as usize;
                    }
                }
            }
            None => {
                let mut found = None;
                let order: Vec<usize> = if delta > 0 {
                    (0..visible.len()).collect()
                } else {
                    (0..visible.len()).rev().collect()
                };
                for position in order {
                    let candidate = visible[position];
                    if self
                        .siblings_at(level)
                        .and_then(|siblings| siblings.get(candidate))
                        .is_some_and(|option| option.enabled)
                    {
                        found = Some(position);
                        break;
                    }
                }
                match found {
                    Some(position) => position,
                    None => return,
                }
            }
        };
        if let Some(&option_index) = visible.get(next) {
            self.expanded_path.truncate(level);
            self.expanded_path.push(option_index);
            self.invalidate_level_rows();
            // Keep the keyboard row inside the visible window when the level is capped.
            let page = self.page_size(visible.len());
            if next < self.scroll_offset(level) {
                self.set_scroll_offset(level, next);
            } else if next >= self.scroll_offset(level) + page {
                self.set_scroll_offset(level, next + 1 - page);
            }
            self.base.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    /// A three-level tree: 2 provinces, each with 2 cities, each with 2 districts.
    fn tree() -> Vec<CascaderOption> {
        let district = |prefix: &str| {
            CascaderOption::branch(
                format!("{prefix}-d"),
                format!("{prefix} District"),
                vec![
                    CascaderOption::new(format!("{prefix}-d1"), format!("{prefix} D1")),
                    CascaderOption::new(format!("{prefix}-d2"), format!("{prefix} D2")),
                ],
            )
        };
        let city = |prefix: &str| {
            CascaderOption::branch(
                format!("{prefix}-c"),
                format!("{prefix} City"),
                vec![district(prefix)],
            )
        };
        vec![
            CascaderOption::branch("p1", "Province 1", vec![city("Alpha"), city("Beta")]),
            CascaderOption::branch("p2", "Province 2", vec![city("Gamma")]),
        ]
    }

    fn cascader() -> Cascader {
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(tree());
        c
    }

    /// Renders and returns the RGBA frame.
    ///
    /// Gated like its consumer: the software backend and `Size` only exist behind a device
    /// profile with `software`, so an ungated helper is a dead-code warning (or a hard error in a
    /// `no_std` profile) for a build that never calls it.
    fn render(c: &mut Cascader, size: Size) -> Vec<u8> {
        render_on(c, size, Color::WHITE)
    }

    /// Renders over an explicit backdrop, which is the surface the control would sit on.
    fn render_on(c: &mut Cascader, size: Size, backdrop: Color) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(backdrop);
        let mut context = RenderContext::new(&mut backend);
        c.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The RGBA pixel at `(x, y)` of a `size`-wide frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn pixel(frame: &[u8], size: Size, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * size.width + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    }

    #[test]
    fn cascader_creation_defaults() {
        let c = Cascader::new(Rect::new(0, 0, 200, 32));
        assert_eq!(c.kind(), WidgetKind::Cascader);
        assert!(c.options().is_empty());
        assert!(c.selected_path().is_empty());
        assert!(!c.is_expanded());
        assert_eq!(c.separator(), "/");
        assert!(!c.is_filterable());
        assert_eq!(c.display_text(), "");
    }

    #[test]
    fn the_closed_field_placeholder_is_configurable() {
        // I-11: the closed field painted a hardcoded "Select…".
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        assert_eq!(c.placeholder(), "Select…");
        c.set_placeholder("Pick a region");
        assert_eq!(c.placeholder(), "Pick a region");
    }

    #[test]
    fn a_filter_with_no_matches_still_has_a_row_to_paint() {
        // I-13: the empty state is a row of its own, so the column is one row tall even when the
        // filter keeps nothing.
        let mut c = cascader();
        c.set_filterable(true);
        c.expand();
        c.filter = String::from("zzzz");
        c.invalidate_level_rows();
        assert!(c.filtered_indices(0).is_empty());
        assert!(c.level_rect(0).height >= ROW_HEIGHT);
    }

    #[test]
    fn cascader_resolves_a_path_to_its_option() {
        let c = cascader();
        assert_eq!(c.resolve(&[0]).map(|o| o.label.as_str()), Some("Province 1"));
        // Index 1 at level 1 is the *second* city, so this names Beta's district.
        assert_eq!(c.resolve(&[0, 1, 0]).map(|o| o.label.as_str()), Some("Beta District"));
        assert_eq!(c.resolve(&[0, 0, 0]).map(|o| o.label.as_str()), Some("Alpha District"));
        // Out of range at any level is `None`, not a clamped neighbour.
        assert!(c.resolve(&[9]).is_none());
        assert!(c.resolve(&[0, 9]).is_none());
        assert!(c.resolve(&[]).is_none());
    }

    #[test]
    fn cascader_distinguishes_leaves_from_branches() {
        let c = cascader();
        // A branch is not a value.
        assert!(!c.is_leaf_path(&[0]));
        assert!(!c.is_leaf_path(&[0, 0]));
        assert!(!c.is_leaf_path(&[0, 0, 0]));
        // A leaf terminates the path and is the only thing selectable.
        assert!(c.is_leaf_path(&[0, 0, 0, 0]));
        assert!(c.is_leaf_path(&[1, 0, 0, 1]));
    }

    #[test]
    fn cascader_set_selected_path_refuses_a_branch() {
        let mut c = cascader();
        assert!(!c.set_selected_path(vec![0]), "a branch is not a value");
        assert!(c.selected_path().is_empty(), "a refused path must not be stored");

        assert!(c.set_selected_path(vec![0, 0, 0, 1]));
        assert_eq!(c.selected_path(), &[0, 0, 0, 1]);
    }

    #[test]
    fn cascader_display_text_joins_the_path() {
        let mut c = cascader();
        c.set_selected_path(vec![0, 1, 0, 0]);
        assert_eq!(c.display_text(), "Province 1/Beta City/Beta District/Beta D1");
        assert_eq!(
            c.selected_labels(),
            vec!["Province 1", "Beta City", "Beta District", "Beta D1"]
        );

        c.set_separator(" › ");
        assert_eq!(c.display_text(), "Province 1 › Beta City › Beta District › Beta D1");

        // An empty separator concatenates, which is what a convention without a
        // glyph between parts needs.
        c.set_separator("");
        assert_eq!(c.display_text(), "Province 1Beta CityBeta DistrictBeta D1");
    }

    #[test]
    fn cascader_selection_changed_signal_carries_the_path() {
        let mut c = cascader();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<usize>>::new()));
        let sink = seen.clone();
        c.selection_changed.connect(move |path| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*path).clone());
            }
        });

        c.set_selected_path(vec![0, 0, 0, 0]);
        c.set_selected_path(vec![1, 0, 0, 1]);
        // Setting the same path again is not a change.
        c.set_selected_path(vec![1, 0, 0, 1]);

        assert_eq!(
            *seen.lock().expect("signal lock poisoned"),
            vec![vec![0, 0, 0, 0], vec![1, 0, 0, 1]]
        );
    }

    #[test]
    fn cascader_clear_selected_emits_an_empty_path() {
        let mut c = cascader();
        c.set_selected_path(vec![0, 0, 0, 0]);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<usize>>::new()));
        let sink = seen.clone();
        c.selection_changed.connect(move |path| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*path).clone());
            }
        });
        c.clear_selected();
        assert!(c.selected_path().is_empty());
        assert_eq!(*seen.lock().expect("signal lock poisoned"), vec![Vec::<usize>::new()]);
        // Clearing an already-empty selection is not a change.
        c.clear_selected();
        assert_eq!(seen.lock().expect("signal lock poisoned").len(), 1);
    }

    #[test]
    fn cascader_set_options_drops_a_selection_that_no_longer_resolves() {
        let mut c = cascader();
        assert!(c.set_selected_path(vec![1, 0, 0, 1]));
        // A tree with no level-1 sibling at index 1 makes the old path address a
        // different node, so keeping it would report a selection never made.
        c.set_options(vec![CascaderOption::branch("only", "Only", Vec::new())]);
        assert!(c.selected_path().is_empty());
    }

    #[test]
    fn cascader_set_options_keeps_a_still_valid_selection() {
        let mut c = cascader();
        assert!(c.set_selected_path(vec![0, 0, 0, 0]));
        c.set_options(tree());
        assert_eq!(c.selected_path(), &[0, 0, 0, 0], "the same tree still resolves");
    }

    // ── Interaction ─────────────────────────────────────────────────────────

    #[test]
    fn cascader_click_on_the_field_toggles_the_overlay() {
        let mut c = cascader();
        c.handle_event(&Event::mouse_press(50, 16, 1));
        assert!(c.is_expanded(), "first click opens");
        c.handle_event(&Event::mouse_press(50, 16, 1));
        assert!(!c.is_expanded(), "second click closes");
    }

    #[test]
    fn cascader_clicking_a_branch_reveals_the_next_level() {
        let mut c = cascader();
        c.expand();
        // One column on screen: level 0. `open_depth()` counts walked nodes, and
        // `visible_level_count()` counts columns; a fresh overlay has 0 and 1.
        assert_eq!(c.open_depth(), 0, "nothing walked yet");
        assert_eq!(c.visible_level_count(), 1, "only level 0 is on screen");

        // Click the first row of level 0.
        let level = c.level_rect(0);
        c.handle_event(&Event::mouse_press(level.x + 10, level.y + 10, 1));
        assert_eq!(c.expanded_path, vec![0], "the branch was walked");
        assert_eq!(c.visible_level_count(), 2, "a branch revealed the next column");
        assert!(c.is_expanded());
        assert!(c.selected_path().is_empty(), "a branch is not committed");

        // And the next level's options are the chosen branch's children.
        let visible = c.visible_options_at(1);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].label, "Alpha City");
    }

    #[test]
    fn cascader_clicking_a_leaf_commits_and_closes() {
        let mut c = cascader();
        c.expand();
        for level in 0..3 {
            let rect = c.level_rect(level);
            c.handle_event(&Event::mouse_press(rect.x + 10, rect.y + 10, 1));
        }
        assert!(c.is_expanded(), "still browsing at level 3");
        // The fourth row click lands on a leaf.
        let rect = c.level_rect(3);
        c.handle_event(&Event::mouse_press(rect.x + 10, rect.y + 10, 1));

        assert!(!c.is_expanded(), "committing closes the overlay");
        assert_eq!(c.selected_path(), &[0, 0, 0, 0]);
        assert_eq!(c.display_text(), "Province 1/Alpha City/Alpha District/Alpha D1");
    }

    #[test]
    fn cascader_choosing_from_an_earlier_level_discards_the_deeper_path() {
        let mut c = cascader();
        c.expand();
        // Walk two levels down the first branch: each click at the deepest visible
        // column adds one node to the path.
        let l0 = c.level_rect(0);
        c.handle_event(&Event::mouse_press(l0.x + 10, l0.y + 10, 1));
        let l1 = c.level_rect(1);
        c.handle_event(&Event::mouse_press(l1.x + 10, l1.y + 10, 1));
        assert_eq!(c.expanded_path, vec![0, 0]);

        // Now click a different row at level 0: the deeper choice is discarded
        // rather than appended to, so the path returns to length one.
        let l0 = c.level_rect(0);
        c.handle_event(&Event::mouse_press(l0.x + 10, l0.y + 10 + ROW_HEIGHT as i32, 1));
        assert_eq!(c.expanded_path, vec![1], "the deeper choice was discarded");
        assert_eq!(c.visible_level_count(), 2, "two columns remain on screen");
        assert_eq!(c.visible_options_at(1)[0].label, "Gamma City");
    }

    #[test]
    fn cascader_escape_closes_without_committing() {
        let mut c = cascader();
        c.expand();
        let l0 = c.level_rect(0);
        c.handle_event(&Event::mouse_press(l0.x + 10, l0.y + 10, 1));
        assert_eq!(c.expanded_path.len(), 1);

        c.handle_event(&Event::KeyDown((27, 0)));
        assert!(!c.is_expanded());
        assert!(c.selected_path().is_empty(), "cancel is not a selection");
    }

    #[test]
    fn cascader_collapse_drops_the_browsed_path_but_not_the_value() {
        let mut c = cascader();
        c.set_selected_path(vec![0, 0, 0, 0]);
        c.expand();
        let l0 = c.level_rect(0);
        c.handle_event(&Event::mouse_press(l0.x + 10, l0.y + 10 + ROW_HEIGHT as i32, 1));
        assert_eq!(c.expanded_path.len(), 1);

        c.collapse();
        assert!(!c.is_expanded());
        assert_eq!(c.selected_path(), &[0, 0, 0, 0], "the committed value survives");
    }

    #[test]
    fn cascader_reopening_resumes_at_the_selection() {
        let mut c = cascader();
        c.set_selected_path(vec![1, 0, 0, 1]);
        c.expand();
        assert_eq!(
            c.expanded_path,
            vec![1, 0, 0, 1],
            "opening resumes where the selection left off, not at the root"
        );
        // The deepest walked node is a leaf, so no further column is revealed.
        assert_eq!(c.visible_level_count(), 5);
    }

    #[test]
    fn cascader_click_outside_closes() {
        let mut c = cascader();
        c.expand();
        // Far right and below every column.
        c.handle_event(&Event::mouse_press(5000, 5000, 1));
        assert!(!c.is_expanded());
    }

    #[test]
    fn cascader_arrow_keys_move_within_the_visible_rows() {
        let mut c = cascader();
        c.expand();
        // Nothing is walked on level 0 yet, so the first Down *enters* the column
        // at row 0 rather than skipping to row 1.
        c.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(c.expanded_path, vec![0]);
        // A second Down moves within the column.
        c.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(c.expanded_path, vec![1]);
        // Clamped at the end: level 0 has two rows.
        c.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(c.expanded_path, vec![1]);
        c.handle_event(&Event::KeyDown((38, 0)));
        assert_eq!(c.expanded_path, vec![0]);
    }

    #[test]
    fn cascader_arrow_keys_do_nothing_when_closed() {
        let mut c = cascader();
        c.handle_event(&Event::KeyDown((40, 0)));
        assert!(c.expanded_path.is_empty());
        assert!(c.selected_path().is_empty());
    }

    #[test]
    fn cascader_disabled_ignores_clicks() {
        let mut c = cascader();
        c.set_enabled(false);
        c.handle_event(&Event::mouse_press(50, 16, 1));
        assert!(!c.is_expanded());
    }

    // ── Filtering ───────────────────────────────────────────────────────────

    #[test]
    fn cascader_filter_narrows_the_browsed_level() {
        let mut c = cascader();
        c.set_filterable(true);
        c.expand();
        // Both provinces match an empty filter.
        assert_eq!(c.filtered_indices(0), vec![0, 1]);

        c.filter = "2".to_string();
        assert_eq!(c.filtered_indices(0), vec![1], "only Province 2 matches");
    }

    #[test]
    fn cascader_filter_does_not_apply_to_unbrowsed_levels() {
        let mut c = cascader();
        c.set_filterable(true);
        c.expand();
        // Level 0 is the browsed level (nothing walked yet), so it is untouched.
        assert_eq!(c.filtered_indices(0), vec![0, 1]);

        // Walk into level 0, which makes level 1 the browsed one. The filter now
        // applies there, while level 0 stays full — this is what keeps the rows
        // that lead to a match from being hidden.
        let l0 = c.level_rect(0);
        c.handle_event(&Event::mouse_press(l0.x + 10, l0.y + 10, 1));
        c.filter = "Beta".to_string();
        assert_eq!(c.filtered_indices(0), vec![0, 1], "level 0 is not browsed");
        assert_eq!(c.filtered_indices(1), vec![1], "only Beta City matches at level 1");
    }

    #[test]
    fn cascader_same_value_filterable_keeps_the_filter() {
        let mut c = cascader();
        c.set_filterable(true);
        c.filter = "2".to_string();

        // Re-applying the current flag is a no-op: the filter the user typed survives (WDGT-15).
        c.set_filterable(true);
        assert_eq!(c.filter(), "2");

        // A real change still resets it.
        c.set_filterable(false);
        assert_eq!(c.filter(), "");
    }

    #[test]
    fn cascader_filter_off_ignores_typing() {
        let mut c = cascader();
        c.expand();
        // Not filterable, so text input must not accumulate.
        c.handle_event(&Event::TextInput { text: "x".to_string() });
        assert_eq!(c.filter(), "");
    }

    /// An IME commit reaches the filter through the same editable branch as typed text.
    ///
    /// # The defect this pins
    ///
    /// The filter matched only `TextInput`, so an input method's committed string was dropped
    /// and a filterable cascader could not be filtered from a CJK keyboard. The guard is the
    /// same one the typed path uses: only a filterable, open overlay has an editable filter to
    /// receive the text.
    #[test]
    fn cascader_ime_commit_filters_the_browsed_level() {
        let mut c = cascader();
        c.set_filterable(true);
        c.expand();

        c.handle_event(&Event::ime_commit("2"));
        assert_eq!(c.filter(), "2", "the commit must reach the filter");
        assert_eq!(c.filtered_indices(0), vec![1], "only Province 2 matches");

        // The guard is shared with `TextInput`: a closed picker has no filter to type into.
        let mut closed = cascader();
        closed.set_filterable(true);
        closed.handle_event(&Event::ime_commit("2"));
        assert_eq!(closed.filter(), "", "a closed cascader must not accumulate a filter");
    }

    #[test]
    fn cascader_choosing_clears_the_filter() {
        let mut c = cascader();
        c.set_filterable(true);
        c.expand();
        c.filter = "1".to_string();
        let l0 = c.level_rect(0);
        c.handle_event(&Event::mouse_press(l0.x + 10, l0.y + 10, 1));
        assert_eq!(c.filter(), "", "a fresh level starts unfiltered");
    }

    // ── Drawing ─────────────────────────────────────────────────────────────

    #[test]
    fn cascader_draw_closed_paints_without_panicking() {
        let mut c = cascader();
        let rgba = render(&mut c, Size::new(200, 32));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn cascader_open_overlay_differs_from_the_closed_frame() {
        let mut c = cascader();
        let closed = render(&mut c, Size::new(400, 200));
        c.expand();
        let open = render(&mut c, Size::new(400, 200));
        assert_ne!(closed, open, "opening the overlay must be visible");
    }

    #[test]
    fn cascader_draw_zero_geometry_does_not_panic() {
        let mut c = cascader();
        c.expand();
        let rgba = render(&mut c, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn cascader_placeholder_differs_from_a_value() {
        let mut c = cascader();
        let placeholder = render(&mut c, Size::new(200, 32));
        c.set_selected_path(vec![0, 0, 0, 0]);
        let with_value = render(&mut c, Size::new(200, 32));
        assert_ne!(placeholder, with_value, "a value must look different from the hint");
    }

    /// The open overlay column follows the appearance, not just the closed field.
    ///
    /// # The defect this pins
    ///
    /// `draw_level` painted `Color::WHITE` for the column and three fixed greys for its ink and
    /// separators, so nothing the theme said reached the list a dark build opens — the same
    /// "half themed, half literal" shape `dropdown` had. The field was themed because it reads
    /// `style.*`; the popup hung off it was not.
    ///
    /// # Why the assertion names the column's own pixel
    ///
    /// Comparing the two whole frames would pass even with `Color::WHITE` restored, because the
    /// field's own fill and the backdrop still differ between appearances — a document-wide or
    /// frame-wide comparison is satisfied by a colour this test is not about. The sample point is
    /// inside the **overlay column** (below the field's band) and away from any glyph, so the pixel
    /// it reads is the column surface and nothing else.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_open_overlay_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let size = Size::new(400, 200);

        let column_pixel = |appearance| -> [u8; 4] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let backdrop = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let mut c = cascader();
            c.expand();
            crate::theme::apply_theme_to_widget(&mut c);
            let frame = render_on(&mut c, size, backdrop);
            // The column's **interior**, well clear of any rounded corner: a point on a corner's
            // antialiased arc reads the *border*, which is themed for a different reason and, when
            // this sample was at the bottom-right pixel, made the assertion below pass even with the
            // fill restored to `Color::WHITE` (measured). Half the column's width and a y below the
            // last row's text are both unambiguously the column surface.
            let x = LEVEL_WIDTH / 2;
            let y = 32 + 2 * ROW_HEIGHT - 4;
            pixel(&frame, size, x, y)
        };

        let dark = column_pixel(crate::theme::AppearanceMode::Dark);
        let light = column_pixel(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "the overlay column must follow the appearance; both were {dark:?}"
        );
        assert_ne!(
            dark,
            [255, 255, 255, 255],
            "the overlay column must not be the fixed white the defect used"
        );
    }

    // ── Property contract ───────────────────────────────────────────────────

    #[test]
    fn cascader_properties_round_trip() {
        let mut c = cascader();
        assert_eq!(c.get("expanded").unwrap(), CapabilityValue::Bool(false));
        c.set("expanded", CapabilityValue::Bool(true)).unwrap();
        assert!(c.is_expanded());

        c.set("separator", CapabilityValue::String(" :: ".to_string())).unwrap();
        assert_eq!(c.separator(), " :: ");

        c.set("filterable", CapabilityValue::Bool(true)).unwrap();
        assert!(c.is_filterable());

        // The per-level row cap is nullable, and a cap of zero floors to one row.
        assert_eq!(c.get("max_visible").unwrap(), CapabilityValue::Null);
        c.set("max_visible", CapabilityValue::UInt(0)).unwrap();
        assert_eq!(c.max_visible(), Some(1));
        c.set("max_visible", CapabilityValue::UInt(3)).unwrap();
        assert_eq!(c.get("max_visible").unwrap(), CapabilityValue::UInt(3));
        c.set("max_visible", CapabilityValue::Null).unwrap();
        assert_eq!(c.max_visible(), None);

        // Wrong types are refused rather than coerced.
        assert!(c.set("expanded", CapabilityValue::UInt(1)).is_err());
        assert!(c.set("separator", CapabilityValue::Bool(true)).is_err());
    }

    #[test]
    fn cascader_derived_properties_are_read_only() {
        let mut c = cascader();
        c.set_selected_path(vec![0, 0, 0, 0]);
        assert_eq!(c.get("depth").unwrap(), CapabilityValue::UInt(4));
        assert_eq!(
            c.get("value").unwrap(),
            CapabilityValue::String("Province 1/Alpha City/Alpha District/Alpha D1".to_string())
        );
        for name in ["value", "depth", "filter"] {
            assert_eq!(
                c.set(name, CapabilityValue::UInt(1)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
    }

    // ── Asynchronous level loading ─────────────────────────

    /// A tree whose single branch has not been fetched yet.
    fn pending_tree() -> Vec<CascaderOption> {
        vec![CascaderOption::loading("remote", "Remote")]
    }

    #[test]
    fn a_loading_branch_is_not_a_selectable_value() {
        // The distinction this feature exists for: a pending branch and a leaf both have no
        // children in the data, but only one of them is an answer. If `is_leaf` ignored the
        // flag the user could commit a category that holds nothing.
        let option = CascaderOption::loading("r", "Remote");
        assert!(option.children.is_empty());
        assert!(!option.is_leaf(), "a pending branch is not a value");
        assert!(option.is_loading());
    }

    #[test]
    fn a_loading_path_is_not_committable() {
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        assert!(c.is_loading_path(&[0]));
        assert!(!c.is_leaf_path(&[0]));
        assert!(!c.set_selected_path(vec![0]), "a pending branch must not commit");
        assert_eq!(c.selected_path(), &[] as &[usize]);
    }

    #[test]
    fn opening_a_pending_branch_asks_the_host_for_its_children() {
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        let requested = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        c.load_requested.connect({
            let requested = std::sync::Arc::clone(&requested);
            move |path: std::sync::Arc<Vec<usize>>| {
                *requested.lock().expect("request list") = (*path).clone();
            }
        });
        c.expand();
        assert!(c.choose_at(0, 0));
        assert_eq!(*requested.lock().expect("request list"), vec![0]);
    }

    #[test]
    fn completing_a_load_makes_the_branch_selectable() {
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        assert!(c.complete_load(
            &[0],
            vec![CascaderOption::new("r-a", "A"), CascaderOption::new("r-b", "B"),]
        ));
        assert!(!c.is_loading_path(&[0]), "the flag must clear on success");
        // The children are read from the tree rather than from `visible_options_at`: that
        // accessor walks the *browsed* path, so a collapsed control reports no level 1 even
        // though the data is present.
        assert_eq!(c.resolve(&[0]).map(|o| o.children.len()), Some(2));
        assert!(c.choose_at(0, 0), "the branch is openable now");
        assert_eq!(c.visible_options_at(1).len(), 2, "the fetched level is browsable");
        assert!(c.set_selected_path(vec![0, 1]), "a fetched leaf is committable now");
        assert_eq!(c.display_text(), "Remote/B");
    }

    #[test]
    fn an_empty_load_marks_the_branch_failed_not_leaf() {
        // A fetch that returned nothing has not produced a value. Treating it as a leaf
        // would silently turn a failed request into a selectable empty category.
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        assert!(c.fail_load(&[0]));
        let option = c.resolve(&[0]).expect("the branch still exists");
        assert_eq!(option.load_state, CascaderLoadState::Failed);
        assert!(!option.is_leaf(), "a failed branch is still not a value");
        assert!(!option.is_loading(), "it is no longer waiting");
        assert!(!c.set_selected_path(vec![0]));
    }

    #[test]
    fn completing_a_load_twice_is_refused() {
        // Accepting the second write would discard a subtree the caller may still be
        // holding indices into.
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        assert!(c.complete_load(&[0], vec![CascaderOption::new("x", "X")]));
        assert!(!c.complete_load(&[0], vec![CascaderOption::new("y", "Y")]));
        assert_eq!(c.resolve(&[0]).map(|o| o.children.len()), Some(1), "the first wins");
        assert_eq!(c.resolve(&[0, 0]).map(|o| o.label.as_str()), Some("X"));
    }

    #[test]
    fn completing_a_synchronous_leaf_is_refused() {
        // A leaf is a value; supplying children after the fact would turn it into a branch
        // behind the caller's back, invalidating any selection already made on it.
        let mut c = cascader();
        assert!(!c.complete_load(&[0, 0, 0, 0], vec![CascaderOption::new("z", "Z")]));
        assert_eq!(c.resolve(&[0, 0, 0, 0]).map(|o| o.children.len()), Some(0));
    }

    #[test]
    fn completing_an_unknown_path_is_refused() {
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        assert!(!c.complete_load(&[9], vec![CascaderOption::new("x", "X")]));
        assert!(!c.complete_load(&[], vec![CascaderOption::new("x", "X")]));
        assert!(c.is_loading_path(&[0]), "the pending branch is untouched");
    }

    #[test]
    fn adding_a_child_to_a_pending_branch_clears_the_flag() {
        // A branch that has children is by definition not still waiting for them.
        let option = CascaderOption::loading("r", "Remote").child(CascaderOption::new("a", "A"));
        assert!(!option.is_loading());
        assert!(!option.is_leaf(), "it has children, so it is a branch");
    }

    #[test]
    fn a_pending_branch_draws_an_ellipsis_not_a_chevron() {
        // Pixel assertion: the pending marker is drawn in a dimmer colour than the label,
        // so the row cannot look like an openable branch with nothing behind it.
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        c.expand();
        let pending_frame = render(&mut c, Size::new(400, 240));

        let mut loaded = Cascader::new(Rect::new(0, 0, 200, 32));
        loaded.set_options(vec![CascaderOption::branch(
            "r",
            "Remote",
            vec![CascaderOption::new("a", "A")],
        )]);
        loaded.expand();
        let loaded_frame = render(&mut loaded, Size::new(400, 240));

        assert_ne!(
            pending_frame, loaded_frame,
            "a pending branch must not render identically to a loaded one"
        );
        assert!(
            pending_frame.iter().any(|&b| b != 255),
            "the pending frame must actually paint something"
        );
    }

    #[test]
    fn expanding_into_a_pending_branch_leaves_the_next_column_empty() {
        // The honest answer to "what is under here": nothing yet. The row stays put so
        // the tree does not jump when the children arrive.
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(pending_tree());
        c.expand();
        assert!(c.choose_at(0, 0));
        assert!(c.is_expanded(), "it is a branch, so the overlay stays open");
        assert!(c.visible_options_at(1).is_empty());
        assert_eq!(c.selected_path(), &[] as &[usize], "nothing was committed");
    }

    /// A disabled row is skipped by the keyboard and refused by a programmatic write.
    #[test]
    fn disabled_rows_are_not_choosable() {
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(vec![
            CascaderOption::new("a", "A").with_enabled(false),
            CascaderOption::new("b", "B"),
        ]);
        c.expand();
        c.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        c.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(c.selected_labels(), vec!["B"]);
        assert!(!c.set_selected_path(vec![0]), "a disabled leaf is refused");
    }

    /// Right/Left walk levels, Enter commits, Home/End jump within a level.
    #[test]
    fn keyboard_walks_levels_and_commits() {
        let mut c = cascader();
        c.expand();
        c.handle_event(&Event::KeyPress { key: 40, modifiers: 0 }); // first branch
        c.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        c.handle_event(&Event::KeyPress { key: 39, modifiers: 0 }); // into the district
        assert!(c.is_expanded());
        c.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter on a branch
        assert!(c.is_expanded(), "a branch is not committable");
        c.handle_event(&Event::KeyPress { key: 39, modifiers: 0 }); // into the leaf
        assert!(!c.is_expanded());
        assert_eq!(c.selected_path().len(), 4);

        c.expand();
        c.handle_event(&Event::KeyPress { key: 35, modifiers: 0 }); // End
        c.handle_event(&Event::KeyPress { key: 37, modifiers: 0 }); // Left retreats
        assert!(c.is_expanded());
    }

    /// A capped level scrolls with the wheel and paints a window.
    #[test]
    fn a_capped_level_scrolls_with_the_wheel() {
        let options: Vec<CascaderOption> = (0..8)
            .map(|index| CascaderOption::new(format!("o{index}"), format!("Option {index}")))
            .collect();
        let mut c = Cascader::new(Rect::new(0, 0, 200, 32));
        c.set_options(options);
        c.set_max_visible(Some(3));
        c.expand();
        assert_eq!(c.page_size(8), 3);
        c.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(c.scroll_offset(0), 1);
        assert!(c.tick(0), "the reveal is still in progress");
        c.tick(16);
        c.tick(200);
        assert_eq!(c.scroll_offset(0), 1);
    }

    // ── Multi-select (I-122) ────────────────────────────────────────────────

    /// Every set emission seen by the recorder, in order.
    type RecordedSets = std::sync::Arc<std::sync::Mutex<Vec<Vec<Vec<usize>>>>>;

    /// Records every `selection_set_changed` emission, so "once per toggle" is observable.
    fn record_set_changes(c: &mut Cascader) -> RecordedSets {
        let seen: RecordedSets = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        c.selection_set_changed.connect(move |paths| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*paths).clone());
            }
        });
        seen
    }

    /// Walks to the leaf at `[0, 0, 0]`'s column, leaving the path on the district branch.
    fn browse_to_the_last_column(c: &mut Cascader) {
        for level in 0..3 {
            assert!(c.choose_at(level, 0), "level {level} is a branch and navigates");
        }
    }

    #[test]
    fn multi_select_toggles_a_leaf_and_untoggles_it() {
        let mut c = cascader();
        c.set_multi_select(true);
        assert!(c.is_multi_select());
        c.expand();
        let seen = record_set_changes(&mut c);

        browse_to_the_last_column(&mut c);
        assert!(c.choose_at(3, 0), "the leaf toggles on");
        assert_eq!(c.selected_paths(), &[vec![0, 0, 0, 0]]);
        assert!(c.is_expanded(), "multi-select keeps the overlay open");
        assert!(c.selected_path().is_empty(), "the single-select value is not replaced");
        assert_eq!(c.expanded_path, vec![0, 0, 0], "a leaf adds no column");

        assert!(c.choose_at(3, 1), "a second leaf toggles on");
        assert_eq!(c.selected_paths(), &[vec![0, 0, 0, 0], vec![0, 0, 0, 1]]);

        assert!(c.choose_at(3, 0), "the first leaf toggles off");
        assert_eq!(c.selected_paths(), &[vec![0, 0, 0, 1]]);

        assert_eq!(
            *seen.lock().expect("signal lock poisoned"),
            vec![
                vec![vec![0, 0, 0, 0]],
                vec![vec![0, 0, 0, 0], vec![0, 0, 0, 1]],
                vec![vec![0, 0, 0, 1]],
            ],
            "one emission per toggle, each carrying the whole set"
        );
    }

    #[test]
    fn multi_select_branch_rows_only_navigate() {
        let mut c = cascader();
        c.set_multi_select(true);
        c.expand();
        let seen = record_set_changes(&mut c);

        assert!(c.choose_at(0, 0), "a province is a branch");
        assert!(c.choose_at(1, 1), "a city is a branch");
        assert!(c.choose_at(2, 0), "a district is a branch");
        assert_eq!(c.expanded_path, vec![0, 1, 0], "each branch walked one level");
        assert!(c.is_expanded());
        assert!(c.selected_paths().is_empty(), "a branch is not a value in the set");
        assert!(c.selected_path().is_empty());
        assert!(
            seen.lock().expect("signal lock poisoned").is_empty(),
            "navigation must not emit a set change"
        );

        assert!(c.choose_at(0, 1), "choosing another branch keeps the overlay open");
        assert_eq!(c.expanded_path, vec![1], "the deeper path was discarded");
        assert!(c.selected_paths().is_empty());
    }

    #[test]
    fn single_select_mode_is_the_unchanged_default() {
        let mut c = cascader();
        assert!(!c.is_multi_select());
        assert!(c.selected_paths().is_empty());
        let seen = record_set_changes(&mut c);

        c.expand();
        browse_to_the_last_column(&mut c);
        assert!(c.choose_at(3, 0));

        assert!(!c.is_expanded(), "single-select still commits and closes");
        assert_eq!(c.selected_path(), &[0, 0, 0, 0]);
        assert!(c.selected_paths().is_empty(), "the set is unused in single-select");
        assert!(seen.lock().expect("signal lock poisoned").is_empty());
    }

    #[test]
    fn set_selected_paths_replaces_the_set_atomically() {
        let mut c = cascader();
        c.set_multi_select(true);

        // Duplicates collapse to their first occurrence; the caller's order is kept.
        assert!(c.set_selected_paths(vec![vec![0, 0, 0, 1], vec![1, 0, 0, 0], vec![0, 0, 0, 1],]));
        assert_eq!(c.selected_paths(), &[vec![0, 0, 0, 1], vec![1, 0, 0, 0]]);

        // Any non-leaf entry refuses the whole write, leaving the previous set untouched.
        assert!(!c.set_selected_paths(vec![vec![0, 0, 0, 0], vec![0]]));
        assert_eq!(c.selected_paths(), &[vec![0, 0, 0, 1], vec![1, 0, 0, 0]]);
        assert!(!c.set_selected_paths(vec![vec![9]]));
        assert_eq!(c.selected_paths(), &[vec![0, 0, 0, 1], vec![1, 0, 0, 0]]);

        // An empty list clears the set.
        assert!(c.set_selected_paths(Vec::new()));
        assert!(c.selected_paths().is_empty());
    }

    #[test]
    fn set_options_prunes_paths_that_no_longer_resolve() {
        let mut c = cascader();
        c.set_multi_select(true);
        assert!(c.set_selected_paths(vec![vec![0, 0, 0, 0]]));
        // The replacement tree has no level 1 at all, so the stored indices would address
        // a different node; dropping the entry is the only answer that cannot lie.
        c.set_options(vec![CascaderOption::branch("only", "Only", Vec::new())]);
        assert!(c.selected_paths().is_empty());
    }

    #[test]
    fn clear_selected_empties_the_set_in_multi_select_mode() {
        let mut c = cascader();
        c.set_multi_select(true);
        assert!(c.set_selected_paths(vec![vec![0, 0, 0, 0], vec![0, 0, 0, 1]]));
        let seen = record_set_changes(&mut c);
        c.clear_selected();
        assert!(c.selected_paths().is_empty());
        assert_eq!(*seen.lock().expect("signal lock poisoned"), vec![Vec::<Vec<usize>>::new()]);
        // Clearing an already-empty set is not a change.
        c.clear_selected();
        assert_eq!(seen.lock().expect("signal lock poisoned").len(), 1);
    }
}
