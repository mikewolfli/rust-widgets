// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Combo box widget — a text field with a drop-down indicator (BLUE13 R2.4).
//!
//! # The indicator drives the value's padding, and the row is assembled
//!
//! The indicator is part of the control's trailing chrome, and the value's box must *yield*
//! to it: the shared relation states this as `rightPadding: padding + indicator.width`, and
//! the reason is that a fixed text inset and a fixed indicator inset are two unrelated
//! derivations from the *same* edge. A wider indicator — or a larger font measuring one —
//! then overlaps the value instead of pushing it. The previous form could not express that
//! at all: the value's width was `rect.width - (PADDING + ARROW_SIZE + PADDING)` and the
//! indicator sat at `rect.x + rect.width - PADDING - ARROW_SIZE`, two spellings of one fact
//! in two different orders.
//!
//! BLUE22 §B.8 asks for `HBox` with the direction-appropriate padding, so the two boxes are now
//! produced by assembling the field: the value column (which fills) and the indicator column,
//! handed to a [`FlexLayout`]. The value takes the remainder *because it asked to fill*, which
//! makes "the value yields to the indicator" true by construction rather than by two subtractions
//! that happen to agree.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
#[cfg(full_widgets)]
use crate::layout::{
    AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent, LayoutParams,
};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_string, expect_text_direction, expect_usize,
    horizontal_alignment_to_str, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
use crate::widget::composite::CompositeBuilder;
use crate::widget::metrics::{dimensions, ControlMetrics};
#[cfg(full_widgets)]
use crate::widget::WidgetFactory;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Space between the indicator's leading edge and the end of the value's box.
///
/// Half the field's own horizontal padding, so the gap between the value and the indicator is
/// visibly tighter than the gap between the value and the field's edge — the reading every
/// toolkit uses, and a *relation* rather than a third independent numeral.
const INDICATOR_LEADING_GAP: u32 = dimensions::TEXT_FIELD_PADDING_H / 2;

/// The box the drop-down indicator occupies, and the space the value leaves for it.
///
/// # Why the two boxes come from one assembly
///
/// The indicator's box, the value's right inset and the value's available width were three
/// separate computations in `draw`, all spelled from the same two numerals (`PADDING = 4`,
/// `ARROW_SIZE = 8`) in three different orders, and none of them was reachable from a test.
/// Both boxes are now the output of one [`FlexLayout`] assembly, which is what makes "the value
/// yields to the indicator" a property the suite can assert instead of a coincidence — and §B.6
/// rule 2's requirement that a composite's sub-part positions come from a layout rather than from
/// arithmetic.
#[derive(Debug, Clone, Copy, PartialEq)]
struct IndicatorGeometry {
    /// The triangle's bounding box.
    box_rect: Rect,
    /// The rectangle the value text may occupy.
    text_box: Rect,
    /// The wide indicator column the triangle is the leading slice of.
    ///
    /// Kept so a right-to-left mirror can reflect the **column** rather than the slice: the column
    /// carries the field's trailing inset, and reflecting only the narrow box would drop the inset
    /// on the wrong side of it. Private because it is a derivation detail — callers want the two
    /// boxes, and publishing the middle value would invite a second way to compute them.
    column: Rect,
}

impl IndicatorGeometry {
    /// Re-slices the triangle's box out of the current column, on the side that faces the inside.
    ///
    /// # Why the side is a parameter
    ///
    /// In a left-to-right field the triangle is the column's **leading** slice, so the field's
    /// trailing inset ends up outside it against the band's right edge. A right-to-left field is the
    /// mirror: the triangle is the column's **trailing** slice, and the same inset ends up outside it
    /// against the band's left edge. Picking the wrong slice puts the icon hard against the band's
    /// edge with the inset on its inner side — a visible asymmetry that only shows up in one
    /// direction.
    fn recompute_box(&mut self, mirror: bool) {
        let width = self.box_rect.width.min(self.column.width);
        let x = if mirror {
            self.column.x + self.column.width as i32 - width as i32
        } else {
            self.column.x
        };
        self.box_rect.x = x;
        self.box_rect.width = width;
    }
    /// Assembles the field's three columns and reports the two boxes.
    ///
    /// `line_height` is a parameter rather than measured here, for the same reason
    /// `CheckBox::indicator_rect` takes one: the indicator must sit on the *value's* line box,
    /// and only the caller knows which font the value is drawn in. Deriving the two from the
    /// band's midpoint instead is how a glyph ends up half a line from the text it labels.
    ///
    /// # The children and what each is for
    ///
    /// 1. **The value column**, which declares `fill`: it takes whatever the trailing columns
    ///    leave. Its own floor is the field's leading padding, so a band narrower than the
    ///    indicator plus that padding collapses it to zero rather than inverting it.
    ///    Its trailing margin is the gap to the indicator, expressed on the value's *own*
    ///    trailing side — a margin, so it is room the text never draws in.
    /// 2. **The indicator column**, which declares the field's trailing padding on its own
    ///    trailing side, so the indicator lines up with the value's leading inset. That symmetry is
    ///    the whole reason the box is derived from the band rather than from the control's
    ///    rectangle.
    ///
    /// Two children rather than three: the leading gap rides on the value column's trailing
    /// margin, which is how [`LayoutParams`] expresses inter-element space — a third child holding
    /// a fixed gap would be a column whose only job is to be empty.
    /// [`Self::for_band_in`] for an explicit writing direction.
    ///
    /// # How the mirror is expressed
    ///
    /// The boxes are derived **once**, left-to-right, and then reflected within the band when the
    /// direction is right-to-left. Deriving a second set of rectangles for RTL is what makes the two
    /// directions drift — BLUE21 §4.3's lesson is that both directions must share **one inset** — so
    /// the reflection is applied to the finished boxes rather than to the arithmetic that produced
    /// them.
    ///
    /// A horizontal reflection of a box inside the band is exact: `x' = band.right - (x - band.x) -
    /// width`. Because the two boxes tile the band's width, the reflected pair tiles it too, and the
    /// values that were adjacent inward stay adjacent inward — which is the relation the row exists
    /// to state.
    fn for_band_in(band: Rect, line_height: u32, direction: crate::core::TextDirection) -> Self {
        let mut placed = Self::for_band_ltr(band, line_height);
        if !direction.is_right_to_left() {
            return placed;
        }
        // Mirror the two **columns**, not the finished boxes. The narrow triangle box is the
        // *leading slice* of the wide indicator column (which also carries the field's trailing
        // inset), so reflecting only the slice would place it at a third position: the inset has to
        // travel with the column it belongs to. That is the "both directions share one inset" rule
        // BLUE21 §4.3 records, expressed as one reflection of the pair.
        let band_right = band.x + band.width as i32;
        let reflect = |rect: Rect| -> Rect {
            let x = band_right - (rect.x - band.x) - rect.width as i32;
            Rect::new(x, rect.y, rect.width, rect.height)
        };
        placed.text_box = reflect(placed.text_box);
        placed.column = reflect(placed.column);
        placed.recompute_box(true);
        placed
    }

    /// The left-to-right derivation, which is the only one written out.
    fn for_band_ltr(band: Rect, line_height: u32) -> Self {
        let indicator_width = dimensions::BUTTON_ICON_SIZE.min(band.width);
        let height = line_height.min(band.height);
        // # Why the stripped profiles take the direct route
        //
        // `mini`/`embedded` have neither `WidgetFactory` nor `Box` under `alloc_frugal`
        // (principle #47), so the assembly cannot exist there. Both arms read the *same* two
        // numbers (`indicator_width`, `INDICATOR_LEADING_GAP` and the field's padding), so the
        // fallback is the same relation written the only way that profile can express it rather
        // than a second derivation.
        #[cfg(not(full_widgets))]
        let (text_box, column) = {
            let text_right = band.x
                + band.width.saturating_sub(
                    dimensions::TEXT_FIELD_PADDING_H + INDICATOR_LEADING_GAP + indicator_width,
                ) as i32;
            let text_left = band.x + dimensions::TEXT_FIELD_PADDING_H as i32;
            let text_left = text_left.min(text_right);
            (
                Rect::new(
                    text_left,
                    band.y,
                    text_right.saturating_sub(text_left) as u32,
                    band.height,
                ),
                Rect::new(
                    text_right,
                    band.y,
                    (band.x + band.width as i32 - text_right).max(0) as u32,
                    band.height,
                ),
            )
        };
        #[cfg(full_widgets)]
        let (text_box, column) = {
            let factory = WidgetFactory::new_with_defaults();
            let mut row = CompositeBuilder::new(
                Box::new(FlexLayout::with_params(
                    FlexDirection::Row,
                    FlexWrap::NoWrap,
                    JustifyContent::FlexStart,
                    AlignItems::Stretch,
                    0,
                    0,
                )),
                EdgeOffsets::all(0),
                Size::new(0, 0),
            );
            // The value column's own floor is the field's leading padding plus the gap to the
            // indicator: it must never be squeezed to nothing while the field still has room for a
            // value.
            let value = row.add_sized(
                &factory,
                "label",
                "",
                Size::new(dimensions::TEXT_FIELD_PADDING_H + INDICATOR_LEADING_GAP, height),
                LayoutParams::filled().with_margins(EdgeOffsets::new(
                    0,
                    INDICATOR_LEADING_GAP,
                    0,
                    0,
                )),
            );
            debug_assert!(value.is_some(), "the value column is a core control");
            // The indicator column carries the field's trailing inset in its **own preferred
            // width**, not as a trailing margin. A trailing margin was the first spelling here, and
            // it was wrong: `FlexLayout::arrange` measures the leftover after the margins, so the
            // inset became absorbable by the preceding `fill` child — the value column ate it and
            // the indicator was pushed to the band's very edge. Declaring the inset as part of the
            // column's own wide box keeps it: the solver satisfies a child's preferred size before
            // it hands anything to a sibling's `fill`.
            let indicator = row.add_sized(
                &factory,
                "label",
                "",
                Size::new(indicator_width + dimensions::TEXT_FIELD_PADDING_H, height),
                LayoutParams::new(),
            );
            debug_assert!(indicator.is_some(), "the indicator column is a core control");

            let mut placed: Vec<Rect> = Vec::with_capacity(2);
            row.arrange(band, &mut |_, rect| placed.push(rect));
            match (placed.first(), placed.get(1)) {
                (Some(value), Some(indicator)) => (*value, *indicator),
                // `debug_assert!` above makes this unreachable in a debug build; the fallback
                // places an empty value box and no indicator rather than an inverted rectangle.
                _ => (
                    Rect::new(band.x, band.y, 0, band.height),
                    Rect::new(band.x + band.width as i32, band.y, 0, band.height),
                ),
            }
        };
        // The column's box is *wide* — it includes the field's trailing inset — so the indicator
        // itself is the column's leading part, inset by nothing. Stating it as a slice rather than
        // drawing the whole column is what keeps "the inset belongs to the column" and "the triangle
        // is one icon wide" from being the same number.
        let box_rect = Rect::new(
            column.x,
            band.y + (band.height.saturating_sub(height) / 2) as i32,
            column.width.min(indicator_width),
            height,
        );
        Self { box_rect, text_box, column }
    }

    /// The triangle's three points, derived from its own box.
    ///
    /// The points were previously computed as three loose `y` values around the field's
    /// middle line with the half-width spelled inline, which made the shape a fourth
    /// expression of the same fact and left it unclamped when the field was short.
    fn points(&self) -> [Point; 3] {
        let b = self.box_rect;
        let mid_y = b.y + b.height as i32 / 2;
        let half_height = b.height as i32 / 4;
        [
            Point::new(b.x, mid_y - half_height),
            Point::new(b.x + b.width as i32, mid_y - half_height),
            Point::new(b.x + b.width as i32 / 2, mid_y + half_height),
        ]
    }
}
/// Combo box widget.
pub struct ComboBox {
    base: BaseWidget,
    items: Vec<String>,
    current_index: Option<usize>,
    editable: bool,
    max_visible_items: usize,
    /// The writing direction the field runs in.
    ///
    /// # Why a combo box needs this
    ///
    /// A combo box is a field with an **indicator at its trailing edge**, and "trailing" is a fact
    /// about the writing direction: in a right-to-left interface the arrow belongs on the left, just
    /// as `AppBar`'s action does. The row was assembled `[value, indicator]` for every caller, so an
    /// RTL host got its arrow on the wrong side — and, worse, the value's box yielded to an
    /// indicator that was no longer between it and the page edge.
    ///
    /// Only the **horizontal** axis is affected, so this is scoped exactly like
    /// [`crate::core::TextDirection`] documents: a vertical control ignores it. Defaults to
    /// left-to-right, so a caller that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field band as a matter of the
    /// field's own layout, so a `top`/`bottom` value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    /// Whether the drop-down list is showing.
    ///
    /// # Why this is the field the list needed
    ///
    /// The widget published `max_visible_items` ("how many items the drop-down shows at once") and
    /// carried `items` and `current_index` -- but **there was no drop-down at all**. The field
    /// therefore described a control that did not exist: `max_visible_items` was stored, published
    /// via `get`/`set` and covered by a round-trip test, and read by nothing, because there was
    /// nothing to clamp. This flag is what makes the list reachable, and the list is what makes the
    /// field meaningful.
    open: bool,
    /// The item the pointer is over while the list is open, if any.
    ///
    /// The highlight is what tells a user which row a click will take, so it is derived from the
    /// same row geometry the draw uses rather than from a second calculation in `handle_event`.
    hovered_item: Option<usize>,
    /// The first item the list is scrolled to.
    ///
    /// The list shows `max_visible_items` rows starting here, so a list longer than the window is
    /// reachable by keyboard even though the rows that fit on screen never change height. It is
    /// clamped against the item count on every read, so removing items cannot leave the list
    /// scrolled past its own end.
    first_visible_item: usize,
    /// Emitted with the new index after `current_index` changes, including when
    /// it is cleared to `None`. An out-of-range index is ignored (and emits
    /// nothing); re-applying the same value emits nothing.
    pub current_index_changed: Signal1<Option<usize>>,
    /// Emitted with the text of the new item after `current_index` changes; the
    /// empty string is emitted when the index is cleared.
    pub current_text_changed: Signal1<String>,
    /// Emitted after `current_index_changed` when the user activates an item
    /// (click, or keyboard confirm) with the activated item's index. Not emitted
    /// by programmatic `set_current_index`.
    pub activated: Signal1<usize>,
    /// Emitted with the new open state when the drop-down is shown or hidden, so a
    /// host can dismiss its other popups without polling.
    pub popup_visibility_changed: Signal1<bool>,
}
impl ComboBox {
    /// The band the control actually paints: full width, one field tall, centred.
    ///
    /// # Why the control is not its own rectangle
    ///
    /// A combo box is a text field with an indicator in it, and
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is what every field in this crate occupies. The
    /// 240x120 census cell drew a 240x120 slab, so a combo box and the `line_edit` beside it
    /// in the same form were different objects even though a user reads them as one. The band
    /// is the single derivation the fill, the border, the indicator and the value's box all
    /// read.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's box and the value's box, derived from the band and one line height.
    fn indicator_geometry(&self, line_height: u32) -> IndicatorGeometry {
        IndicatorGeometry::for_band_in(self.field_band(), line_height, self.direction)
    }

    /// The writing direction the field runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Sets the writing direction.
    ///
    /// The indicator moves to the other edge and the value's box yields to it there, which is the
    /// whole effect — spelled through the shared derivation so the paint, the hit test and the
    /// published geometry all follow one reflection.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.base.request_redraw();
        self.base.request_layout();
    }

    /// How the field's value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the value is centred
    /// vertically in the field band by the field's own layout — the property route refuses it
    /// through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Creates an empty combo box with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ComboBox, geometry, "ComboBox"),
            items: Vec::new(),
            current_index: None,
            editable: false,
            max_visible_items: 10,
            direction: crate::core::TextDirection::LeftToRight,
            alignment: crate::core::Alignment::Left,
            open: false,
            hovered_item: None,
            first_visible_item: 0,
            current_index_changed: Signal1::new(),
            current_text_changed: Signal1::new(),
            activated: Signal1::new(),
            popup_visibility_changed: Signal1::new(),
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns whether the combo box is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Returns item at specified index.
    pub fn item(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|s| s.as_str())
    }
    /// Adds an item.
    pub fn add_item(&mut self, text: String) {
        self.items.push(text);
        self.base.request_redraw();
    }
    /// Adds multiple items.
    ///
    /// Repaints: the popup list is what `draw` renders, so growing it without a redraw left the
    /// control showing the old list until something unrelated repainted.
    pub fn add_items(&mut self, items: Vec<String>) {
        if items.is_empty() {
            return;
        }
        self.items.extend(items);
        self.base.request_redraw();
    }
    /// Replaces all items with the given items. Clears the current selection.
    ///
    /// Repaints: both the item list and the now-empty current text are painted, so a caller that
    /// swapped the items saw the previous value and list until an unrelated event repainted.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.current_index = None;
        self.current_index_changed.emit(None);
        self.current_text_changed.emit(String::new());
        self.base.request_redraw();
    }
    /// Inserts an item at specified position.
    ///
    /// Repaints: the item list is painted, so a caller that grew it saw the old list until an
    /// unrelated event repainted.
    pub fn insert_item(&mut self, index: usize, text: String) {
        if index <= self.items.len() {
            self.items.insert(index, text);
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
                if index <= *current {
                    *current += 1;
                }
            }
            self.base.request_redraw();
        }
    }
    /// Removes item at specified index.
    ///
    /// Repaints: both the item list and (when the removed row was the current one) the cleared
    /// value are painted.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
                if index == *current {
                    self.current_index = None;
                    self.current_text_changed.emit(String::new());
                    self.current_index_changed.emit(None);
                } else if index < *current {
                    *current -= 1;
                }
            }
            self.base.request_redraw();
        }
    }
    /// Clears all items.
    ///
    /// Repaints: the list and the now-empty value are both painted.
    pub fn clear(&mut self) {
        if self.items.is_empty() && self.current_index.is_none() {
            return;
        }
        self.items.clear();
        self.current_index = None;
        self.current_text_changed.emit(String::new());
        self.current_index_changed.emit(None);
        self.base.request_redraw();
    }
    /// Returns current index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    /// Sets current index.
    pub fn set_current_index(&mut self, index: Option<usize>) {
        if index == self.current_index {
            return;
        }
        if let Some(idx) = index {
            if idx < self.items.len() {
                self.current_index = Some(idx);
                self.current_text_changed.emit(self.items[idx].clone());
                self.current_index_changed.emit(Some(idx));
            }
        } else {
            self.current_index = None;
            self.current_text_changed.emit(String::new());
            self.current_index_changed.emit(None);
        }
        self.base.request_redraw();
    }
    /// Returns current text.
    pub fn current_text(&self) -> String {
        self.current_index.and_then(|idx| self.items.get(idx)).cloned().unwrap_or_default()
    }
    /// Sets current text (for editable combo boxes).
    ///
    /// When the text matches an existing item, that item becomes current. When
    /// it does not match and the box is editable, the text is added as a new
    /// item, which then becomes current — the usual editable-combobox contract of
    /// "type a custom value and it is kept". An empty string selects nothing.
    /// Non-editable boxes ignore the call.
    pub fn set_current_text(&mut self, text: String) {
        if !self.editable {
            return;
        }
        let index = self.items.iter().position(|item| item == &text);
        let index = match index {
            Some(idx) => Some(idx),
            None if !text.is_empty() => {
                self.items.push(text);
                Some(self.items.len() - 1)
            }
            None => None,
        };
        self.set_current_index(index);
    }
    /// Returns whether the combo box is editable.
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    /// Sets editable state.
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
        self.base.request_redraw();
    }
    /// Returns maximum number of visible items in dropdown.
    pub fn max_visible_items(&self) -> usize {
        self.max_visible_items
    }
    /// Sets maximum number of visible items in dropdown.
    ///
    /// Read by [`Self::visible_item_count`], which clamps it against the item count to size
    /// [`Self::list_rect`]. It was stored, published (`get`/`set` plus a schema row and a round-trip
    /// test) and read by nothing, because there was no list to clamp.
    ///
    /// The window is re-clamped as well as the field, so shrinking the limit on a list scrolled past
    /// its new end cannot leave the window showing nothing.
    pub fn set_max_visible_items(&mut self, max: usize) {
        self.max_visible_items = max.max(1);
        self.first_visible_item = self.first_visible_item();
        self.base.request_redraw();
    }

    /// Whether the drop-down list is showing.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Shows or hides the drop-down list.
    ///
    /// Opening scrolls the window so the chosen item is on screen, which is the only thing that makes
    /// opening a list land on the value the user already has rather than at its beginning. Closing
    /// clears the pointer highlight, because a row cannot be hovered by a list that is not showing --
    /// leaving it set would paint a highlight the moment the list re-opened, before the pointer moved.
    pub fn set_open(&mut self, open: bool) {
        if self.open == open {
            return;
        }
        self.open = open;
        if open {
            self.scroll_to_current();
        } else {
            self.hovered_item = None;
        }
        self.popup_visibility_changed.emit(open);
        self.base.request_redraw();
    }

    /// Opens the list if it is closed, closes it if it is open.
    pub fn toggle_open(&mut self) {
        self.set_open(!self.open);
    }

    /// The number of rows the list shows at once: the window [`Self::list_rect`] is sized by.
    ///
    /// Never more than the items there are, so a two-item list does not leave eight rows of empty
    /// space under it, and never zero, so an empty list still has a box to draw its "empty" row in.
    fn visible_item_count(&self) -> usize {
        self.max_visible_items.max(1).min(self.items.len().max(1))
    }

    /// The first item the list's window shows, clamped to the rows that exist.
    ///
    /// Read through this rather than the field directly, so removing items or shrinking
    /// `max_visible_items` cannot leave the window scrolled past its own end -- which would show an
    /// empty list that still claims to be open.
    fn first_visible_item(&self) -> usize {
        let last_first = self.items.len().saturating_sub(self.visible_item_count());
        self.first_visible_item.min(last_first)
    }

    /// The list's box: as wide as the field, as tall as its visible rows, below the field.
    ///
    /// # Why below rather than over
    ///
    /// A combo box normally sits at the top of whatever it is in, and a list hanging *over* the field
    /// would hide the value the user is choosing between. Below the field's bottom edge is also where
    /// the indicator points.
    ///
    /// # Why the height is derived and not stored
    ///
    /// The height is `rows x row_height`, and both terms are facts the control already has
    /// (`max_visible_items` and the shared row metric). A stored third number would be a third answer
    /// to one question.
    fn list_rect(&self) -> Rect {
        let band = self.field_band();
        let rows = self.visible_item_count();
        Rect::new(
            band.x,
            band.y + band.height as i32,
            band.width,
            dimensions::MENU_ROW_HEIGHT * rows as u32,
        )
    }

    /// The box of the item at `index`, or `None` when the list's window does not show it.
    ///
    /// One derivation for the paint, the hit test and the keyboard's "scroll to selection", so the
    /// row that is highlighted is the row a click would take -- the same rule `list_view` records for
    /// its own rows, and the same reason it states: two independent calculations of "where is row 3"
    /// drift, and the symptom is a click activating the row next to the one that was clicked.
    fn item_rect(&self, index: usize) -> Option<Rect> {
        if !self.open {
            return None;
        }
        let offset = index.checked_sub(self.first_visible_item())?;
        if offset >= self.visible_item_count() {
            return None;
        }
        let list = self.list_rect();
        Some(Rect::new(
            list.x,
            list.y + (dimensions::MENU_ROW_HEIGHT * offset as u32) as i32,
            list.width,
            dimensions::MENU_ROW_HEIGHT,
        ))
    }

    /// The item index a point falls on, if the list is open and the point is on a shown row.
    ///
    /// Built on [`Self::item_rect`] rather than recomputing the rows, so the two directions cannot
    /// disagree about where row 0 begins.
    fn item_at_point(&self, point: Point) -> Option<usize> {
        if !self.open || !self.list_rect().contains_point(point) {
            return None;
        }
        (self.first_visible_item()..self.items.len())
            .find(|index| self.item_rect(*index).is_some_and(|row| row.contains_point(point)))
    }

    /// Scrolls the window so `index` is shown, moving it as little as it takes.
    ///
    /// A window that jumped as far as it could on every step would make arrowing down a long list
    /// flicker; moving by one row when the selection leaves the window is what a list does.
    fn scroll_to(&mut self, index: usize) {
        let count = self.visible_item_count();
        if index < self.first_visible_item {
            self.first_visible_item = index;
        } else if index >= self.first_visible_item + count {
            self.first_visible_item = index + 1 - count;
        }
        self.first_visible_item = self.first_visible_item();
    }

    /// Scrolls the window so the chosen item is shown.
    fn scroll_to_current(&mut self) {
        if let Some(index) = self.current_index {
            // Open on the selected row rather than at the top: a list that always opened at item 0
            // would make the user re-find the value the field already displays, and would make
            // `max_visible_items` irrelevant for every selection past the first window.
            let count = self.visible_item_count();
            self.first_visible_item = index.saturating_sub(count / 2);
            self.first_visible_item = self.first_visible_item();
        } else {
            self.first_visible_item = 0;
        }
    }

    /// Moves the highlight by `delta` rows, clamped to the items that exist.
    ///
    /// Returns whether the highlight moved, so a caller can repaint only when something changed.
    fn move_hover(&mut self, delta: isize) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let last = self.items.len() - 1;
        let next = match self.hovered_item {
            Some(index) => (index as isize + delta).clamp(0, last as isize) as usize,
            // No highlight yet: the first move starts from whichever end the caller is heading
            // towards, so an up-arrow from nothing selects the last row rather than the first.
            None => {
                if delta < 0 {
                    last
                } else {
                    0
                }
            }
        };
        let moved = self.hovered_item != Some(next);
        self.hovered_item = Some(next);
        self.scroll_to(next);
        moved
    }

    /// Takes the highlighted row, if there is one, and closes the list.
    ///
    /// Returns whether a row was taken. The chooser is the list's only commit path, so it is where
    /// `activated` is emitted -- a programmatic `set_current_index` deliberately does not emit it.
    fn commit_hovered(&mut self) -> bool {
        let Some(index) = self.hovered_item else {
            return false;
        };
        if index >= self.items.len() {
            return false;
        }
        self.set_current_index(Some(index));
        self.activated.emit(index);
        self.set_open(false);
        true
    }
    /// Finds index of item with specified text.
    pub fn find_text(&self, text: &str) -> Option<usize> {
        self.items.iter().position(|item| item == text)
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }
    /// Shared activation logic for mouse/touch/gesture input.
    ///
    /// # What this does now, and what it used to
    ///
    /// It used to advance `current_index` by one, wrapping at the end -- so reaching item 8 of a
    /// nine-item list took eight clicks and there was no way back. That is not what a combo box's
    /// field does: clicking it **opens the list**, and the choice is made on a row. Advancing a value
    /// is what the arrow keys are for.
    ///
    /// A single-item list still commits on the click, because there is nothing to choose between and
    /// an open list of one row would make the user click twice for a foregone answer.
    fn activate_combo(&mut self) {
        self.base.clicked.emit();
        if self.items.is_empty() {
            return;
        }
        if self.items.len() == 1 {
            self.set_current_index(Some(0));
            self.activated.emit(0);
            return;
        }
        self.toggle_open();
    }
}
// Implement Widget trait
impl Widget for ComboBox {
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
        // Find widest item.
        //
        // The measured width is the item's own content width and is handed to
        // `ControlMetrics::implicit_size` as content, with the field's padding-plus-indicator
        // requirement supplying the floor. The width was previously `max_w * 8 + 30` and the
        // height a flat `24` — neither had any relation to the 120 px slab `draw` painted, nor
        // to the 48 px band it paints now.
        let widest = self.items().iter().map(|s| s.len() as u32).max().unwrap_or(8) * 8;
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing =
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: dimensions::TEXT_FIELD_PADDING_H,
        };
        // The floor: a field wide enough to hold its own leading inset, the indicator and the
        // indicator's gap, at the height every entry control in the crate shares.
        let floor = Size::new(
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(Size::new(widest, 0), padding, floor);
        // The height is the band's, not a second numeral: this is what makes "the reported
        // size and the drawn box agree" checkable rather than merely intended.
        Size::new(hint.width, self.field_band().height.max(floor.height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ComboBox`'s property contract.
impl WidgetProperties for ComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "current_index" => match self.current_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "current_text" => Ok(CapabilityValue::String(self.current_text().to_string())),
            "editable" => Ok(CapabilityValue::Bool(self.is_editable())),
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            "max_visible_items" => Ok(CapabilityValue::UInt(self.max_visible_items() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                match value {
                    CapabilityValue::Null => self.set_current_index(None),
                    other => self.set_current_index(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "current_text" => {
                self.set_current_text(expect_string(value)?);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "max_visible_items" => {
                self.set_max_visible_items(expect_usize(value)?);
                Ok(())
            }
            // Derived from the item list.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `COMBO_BOX_PROPERTIES`.
        property_names_of![
            "item_count",
            "current_index",
            "current_text",
            "editable",
            "max_visible_items",
            "direction",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `combo_box` publishes.
    ///
    /// `clear` empties the item list and drops the selection — the only zero-argument
    /// action in the set. `set_items` and `set_current_index` assign state and need a
    /// payload, so they are answered through the property route: `OutOfRange` tells the
    /// caller the name is right and the property form is the one to use.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "set_items" | "set_current_index" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ComboBox {
    /// The drop-down's interaction, and the field's activation when it is closed.
    ///
    /// # What changed, and why
    ///
    /// A press used to *cycle the value* (`activate_combo` advanced `current_index` by one). That is
    /// not what a combo box does: the user cannot reach item 7 of a nine-item list without seven
    /// clicks, and there was no way to back up. Now a press on the field opens the list, a press on a
    /// row takes it, and a press outside closes it -- which is also what makes
    /// `popup_visibility_changed` worth publishing.
    ///
    /// The events are routed in one order: the list first (it is over everything), then the field,
    /// then the keyboard. A press that lands on neither still closes an open list rather than being
    /// swallowed, because a popup that cannot be dismissed by clicking away traps the user.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                // A row of the open list takes priority: it is drawn over the field, so a press
                // inside it belongs to the list even where the two boxes overlap.
                if let Some(index) = self.item_at_point(*pos) {
                    self.hovered_item = Some(index);
                    self.commit_hovered();
                    return;
                }
                if self.open {
                    // A press anywhere else dismisses the list. `activate_combo` is deliberately not
                    // called here: the field is already showing a value, and re-opening on the same
                    // press that closed it would make the list impossible to close.
                    self.set_open(false);
                    return;
                }
                if self.field_band().contains_point(*pos) {
                    self.activate_combo();
                }
            }
            // The highlight follows the pointer, derived from the same row geometry the paint uses,
            // so the row the user sees highlighted is the row a click would take. A pointer that
            // leaves the rows (the gap, or outside the list) clears it rather than latching the last
            // row it crossed.
            Event::MouseMove { pos } => {
                if self.open {
                    let hovered = self.item_at_point(*pos);
                    if hovered != self.hovered_item {
                        self.hovered_item = hovered;
                        self.base.request_redraw();
                    }
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                if let Some(index) = self.item_at_point(*pos) {
                    self.hovered_item = Some(index);
                    self.commit_hovered();
                } else if self.open {
                    self.set_open(false);
                } else if self.field_band().contains_point(*pos) {
                    self.activate_combo();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } => {
                self.activate_combo();
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                // Up/Down move the *highlight* while the list is open, which is what makes the list
                // navigable without a pointer. With the list closed they keep the old behaviour of
                // stepping the value, because a closed field has no highlight to move.
                38 => {
                    if self.open {
                        if self.move_hover(-1) {
                            self.base.request_redraw();
                        }
                    } else if let Some(current) = self.current_index {
                        if current > 0 {
                            self.set_current_index(Some(current - 1));
                            self.activated.emit(current - 1);
                        }
                    } else if !self.items.is_empty() {
                        self.set_current_index(Some(self.items.len() - 1));
                        self.activated.emit(self.items.len() - 1);
                    }
                }
                40 => {
                    if self.open {
                        if self.move_hover(1) {
                            self.base.request_redraw();
                        }
                    } else if let Some(current) = self.current_index {
                        if current < self.items.len().saturating_sub(1) {
                            self.set_current_index(Some(current + 1));
                            self.activated.emit(current + 1);
                        }
                    } else if !self.items.is_empty() {
                        self.set_current_index(Some(0));
                        self.activated.emit(0);
                    }
                }
                13 => {
                    // Enter takes the highlighted row when the list is open, and re-activates the
                    // current one when it is closed -- two different commitments, so the open arm
                    // does not fall through to the closed one and emit `activated` twice.
                    if self.open {
                        if !self.commit_hovered() {
                            self.set_open(false);
                        }
                    } else if let Some(current) = self.current_index {
                        self.activated.emit(current);
                    }
                }
                // Escape closes an open list without committing, which is the one way out that does
                // not change the value.
                27 => {
                    if self.open {
                        self.set_open(false);
                    }
                }
                // Space and the "open" key both toggle the list, so a keyboard user has a key that
                // does what a click on the field does. The empty-list guard is in the match arm's own
                // condition rather than nested inside it, which is also what keeps the two spellings
                // of "can this list be opened" from drifting.
                32 | 113 if !self.items.is_empty() => {
                    self.toggle_open();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for ComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let style = self.style();

        // ── The band actually painted ──
        //
        // `geometry()` is the area the control was *given*; a combo box is a text field, and a
        // field is a fixed-height band. Painting the given rectangle made a 240x120 census cell
        // a 240x120 surface, and the value and the indicator were then positioned against an
        // edge that was itself not the field's.
        let band = self.field_band();
        if band.width == 0 || band.height == 0 {
            return;
        }

        // Draw background
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        // # Why the field acknowledges the pointer
        //
        // A closed combo box is a field the user is invited to press to open a list, and it
        // used to paint its resting fill whatever the pointer did — the only hover in this
        // control was `hovered_item`, which applies to *list rows* and therefore cannot fire
        // while the list is shut, i.e. in the state the field is normally seen in. The weight
        // comes from `StateOverlay::fill_blend` so it matches the theme's `"<kind>:hover"` key.
        let overlay = crate::style::StateOverlay::from_base(
            self.base.is_hovered(),
            self.base.is_pressed(),
            self.base.draws_focus_ring(),
        );
        let bg = if overlay.is_empty() {
            bg
        } else {
            bg.blend(&bg.contrast_color(), overlay.fill_blend())
        };
        context.fill_rect(band, bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(band, border_color);
        }

        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        // One line box for both the current value and the indicator, so the two cannot end up at
        // different heights. The previous form used the control's middle for the text origin
        // — which is the glyph box's top edge, so the value sat half a line low — and the same
        // point for the indicator, which is why they agreed with each other while both being
        // wrong.
        let line = context.text_line(band, font);
        // The indicator's box and the value's box come from one derivation, so the value
        // yields to the indicator instead of being clipped by a second, unrelated inset.
        let geometry = self.indicator_geometry(line.height);

        // Draw dropdown indicator, on the same line box as the value.
        let arrow_color = style.text_color.unwrap_or(Color::rgb(100, 100, 100));
        let [apex_left, apex_right, tip] = geometry.points();
        context.draw_line(apex_left, apex_right, arrow_color);
        context.draw_line(apex_right, tip, arrow_color);
        context.draw_line(tip, apex_left, arrow_color);

        // Draw current text, or the placeholder when the list is empty. Bounded to the box the
        // indicator left, so a long value is fitted rather than run under the indicator.
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        let current_text = self.current_text();
        let value_line = context.text_line(geometry.text_box, font);
        let text_box = Rect::new(
            geometry.text_box.x,
            value_line.y,
            geometry.text_box.width,
            value_line.height,
        );
        // The value's alignment is the control's own horizontal alignment; the vertical is the
        // field band's business, so only the horizontal axis is read here. The placeholder takes
        // the same alignment, so the field reads as one column whichever value it is showing.
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        if !current_text.is_empty() {
            context.draw_text_fitted(text_box, &current_text, font, text_color, value_align);
        } else if self.items.is_empty() {
            context.draw_text_fitted(text_box, "(Empty)", font, text_color, value_align);
        }

        // ── The drop-down list ──
        //
        // Drawn last so it sits over whatever follows the field, and drawn *from* `item_rect`, which
        // is also what the hit test resolves through -- so the row that is highlighted is the row a
        // click would take. `max_visible_items` is what sizes the window here; before this arm existed
        // the field described a list that no code drew.
        if self.open {
            let list = self.list_rect();
            let border_color = style.border_color.unwrap_or_else(|| bg.blend(&text_color, 0.35));
            context.fill_rect(list, bg);
            context.draw_rect(list, border_color);

            if self.items.is_empty() {
                // An open list with nothing in it still has to say so: a bare rectangle reads as a
                // rendering failure rather than as "there is nothing to choose", and a user cannot
                // tell it from a list that failed to load.
                let row = Rect::new(list.x, list.y, list.width, dimensions::MENU_ROW_HEIGHT);
                context.draw_text_fitted(
                    context.text_line(row, font),
                    "(No items)",
                    font,
                    text_color.blend(&bg, 0.45),
                    HorizontalAlignment::Left,
                );
            } else {
                for index in self.first_visible_item()..self.items.len() {
                    let Some(row) = self.item_rect(index) else { break };
                    // The highlight is two facts at two weights: a row the pointer is over is a
                    // pointer position, and the row the field currently holds is a committed value.
                    // Painting them the same would lose "which one am I already on".
                    if self.hovered_item == Some(index) {
                        context.fill_rect(row, bg.blend(&text_color, 0.18));
                    } else if self.current_index == Some(index) {
                        context.fill_rect(row, bg.blend(&text_color, 0.08));
                    }
                    // The label's band is the row inset by the field's own padding, so a list entry
                    // lines up with the value shown in the field above it.
                    let inset = dimensions::TEXT_FIELD_PADDING_H;
                    let label = Rect::new(
                        row.x + inset as i32,
                        context.text_line(row, font).y,
                        row.width.saturating_sub(inset * 2),
                        context.text_line(row, font).height,
                    );
                    context.draw_text_fitted(
                        label,
                        &self.items[index],
                        font,
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn combobox_creation_defaults() {
        let cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.items().is_empty());
        assert!(cb.is_empty());
        assert_eq!(cb.count(), 0);
        assert_eq!(cb.current_index(), None);
        assert!(!cb.is_editable());
        assert_eq!(cb.max_visible_items(), 10);
    }

    #[test]
    fn combobox_add_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_item("Item 1".to_string());
        cb.add_item("Item 2".to_string());
        assert_eq!(cb.items().len(), 2);
        assert_eq!(cb.items()[0], "Item 1");
        assert_eq!(cb.items()[1], "Item 2");
    }

    #[test]
    fn combobox_add_items_vec() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert_eq!(cb.count(), 3);
    }

    #[test]
    fn combobox_set_current_index() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        cb.set_current_index(Some(1));
        assert_eq!(cb.current_index(), Some(1));
        assert_eq!(cb.current_text(), "B".to_string());
    }

    #[test]
    fn combobox_set_current_index_none() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string()]);
        cb.set_current_index(Some(1));
        cb.set_current_index(None);
        assert_eq!(cb.current_index(), None);
        assert!(cb.current_text().is_empty());
    }

    #[test]
    fn combobox_editable() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(!cb.is_editable());
        cb.set_editable(true);
        assert!(cb.is_editable());
        cb.set_editable(false);
        assert!(!cb.is_editable());
    }

    #[test]
    fn combobox_set_current_text_selects_existing_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.add_items(vec!["Apple".to_string(), "Banana".to_string()]);
        cb.set_current_text("Banana".to_string());
        assert_eq!(cb.current_index(), Some(1));
        assert_eq!(cb.current_text(), "Banana");
        assert_eq!(cb.count(), 2);
    }

    #[test]
    fn combobox_set_current_text_adds_unknown_custom_value() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.add_items(vec!["Apple".to_string()]);
        cb.set_current_text("Custom entry".to_string());
        assert_eq!(cb.current_text(), "Custom entry");
        assert_eq!(cb.count(), 2);
        assert_eq!(cb.current_index(), Some(1));
    }

    #[test]
    fn combobox_set_current_text_ignored_when_not_editable() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["Apple".to_string()]);
        cb.set_current_text("Apple".to_string());
        assert_eq!(cb.current_index(), None);
        assert_eq!(cb.count(), 1);
    }

    #[test]
    fn combobox_set_current_text_empty_selects_nothing() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.set_current_text(String::new());
        assert_eq!(cb.current_index(), None);
        assert_eq!(cb.count(), 0);
    }

    #[test]
    fn combobox_max_visible_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert_eq!(cb.max_visible_items(), 10);
        cb.set_max_visible_items(5);
        assert_eq!(cb.max_visible_items(), 5);
        cb.set_max_visible_items(0); // floors at 1
        assert_eq!(cb.max_visible_items(), 1);
    }

    #[test]
    fn combobox_insert_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "C".to_string()]);
        cb.insert_item(1, "B".to_string());
        assert_eq!(cb.count(), 3);
        assert_eq!(cb.item(1), Some("B"));
    }

    #[test]
    fn combobox_remove_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        cb.remove_item(1);
        assert_eq!(cb.count(), 2);
        assert_eq!(cb.item(1), Some("C"));
    }

    #[test]
    fn combobox_clear() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string()]);
        cb.set_current_index(Some(0));
        cb.clear();
        assert!(cb.is_empty());
        assert_eq!(cb.current_index(), None);
    }

    #[test]
    fn combobox_find_text() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["Apple".to_string(), "Banana".to_string(), "Cherry".to_string()]);
        assert_eq!(cb.find_text("Banana"), Some(1));
        assert_eq!(cb.find_text("Missing"), None);
    }

    #[test]
    fn combobox_set_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items(vec!["X".to_string(), "Y".to_string(), "Z".to_string()]);
        assert_eq!(cb.count(), 3);
        assert_eq!(cb.current_index(), None);
    }

    #[test]
    fn combobox_geometry_delegation() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_geometry(Rect::new(10, 10, 150, 30));
        assert_eq!(cb.geometry(), Rect::new(10, 10, 150, 30));
    }

    #[test]
    fn combobox_visibility() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.is_visible());
        cb.hide();
        assert!(!cb.is_visible());
        cb.show();
        assert!(cb.is_visible());
    }

    #[test]
    fn combobox_enabled() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.is_enabled());
        cb.set_enabled(false);
        assert!(!cb.is_enabled());
        cb.set_enabled(true);
        assert!(cb.is_enabled());
    }

    #[test]
    fn combobox_id_kind() {
        let cb_a = ComboBox::new(Rect::new(0, 0, 100, 24));
        let cb_b = ComboBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(cb_a.id(), cb_b.id());
        assert_eq!(cb_a.kind(), WidgetKind::ComboBox);
        assert_eq!(cb_b.kind(), WidgetKind::ComboBox);
    }

    #[test]
    fn combobox_signal_accessors() {
        let cb = ComboBox::new(Rect::new(0, 0, 100, 24));
        let _ = &cb.current_index_changed;
        let _ = &cb.current_text_changed;
        let _ = &cb.activated;
    }

    /// The value's box ends where the indicator's box begins, at every width that can hold both.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the value *yields*
    /// to the indicator. Both boxes used to be computed from the same two numerals in two
    /// different orders — the value's width was `width - (PADDING + ARROW_SIZE + PADDING)`
    /// while the indicator sat at `width - PADDING - ARROW_SIZE` — so the two agreed only by
    /// coincidence and neither could be read from a test.
    ///
    /// The values are now the two columns of one assembled row, so the relation holds by
    /// construction. The iteration starts at the narrowest band that can hold all three
    /// requirements (the value's own floor, the gap, and the indicator with its trailing inset);
    /// a narrower one cannot be tiled and `a_band_too_narrow_for_the_indicator_overhangs` records
    /// what happens instead.
    #[test]
    fn the_value_box_ends_where_the_indicator_begins() {
        let narrowest = dimensions::TEXT_FIELD_PADDING_H  // the value's own floor
            + INDICATOR_LEADING_GAP
            + dimensions::BUTTON_ICON_SIZE
            + dimensions::TEXT_FIELD_PADDING_H; // the indicator's trailing inset
        for width in [narrowest, 64, 240, 400] {
            let cb = ComboBox::new(Rect::new(0, 0, width, 120));
            let geometry = cb.indicator_geometry(14);
            let band = cb.field_band();
            assert_eq!(
                geometry.text_box.x + geometry.text_box.width as i32 + INDICATOR_LEADING_GAP as i32,
                geometry.box_rect.x,
                "the value must stop one gap short of the indicator at width {width}"
            );
            assert_eq!(
                geometry.text_box.x, band.x,
                "the value starts at the field's leading edge at width {width}"
            );
            assert!(
                geometry.box_rect.x + geometry.box_rect.width as i32 <= band.x + band.width as i32,
                "the indicator must stay inside the band at width {width}"
            );
        }
    }

    /// A band too narrow for the value floor, the gap and the indicator keeps both inside it.
    ///
    /// # What this pins (and what G-1 changed)
    ///
    /// The three requirements together are `TEXT_FIELD_PADDING_H + INDICATOR_LEADING_GAP +
    /// BUTTON_ICON_SIZE + TEXT_FIELD_PADDING_H` (4 + 2 + 18 + 4 = 28 px). A narrower band cannot be
    /// tiled at those sizes — that is arithmetic.
    ///
    /// What the layout owes is that the shortfall is contained: before G-1 was resolved the value's
    /// column kept its floor and the indicator was pushed past the band's trailing edge, which on
    /// the SVG backend (absolute coordinates, no clip at this layer) means an **absent** drop-down
    /// arrow rather than an overflowing one. The test asserts containment plus the relation that
    /// makes a combo box a combo box: the value box is still the room the indicator leaves.
    ///
    /// The field's own `size_hint` floor is wider than this, so the case is not reachable from a
    /// form.
    #[test]
    fn a_band_too_narrow_for_the_indicator_keeps_both_inside_it() {
        let width = 20u32;
        let cb = ComboBox::new(Rect::new(0, 0, width, 120));
        let geometry = cb.indicator_geometry(14);
        assert!(
            geometry.text_box.width <= width,
            "the value box never exceeds the band: {:?}",
            geometry.text_box
        );
        assert!(
            geometry.text_box.x + geometry.text_box.width as i32 <= width as i32,
            "the value box stays inside the band: {:?}",
            geometry.text_box
        );
        assert!(
            geometry.box_rect.x >= 0
                && geometry.box_rect.x + geometry.box_rect.width as i32 <= width as i32,
            "and so does the indicator, rather than being pushed out of the field: {:?}",
            geometry.box_rect
        );
        // The value still yields to the indicator: the gap between them is the field's own, and the
        // two boxes plus that gap are the band.
        assert_eq!(
            geometry.text_box.x + geometry.text_box.width as i32 + INDICATOR_LEADING_GAP as i32,
            geometry.box_rect.x,
            "the value must still stop one gap short of the indicator"
        );
        assert_eq!(
            geometry.text_box.width + INDICATOR_LEADING_GAP + geometry.box_rect.width,
            width,
            "and the two boxes must account for the band"
        );
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let cb = ComboBox::new(Rect::new(0, 0, 240, 120));
        let band = cb.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(cb.size_hint().height, band.height);
        assert_eq!(band.width, 240, "a field spans its width");
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }

    /// The indicator is laid out on the value's own line box, not on the band's midpoint.
    #[test]
    fn the_indicator_follows_the_line_box_it_shares_with_the_value() {
        let cb = ComboBox::new(Rect::new(0, 0, 240, 120));
        for line_height in [4u32, 14, 32] {
            let geometry = cb.indicator_geometry(line_height);
            assert_eq!(
                geometry.box_rect.height, line_height,
                "the indicator's box is the line at height {line_height}"
            );
            assert_eq!(
                geometry.box_rect.y + geometry.box_rect.height as i32 / 2,
                cb.field_band().y + cb.field_band().height as i32 / 2,
                "the indicator must sit on the band's middle line at line height {line_height}"
            );
        }
    }

    /// The triangle is drawn from its own box, so its points cannot leave it.
    #[test]
    fn the_indicator_points_stay_inside_the_indicator_box() {
        let cb = ComboBox::new(Rect::new(0, 0, 240, 120));
        let geometry = cb.indicator_geometry(14);
        let b = geometry.box_rect;
        for point in geometry.points() {
            assert!(
                point.x >= b.x
                    && point.x <= b.x + b.width as i32
                    && point.y >= b.y
                    && point.y <= b.y + b.height as i32,
                "{point:?} escaped the indicator box {b:?}"
            );
        }
    }

    /// A right-to-left field puts its indicator on the other edge, and its value yields to it there.
    ///
    /// # The defect this pins
    ///
    /// The row was assembled `[value, indicator]` for every caller, so an RTL host got its arrow on
    /// the wrong side — and the value's box yielded to an indicator that was no longer between it and
    /// the page edge. `AppBar` states the same rule for its own two affordances; a combo box is that
    /// shape on a field.
    ///
    /// The assertion is a relation between the two directions, not a literal x: the indicator must
    /// keep its trailing inset in both, and both boxes must stay inside the band, so a mirror that
    /// pushed something out of the field fails.
    #[test]
    fn a_right_to_left_field_mirrors_the_indicator() {
        use crate::core::TextDirection;
        use crate::widget::capability::WidgetProperties;

        let mut ltr = ComboBox::new(Rect::new(0, 0, 200, 30));
        ltr.set_direction(TextDirection::LeftToRight);
        let mut rtl = ComboBox::new(Rect::new(0, 0, 200, 30));
        rtl.set_direction(TextDirection::RightToLeft);

        let band = ltr.field_band();
        let left = ltr.indicator_geometry(0);
        let right = rtl.indicator_geometry(0);

        // The indicator keeps its trailing inset and hugs the other edge. In a reflection it is the
        // *insets* that correspond, not the right edges: the LTR box ends `inset` from the band's
        // right edge, and the RTL box must end `inset` from the band's **left** edge.
        let band_left_inset = left.box_rect.x - band.x;
        let band_right_inset =
            (band.x + band.width as i32) - (left.box_rect.x + left.box_rect.width as i32);
        assert_eq!(
            right.box_rect.x - band.x,
            band_right_inset,
            "the RTL inset must equal the LTR trailing inset, or the two directions are not mirrors"
        );
        assert_eq!(
            (band.x + band.width as i32) - (right.box_rect.x + right.box_rect.width as i32),
            band_left_inset,
            "and the RTL trailing inset must equal the LTR leading inset"
        );
        // The two are reflections of each other across the band's centre, so each one is on the edge
        // the *other* direction leaves free. (An LTR field's arrow is already on the right, which is
        // what makes "RTL moves it left" the correct reading rather than the reverse.)
        assert_eq!(
            left.box_rect.x + right.box_rect.x + right.box_rect.width as i32,
            band.x + band.x + band.width as i32,
            "the two boxes must be mirror images about the band's centre"
        );
        assert!(
            right.box_rect.x < left.box_rect.x,
            "an RTL field's indicator belongs on the other edge: ltr x={}, rtl x={}",
            left.box_rect.x,
            right.box_rect.x
        );

        // The whole row still tiles the band, so nothing was pushed out by the mirror.
        for (label, geom) in [("ltr", left), ("rtl", right)] {
            let band_right = band.x + band.width as i32;
            assert!(
                geom.text_box.x >= band.x
                    && geom.text_box.x + geom.text_box.width as i32 <= band_right,
                "{label}: the value box left the band: {:?}",
                geom.text_box
            );
            assert!(
                geom.box_rect.x >= band.x
                    && geom.box_rect.x + geom.box_rect.width as i32 <= band_right,
                "{label}: the indicator box left the band: {:?}",
                geom.box_rect
            );
        }

        // And the direction round-trips through the property API, so a host can read what it set.
        assert_eq!(rtl.get("direction").expect("direction is published").as_str(), Some("rtl"));
    }

    // ── The drop-down list, and `max_visible_items` ──

    /// `max_visible_items` sizes the list, and the list is what makes it meaningful.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// The field was stored, published (`get`/`set` plus a schema row and a round-trip test) and read
    /// by nothing -- because **there was no list at all**. `count`, `current_index` and
    /// `max_visible_items` coexisted while nothing was ever drawn for them, so a user could not see a
    /// single item they had added. The assertion is on the list's own box, which is what a paint has
    /// to obey.
    #[test]
    fn max_visible_items_sizes_the_drop_down_list() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        let mut items = Vec::new();
        for n in 0..20 {
            items.push(format!("item {n}"));
        }
        cb.set_items(items);
        let band = cb.field_band();

        // Closed: there is no list, so no row has a box either.
        assert!(!cb.is_open());
        assert_eq!(cb.item_rect(0), None, "a closed list has no rows");

        cb.set_open(true);
        assert!(cb.is_open());
        // The list is as wide as the field and sits directly below it, so it never covers the value.
        let list = cb.list_rect();
        assert_eq!(list.x, band.x);
        assert_eq!(list.width, band.width);
        assert_eq!(list.y, band.y + band.height as i32, "the list hangs below the field");
        // And its height is the window, not the item count: 20 items with a limit of 10 must not
        // produce a 20-row box.
        assert_eq!(list.height, dimensions::MENU_ROW_HEIGHT * 10);

        cb.set_max_visible_items(3);
        assert_eq!(
            cb.list_rect().height,
            dimensions::MENU_ROW_HEIGHT * 3,
            "the limit is what sizes the window"
        );
        // Fewer items than the limit: the list shrinks to the items rather than leaving empty rows.
        let mut short = ComboBox::new(Rect::new(0, 0, 200, 24));
        short.set_items(vec!["a".to_string(), "b".to_string()]);
        short.set_max_visible_items(10);
        short.set_open(true);
        assert_eq!(short.list_rect().height, dimensions::MENU_ROW_HEIGHT * 2);
        // An empty list still has a row to draw its "no items" message in.
        let mut empty = ComboBox::new(Rect::new(0, 0, 200, 24));
        empty.set_open(true);
        assert_eq!(empty.list_rect().height, dimensions::MENU_ROW_HEIGHT);
    }

    /// The rows the list paints are the rows a click resolves to: the two share one derivation.
    #[test]
    fn a_click_takes_the_row_it_is_painted_on() {
        use crate::core::Point;

        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items((0..8).map(|n| format!("item {n}")).collect());
        cb.set_max_visible_items(4);
        // Scrolled into the middle of the list, so the window is *not* the first `max` rows: a
        // window that ignored the limit (or the scroll) would lay these rows out at 0..4 and the
        // centre of each painted row would hit-test to a different index.
        cb.set_current_index(Some(5));
        cb.set_open(true);
        assert!(cb.first_visible_item() > 0, "the window moved, or this proves nothing");

        let first = cb.first_visible_item();
        for index in first..first + 4 {
            let row = cb.item_rect(index).expect("four rows fit");
            let centre = Point::new(row.x + row.width as i32 / 2, row.y + row.height as i32 / 2);
            assert_eq!(
                cb.item_at_point(centre),
                Some(index),
                "the centre of row {index} at {row:?} must hit-test back to it"
            );
        }
        // A row outside the window has no box, so it is not a target either. Both ends are checked:
        // one is below the window and the other is above it.
        assert_eq!(cb.item_rect(first + 4), None, "the row past the window has no box");
        assert_eq!(cb.item_rect(first.saturating_sub(1)), None, "nor the one before it");
        assert_eq!(
            cb.item_at_point(Point::new(100, cb.list_rect().y + cb.list_rect().height as i32 + 5)),
            None,
            "a point past the list is no row"
        );
        // A point inside the list but *above* the first shown row is no row either, which is what
        // pins the rows to the list's own origin rather than to the item's index.
        assert_eq!(
            cb.item_at_point(Point::new(100, cb.list_rect().y - 3)),
            None,
            "a point above the list is no row"
        );
        // The rows tile the list exactly, in sequence.
        for index in (first + 1)..(first + 4) {
            let previous = cb.item_rect(index - 1).expect("visible");
            let row = cb.item_rect(index).expect("visible");
            assert_eq!(
                row.y,
                previous.y + previous.height as i32,
                "row {index} follows row {}",
                index - 1
            );
        }
    }

    /// The list is reachable and committable: opening, choosing, and dismissing all work.
    #[test]
    fn pressing_the_field_opens_the_list_and_a_row_commits() {
        use crate::event::{Event, EventHandler};
        use crate::widget::svg::{render_to_svg, text_ink_boxes};

        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items((0..6).map(|n| format!("item {n}")).collect());
        cb.set_max_visible_items(3);

        // The resting frame has no list, so the two frames below differ by the list alone.
        let resting = render_to_svg(&mut cb);

        // A press on the field opens the list rather than cycling the value -- the defect this
        // replaces made a nine-item list take nine clicks to reach item 8.
        let band_centre = crate::core::Point::new(50, cb.field_band().y + 8);
        cb.handle_event(&Event::MousePress { pos: band_centre, button: 1 });
        assert!(cb.is_open(), "a press on the field opens the list");
        assert_eq!(cb.current_index(), None, "and does not change the value");

        let open = render_to_svg(&mut cb);
        assert_ne!(open, resting, "an open list must be visible -- that was the dead state");
        assert!(
            text_ink_boxes(&open).len() > text_ink_boxes(&resting).len(),
            "the list adds text runs for its rows: {} vs {}",
            text_ink_boxes(&open).len(),
            text_ink_boxes(&resting).len()
        );

        // A press on the second row takes it and closes the list.
        let row = cb.item_rect(1).expect("the second row is in the window");
        cb.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(row.x + 5, row.y + row.height as i32 / 2),
            button: 1,
        });
        assert_eq!(cb.current_index(), Some(1), "the row under the press is taken");
        assert_eq!(cb.current_text(), "item 1");
        assert!(!cb.is_open(), "and taking a row closes the list");
    }

    /// Re-opening the list lands on the value already chosen, and the window follows it.
    ///
    /// This is the one thing that makes `max_visible_items` matter for a selection past the first
    /// window: a list that always opened at item 0 would show rows 0..3 while the field displayed
    /// item 9, so the user could not see which row was chosen.
    #[test]
    fn opening_the_list_scrolls_to_the_chosen_row() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items((0..20).map(|n| format!("item {n}")).collect());
        cb.set_max_visible_items(4);
        cb.set_current_index(Some(15));

        cb.set_open(true);
        let first = cb.first_visible_item();
        let last = first + 4 - 1;
        assert!(
            (first..=last).contains(&15),
            "the chosen row is inside the window: {first}..={last}"
        );
        assert!(cb.item_rect(15).is_some(), "and it has a box");
        // Not pinned to the top: the window moved, which is what "scroll to" means.
        assert!(first > 0, "the window moved to reach a row past the first page");
    }

    /// Escape closes without committing; the keyboard can move the highlight and take it.
    #[test]
    fn the_keyboard_drives_the_open_list() {
        use crate::event::{Event, EventHandler};

        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items((0..5).map(|n| format!("item {n}")).collect());
        let press = |cb: &mut ComboBox, key: u32| {
            cb.handle_event(&Event::KeyPress { key, modifiers: 0 });
        };

        cb.set_open(true);
        // Down twice highlights rows 0 then 1, without committing anything.
        press(&mut cb, 40);
        assert_eq!(cb.current_index(), None, "moving the highlight is not a commitment");
        press(&mut cb, 40);
        // Enter takes the highlighted row and closes the list.
        press(&mut cb, 13);
        assert_eq!(cb.current_index(), Some(1));
        assert!(!cb.is_open());

        // Escape abandons the list: the value is unchanged and the list is closed.
        cb.set_open(true);
        press(&mut cb, 40);
        press(&mut cb, 27);
        assert!(!cb.is_open(), "escape closes the list");
        assert_eq!(cb.current_index(), Some(1), "and does not change the value");

        // A key that is neither of the above leaves the state alone.
        cb.set_open(true);
        press(&mut cb, 65);
        assert!(cb.is_open());
    }

    /// Shrinking the limit on a scrolled list re-clamps the window instead of showing nothing.
    #[test]
    fn shrinking_the_window_cannot_scroll_past_the_last_row() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items((0..20).map(|n| format!("item {n}")).collect());
        cb.set_max_visible_items(10);
        cb.set_current_index(Some(19));
        cb.set_open(true);
        assert!(cb.item_rect(19).is_some(), "the last row is reachable at first");

        // Removing items is the other way the window can end up past its own end.
        cb.set_items((0..3).map(|n| format!("item {n}")).collect());
        assert!(
            cb.first_visible_item() + cb.visible_item_count() <= 3,
            "the window is clamped to the rows that exist"
        );
        assert!(cb.item_rect(0).is_some(), "and item 0 is still shown");
    }
}
