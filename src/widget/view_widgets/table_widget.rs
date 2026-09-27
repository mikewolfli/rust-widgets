// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Table widget.
use crate::core::Color;
use crate::core::HorizontalAlignment;
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::access::selection_mode_to_str;
use crate::widget::capability::coercion::expect_selection_mode;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::ControlMetrics;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::HashMap;
use std::sync::Arc;

/// The margin a table leaves between its own frame and its rows: 2 px on every edge.
///
/// Named once so the header row and the first content row are both measured from the same
/// inset. The rows used to start at the control's literal `rect.y`, so the first row's glyph
/// box sat flush on the frame's own stroke with no header row above it.
const TABLE_INSET: u32 = 2;

/// The height of the table's header row: 20, the same row the model's data rows use.
///
/// A table has a header whether or not the model supplies column titles, so the first data
/// row is always one header below the frame rather than pinned to the top edge.
const HEADER_ROW_HEIGHT: u32 = 20;

/// The height of a content row: 20, the same as the header's.
///
/// Extracted because it was a `let row_h = 20;` in **two** places — the paint loop and the press
/// arm — and they did not even measure from the same origin. [`TableWidget::row_rect`] is now the
/// one derivation, which is what makes the row a click selects the row a hover highlights.
const TABLE_ROW_HEIGHT: u32 = 20;

/// Table model abstraction for table-like views.
pub trait TableModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Number of columns exposed by model.
    fn column_count(&self) -> usize;
    /// Data for row and column index, if present.
    fn data(&self, row: usize, column: usize) -> Option<String>;
    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
}
/// Item delegate for custom display/editing.
pub trait ItemDelegate: Send + Sync {
    /// Creates editor for given cell.
    fn create_editor(
        &self,
        parent: &mut BaseWidget,
        row: usize,
        column: usize,
    ) -> Option<Box<dyn Widget>>;
    /// Sets editor data.
    fn set_editor_data(&self, editor: &mut dyn Widget, row: usize, column: usize);
    /// Gets editor data.
    fn get_editor_data(&self, editor: &dyn Widget, row: usize, column: usize) -> Option<String>;
}
/// Table widget with model/view helpers and selection state.
pub struct TableWidget {
    base: BaseWidget,
    /// Optional bound data model.
    model: Option<Arc<dyn TableModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// View-side selection state.
    selection: crate::widget::view_widgets::list_view::SelectionModel,
    /// View-side focused row.
    focused_row: Option<usize>,
    /// The content row under the pointer, or `None` when the pointer is elsewhere.
    ///
    /// The same reasoning as `ListView::hovered_row`: a table is a stack of independent rows, so
    /// the hover has to name a row rather than the control, and the row it names is the one a click
    /// would affect. Held here rather than derived from `focused_row` because a focus row is
    /// persistent while this is transient pointer position.
    hovered_row: Option<usize>,
    /// Explicit column width overrides in logical pixels.
    column_widths: HashMap<usize, u32>,
    /// Explicit row height overrides in logical pixels.
    row_heights: HashMap<usize, u32>,
    /// Optional display/editor delegate.
    ///
    /// Driven by [`TableWidget::begin_edit`] / [`TableWidget::commit_edit`]; see those for why the
    /// three-method trait is an operation rather than a rendering hook.
    delegate: Option<Arc<dyn ItemDelegate>>,
    /// The cell an in-place edit is currently open on, if any.
    editing: Option<(usize, usize)>,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused row changes.
    pub focused_row_changed: Signal1<Option<usize>>,
}
impl TableWidget {
    /// Creates an empty table widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "TableWidget"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selection: crate::widget::view_widgets::list_view::SelectionModel::new(),
            focused_row: None,
            hovered_row: None,
            column_widths: HashMap::new(),
            row_heights: HashMap::new(),
            delegate: None,
            editing: None,
            selection_changed: Signal1::new(),
            focused_row_changed: Signal1::new(),
        }
    }
    /// Binds an external table model.
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
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
    /// Returns the bound table model, if present.
    pub fn model_ref(&self) -> Option<&Arc<dyn TableModel>> {
        self.model.as_ref()
    }
    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        self.model.as_ref().map(|m| m.row_count()).unwrap_or(0)
    }
    /// Returns visible column count.
    pub fn column_count(&self) -> usize {
        self.model.as_ref().map(|m| m.column_count()).unwrap_or(0)
    }
    /// Returns item text by row and column index.
    pub fn item(&self, row: usize, column: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data(row, column))
    }

    /// The content box the rows live in: the control inset, less the header band.
    ///
    /// The one derivation both the paint loop and the hit test read. They used to disagree: the loop
    /// measured from the content box while the press arm measured from `rect.y`, so a press on the
    /// first content row selected the row **above** it — the same defect `ListView::row_rect` was
    /// extracted to fix, which this control's sibling had kept.
    fn rows_band(&self) -> Rect {
        let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
        let header = ControlMetrics::top_band(content, HEADER_ROW_HEIGHT);
        Rect::new(
            content.x,
            content.y + header.height as i32,
            content.width,
            content.height.saturating_sub(header.height),
        )
    }

    /// The height of content row `index`: its override, or the default.
    ///
    /// # Why a row's height is resolved per row rather than per table
    ///
    /// `row_heights` is an override map, so a table may have a tall header row, a set of normal ones
    /// and a tall summary row. The `TABLE_ROW_HEIGHT` constant is the *default*, and a table with no
    /// overrides must lay out exactly as it did before this existed — which is the property its
    /// snapshot and the hit-test tests pin.
    fn resolved_row_height(&self, index: usize) -> u32 {
        self.row_heights.get(&index).copied().unwrap_or(TABLE_ROW_HEIGHT)
    }

    /// The y offset of content row `index` from the top of the rows band.
    ///
    /// # Why the rows are accumulated rather than multiplied
    ///
    /// `TABLE_ROW_HEIGHT * index` is the same thing only while every row is the default height. With
    /// per-row overrides the rows **compose**: row `n` starts where rows `0..n` end, and multiplying
    /// would overlap a tall row with its successors. The accumulation is over the overrides that are
    /// actually set, so a table with none takes the same arithmetic as before (`default * index`) and
    /// cannot drift from it.
    fn row_offset(&self, index: usize) -> u32 {
        if self.row_heights.is_empty() {
            return TABLE_ROW_HEIGHT * index as u32;
        }
        (0..index).map(|i| self.resolved_row_height(i)).sum()
    }

    /// The width of column `index`: its override, or an equal share of the content box.
    ///
    /// # Why the override replaces the share rather than trimming it
    ///
    /// A host that sets a column width is saying how wide it wants that column. Clamping the result
    /// back into the box would make `set_column_width(0, 400)` on a 240-wide table give neither 400
    /// nor a sensible share, and the table must not silently disagree with the value it stored -- the
    /// same defect `grid`'s sentinel had, in a different form.
    fn resolved_column_width(&self, index: usize, content_width: u32) -> u32 {
        if let Some(width) = self.column_widths.get(&index) {
            return *width;
        }
        if self.column_count() == 0 {
            return content_width;
        }
        (content_width / self.column_count() as u32).max(40)
    }

    /// The x offset of column `index` from the content box's left edge.
    ///
    /// # Why the columns are accumulated rather than multiplied
    ///
    /// `col_w * index` is the same thing only while every column is the same width. With overrides the
    /// columns **compose**: column `n` starts where columns `0..n` end. The multiplication was also
    /// the reason a wide column could overlap its neighbour rather than push it, which is the same
    /// defect the crate records for the footer's button row.
    ///
    /// The fast path keeps a table with no overrides on the original arithmetic, so its snapshot and
    /// every geometry assertion are unchanged.
    fn column_offset(&self, index: usize, content_width: u32) -> u32 {
        if self.column_widths.is_empty() {
            let col_w = if self.column_count() > 0 {
                (content_width / self.column_count() as u32).max(40)
            } else {
                content_width
            };
            return col_w * index as u32;
        }
        (0..index).map(|c| self.resolved_column_width(c, content_width)).sum()
    }

    /// The rectangle of content row `index`, or `None` when it is not fully visible.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        let band = self.rows_band();
        let height = self.resolved_row_height(index);
        let y = band.y + self.row_offset(index) as i32;
        // A row that would extend past the content box is not painted at all rather than painted
        // truncated: a half-height row reads as a rendering error.
        if y + height as i32 > band.y + band.height as i32 {
            return None;
        }
        Some(Rect::new(band.x, y, band.width, height))
    }

    /// The content row a point falls on, if it falls on a visible one.
    ///
    /// The inverse of [`Self::row_rect`] and built on it, so the row a click selects and the row a
    /// hover highlights cannot disagree about where a row begins.
    fn row_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let band = self.rows_band();
        if !band.contains_point(point) {
            return None;
        }
        let offset = (point.y - band.y) as u32;
        // Walk the rows rather than dividing: with per-row heights the row a y belongs to is the one
        // whose offset range contains it, and a division by the *default* height would be wrong from
        // the first overridden row down. The walk is over the rows the band can show, which is bounded
        // by the band's height, so it cannot run away on a long table.
        let mut y = 0u32;
        let mut index = 0usize;
        while index < self.row_count() {
            let height = self.resolved_row_height(index);
            if offset < y + height {
                break;
            }
            y += height;
            index += 1;
        }
        // The last partial row is not a target, matching `row_rect`'s refusal to paint it.
        (index < self.row_count() && self.row_rect(index).is_some()).then_some(index)
    }
    /// Select one row in the current view projection.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row < self.row_count() {
            self.selection.select_row(row);
            self.selection_changed.emit(row);
            self.set_focused_row(row);
            true
        } else {
            false
        }
    }
    /// Clear current row selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }
    /// Sets focused row in current projection.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        if self.focused_row == Some(row) {
            return true;
        }
        self.focused_row = Some(row);
        self.focused_row_changed.emit(self.focused_row);
        true
    }
    /// Clears focused row.
    pub fn clear_focused_row(&mut self) {
        if self.focused_row.is_none() {
            return;
        }
        self.focused_row = None;
        self.focused_row_changed.emit(None);
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
    pub fn set_selection_mode(
        &mut self,
        mode: crate::widget::view_widgets::list_view::SelectionMode,
    ) {
        self.selection.set_mode(mode);
    }
    /// Returns current selection mode.
    pub fn selection_mode(&self) -> crate::widget::view_widgets::list_view::SelectionMode {
        self.selection.mode()
    }
    /// Sets column width.
    pub fn set_column_width(&mut self, column: usize, width: u32) {
        self.column_widths.insert(column, width);
        self.base.request_layout();
    }
    /// Returns explicit column width override when present.
    pub fn column_width(&self, column: usize) -> Option<u32> {
        self.column_widths.get(&column).copied()
    }
    /// Sets row height.
    pub fn set_row_height(&mut self, row: usize, height: u32) {
        self.row_heights.insert(row, height);
        self.base.request_layout();
    }
    /// Returns explicit row height override when present.
    pub fn row_height(&self, row: usize) -> Option<u32> {
        self.row_heights.get(&row).copied()
    }
    /// Sets item delegate.
    pub fn set_delegate(&mut self, delegate: Arc<dyn ItemDelegate>) {
        self.delegate = Some(delegate);
    }

    /// Begins an in-place edit of cell `(row, column)`, returning the editor the delegate built.
    ///
    /// # Why this is the delegate's *only* entry point
    ///
    /// `ItemDelegate` declares three methods -- `create_editor`, `set_editor_data`,
    /// `get_editor_data` -- and **none of them was ever called**. The delegate was stored, published
    /// (`has_delegate`), reachable through `delegate_ref`, covered by a test that set one and read it
    /// back, and had no effect on anything: a host that supplied a delegate to get custom editing got
    /// exactly the read-only table it would have had without one.
    ///
    /// The three methods are one operation, so they are driven from one place and in order:
    /// create, then seed from the model. `begin_edit` is that place, and it is public because the
    /// host decides *when* an edit starts (a double click, a key, a toolbar command) -- the widget has
    /// no opinion about the gesture, only about the sequence.
    ///
    /// Returns `None` when there is no delegate, no model, or the cell is out of range; those are
    /// three ways for the request to be meaningless rather than errors.
    pub fn begin_edit(&mut self, row: usize, column: usize) -> Option<Box<dyn Widget>> {
        let delegate = self.delegate.clone()?;
        let model = self.model.clone()?;
        if row >= model.row_count() || column >= model.column_count() {
            return None;
        }
        let mut editor = delegate.create_editor(&mut self.base, row, column)?;
        // Seeded from the delegate rather than from the model: the delegate owns the mapping between
        // a cell value and an editor's own text form (a number's units, a date's spelling), and asking
        // the model for a `String` here would bypass exactly the customisation the delegate exists for.
        delegate.set_editor_data(editor.as_mut(), row, column);
        self.editing = Some((row, column));
        self.base.request_redraw();
        Some(editor)
    }

    /// Reads the current text back out of an editor and commits it.
    ///
    /// The counterpart to [`Self::begin_edit`], and the reason `get_editor_data` exists: the editor's
    /// own representation is the delegate's business, so the value is read back through the delegate
    /// rather than by downcasting the editor to a known control. Returns the text that was committed,
    /// or `None` when there is no edit in progress or the delegate declines the cell.
    pub fn commit_edit(&mut self, editor: &dyn Widget) -> Option<String> {
        let delegate = self.delegate.clone()?;
        let (row, column) = self.editing?;
        let text = delegate.get_editor_data(editor, row, column);
        self.editing = None;
        self.base.request_redraw();
        text
    }

    /// The cell currently being edited, if any.
    pub fn editing_cell(&self) -> Option<(usize, usize)> {
        self.editing
    }

    /// Abandons an in-place edit without committing it.
    pub fn cancel_edit(&mut self) {
        if self.editing.take().is_some() {
            self.base.request_redraw();
        }
    }
    /// Returns whether a delegate is currently bound.
    pub fn has_delegate(&self) -> bool {
        self.delegate.is_some()
    }
    /// Returns the bound item delegate, if present.
    pub fn delegate_ref(&self) -> Option<&Arc<dyn ItemDelegate>> {
        self.delegate.as_ref()
    }
    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        // Get current selection and filter out invalid rows
        let mut selected_rows = self.selection.rows();
        selected_rows.retain(|row| *row < row_count);
        // Clear and re-add valid rows
        self.selection.clear();
        for row in selected_rows {
            self.selection.select_row(row);
        }
        // Update current row if invalid
        if let Some(current_row) = self.selection.current_row() {
            if current_row >= row_count {
                self.selection.clear();
            }
        }
        self.focused_row = self.focused_row.filter(|row| *row < row_count);
    }
}
impl Widget for TableWidget {
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
            crate::widget::metrics::dimensions::TABLE_WIDGET_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::TABLE_WIDGET_DEFAULT_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TableWidget`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
///
/// The old `WidgetKind::Table` arms were shared with `DataGrid` and
/// `VirtualTable`, which carry most of the names (`scroll_row`, `row_height`,
/// `sort_specs`, `visible_window`, …). Those stay with the controls that own the
/// fields; this block publishes only what `TableWidget` itself answers.
impl WidgetProperties for TableWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_model" => Ok(CapabilityValue::Bool(self.has_model())),
            "has_delegate" => Ok(CapabilityValue::Bool(self.has_delegate())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "selection_mode" => Ok(CapabilityValue::String(
                selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selection_mode" => {
                self.set_selection_mode(expect_selection_mode(value)?);
                Ok(())
            }
            "column_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "has_delegate" => Err(CapabilityAccessError::ReadOnlyProperty),
            "has_model" => Err(CapabilityAccessError::ReadOnlyProperty),
            "row_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_model",
            "has_delegate",
            "row_count",
            "column_count",
            "selection_mode",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `table` publishes. Both are payload-free.
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

impl Draw for TableWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        // The table's content sits inside its own surface, not flush against it. Every row
        // used to start at the control's literal top edge, so the first row's text began at
        // y=0 — with no header row above it and no margin anywhere, the glyph box's top edge
        // landed exactly on the frame's own stroke. The rows are laid out from the inset
        // content box instead, and the header row the inset reserves is what the first
        // content row then sits below.
        let content = ControlMetrics::band_inset(rect, TABLE_INSET);
        let header = ControlMetrics::top_band(content, HEADER_ROW_HEIGHT);
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the grid lines, the focused-row highlight
        // and the text colour used to be hardcoded literals, so light and dark rendered
        // identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("table");
        let mut surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A panel
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The row and column separators are the theme's `outline_variant` — the **weak**
        // separator role — so a table's grid lines are visibly weaker than the focus ring,
        // which uses the stronger `outline`. Both previously resolved to the same grey,
        // because the grid line was a blend of the surface and the ring read `secondary`;
        // reading the two roles keeps them distinct in every appearance, and lets a theme
        // dim its grid lines without touching every control that draws one.
        let grid_ink = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.12));
        // The focused row is a selection state, so it reads the theme's accent token and is
        // laid over the surface, which keeps it legible in either appearance.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let focused_bg = surface.blend(&accent, 0.30);

        // Draw background and border over the control's own rectangle, so the surface is
        // the whole control; the rows below are inset from it.
        context.face(
            rect,
            surface,
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);
        // Header row: the band the content rows begin below. It carries the surface's own
        // ink as a rule, so a table with a model reads as a table before its first row.
        if content.height > header.height {
            context.fill_rect(header, surface.blend(&ink, 0.06));
            context.draw_line(
                crate::core::Point::new(header.x, header.y + header.height as i32),
                crate::core::Point::new(
                    header.x + header.width as i32,
                    header.y + header.height as i32,
                ),
                border,
            );
        }
        // The rows begin at the header's bottom edge, which is what keeps the first content
        // row off y=0 (the defect the inset exists to fix).
        // A hovered row reads the same accent at a **lighter** weight than the focused row: it is
        // pointer position rather than a committed selection, so it must not compete with the row
        // that is actually selected.
        let hovered_bg = surface.blend(&accent, 0.12);
        // Draw grid from model
        if let Some(ref model) = self.model {
            let row_count = model.row_count();
            let col_count = model.column_count();
            let current_row = self.focused_row;
            for r in 0..row_count {
                // The row box comes from `row_rect`, the same derivation the hit test reads. It was
                // a local `let row_h = 20` here and a second one in the press arm, measured from
                // `rect.y`, so the row drawn and the row clicked were off by the header.
                let Some(row_box) = self.row_rect(r) else { break };
                let (y, row_h) = (row_box.y, row_box.height as i32);
                if Some(r) == current_row {
                    context.fill_rect(row_box, focused_bg);
                } else if Some(r) == self.hovered_row {
                    context.fill_rect(row_box, hovered_bg);
                }
                for c in 0..col_count {
                    // The column's box comes from the same two derivations the hit test and the header
                    // read, so a click, the heading and the cell cannot disagree about which column
                    // an x belongs to.
                    let col_w = self.resolved_column_width(c, content.width);
                    let x = content.x + self.column_offset(c, content.width) as i32;
                    if let Some(text) = model.data(r, c) {
                        // The cell is the band: a text origin of `y + row_h / 2` would put the
                        // glyph box's *top* edge on the row's middle line and draw every label
                        // half a line low. `text_line` derives the centred line box instead, and
                        // it also bounds the label so a long cell value cannot run into the next
                        // column.
                        let cell = crate::core::Rect::new(x, y, col_w, row_h as u32);
                        if !text.is_empty() {
                            context.draw_text_fitted(
                                context.text_line(cell, &crate::core::Font::default()),
                                &text,
                                &crate::core::Font::default(),
                                ink,
                                HorizontalAlignment::Left,
                            );
                        }
                    }
                    // Draw column separator
                    if c < col_count - 1 {
                        context.draw_line(
                            crate::core::Point::new(x + col_w as i32, y),
                            crate::core::Point::new(x + col_w as i32, y + row_h),
                            grid_ink,
                        );
                    }
                }
                // Draw row separator
                if r < row_count - 1 {
                    context.draw_line(
                        crate::core::Point::new(content.x, y + row_h),
                        crate::core::Point::new(content.x + content.width as i32, y + row_h),
                        grid_ink,
                    );
                }
            }
        }
    }
}
impl crate::event::EventHandler for TableWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        // The base keeps the control-level hover/press facts and answers `widget_state()`. This
        // handler did not forward to it, so `"table:hover"` could never fire.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button } if *button == 1 => {
                // The row is read from `row_at_point`, the same derivation the paint loop uses.
                // The press arm used to compute its own `(pos.y - rect.y) / 20`, which measured from
                // the control instead of from the content box below the header, so a click on the
                // first content row selected the row above it.
                if let Some(index) = self.row_at_point(*pos) {
                    self.select_row(index);
                }
            }
            crate::event::Event::MouseMove { pos } => {
                let hovered = self.row_at_point(*pos);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseLeave { .. } => {
                let had_hover = self.hovered_row.take().is_some();
                if had_hover {
                    self.base.request_redraw();
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::event::types::EventHandler;
    use std::sync::Arc;
    use std::sync::Mutex;

    struct StaticTableModel;

    impl TableModel for StaticTableModel {
        fn row_count(&self) -> usize {
            2
        }

        fn column_count(&self) -> usize {
            2
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            match (row, column) {
                (0, 0) => Some("r0c0".to_string()),
                (0, 1) => Some("r0c1".to_string()),
                (1, 0) => Some("r1c0".to_string()),
                (1, 1) => Some("r1c1".to_string()),
                _ => None,
            }
        }
    }

    struct NoopDelegate;

    impl ItemDelegate for NoopDelegate {
        fn create_editor(
            &self,
            _parent: &mut BaseWidget,
            _row: usize,
            _column: usize,
        ) -> Option<Box<dyn Widget>> {
            None
        }

        fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}

        fn get_editor_data(
            &self,
            _editor: &dyn Widget,
            _row: usize,
            _column: usize,
        ) -> Option<String> {
            None
        }
    }

    #[test]
    fn table_widget_model_delegate_and_size_accessors() {
        let mut table = TableWidget::new(Rect::new(0, 0, 200, 120));

        assert!(!table.has_model());
        assert!(table.model_ref().is_none());
        assert!(!table.has_delegate());
        assert!(table.delegate_ref().is_none());

        table.set_model(Arc::new(StaticTableModel));
        assert!(table.has_model());
        assert!(table.model_ref().is_some());
        assert_eq!(table.item(99, 0), None);

        table.set_column_width(2, 140);
        table.set_row_height(1, 28);
        assert_eq!(table.column_width(2), Some(140));
        assert_eq!(table.row_height(1), Some(28));
        assert_eq!(table.column_width(999), None);
        assert_eq!(table.row_height(999), None);

        table.set_delegate(Arc::new(NoopDelegate));
        assert!(table.has_delegate());
        assert!(table.delegate_ref().is_some());
    }

    // ── Tests moved from deprecated `TableView` alias ────────────────

    struct TestTableModel {
        rows: usize,
        cols: usize,
        data_accessed: std::sync::atomic::AtomicBool,
    }

    impl TestTableModel {
        fn new(rows: usize, cols: usize) -> Self {
            Self { rows, cols, data_accessed: std::sync::atomic::AtomicBool::new(false) }
        }
    }

    impl TableModel for TestTableModel {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            self.cols
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.data_accessed.store(true, std::sync::atomic::Ordering::SeqCst);
            Some(format!("r{}c{}", row, column))
        }
    }

    #[test]
    fn test_default_creation() {
        let tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.kind(), WidgetKind::Table);
        assert_eq!(tv.geometry(), Rect::new(0, 0, 400, 300));
        assert!(!tv.has_model());
        assert!(tv.model_ref().is_none());
        assert_eq!(tv.row_count(), 0);
        assert_eq!(tv.column_count(), 0);
        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
        assert!(tv.focused_row().is_none());
        assert!(!tv.has_delegate());
        assert!(tv.delegate_ref().is_none());
        assert!(tv.is_visible());
        assert!(tv.is_enabled());
    }

    #[test]
    fn test_set_columns() {
        let model = Arc::new(TestTableModel::new(3, 5));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert!(tv.has_model());
        assert_eq!(tv.column_count(), 5);

        tv.set_column_width(0, 120);
        assert_eq!(tv.column_width(0), Some(120));
        assert_eq!(tv.column_width(99), None);
    }

    /// A column width the caller set actually **narrows the neighbours**.
    ///
    /// # The defect this pins
    ///
    /// `column_widths` was stored, published, and never read by the layout: the draw computed an
    /// equal share per column and multiplied it by the index, so a wide column **overlapped** its
    /// neighbour instead of pushing it. `set_column_width(0, 180)` on a three-column table therefore
    /// drew column 1 at the same x as before, on top of column 0's last 60 pixels.
    ///
    /// The assertion is on the **composited geometry**: column 1 must start where column 0 ends, and
    /// the table without an override must still share equally -- the second half is what makes this
    /// about the override rather than about any layout change.
    #[test]
    fn a_column_width_the_caller_set_pushes_its_neighbours() {
        use crate::widget::svg::render_to_svg;
        let _theme_guard = crate::style::theme_test_guard();

        let build = |width: Option<u32>| {
            let model = Arc::new(TestTableModel::new(3, 3));
            let mut tv = TableWidget::new(Rect::new(0, 0, 400, 200));
            tv.set_model(model);
            if let Some(width) = width {
                tv.set_column_width(0, width);
            }
            tv
        };

        // With no override the three columns share equally, which is the pre-existing layout.
        let plain = build(None);
        let content = plain.rows_band();
        assert_eq!(
            plain.column_offset(1, content.width),
            plain.resolved_column_width(0, content.width),
            "an even share still tiles"
        );

        // With an override the first column takes exactly what it asked for, and the second starts
        // where it ends -- the property the multiplication could not express.
        let wide = build(Some(180));
        assert_eq!(wide.resolved_column_width(0, content.width), 180, "the request is honoured");
        assert_eq!(
            wide.column_offset(1, content.width),
            180,
            "and column 1 begins where column 0 ends, rather than overlapping it"
        );
        assert_eq!(
            wide.column_offset(2, content.width),
            180 + wide.resolved_column_width(1, content.width),
            "the offsets accumulate"
        );

        // And it is visible: the two tables must not paint the same cells.
        let mut plain = plain;
        let mut wide = wide;
        assert_ne!(
            render_to_svg(&mut plain),
            render_to_svg(&mut wide),
            "a column width that is read must move the grid"
        );
    }

    /// A row height the caller set moves every row below it.
    ///
    /// The same defect as the column widths, on the other axis: `row_heights` was stored and never
    /// consulted, so `set_row_height(0, 40)` drew row 0 at 20 px and then overlapped it with row 1.
    /// The click must follow the paint, which is why this asserts the hit test too.
    #[test]
    fn a_row_height_the_caller_set_moves_the_rows_below_it() {
        let model = Arc::new(TestTableModel::new(6, 2));
        let mut tv = TableWidget::new(Rect::new(0, 0, 300, 300));
        tv.set_model(model);

        // No override: the rows tile at the default height, which is what the arithmetic path gives.
        let first = tv.row_rect(0).expect("row 0 is visible");
        let second = tv.row_rect(1).expect("row 1 is visible");
        assert_eq!(second.y, first.y + TABLE_ROW_HEIGHT as i32, "the default rows tile");

        // Overriding row 0's height pushes row 1 down by exactly that height.
        tv.set_row_height(0, 44);
        let tall = tv.row_rect(0).expect("row 0 is still visible");
        let pushed = tv.row_rect(1).expect("row 1 is still visible");
        assert_eq!(tall.height, 44, "the request is honoured");
        assert_eq!(pushed.y, tall.y + 44, "row 1 begins where row 0 ends");

        // And the hit test agrees with the paint: a point in the middle of row 1 selects row 1, not
        // whichever row the default-height division would have named.
        let probe = crate::core::Point::new(pushed.x + 2, pushed.y + pushed.height as i32 / 2);
        assert_eq!(
            tv.row_at_point(probe),
            Some(1),
            "the row under the pointer is the row that was drawn there"
        );
    }

    #[test]
    fn test_rows_via_model() {
        let model = Arc::new(TestTableModel::new(4, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_count(), 0);

        tv.set_model(model);
        assert_eq!(tv.row_count(), 4);
    }

    #[test]
    fn test_cell_values() {
        let model = Arc::new(TestTableModel::new(2, 2));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));
        assert_eq!(tv.item(0, 1), Some("r0c1".to_string()));
        assert_eq!(tv.item(1, 0), Some("r1c0".to_string()));
        assert_eq!(tv.item(1, 1), Some("r1c1".to_string()));
    }

    #[test]
    fn test_cell_values_out_of_bounds() {
        let model = Arc::new(TestTableModel::new(2, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert_eq!(tv.item(5, 0), Some("r5c0".to_string()));
        assert_eq!(tv.item(0, 10), Some("r0c10".to_string()));
        assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));

        assert_eq!(TableWidget::new(Rect::new(0, 0, 100, 100)).item(0, 0), None);
    }

    #[test]
    fn test_column_and_row_counts() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_count(), 0);
        assert_eq!(tv.column_count(), 0);

        let model = Arc::new(TestTableModel::new(10, 4));
        tv.set_model(model);

        assert_eq!(tv.row_count(), 10);
        assert_eq!(tv.column_count(), 4);
    }

    #[test]
    fn test_selection_mode() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Single
        );

        tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Multi);
        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Multi
        );

        tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Extended);
        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Extended
        );

        tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Single);
        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Single
        );
    }

    #[test]
    fn test_select_and_clear_row() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
        assert!(tv.focused_row().is_none());

        assert!(tv.select_row(2));
        assert_eq!(tv.selected_row(), Some(2));
        assert_eq!(tv.focused_row(), Some(2));
        assert_eq!(tv.selected_rows(), vec![2]);

        assert!(!tv.select_row(99));
        assert_eq!(tv.selected_row(), Some(2));

        tv.clear_selection();
        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
    }

    #[test]
    fn test_focused_row() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        assert!(tv.focused_row().is_none());

        assert!(tv.set_focused_row(3));
        assert_eq!(tv.focused_row(), Some(3));

        assert!(tv.set_focused_row(3));
        assert_eq!(tv.focused_row(), Some(3));

        assert!(!tv.set_focused_row(99));
        assert_eq!(tv.focused_row(), Some(3));

        tv.clear_focused_row();
        assert!(tv.focused_row().is_none());

        tv.clear_focused_row();
        assert!(tv.focused_row().is_none());
    }

    #[test]
    fn test_selection_changed_signal() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        let captured = Arc::new(Mutex::new(None::<usize>));
        tv.selection_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        tv.select_row(1);
        assert_eq!(*captured.lock().unwrap(), Some(1));

        tv.select_row(3);
        assert_eq!(*captured.lock().unwrap(), Some(3));
    }

    #[test]
    fn test_focused_row_changed_signal() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        let captured = Arc::new(Mutex::new(None));
        tv.focused_row_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<Option<usize>>| {
                *captured.lock().unwrap() = *val;
            }
        });

        tv.set_focused_row(2);
        assert_eq!(*captured.lock().unwrap(), Some(2));

        tv.clear_focused_row();
        assert_eq!(*captured.lock().unwrap(), None);
    }

    #[test]
    fn test_geometry_delegation() {
        let mut tv = TableWidget::new(Rect::new(10, 20, 400, 300));

        assert_eq!(tv.geometry(), Rect::new(10, 20, 400, 300));

        tv.set_geometry(Rect::new(0, 0, 500, 400));
        assert_eq!(tv.geometry(), Rect::new(0, 0, 500, 400));
        assert_eq!(tv.geometry(), Rect::new(0, 0, 500, 400));
        assert_eq!(tv.position(), Point::new(0, 0));
        assert_eq!(tv.size(), crate::core::Size::new(500, 400));
    }

    #[test]
    fn test_widget_id_and_kind() {
        let tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.kind(), WidgetKind::Table);
        assert_ne!(tv.id(), 0);

        let tv2 = TableWidget::new(Rect::new(0, 0, 200, 100));
        assert_ne!(tv.id(), tv2.id());
    }

    #[test]
    fn test_svg_output() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 200, 100));

        // Without model
        let svg = crate::widget::svg::render_to_svg(&mut tv);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"100\""));

        // With model
        let model = Arc::new(TestTableModel::new(2, 3));
        let mut tv2 = TableWidget::new(Rect::new(0, 0, 300, 150));
        tv2.set_model(model);
        let svg2 = crate::widget::svg::render_to_svg(&mut tv2);
        assert!(svg2.starts_with("<svg"));
    }

    #[test]
    fn test_disabled_state_blocks_events() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        tv.set_enabled(false);
        assert!(!tv.is_enabled());

        let captured = Arc::new(Mutex::new(None::<usize>));
        tv.selection_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        // Mouse press inside widget should not trigger selection when disabled.
        //
        // The point is on the **first content row**, which is what makes this a press "inside the
        // widget" that a table is expected to act on. It used to be `(10, 15)` — inside the
        // *header* band under the corrected geometry — and it selected row 0 only because the press
        // arm measured rows from the control's top edge instead of from below the header. Fixing
        // that derivation is what turned this into a real pointer on a real row.
        let on_first_row = crate::core::Point::new(10, 30);
        tv.handle_event(&crate::event::Event::MousePress { pos: on_first_row, button: 1 });
        assert!(captured.lock().unwrap().is_none());

        // Re-enable and verify it works
        tv.set_enabled(true);
        tv.handle_event(&crate::event::Event::MousePress { pos: on_first_row, button: 1 });
        assert_eq!(*captured.lock().unwrap(), Some(0));
    }

    /// A press selects the row the paint loop draws at that point, and a hover highlights it.
    ///
    /// # The defect this pins
    ///
    /// The paint loop measured rows from the content box (below the header) and the press arm
    /// measured from the control's top edge, with `20` written out twice. A click therefore selected
    /// the row **above** the one under the pointer — the exact defect `ListView::row_rect` was
    /// extracted to fix, still present in this sibling. Both directions now read `row_rect`.
    ///
    /// # What makes this a real assertion
    ///
    /// It is written as the **inverse pair**: the point taken from `row_rect`'s midpoint must
    /// resolve, through `row_at_point`, back to the same index — and the drawn row's fill at that
    /// point must be the one that changed. A click that selects "some row" would pass the first half
    /// and fail the second.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn a_click_selects_the_row_that_is_painted_there() {
        // Same guard and pin as the hover test: both reads of a row's fill have to come from one
        // appearance for the comparison to be about selection.
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        // Row 1, not row 0: row 0 is the one a top-edge-relative hit test would also land on for a
        // point near the header, so it cannot distinguish the two derivations.
        let row = 1usize;
        let box1 = tv.row_rect(row).expect("the row must be visible");
        let point = crate::core::Point::new(box1.x + 10, box1.y + box1.height as i32 / 2);
        assert_eq!(tv.row_at_point(point), Some(row), "the point must name its own row");

        let before = row_fill(&mut tv, row).expect("the row paints a fill");
        tv.handle_event(&crate::event::Event::MousePress { pos: point, button: 1 });
        assert_eq!(tv.focused_row(), Some(row), "the press must select the row it is on");
        let after = row_fill(&mut tv, row).expect("the row still paints a fill");
        assert_ne!(before, after, "the selected row must be visibly selected");
    }

    /// The fill of the SVG `<rect>` at the midpoint of content row `index`.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn row_fill(tv: &mut TableWidget, index: usize) -> Option<String> {
        let rect = tv.geometry();
        let row = tv.row_rect(index)?;
        let at = crate::core::Point::new(row.x + 10, row.y + row.height as i32 / 2);
        let svg = crate::widget::svg::render_widget_to_svg(tv, rect);
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
    fn test_row_height_overrides() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        assert_eq!(tv.row_height(0), None);
        assert_eq!(tv.row_height(99), None);

        tv.set_row_height(1, 28);
        assert_eq!(tv.row_height(1), Some(28));

        tv.set_row_height(5, 32);
        assert_eq!(tv.row_height(5), Some(32));
        assert_eq!(tv.row_height(1), Some(28));
    }

    #[test]
    fn test_item_delegate() {
        struct TestDelegate;

        impl ItemDelegate for TestDelegate {
            fn create_editor(
                &self,
                _parent: &mut BaseWidget,
                _row: usize,
                _column: usize,
            ) -> Option<Box<dyn Widget>> {
                None
            }

            fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}

            fn get_editor_data(
                &self,
                _editor: &dyn Widget,
                _row: usize,
                _column: usize,
            ) -> Option<String> {
                None
            }
        }

        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        assert!(!tv.has_delegate());
        assert!(tv.delegate_ref().is_none());

        tv.set_delegate(Arc::new(TestDelegate));
        assert!(tv.has_delegate());
        assert!(tv.delegate_ref().is_some());
    }

    /// The delegate's three methods are **called**, in order, by one entry point.
    ///
    /// # The defect this pins
    ///
    /// `ItemDelegate` declared `create_editor` / `set_editor_data` / `get_editor_data` and the widget
    /// called **none** of them: the delegate was stored, published as `has_delegate`, reachable through
    /// `delegate_ref`, and had no effect on anything. The test above could not see that, because
    /// setting a delegate and reading it back is exactly what the broken version already supported --
    /// the same blind spot the `calendar_popup` round-trip test had.
    ///
    /// The assertion is a **call log**: the delegate records the order it was asked to do things in, so
    /// a `begin_edit` that created an editor without seeding it, or a `commit_edit` that read a cell the
    /// edit was not on, fails here rather than silently returning the wrong text.
    #[test]
    fn the_delegate_is_driven_through_begin_and_commit() {
        use std::sync::Mutex;

        #[derive(Default)]
        struct LoggingDelegate {
            calls: Mutex<Vec<String>>,
        }
        impl ItemDelegate for LoggingDelegate {
            fn create_editor(
                &self,
                _parent: &mut BaseWidget,
                row: usize,
                column: usize,
            ) -> Option<Box<dyn Widget>> {
                self.calls.lock().expect("lock").push(format!("create:{row},{column}"));
                // A real editor: the parent's own label, which the delegate then seeds.
                Some(Box::new(crate::widget::base_widgets::label::Label::new(
                    String::new(),
                    Rect::new(0, 0, 40, 16),
                )))
            }
            fn set_editor_data(&self, _editor: &mut dyn Widget, row: usize, column: usize) {
                self.calls.lock().expect("lock").push(format!("seed:{row},{column}"));
            }
            fn get_editor_data(
                &self,
                _editor: &dyn Widget,
                row: usize,
                column: usize,
            ) -> Option<String> {
                self.calls.lock().expect("lock").push(format!("read:{row},{column}"));
                Some(format!("{row}/{column}"))
            }
        }

        let delegate = Arc::new(LoggingDelegate::default());
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 200));
        tv.set_model(Arc::new(TestTableModel::new(4, 3)));

        // No delegate: the request is meaningless and answers `None` rather than panicking.
        assert!(tv.begin_edit(1, 2).is_none(), "no delegate, no editor");
        assert_eq!(tv.editing_cell(), None);

        tv.set_delegate(delegate.clone());
        let editor = tv.begin_edit(1, 2).expect("the delegate builds an editor");
        assert_eq!(tv.editing_cell(), Some((1, 2)), "the edit is tracked");
        assert_eq!(
            delegate.calls.lock().expect("lock").as_slice(),
            ["create:1,2", "seed:1,2"],
            "the editor is created and then seeded, in that order"
        );

        let committed = tv.commit_edit(editor.as_ref()).expect("the delegate reads it back");
        assert_eq!(committed, "1/2", "the value comes from the delegate, not a downcast");
        assert_eq!(tv.editing_cell(), None, "and the edit is closed");
        assert_eq!(
            delegate.calls.lock().expect("lock").as_slice(),
            ["create:1,2", "seed:1,2", "read:1,2"],
            "and the read is of the cell the edit was on"
        );

        // An out-of-range cell is refused before the delegate is troubled.
        let before = delegate.calls.lock().expect("lock").len();
        assert!(tv.begin_edit(99, 99).is_none(), "the cell must exist");
        assert_eq!(
            delegate.calls.lock().expect("lock").len(),
            before,
            "and the delegate was not asked"
        );

        // Cancelling drops the tracking without reading anything back.
        let _editor = tv.begin_edit(0, 0).expect("a second edit opens");
        tv.cancel_edit();
        assert_eq!(tv.editing_cell(), None, "cancelling closes it");
    }
}
