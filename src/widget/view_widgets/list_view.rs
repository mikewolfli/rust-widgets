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
use crate::widget::capability::coercion::{expect_selection_mode, expect_usize, expect_view_mode};
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
}
crate::impl_default_via_new!(SelectionModel);

impl SelectionModel {
    /// Creates a new selection model.
    pub fn new() -> Self {
        Self { mode: SelectionMode::Single, selected_rows: Vec::new(), current_row: None }
    }
    /// Sets selection mode.
    pub fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
        self.normalize();
    }
    /// Returns current selection mode.
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }
    /// Selects a row.
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
                // Extended selection logic
                self.selected_rows.push(row);
                self.current_row = Some(row);
            }
            SelectionMode::None => {}
        }
    }
    /// Clears selection.
    pub fn clear(&mut self) {
        self.selected_rows.clear();
        self.current_row = None;
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
    /// View mode for rendering items.
    view_mode: ViewMode,
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
            view_mode: ViewMode::default(),
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
        let content = ControlMetrics::band_inset(self.base.geometry(), LIST_INSET);
        (content.height / LIST_ROW_HEIGHT) as usize
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
        let content = ControlMetrics::band_inset(self.base.geometry(), LIST_INSET);
        let y = content.y + (LIST_ROW_HEIGHT * index as u32) as i32;
        // A row that would extend past the content box is not visible: the clip is the content
        // box's bottom edge, and a partially visible row is not painted at all rather than being
        // painted truncated — a half-height row reads as a rendering error.
        if y + LIST_ROW_HEIGHT as i32 > content.y + content.height as i32 {
            return None;
        }
        Some(Rect::new(content.x, y, content.width, LIST_ROW_HEIGHT))
    }

    /// The row index a point falls on, if it falls on a visible row.
    ///
    /// The inverse of [`Self::row_rect`] and deliberately built on it: a point is a row's when it
    /// is inside that row's band, so the two directions cannot disagree about where row 0 begins.
    fn row_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let content = ControlMetrics::band_inset(self.base.geometry(), LIST_INSET);
        if !content.contains_point(point) {
            return None;
        }
        let index = ((point.y - content.y) / LIST_ROW_HEIGHT as i32) as usize;
        (index < self.row_count()).then_some(index)
    }

    /// Focuses and selects the row under `point`, if there is one.
    ///
    /// One implementation for the mouse and the touch paths, which were two copies of the same
    /// sixteen lines that had already drifted from the paint loop's own idea of where rows are.
    fn select_row_at_point(&mut self, point: crate::core::Point) {
        let Some(index) = self.row_at_point(point) else { return };
        self.focused_row = Some(index);
        self.selection.select_row(index);
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
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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

        context.fill_rect(rect, surface);
        context.draw_rect(rect, border);
        // Draw items from model. Each row is a band of the content box, never of the
        // control, and the label's line box is derived from the row rather than from
        // `y + item_height / 2` — which put the glyph box's top edge on the row's middle
        // line and left the first row pinned to y=0.
        //
        // The row box comes from `row_rect`, which is also what the hit test reads: the row height
        // used to be a `20` literal in three places (this loop, and both press arms of
        // `handle_event`) and the press arms measured from `rect.y` while this loop measured from
        // the content box, so a press on the first row selected the second.
        if self.model.is_some() {
            let row_count = self.row_count();
            let current_row = self.focused_row;
            let font = crate::core::Font::default();
            for i in 0..row_count {
                let Some(row) = self.row_rect(i) else { break };
                if Some(i) == current_row {
                    context.fill_rect(row, focused_bg);
                }
                if let Some(text) = self.model.as_ref().and_then(|model| model.data(i)) {
                    if !text.is_empty() {
                        let cell = crate::core::Rect::new(
                            row.x + LIST_TEXT_INSET,
                            row.y,
                            row.width.saturating_sub(LIST_TEXT_INSET as u32),
                            row.height,
                        );
                        context.draw_text_fitted(
                            context.text_line(cell, &font),
                            &text,
                            &font,
                            ink,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }
        }
    }
}
impl crate::event::EventHandler for ListView {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button } if *button == 1 => {
                self.select_row_at_point(*pos);
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
}
