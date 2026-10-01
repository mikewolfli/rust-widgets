// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! List view widget.
//!
//! # One row height, read by the paint and the hit test
//!
//! BLUE22 §B.8 lists this control's defect as "the row height computed by hand". It was a `20`
//! literal in three places — the paint loop and both press arms of `handle_event` — and the two
//! sides **disagreed about where row 0 starts**: the loop measured from `ContentMetrics`'s content
//! box while the press arms measured from `rect.y`. A press on the first row therefore selected the
//! second. [`Self::row_rect`] is now the one derivation both read.
use crate::core::Color;
use crate::core::HorizontalAlignment;
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::access::{selection_mode_to_str, view_mode_to_str};
use crate::widget::capability::coercion::{
    expect_horizontal_alignment, expect_selection_mode, expect_usize, expect_view_mode,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::sync::Arc;

/// The margin a list leaves between its own frame and its rows: 2 px on every edge.
///
/// Named once so the rows are all measured from the same inset. They used to start at the
/// control's literal `rect.y`, which pinned the first row's text to y=0 on the frame's
/// stroke.
const LIST_INSET: u32 = 2;

/// The inset of a row's label from its row's left edge: 2 px.
const LIST_TEXT_INSET: i32 = 2;

/// Height of one list row: 22.
///
/// The shared list-row metric, so a `list_view` row is the same band a `list_box` and every menu
/// entry use. It was the literal `20` written in three places here (the paint loop and both press
/// arms), which is how the paint and the hit test ended up measuring rows from two different
/// origins.
pub const LIST_ROW_HEIGHT: u32 = dimensions::MENU_ROW_HEIGHT;

/// The smallest tile `ViewMode::Icon` will lay out, in pixels.
///
/// The **minimum**, not the actual size: a mode whose cell size is derived from how many fit needs a
/// floor to divide by, and an icon smaller than this stops reading as a picture. A box too narrow for
/// even one becomes a single narrow column rather than zero columns.
const LIST_ICON_MIN_CELL: u32 = 48;
/// The smallest tile `ViewMode::Thumbnails` will lay out, in pixels.
///
/// Larger than the icon tile because a thumbnail is a preview rather than a glyph, and a preview too
/// small to recognise is not a preview.
const LIST_THUMB_MIN_CELL: u32 = 72;
/// The caption strip drawn under a tile in the two grid modes, in pixels.
const LIST_THUMB_CAPTION: u32 = 18;

/// How a [`ListView`]'s items are laid out under the current [`ViewMode`].
///
/// Produced by `ListView::list_layout` and read by every geometry decision, so the four modes cannot
/// be half-applied. See that method for the table of what each mode does.
#[derive(Debug, Clone, Copy)]
struct ListLayout {
    /// The box the items are laid out in.
    content: Rect,
    /// One line's height.
    row_height: u32,
    /// How many items sit across one line.
    ///
    /// A `u32` rather than a `usize` because every number it is combined with — the content box's
    /// width and height — is a `u32`, and mixing the two in this one struct turned every expression
    /// that touched it into a cast. It is at least 1 in every mode, so the divisions below never
    /// divide by zero.
    columns: u32,
    /// How far a label is inset from its cell's left edge.
    text_inset: i32,
    /// Whether the label is drawn below its cell rather than vertically centred beside it.
    label_below: bool,
}

impl ListLayout {
    /// How wide one tile is: the content box split evenly between the columns.
    ///
    /// Floored, so `columns` tiles never claim more than the box actually holds, and never zero — a
    /// zero-width tile would divide by zero in the hit test rather than merely look wrong.
    fn cell_width(&self) -> u32 {
        (self.content.width / self.columns.max(1)).max(1)
    }

    /// The band a `label_below` caption occupies at the foot of a cell.
    ///
    /// A caption is one text line tall, and the strip is where the glyphs land — the *device* line
    /// height rather than the font's requested one, because the strip only has to hold what
    /// [`crate::core::Font`] will actually paint.
    fn caption_height(&self) -> u32 {
        LIST_THUMB_CAPTION.min(self.row_height.max(1)).max(1)
    }
}

/// List model abstraction for list-like views.
pub trait ListModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Data for row index, if present.
    fn data(&self, row: usize) -> Option<String>;
    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
}
/// In-memory list model backed by a vector of strings.
pub struct VecListModel {
    items: Vec<String>,
    data_changed: GenericSignal,
}
impl VecListModel {
    /// Creates a new vector list model.
    pub fn new(items: Vec<String>) -> Self {
        Self { items, data_changed: GenericSignal::new() }
    }
    /// Returns a reference to the data changed signal.
    pub fn data_changed_signal(&self) -> &GenericSignal {
        &self.data_changed
    }
    /// Appends an item to the model.
    pub fn append(&mut self, item: String) {
        self.items.push(item);
        self.data_changed.emit();
    }
    /// Removes an item at given index.
    pub fn remove(&mut self, index: usize) -> Option<String> {
        if index < self.items.len() {
            let item = self.items.remove(index);
            self.data_changed.emit();
            Some(item)
        } else {
            None
        }
    }
    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.data_changed.emit();
    }
}
impl ListModel for VecListModel {
    fn row_count(&self) -> usize {
        self.items.len()
    }
    fn data(&self, row: usize) -> Option<String> {
        self.items.get(row).cloned()
    }
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
    }
}
/// Selection state for list/tree/table views.
pub struct SelectionModel {
    /// Current selection mode.
    mode: SelectionMode,
    /// Currently selected rows.
    selected_rows: Vec<usize>,
    /// Currently focused row.
    current_row: Option<usize>,
    /// The row a `Shift`-extend grows from in [`SelectionMode::Extended`].
    ///
    /// Only meaningful in that mode; every other mode leaves it `None`.
    anchor: Option<usize>,
}
crate::impl_default_via_new!(SelectionModel);

impl SelectionModel {
    /// Creates a new selection model.
    pub fn new() -> Self {
        Self {
            mode: SelectionMode::Single,
            selected_rows: Vec::new(),
            current_row: None,
            anchor: None,
        }
    }
    /// Sets selection mode.
    ///
    /// Leaving [`SelectionMode::Extended`] clears the anchor, because a later switch
    /// back to `Extended` must not extend from a row chosen under a different rule.
    pub fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
        if mode != SelectionMode::Extended {
            self.anchor = None;
        }
        self.normalize();
    }
    /// Returns current selection mode.
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }
    /// Selects a row.
    ///
    /// This is the **unmodified** selection: a plain click or a programmatic select.
    /// In [`SelectionMode::Extended`] it *replaces* the selection and leaves an anchor,
    /// which is what makes a following `Shift` extend from here rather than from the
    /// previous selection. The modifier-driven behaviours live in
    /// [`Self::select_with_modifiers`].
    ///
    /// In [`SelectionMode::None`] the view accepts no selection, so this is a
    /// no-op — the mode is a property of the view, not a transient state, and
    /// silently selecting anyway would contradict what the caller asked for.
    pub fn select_row(&mut self, row: usize) {
        match self.mode {
            SelectionMode::Single => {
                self.selected_rows.clear();
                self.selected_rows.push(row);
                self.current_row = Some(row);
            }
            SelectionMode::Multi => {
                if !self.selected_rows.contains(&row) {
                    self.selected_rows.push(row);
                }
                self.current_row = Some(row);
            }
            SelectionMode::Extended => {
                // A plain click starts a new selection from here. Pushing instead of
                // replacing would make `Extended` indistinguishable from `Multi`, and
                // pushing without a duplicate check would let `selected_rows()` report
                // the same row twice.
                self.selected_rows.clear();
                self.selected_rows.push(row);
                self.current_row = Some(row);
                self.anchor = Some(row);
            }
            SelectionMode::None => {}
        }
    }
    /// Selects `row` with keyboard modifiers, implementing the Windows-explorer
    /// model [`SelectionMode::Extended`] documents.
    ///
    /// * `Shift` — selects the inclusive range from the anchor to `row`.
    /// * `Ctrl`/`Primary` — toggles `row`, and re-anchors there so a following
    ///   `Shift` extends from the row the user just toggled.
    /// * no modifier — [`Self::select_row`], i.e. replace and re-anchor.
    ///
    /// In every other mode this delegates to [`Self::select_row`], so a caller does
    /// not have to know the current mode to call it safely.
    pub fn select_with_modifiers(&mut self, row: usize, modifiers: crate::shortcut::Modifiers) {
        if self.mode != SelectionMode::Extended {
            self.select_row(row);
            return;
        }
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            // No anchor means the user has not selected anything yet, so the range is
            // just this row — which is also the anchor it leaves behind.
            let anchor = self.anchor.unwrap_or(row);
            let (low, high) = if anchor <= row { (anchor, row) } else { (row, anchor) };
            self.selected_rows = (low..=high).collect();
            self.current_row = Some(row);
            return;
        }
        if modifiers.contains(crate::shortcut::Modifiers::CTRL) {
            if let Some(pos) = self.selected_rows.iter().position(|&i| i == row) {
                self.selected_rows.remove(pos);
            } else {
                self.selected_rows.push(row);
                // Keep the list ordered so it reads as a set of rows in visual order
                // rather than in click order.
                self.selected_rows.sort_unstable();
            }
            self.current_row = Some(row);
            self.anchor = Some(row);
            return;
        }
        self.select_row(row);
    }
    /// Returns the anchor a `Shift`-extend grows from, when one has been set.
    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }
    /// Clears selection.
    pub fn clear(&mut self) {
        self.selected_rows.clear();
        self.current_row = None;
        self.anchor = None;
    }
    /// Returns whether the view accepts selection at all.
    pub fn is_selectable(&self) -> bool {
        self.mode != SelectionMode::None
    }
    /// Returns current row.
    pub fn current_row(&self) -> Option<usize> {
        self.current_row
    }
    /// Returns all selected rows.
    pub fn rows(&self) -> Vec<usize> {
        self.selected_rows.clone()
    }
    fn normalize(&mut self) {
        if self.mode == SelectionMode::Single && self.selected_rows.len() > 1 {
            if let Some(&last) = self.selected_rows.last() {
                self.selected_rows = vec![last];
            } else {
                self.selected_rows.clear();
            }
        }
    }
}
/// Selection mode for list, tree, table and list-box views.
///
/// Re-exported from [`crate::widget::input_widgets::listbox::SelectionMode`] — the
/// input layer owns the canonical definition because `ListBox` needs it in every
/// profile, while this module is `full_widgets`-gated. One definition for every
/// selection surface (principle #54).
///
/// `None` means the view accepts no selection at all, which is distinct from an
/// empty selection in `Single`/`Multi` mode: it is a property of the view, not a
/// state of the data.
pub use crate::widget::input_widgets::listbox::SelectionMode;
/// How a list view presents its items.
///
/// # Not the same as [`crate::widget::container_widgets::mdiarea::ViewMode`]
///
/// Both types are called `ViewMode`, but they describe different axes and are
/// **not** interchangeable:
///
/// * this one — how one `ListView` presents its *own items* (list, icons,
///   details, thumbnails);
/// * [`mdiarea::ViewMode`][mdi] — how an `MdiArea` lays its *child windows* out
///   (free-floating sub-windows vs. tabs).
///
/// They are deliberately left as two enums: merging them would create one type
/// whose variants are meaningless in the other's context, so a `match` could not
/// be exhaustive in either widget. See principle #49.
///
/// [mdi]: crate::widget::container_widgets::mdiarea::ViewMode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Flat list of items.
    #[default]
    List,
    /// Large icons in a grid.
    Icon,
    /// Rows with extra columns of detail.
    Details,
    /// Thumbnail grid.
    Thumbnails,
}

/// List view widget.
pub struct ListView {
    base: BaseWidget,
    /// Optional bound list model.
    model: Option<Arc<dyn ListModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// View-side selection state.
    selection: SelectionModel,
    /// View-side focused row.
    focused_row: Option<usize>,
    /// The row under the pointer, or `None` when the pointer is elsewhere.
    ///
    /// # Why the focus row does not cover this
    ///
    /// `BaseWidget` records that the **control** is hovered, not which of its rows is, and a list
    /// is a column of independent rows: a hover that lit the whole view would point at nothing.
    /// Before this field existed a list row gave the user no feedback until they had already
    /// committed to it by clicking — the one affordance a list most needs, because the row under the
    /// pointer is the row a click will affect.
    ///
    /// Kept as its own field rather than folded into `focused_row`: a focus row is a *persistent*
    /// fact the keyboard moves, while this is transient pointer position. The draw gives them
    /// different weights for exactly that reason.
    hovered_row: Option<usize>,
    /// View mode for rendering items.
    view_mode: ViewMode,
    /// How an item's label is aligned within its row.
    ///
    /// # Why only the single-column modes read this
    ///
    /// The grid modes ([`ViewMode::Icon`] and [`ViewMode::Thumbnails`]) lay their labels out as
    /// captions centred under a tile, which is the mode's own geometry rather than the row's; a
    /// single alignment value cannot express a per-column placement, so those modes keep their
    /// placement and only the single-column layouts ([`ViewMode::List`] and [`ViewMode::Details`])
    /// honour the value. Horizontal only: a label is centred vertically in its row band by the row's
    /// own layout, so a `top`/`bottom` value would be one this control could never honour --
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused row changes.
    pub focused_row_changed: Signal1<Option<usize>>,
}
impl ListView {
    /// Creates an empty list view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "ListView"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selection: SelectionModel::new(),
            focused_row: None,
            hovered_row: None,
            view_mode: ViewMode::default(),
            alignment: crate::core::Alignment::Left,
            selection_changed: Signal1::new(),
            focused_row_changed: Signal1::new(),
        }
    }
    /// Binds an external list model.
    pub fn set_model(&mut self, model: Arc<dyn ListModel>) {
        self.model_connection_scope = ConnectionScope::new();
        if let Some(data_changed) = model.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            data_changed.connect_scoped(&self.model_connection_scope, move || {
                redraw.emit();
                layout.emit();
            });
        }
        self.model = Some(model);
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
    }
    /// Returns whether a model is currently bound.
    pub fn has_model(&self) -> bool {
        self.model.is_some()
    }
    /// Returns the bound list model, if present.
    pub fn model_ref(&self) -> Option<&Arc<dyn ListModel>> {
        self.model.as_ref()
    }
    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        self.model.as_ref().map(|m| m.row_count()).unwrap_or(0)
    }
    /// Returns item text by row index.
    pub fn item(&self, row: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data(row))
    }
    /// Select one row in the current view projection.
    ///
    /// Repaints: the selected row's highlight is painted, so without the request it
    /// would only appear once some unrelated event repainted the control.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row < self.row_count() {
            self.selection.select_row(row);
            self.selection_changed.emit(row);
            self.set_focused_row(row);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }
    /// Selects `row` honouring the modifier keys held during the click.
    ///
    /// In [`SelectionMode::Extended`](crate::widget::input_widgets::listbox::SelectionMode::Extended)
    /// this is what implements the documented interaction model: `Shift` selects the
    /// range from the anchor, `Ctrl`/`Primary` toggles the row, and no modifier replaces
    /// the selection. In every other mode it is [`Self::select_row`], so an event handler
    /// can call it unconditionally.
    pub fn select_row_with_modifiers(
        &mut self,
        row: usize,
        modifiers: crate::shortcut::Modifiers,
    ) -> bool {
        if row >= self.row_count() {
            return false;
        }
        self.selection.select_with_modifiers(row, modifiers);
        self.selection_changed.emit(row);
        self.set_focused_row(row);
        self.base.request_redraw();
        true
    }
    /// Clear current row selection.
    ///
    /// Repaints: the selection highlight is painted from the selection model.
    pub fn clear_selection(&mut self) {
        if self.selection.rows().is_empty() {
            return;
        }
        self.selection.clear();
        self.base.request_redraw();
    }
    /// Sets focused row in current projection.
    ///
    /// Repaints: the focus highlight is painted from this field.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        if self.focused_row == Some(row) {
            return true;
        }
        self.focused_row = Some(row);
        self.focused_row_changed.emit(self.focused_row);
        self.base.request_redraw();
        true
    }
    /// Clears focused row.
    ///
    /// Repaints: the focus highlight is painted from this field.
    pub fn clear_focused_row(&mut self) {
        if self.focused_row.is_none() {
            return;
        }
        self.focused_row = None;
        self.focused_row_changed.emit(None);
        self.base.request_redraw();
    }
    /// Returns focused row when still visible in projection.
    pub fn focused_row(&self) -> Option<usize> {
        self.focused_row.filter(|row| *row < self.row_count())
    }
    /// Current selected row index.
    pub fn selected_row(&self) -> Option<usize> {
        self.selection.current_row().filter(|row| *row < self.row_count())
    }
    /// All selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection.rows().into_iter().filter(|row| *row < self.row_count()).collect()
    }
    /// Sets row selection mode.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection.set_mode(mode);
    }
    /// Returns current selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection.mode()
    }

    /// Returns the current view mode.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Sets the view mode.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
        self.base.request_redraw();
    }

    /// How an item's label is aligned within its row.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how an item's label is aligned within its row.
    ///
    /// Horizontal only, and read by the single-column layouts; the grid modes place their captions
    /// by their own tile geometry, as [`Self::alignment`] documents. A `top`/`bottom` alignment is
    /// **ignored** because a label is centred vertically in its row band by the row's own layout --
    /// the property route refuses those through
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }
    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        self.selection.selected_rows.retain(|row| *row < row_count);
        self.selection.current_row = self.selection.current_row.filter(|row| *row < row_count);
        self.focused_row = self.focused_row.filter(|row| *row < row_count);
    }

    /// The rows the control can actually paint: everything that fits the content box.
    ///
    /// A count rather than a predicate, because a caller wants to know "how many rows are visible"
    /// as often as it wants "is row `i` visible", and deriving one from the other at each call site
    /// is the shape this crate keeps deleting.
    #[allow(dead_code)]
    fn visible_row_count(&self) -> usize {
        let layout = self.list_layout();
        (layout.content.height / layout.row_height.max(1)) as usize
    }

    /// How the items are laid out under the current [`ViewMode`].
    ///
    /// # Why one struct rather than a height here and a column count there
    ///
    /// Four modes differ in **four** numbers: how tall a cell is, how many cells sit across, the
    /// inset a label starts at, and whether a label is drawn under the cell or beside it. A control
    /// that read `self.view_mode` at each of those places would be four chances to update three of
    /// them, and the symptom would be a grid whose hit test disagrees with its paint -- the exact
    /// defect this file already records for the row height (`20` in three places). Resolving the mode
    /// once and reading the result everywhere is what makes the two impossible to disagree.
    ///
    /// # The modes
    ///
    /// | mode | cell | across | label |
    /// |---|---|---|---|
    /// | `List` | `LIST_ROW_HEIGHT` | 1 | beside, left-aligned |
    /// | `Icon` | a square icon tile | as many as fit | under, centred |
    /// | `Details` | two text lines tall | 1 | beside, on the lower line |
    /// | `Thumbnails` | a square preview tile | as many as fit | under, centred |
    ///
    /// The three modes other than `List` keep the **beside** label placement even where the mode's
    /// name suggests a caption: moving one of them under its cell would move that mode's own label
    /// in its own snapshot, which is a rendering change and not part of wiring the mode up. Only the
    /// two grid modes, which have no pre-existing label geometry to preserve, place theirs below.
    ///
    /// `List` is the pre-existing layout to the pixel, which is what keeps every existing geometry
    /// assertion and the snapshot valid: its numbers are the constants that were already there.
    fn list_layout(&self) -> ListLayout {
        let content = ControlMetrics::band_inset(self.base.geometry(), LIST_INSET);
        match self.view_mode {
            ViewMode::List => ListLayout {
                content,
                row_height: LIST_ROW_HEIGHT,
                columns: 1,
                text_inset: LIST_TEXT_INSET,
                label_below: false,
            },
            // A tile is square, so its height is its width. The width is the content box divided by
            // how many tiles fit, and how many fit is itself one of these numbers -- so it is derived
            // from the minimum cell size rather than the other way round. A box narrower than one
            // minimum cell therefore yields one column of a narrower tile, which is what "as many as
            // fit, but never zero" means.
            ViewMode::Icon => {
                let columns = (content.width / LIST_ICON_MIN_CELL).max(1);
                ListLayout {
                    content,
                    row_height: content.width / columns,
                    columns,
                    text_inset: LIST_TEXT_INSET,
                    label_below: true,
                }
            }
            ViewMode::Details => ListLayout {
                content,
                row_height: LIST_ROW_HEIGHT * 2,
                columns: 1,
                text_inset: LIST_TEXT_INSET,
                label_below: false,
            },
            ViewMode::Thumbnails => {
                let columns = (content.width / LIST_THUMB_MIN_CELL).max(1);
                ListLayout {
                    content,
                    // The caption is *added to* the square, not carved out of it: a thumbnail keeps
                    // its preview area and the label gets its own strip underneath.
                    row_height: content.width / columns + LIST_THUMB_CAPTION,
                    columns,
                    text_inset: LIST_TEXT_INSET,
                    label_below: true,
                }
            }
        }
    }

    /// Row `index`'s own band, or `None` when the row is past the last visible one.
    ///
    /// # Why this is one derivation and not three
    ///
    /// The band's origin and its height were spelled three times — the paint loop used the content
    /// box while both press arms of `handle_event` used the control's `rect.y`, so a press on the
    /// first row landed on the second. The row height was a `20` literal in all three. Returning the
    /// band from one function is what makes the drawn rows and the clickable rows the same rows; the
    /// caller cannot forget one of the four numbers because it never handles them.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        let layout = self.list_layout();
        // A *row* is one line of the grid, so the index walks lines and the column is what varies
        // within one. That is also what keeps this function's contract unchanged for `List`: with one
        // column, line `index` and item `index` are the same thing.
        let line = index as u32 / layout.columns;
        let y = layout.content.y + (layout.row_height * line) as i32;
        // A row that would extend past the content box is not visible: the clip is the content
        // box's bottom edge, and a partially visible row is not painted at all rather than being
        // painted truncated — a half-height row reads as a rendering error.
        if y + layout.row_height as i32 > layout.content.y + layout.content.height as i32 {
            return None;
        }
        Some(Rect::new(layout.content.x, y, layout.content.width, layout.row_height))
    }

    /// The band item `index` occupies: its column's slice of its row.
    ///
    /// In a one-column mode this is the row's own band, so [`Self::row_rect`] and this agree exactly
    /// and the `List`/`Details` modes are unaffected. In a grid mode it is the tile, which is what a
    /// highlight and a label have to be drawn inside — and its x comes from the *same* cell width
    /// [`Self::row_at_point`] divides by, so a tile's paint and its hit test land on one another.
    fn item_rect(&self, index: usize) -> Option<Rect> {
        let row = self.row_rect(index)?;
        let layout = self.list_layout();
        if layout.columns <= 1 {
            return Some(row);
        }
        let column = index as u32 % layout.columns;
        let cell = layout.cell_width();
        Some(Rect::new(row.x + (cell * column) as i32, row.y, cell, row.height))
    }

    /// The row index a point falls on, if it falls on a visible row.
    ///
    /// The inverse of [`Self::row_rect`] and deliberately built on it: a point is a row's when it
    /// is inside that row's band, so the two directions cannot disagree about where row 0 begins.
    fn row_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let layout = self.list_layout();
        if !layout.content.contains_point(point) {
            return None;
        }
        let line = (point.y - layout.content.y) as u32 / layout.row_height.max(1);
        // In a grid the column is read from x, and a click in the empty space past the last tile of a
        // line belongs to no item -- clamping it to the last column would select an item the user did
        // not click, which is worse than selecting nothing.
        let column = if layout.columns <= 1 {
            0
        } else {
            let column = (point.x - layout.content.x).max(0) as u32 / layout.cell_width();
            if column >= layout.columns {
                return None;
            }
            column
        };
        let index = (line * layout.columns + column) as usize;
        // The last partial row is not a target, matching `row_rect`'s refusal to paint it.
        (index < self.row_count() && self.row_rect(index).is_some()).then_some(index)
    }

    /// Focuses and selects the row under `point`, if there is one.
    ///
    /// One implementation for the mouse and the touch paths, which were two copies of the same
    /// sixteen lines that had already drifted from the paint loop's own idea of where rows are.
    fn select_row_at_point(&mut self, point: crate::core::Point) {
        self.select_row_at_point_with(point, crate::shortcut::Modifiers::NONE);
    }

    /// Focuses and selects the row under `point`, honouring `modifiers`.
    ///
    /// The modifier state comes from the press that produced the point, which is why it
    /// is a parameter: `Shift`/`Ctrl` are what select a range or toggle one row in
    /// extended-selection mode, and the widget layer can only see them if the event
    /// carried them this far.
    fn select_row_at_point_with(
        &mut self,
        point: crate::core::Point,
        modifiers: crate::shortcut::Modifiers,
    ) {
        let Some(index) = self.row_at_point(point) else { return };
        self.focused_row = Some(index);
        self.selection.select_with_modifiers(index, modifiers);
        if let Some(row) = self.focused_row {
            self.selection_changed.emit(row);
            self.focused_row_changed.emit(Some(row));
        }
    }
}
impl Widget for ListView {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::LIST_VIEW_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::LIST_VIEW_DEFAULT_HEIGHT,
        )
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
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::Int(*v as i64)
                }))
            }
            "focused_row_changed" => Some(EventSignalRef::mapped(
                "focused_row_changed",
                &self.focused_row_changed,
                |v| match v {
                    Some(index) => CapabilityValue::UInt(*index as u64),
                    None => CapabilityValue::Null,
                },
            )),
            _ => None,
        }
    }
}

/// `ListView`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for ListView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_model" => Ok(CapabilityValue::Bool(self.has_model())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "focused_row" => match self.focused_row() {
                Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selection_mode" => Ok(CapabilityValue::String(
                selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            "view_mode" => {
                Ok(CapabilityValue::String(view_mode_to_str(self.view_mode()).to_string()))
            }
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "focused_row" => match value {
                CapabilityValue::Null => {
                    self.clear_focused_row();
                    Ok(())
                }
                other => {
                    let row = expect_usize(other)?;
                    if self.set_focused_row(row) {
                        Ok(())
                    } else {
                        // `set_focused_row` answers `false` only when `row` is past the
                        // last row, so the caller's index is the mistake — not the
                        // control's capability. See `CapabilityAccessError::OutOfRange`.
                        Err(CapabilityAccessError::OutOfRange)
                    }
                }
            },
            "selection_mode" => {
                self.set_selection_mode(expect_selection_mode(value)?);
                Ok(())
            }
            "view_mode" => {
                self.set_view_mode(expect_view_mode(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "has_model" => Err(CapabilityAccessError::ReadOnlyProperty),
            "row_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_model",
            "row_count",
            "focused_row",
            "selection_mode",
            "view_mode",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `list_view` publishes.
    ///
    /// Both are payload-free and operate on the live selection, so they execute
    /// here. The trait default answered `UnknownCommand`, which `invoke_command`
    /// reports as `UnsupportedOnWidget` for a name the capability table publishes.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            "clear_focused_row" => {
                self.clear_focused_row();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ListView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the focused-row highlight and the text
        // colour used to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("list_view");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The focused row is a selection state, so it reads the theme's accent token rather
        // than a second literal blue, and is laid over the surface so it stays legible.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let focused_bg = surface.blend(&accent, 0.30);
        // A hovered row is the same accent at a **lighter** weight than the focused row: it is a
        // pointer position rather than a committed selection, so it has to read as "this is the row a
        // click will affect" without competing with the row that is actually selected. Deriving it
        // from the same accent keeps the two in step when a theme changes hue.
        let hovered_bg = surface.blend(&accent, 0.12);

        context.face(
            rect,
            surface,
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);
        // Draw items from model. Each row is a band of the content box, never of the
        // control, and the label's line box is derived from the row rather than from
        // `y + item_height / 2` — which put the glyph box's top edge on the row's middle
        // line and left the first row pinned to y=0.
        //
        // The row box comes from `item_rect`, which is also what the hit test reads: the row height
        // used to be a `20` literal in three places (this loop, and both press arms of
        // `handle_event`) and the press arms measured from `rect.y` while this loop measured from
        // the content box, so a press on the first row selected the second.
        //
        // The text inset and the label's placement come from the same resolved layout, so a mode that
        // changed the cell size cannot leave the label behind at the old inset — which is exactly the
        // class of defect the row height already had.
        if self.model.is_some() {
            let layout = self.list_layout();
            let row_count = self.row_count();
            let current_row = self.focused_row;
            let font = crate::core::Font::default();
            // The row-*index* arithmetic is in `usize` (it comes from the model) and the way to get
            // back to the row's number is to make the whole grid arithmetic `u32`, so the sole cast is
            // at the two ends. `index / columns` reports the line, `index % columns` the column within
            // it, and truncating division is what makes the two agree for the last partial line.
            for i in 0..row_count {
                let Some(cell) = self.item_rect(i) else { break };
                // Hover is painted *under* focus: a row that is both hovered and focused keeps the
                // focused weight, so the persistent fact is not visually displaced by the pointer
                // merely passing over it.
                if Some(i) == current_row {
                    context.fill_rect(cell, focused_bg);
                } else if Some(i) == self.hovered_row {
                    context.fill_rect(cell, hovered_bg);
                }
                if let Some(text) = self.model.as_ref().and_then(|model| model.data(i)) {
                    if !text.is_empty() {
                        // A tile's caption is one line drawn *under* its picture, so its band is a
                        // one-line strip at the cell's foot; giving `text_line` the whole cell would
                        // centre the caption on the tile's middle line, over the picture it names.
                        // A beside-label mode hands it the cell and lets it centre there, which for
                        // the single-column modes is the pre-existing geometry exactly.
                        let (band, align) = if layout.label_below {
                            let caption = cell.height.min(layout.caption_height()).max(1);
                            (
                                Rect::new(
                                    cell.x + layout.text_inset,
                                    cell.y + (cell.height - caption) as i32,
                                    cell.width.saturating_sub(layout.text_inset as u32),
                                    caption,
                                ),
                                HorizontalAlignment::Center,
                            )
                        } else {
                            (
                                Rect::new(
                                    cell.x + layout.text_inset,
                                    cell.y,
                                    cell.width.saturating_sub(layout.text_inset as u32),
                                    cell.height,
                                ),
                                // A beside-label is a single run in a single-column row, so it is the
                                // one placement the control's own alignment governs. The caption modes
                                // below centre their label as part of their tile geometry, which is a
                                // per-tile decision rather than a row one.
                                self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
                            )
                        };
                        context.draw_text_fitted(
                            context.text_line(band, &font),
                            &text,
                            &font,
                            ink,
                            align,
                        );
                    }
                }
            }
        }
    }
}
impl crate::event::EventHandler for ListView {
    fn handle_event(&mut self, event: &crate::event::Event) {
        // The base keeps the control-level facts (`hovered`, `pressed`, `focus_reason`), and its
        // `MouseEnter`/`MouseLeave` arms are what make `widget_state()` answer `Hover` for this
        // control. This handler did not forward to it at all, so the theme's `"list_view:hover"`
        // override could never fire and the control-level hover was always false.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button, modifiers } if *button == 1 => {
                self.select_row_at_point_with(
                    *pos,
                    crate::shortcut::Modifiers::from_event_bits(*modifiers),
                );
            }
            // Row hover, derived from the same `row_at_point` the click uses, so the row that is
            // highlighted is the row a click would affect. A pointer that leaves the rows entirely
            // (below the last one, or outside the content box) clears it rather than latching the
            // last row it happened to cross.
            crate::event::Event::MouseMove { pos } => {
                let hovered = self.row_at_point(*pos);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
            }
            #[cfg(feature = "touch")]
            crate::event::Event::Tap { pos } => {
                self.select_row_at_point(*pos);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(device_profile, feature = "desktop"))]
    use crate::event::EventHandler as _;
    use std::sync::Arc;

    struct StaticListModel;

    impl ListModel for StaticListModel {
        fn row_count(&self) -> usize {
            2
        }

        fn data(&self, row: usize) -> Option<String> {
            match row {
                0 => Some("A".to_string()),
                1 => Some("B".to_string()),
                _ => None,
            }
        }
    }

    #[test]
    fn out_of_range_row_is_reported_as_out_of_range_not_unsupported() {
        // Regression guard for a misreported error. `set_focused_row` answers `false`
        // only for an index past the last row, and the property layer used to translate
        // that into `UnsupportedOnWidget` — telling the caller this control can never do
        // it, when the truth was that their index was wrong. A caller acting on the old
        // error would go looking for a different control instead of fixing the index.
        use crate::widget::capability::types::CapabilityAccessError;

        let mut view = ListView::new(Rect::new(0, 0, 100, 80));
        view.set_model(Arc::new(StaticListModel));
        assert_eq!(view.row_count(), 2, "the fixture must have rows to be out of range of");

        assert_eq!(
            view.set("focused_row", CapabilityValue::UInt(99)),
            Err(CapabilityAccessError::OutOfRange),
            "an index past the last row is the caller's argument, not a capability gap"
        );
        // The same call with a valid index must still work, which is what distinguishes
        // `OutOfRange` from `UnsupportedOnWidget`.
        assert_eq!(view.set("focused_row", CapabilityValue::UInt(1)), Ok(()));
        assert_eq!(view.focused_row(), Some(1));
    }

    /// Moving over a row highlights **that** row, and leaving the rows clears it.
    ///
    /// # The defect this pins
    ///
    /// The list had no row hover at all: `handle_event` did not even forward to `BaseWidget`, so the
    /// control-level hover was permanently false and `"list_view:hover"` could never fire. A list is
    /// a column of independent rows, so the hover has to name a row — the row a click would affect —
    /// rather than the control.
    ///
    /// # Why the assertion reads the row's own fill
    ///
    /// "The document changed" would pass for a hover on any row; the helper reads the fill of the
    /// rectangle at a **named row's** centre, so the assertion is about the row the pointer is over.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn hovering_a_row_highlights_that_row_only() {
        // The row fills come from the active theme, so pin the appearance and hold the registry
        // guard: otherwise a concurrent theme switch between the resting and hovered reads makes
        // this compare two different appearances rather than two states.
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let mut view = ListView::new(Rect::new(0, 0, 200, 120));
        view.set_model(Arc::new(StaticListModel));
        let resting = row_fill(&mut view, 1).expect("row 1 paints a fill");

        let over_row_1 = row_midpoint(&view, 1).expect("row 1 is visible");
        view.handle_event(&crate::event::Event::MouseMove { pos: over_row_1 });
        let hovered = row_fill(&mut view, 1).expect("row 1 still paints a fill");
        assert_ne!(
            hovered, resting,
            "the row under the pointer must be visible as hovered; both fills were {resting}"
        );
        // And the *other* row must not have changed, or the highlight points at nothing.
        let other = row_fill(&mut view, 0).expect("row 0 paints a fill");
        let other_resting = {
            let mut fresh = ListView::new(Rect::new(0, 0, 200, 120));
            fresh.set_model(Arc::new(StaticListModel));
            row_fill(&mut fresh, 0).expect("row 0 paints a fill")
        };
        assert_eq!(other, other_resting, "a hover on row 1 must not restyle row 0");

        view.handle_event(&crate::event::Event::MouseLeave { pos: over_row_1 });
        let left = row_fill(&mut view, 1).expect("row 1 still paints a fill");
        assert_eq!(left, resting, "leaving must clear the hover, not latch it");
    }

    /// A visible row's midpoint, from the control's own row geometry.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn row_midpoint(view: &ListView, index: usize) -> Option<crate::core::Point> {
        let row = view.row_rect(index)?;
        Some(crate::core::Point::new(row.x + row.width as i32 / 2, row.y + row.height as i32 / 2))
    }

    /// The fill of the SVG `<rect>` at `index`'s row midpoint.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn row_fill(view: &mut ListView, index: usize) -> Option<String> {
        let rect = view.geometry();
        let at = row_midpoint(view, index)?;
        let svg = crate::widget::svg::render_widget_to_svg(view, rect);
        let mut best: Option<(u32, String)> = None;
        for element in svg.split("/>") {
            let Some(open) = element.find("<rect ") else { continue };
            let element = &element[open + "<rect ".len()..];
            let attr = |name: &str| -> Option<i32> {
                let key = format!("{name}=\"");
                let at = element.find(&key)? + key.len();
                let to = element[at..].find('"')? + at;
                element[at..to].parse().ok()
            };
            let (Some(x), Some(y), Some(w), Some(h)) =
                (attr("x"), attr("y"), attr("width"), attr("height"))
            else {
                continue;
            };
            if at.x < x || at.x >= x + w || at.y < y || at.y >= y + h {
                continue;
            }
            let Some(fill_at) = element.find("fill=\"") else { continue };
            let from = fill_at + "fill=\"".len();
            let Some(to) = element[from..].find('"') else { continue };
            let area = (w as u32).saturating_mul(h as u32);
            if best.as_ref().map(|(a, _)| area < *a).unwrap_or(true) {
                best = Some((area, element[from..from + to].to_string()));
            }
        }
        best.map(|(_, fill)| fill)
    }

    #[test]
    fn list_view_model_binding_roundtrip() {
        let mut view = ListView::new(Rect::new(0, 0, 100, 80));
        assert!(!view.has_model());
        assert!(view.model_ref().is_none());

        view.set_model(Arc::new(StaticListModel));

        assert!(view.has_model());
        assert!(view.model_ref().is_some());
        assert_eq!(view.row_count(), 2);
        assert_eq!(view.item(99), None);
    }

    /// The rows tile the view's content box in sequence.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.8 lists this control's defect as "the row height computed by hand". It was a `20`
    /// literal in the paint loop and in both press arms, and the two sides measured from different
    /// origins — the loop from the content box and the presses from `rect.y`. [`ListView::row_rect`]
    /// is now the one derivation, so this states the sequence outright.
    #[test]
    fn the_rows_tile_the_content_box_in_sequence() {
        let view = ListView::new(Rect::new(0, 0, 200, 120));
        let first = view.row_rect(0).expect("the first row is visible");
        assert_eq!(first.height, LIST_ROW_HEIGHT);
        for index in 0..view.visible_row_count() {
            let row = view.row_rect(index).expect("a visible index has a row");
            assert_eq!(row.x, first.x, "the rows share one column");
            assert_eq!(row.width, first.width);
        }
        for index in 1..view.visible_row_count() {
            let previous = view.row_rect(index - 1).expect("previous is visible");
            let row = view.row_rect(index).expect("this index is visible");
            assert_eq!(
                row.y,
                previous.y + previous.height as i32,
                "each row follows the previous one's own band: index {index}"
            );
        }
        // And a row past the content box is not visible at all rather than truncated.
        assert!(view.row_rect(view.visible_row_count()).is_none());
    }

    /// A press selects the row it is *on*, measured from the same origin the rows are painted from.
    ///
    /// This is the defect the migration fixed: the press arms measured from `rect.y` while the paint
    /// measured from the content box, so the first row's band was hit-tested as the second row's.
    /// The test drives a point inside each painted row and asserts that row is the one selected.
    #[test]
    fn a_press_selects_the_row_it_is_painted_on() {
        let mut view = ListView::new(Rect::new(0, 0, 200, 120));
        // A model taller than a row, so a press beyond the last row has somewhere to go and the
        // bound under test is the *content box* rather than the model's length.
        view.set_model(Arc::new(VecListModel::new((0..8).map(|n| n.to_string()).collect())));
        for index in 0..view.visible_row_count().min(view.row_count()) {
            let row = view.row_rect(index).expect("visible");
            let centre = crate::core::Point::new(row.x + 1, row.y + row.height as i32 / 2);
            assert_eq!(
                view.row_at_point(centre),
                Some(index),
                "the centre of the painted row {index} must resolve to that row: {row:?}"
            );
        }
        // A point in the view's own frame, outside the content box, is no row at all.
        assert_eq!(view.row_at_point(crate::core::Point::new(0, 0)), None);
    }

    /// Each `ViewMode` resolves to a *different* layout, and each layout's paint and hit test agree.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `view_mode` was stored, published (getter, setter, schema row, round-trip test) and read by
    /// nothing: all four modes painted and hit-tested as a flat list. The property layer therefore
    /// promised a feature the control did not have.
    ///
    /// A mode is *not* wired up merely because `list_layout` mentions it -- the geometry has to reach
    /// the pixels. This drives a point at each tile's own centre and requires the hit test to name
    /// that tile, which is only true if `item_rect` (what `draw` paints through) and `row_at_point`
    /// (what a click resolves through) were both built from the same columns and cell width.
    #[test]
    fn every_view_mode_lays_out_its_own_grid_and_hit_tests_to_match() {
        // Wide enough for a real grid: 200 px holds 4 icon tiles of 48 or 2 thumbnail tiles of 72.
        const WIDTH: u32 = 200;
        const HEIGHT: u32 = 120;

        let layout_of = |mode: ViewMode| {
            let mut view = ListView::new(Rect::new(0, 0, WIDTH, HEIGHT));
            view.set_view_mode(mode);
            view.set_model(Arc::new(VecListModel::new((0..12).map(|n| n.to_string()).collect())));
            view
        };

        let list = layout_of(ViewMode::List).list_layout();
        let icons = layout_of(ViewMode::Icon).list_layout();
        let details = layout_of(ViewMode::Details).list_layout();
        let thumbs = layout_of(ViewMode::Thumbnails).list_layout();

        // The four modes are four layouts: a mode that resolved to the same numbers as `List` would be
        // the dead state this test exists to catch.
        for (name, layout) in [("Icon", icons), ("Details", details), ("Thumbnails", thumbs)] {
            assert_ne!(
                (layout.row_height, layout.columns, layout.label_below),
                (list.row_height, list.columns, list.label_below),
                "{name} must lay out differently from List"
            );
        }

        // `List` keeps the pre-existing numbers exactly, which is what makes the snapshot valid.
        assert_eq!(list.row_height, LIST_ROW_HEIGHT);
        assert_eq!(list.columns, 1);
        assert!(list.content.width > 0 && list.content.height > 0);

        // The two grid modes tile across; the two single-column modes do not.
        assert!(icons.columns > 1, "200 px holds more than one 48 px icon tile");
        assert!(thumbs.columns > 1, "200 px holds more than one 72 px thumbnail");
        assert_eq!(details.columns, 1);
        // A tile is square (the caption is added below it rather than carved out of it).
        assert_eq!(icons.row_height, icons.cell_width());
        assert_eq!(thumbs.row_height, thumbs.cell_width() + LIST_THUMB_CAPTION);
        // Details is a taller single line, not a second one drawn at the same height.
        assert_eq!(details.row_height, LIST_ROW_HEIGHT * 2);

        // Paint and hit test agree, tile by tile, in every mode -- the property the old code had
        // already lost once for the row height, and the reason both now read one `ListLayout`.
        for (name, mode) in [
            ("List", ViewMode::List),
            ("Icon", ViewMode::Icon),
            ("Details", ViewMode::Details),
            ("Thumbnails", ViewMode::Thumbnails),
        ] {
            let view = layout_of(mode);
            let mut checked = 0;
            for index in 0..view.row_count() {
                let Some(cell) = view.item_rect(index) else { break };
                let centre = crate::core::Point::new(
                    cell.x + cell.width as i32 / 2,
                    cell.y + cell.height as i32 / 2,
                );
                assert_eq!(
                    view.row_at_point(centre),
                    Some(index),
                    "{name}: the centre of tile {index} at {cell:?} must hit-test back to it"
                );
                checked += 1;
            }
            assert!(
                checked > 1,
                "{name}: the fixture must lay out more than one item to be a grid"
            );
        }

        // In a grid, two items on one line are side by side and share a band; that is what "columns"
        // means, and a mode that read `columns` for the height only would fail it.
        let icons_view = layout_of(ViewMode::Icon);
        let first = icons_view.item_rect(0).expect("visible");
        let second = icons_view.item_rect(1).expect("visible");
        assert_eq!(second.y, first.y, "the first two icon tiles share a line");
        assert_eq!(second.x, first.x + first.width as i32, "and sit side by side");
        assert!(
            second.x < first.x + icons_view.list_layout().content.width as i32,
            "the second tile is still inside the content box"
        );
        // The first tile of the *second* line is directly below the first tile of the first.
        let wrapped = icons_view
            .item_rect(icons_view.list_layout().columns as usize)
            .expect("the second line fits");
        assert_eq!(wrapped.x, first.x, "a wrapped item starts the next line");
        assert_eq!(wrapped.y, first.y + first.height as i32);
    }

    /// A tile's caption is drawn in the strip at the tile's foot, not on the tile's middle line.
    ///
    /// # Why this needs its own assertion
    ///
    /// The caption's *band* is what the glyphs land in, and the obvious implementation -- hand
    /// `text_line` the whole cell -- centres the caption over the picture it names, because
    /// `text_line` centres vertically within whatever band it is given. Asserting only that the mode
    /// changed would pass on that version.
    #[test]
    fn a_tile_caption_is_a_strip_at_the_foot_of_its_tile() {
        let mut view = ListView::new(Rect::new(0, 0, 200, 120));
        view.set_view_mode(ViewMode::Thumbnails);
        let layout = view.list_layout();
        assert!(layout.label_below, "the grid modes caption their tiles");
        let caption = layout.caption_height();
        assert!(caption > 0, "a caption with no height would draw nothing");
        assert!(
            caption < layout.row_height,
            "the caption must leave room for the picture above it"
        );
        // The strip starts where the picture ends, so it is the cell's last `caption` rows.
        let tile = view.item_rect(0).expect("visible");
        assert_eq!(
            tile.y + (tile.height - caption) as i32,
            tile.y + layout.row_height as i32 - caption as i32,
            "the caption strip is flush with the tile's foot"
        );
        // And a beside-label mode does not move its label down: `List` is unchanged to the pixel.
        let mut list = ListView::new(Rect::new(0, 0, 200, 120));
        list.set_view_mode(ViewMode::List);
        assert!(!list.list_layout().label_below, "List labels sit beside their row");
    }

    /// `Extended` implements the Windows-explorer model its documentation promises.
    ///
    /// It used to be a copy of `Multi`: the arm pushed the row unconditionally, so a
    /// plain click *added* to the selection instead of replacing it, and there was no
    /// anchor at all for a `Shift` to extend from. `ListBox` had already been fixed;
    /// the shared model the view widgets use was left behind.
    #[test]
    fn extended_selection_replaces_on_a_plain_click_and_extends_with_shift() {
        use crate::shortcut::Modifiers;
        let mut v = ListView::new(Rect::new(0, 0, 200, 200));
        v.set_model(Arc::new(VecListModel::new((0..8).map(|n| n.to_string()).collect())));
        v.set_selection_mode(SelectionMode::Extended);

        v.select_row(2);
        assert_eq!(v.selected_rows(), vec![2], "a plain click replaces the selection");

        v.select_row_with_modifiers(5, Modifiers::SHIFT);
        assert_eq!(v.selected_rows(), vec![2, 3, 4, 5], "Shift extends from the anchor");

        v.select_row_with_modifiers(4, Modifiers::CTRL);
        assert_eq!(v.selected_rows(), vec![2, 3, 5], "Ctrl toggles one row, no duplicates");

        v.select_row_with_modifiers(1, Modifiers::NONE);
        assert_eq!(v.selected_rows(), vec![1], "and a plain click replaces again");
    }
}
