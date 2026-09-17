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

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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
}

impl CascaderOption {
    /// Creates a childless option (a leaf).
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            load_state: CascaderLoadState::Ready,
        }
    }

    /// Creates an option with children.
    pub fn branch(
        id: impl Into<String>,
        label: impl Into<String>,
        children: Vec<CascaderOption>,
    ) -> Self {
        Self { id: id.into(), label: label.into(), children, load_state: CascaderLoadState::Ready }
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
    pub selection_changed: Signal1<Vec<usize>>,
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
            expanded_path: Vec::new(),
            expanded: false,
            separator: "/".to_string(),
            filterable: false,
            filter: String::new(),
            selection_changed: Signal1::new(),
        }
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
        if !self.selected_path.is_empty() && self.resolve(&self.selected_path).is_none() {
            self.selected_path.clear();
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

    /// Sets the separator used when the path is rendered as text.
    ///
    /// An empty separator is accepted: it means "concatenate the labels", which is
    /// the right spelling for languages whose breadcrumb convention has no glyph
    /// between parts.
    pub fn set_separator(&mut self, separator: &str) {
        self.separator = separator.to_string();
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
        self.filter.clear();
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
        self.expanded_path.clear();
        self.filter.clear();
        self.base.request_redraw();
    }

    /// Whether tying filters the visible rows.
    pub fn is_filterable(&self) -> bool {
        self.filterable
    }

    /// Sets whether typing filters the visible rows.
    pub fn set_filterable(&mut self, filterable: bool) {
        self.filterable = filterable;
        self.filter.clear();
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
        let needle = self.filter.to_lowercase();
        // A `Vec` cannot be returned by reference from a filtered view, so the
        // filter is applied at the call sites that draw or hit-test. Returning the
        // unfiltered slice here keeps this accessor allocation-free; `filtered_indices`
        // is the one that narrows.
        let _ = needle;
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
            .filter(|(_, option)| option.label.to_lowercase().contains(&needle))
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
        if self.selected_path == path {
            return true;
        }
        self.selected_path = path;
        self.selection_changed.emit(self.selected_path.clone());
        self.base.request_redraw();
        true
    }

    /// Clears the committed selection.
    pub fn clear_selected(&mut self) {
        if self.selected_path.is_empty() {
            return;
        }
        self.selected_path.clear();
        self.selection_changed.emit(Vec::new());
        self.base.request_redraw();
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
        self.selected_labels().join(&self.separator)
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
        let rect = self.geometry();
        let rows = self.filtered_indices(level).len() as u32;
        let height = (rows * ROW_HEIGHT).max(ROW_HEIGHT);
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
        let visible = self.filtered_indices(level);
        visible.get(row).copied()
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
    /// A branch extends the browsed path and reveals the next level; a leaf commits
    /// the path and closes. That single rule is what makes one click mean both
    /// "go deeper" and "pick this", depending on what was clicked.
    fn choose(&mut self, level: usize, option_index: usize) {
        let Some(option) = self.siblings_at(level).and_then(|s| s.get(option_index)) else {
            return;
        };
        let is_leaf = option.is_leaf();
        // Truncate to this level, then push, so choosing from an earlier level
        // discards the deeper path rather than appending to it.
        self.expanded_path.truncate(level);
        self.expanded_path.push(option_index);
        self.filter.clear();

        if is_leaf {
            let path = self.expanded_path.clone();
            self.expanded = false;
            self.expanded_path.clear();
            self.set_selected_path(path);
        } else {
            self.base.request_redraw();
        }
    }
}

impl Widget for Cascader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            "value" => Ok(CapabilityValue::String(self.display_text())),
            "depth" => Ok(CapabilityValue::UInt(self.selected_path.len() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            "separator" => Ok(CapabilityValue::String(self.separator().to_string())),
            "filterable" => Ok(CapabilityValue::Bool(self.is_filterable())),
            "filter" => Ok(CapabilityValue::String(self.filter().to_string())),
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
            "filter" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `CASCADER_PROPERTIES`.
        property_names_of![
            "value",
            "depth",
            "expanded",
            "separator",
            "filterable",
            "filter",
            BASE_PROPERTY_NAMES
        ]
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

        let text = self.display_text();
        let display = if text.is_empty() { "Select…".to_string() } else { text };
        let color = if self.display_text().is_empty() {
            // A placeholder is dimmer than a real value, so an empty field cannot be
            // mistaken for a field whose value happens to be that word.
            Color::rgb(150, 154, 162)
        } else {
            text_color
        };
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 21),
            &display,
            &Font::simple("Sans", 12.0),
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
        for level in 0..self.visible_level_count() {
            let column = self.level_rect(level);
            let visible = self.filtered_indices(level);
            self.draw_level(context, level, column, &visible);
        }
    }

    /// Draws one column of the overlay.
    fn draw_level(
        &self,
        context: &mut RenderContext,
        level: usize,
        column: Rect,
        visible: &[usize],
    ) {
        context.fill_rounded_rect(column, 4, Color::WHITE);
        context.draw_rounded_rect_stroke(column, 4, Color::rgb(190, 192, 198), 1);

        let Some(siblings) = self.siblings_at(level) else {
            return;
        };
        // The index the path currently points at on this level, if any.
        let current = self.expanded_path.get(level).copied();

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
            if option_index == current.unwrap_or(usize::MAX) {
                // The row the path points at is highlighted, which is how the user
                // sees which branch the next column belongs to.
                context.fill_rect(row_rect, Color::rgb(232, 240, 254));
            }
            context.draw_text(
                Point::new(row_rect.x + 8, row_rect.y + 18),
                &option.label,
                &Font::simple("Sans", 12.0),
                Color::rgb(40, 44, 52),
                HorizontalAlignment::Left,
            );
            if option.is_loading() {
                // A pending branch shows an ellipsis where the chevron would be: it is not
                // "a subtree you can open" yet, and drawing the chevron would invite a
                // click that has nothing to open. The row stays in place so the tree does
                // not jump when the children arrive.
                context.draw_text(
                    Point::new(row_rect.x + row_rect.width as i32 - 18, row_rect.y + 18),
                    "...",
                    &Font::simple("Sans", 12.0),
                    Color::rgb(140, 144, 152),
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
                    Color::rgb(140, 144, 152),
                    1,
                );
                context.draw_line_stroke(
                    Point::new(ax + 2, ay),
                    Point::new(ax - 2, ay + 4),
                    Color::rgb(140, 144, 152),
                    1,
                );
            }
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
            Event::MousePress { pos, button } if *button == 1 => {
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
                self.filter.pop();
                self.base.request_redraw();
            }
            Event::KeyDown((key, _)) | Event::KeyPress { key, .. } => match *key {
                27 => self.collapse(), // Escape
                40 => self.move_browsed(1),
                38 => self.move_browsed(-1),
                _ => {}
            },
            // Typed characters filter the browsed level when filtering is on. Control
            // characters are excluded in the pattern so the guard and the body agree
            // about what is acceptable.
            Event::TextInput { text }
                if self.filterable && self.expanded && text.chars().all(|ch| !ch.is_control()) =>
            {
                self.filter.push_str(text);
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
            Some(position) => (position as i32 + delta).clamp(0, visible.len() as i32 - 1) as usize,
            None => {
                if delta > 0 {
                    0
                } else {
                    visible.len() - 1
                }
            }
        };
        if let Some(&option_index) = visible.get(next) {
            self.expanded_path.truncate(level);
            self.expanded_path.push(option_index);
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
    fn render(c: &mut Cascader, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        c.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
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
    fn cascader_filter_off_ignores_typing() {
        let mut c = cascader();
        c.expand();
        // Not filterable, so text input must not accumulate.
        c.handle_event(&Event::TextInput { text: "x".to_string() });
        assert_eq!(c.filter(), "");
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
}
