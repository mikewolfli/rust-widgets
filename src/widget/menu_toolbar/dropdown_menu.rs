// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DropdownMenu widget — a cascading/linked dropdown selector.
//!
//! The DropdownMenu displays a text field with a dropdown arrow. When clicked,
//! it expands to show a scrollable list of items. Each item has a value, label,
//! optional icon, enabled state, and optional children for submenu support.
//! Selecting an item emits an `item_selected` signal with the item's value.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_usize, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Default height of the dropdown list when expanded.
const DEFAULT_DROPDOWN_HEIGHT: u32 = 200;
/// Height of each item row in the dropdown list.
const ITEM_HEIGHT: i32 = 30;
/// Padding inside the control.
const PADDING: i32 = 8;
/// Maximum number of items visible before scrolling.
const MAX_VISIBLE_ITEMS: usize = 8;

/// The record form of the `items` property: one row per **top-level** dropdown item.
///
/// # Why this is read-only, and why the rows stop at the top level
///
/// `DropdownItem` is a public-field struct with a nested `children` vector, and a record row cannot
/// carry a subtree. A *writable* `items` property would therefore have to invent children or silently
/// drop them — and a write that loses a submenu is precisely the silent-loss shape §19.6 forbids. The
/// rows publish what a document can hold faithfully (identity, label, enabled, typed icon) and a write
/// is refused with a reason instead of a lossy success; behaviour changes through the control's own
/// API (`add_item` / `set_item_icon_source` / `items_mut`), which is what "public-field collections
/// are a version boundary" prescribes.
pub const DROPDOWN_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "value",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "label",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
        ],
        id_field: "value",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`DROPDOWN_ITEM_SCHEMA`]; refuses a source with no document form.
fn dropdown_item_record(
    item: &DropdownItem,
    icon_source: Option<IconSource>,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("value", CapabilityValue::String(item.value.clone()))
        .with("label", CapabilityValue::String(item.label.clone()))
        .with("enabled", CapabilityValue::Bool(item.enabled));
    match icon_source {
        // "No `icon_source` field" is how a document says "this row has no typed icon", and a row
        // that never had one is the normal case.
        None => {}
        Some(IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry cannot be written as a document value, and omitting it would be
        // indistinguishable from "no icon" — the same rule the six control-level owners follow.
        Some(IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    Ok(record)
}

/// A single item in a DropdownMenu.
///
/// Each item has a value (string identifier), a display label, an optional
/// icon, an enabled state, and optional children for cascading submenus.
pub struct DropdownItem {
    /// Unique value identifier for this item.
    pub value: String,
    /// Display label shown in the dropdown.
    pub label: String,
    /// Optional icon for additional visual context.
    pub icon: Option<String>,
    /// Whether this item is selectable.
    pub enabled: bool,
    /// Optional child items for submenu support.
    pub children: Vec<DropdownItem>,
}

impl DropdownItem {
    /// Creates a new DropdownItem with the given value and label.
    pub fn new(value: &str, label: &str) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
            icon: None,
            enabled: true,
            children: Vec::new(),
        }
    }

    /// Sets the icon for this item.
    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Sets the enabled state for this item.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the children for this item (submenu support).
    pub fn with_children(mut self, children: Vec<DropdownItem>) -> Self {
        self.children = children;
        self
    }
}

/// DropdownMenu widget — a text field with a dropdown list.
///
/// Displays the currently selected value (or placeholder) with a dropdown
/// arrow. When expanded, shows a scrollable list of items. Selecting an
/// item updates the selected value and emits the `item_selected` signal.
pub struct DropdownMenu {
    base: BaseWidget,
    selected_value: Option<String>,
    items: Vec<DropdownItem>,
    /// Typed icon overrides, keyed by [`DropdownItem::value`] — the identity the item type declares.
    item_icon_sources: Vec<(crate::compat::String, IconSource)>,
    expanded: bool,
    /// The list's open/close reveal, so the list grows out of the field rather than appearing.
    ///
    /// A `bool` cannot express a popup mid-open, and the draw path needs the fraction. The flag
    /// above stays the *authority* (it is what `get("expanded")` answers, and what hit testing
    /// consults); this only says how far the transition has got.
    reveal: PopupReveal,
    /// Scroll offset for the item list.
    scroll_offset: usize,
    /// How the field's value and each item's label are aligned within their own row.
    ///
    /// Both runs take the same value because they are the same column of text seen twice -- what the
    /// field shows and what the list offers below it -- so aligning one and not the other would make
    /// the selected value and the row it was chosen from disagree about where the column starts.
    /// Horizontal only: a row is centred vertically in its band by the row's own layout, so a
    /// `top`/`bottom` value would be one this control could never honour --
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    /// Emitted when an item is selected, providing the item's value.
    pub item_selected: Signal1<String>,
}

impl DropdownMenu {
    /// Creates a new DropdownMenu widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DropdownMenu, geometry, "DropdownMenu"),
            selected_value: None,
            items: Vec::new(),
            item_icon_sources: Vec::new(),
            expanded: false,
            reveal: PopupReveal::new(false),
            scroll_offset: 0,
            alignment: crate::core::Alignment::Left,
            item_selected: Signal1::new(),
        }
    }

    /// Sets the selected item by value. If the value does not match any
    /// item, the selection is cleared.
    pub fn select(&mut self, value: &str) {
        if self.items.iter().any(|item| item.value == value && item.enabled) {
            self.selected_value = Some(value.to_string());
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected value, if any.
    pub fn selected_value(&self) -> Option<String> {
        self.selected_value.clone()
    }

    /// Returns the display label for the currently selected value.
    pub fn selected_label(&self) -> Option<String> {
        self.selected_value.as_ref().and_then(|val| {
            self.items.iter().find(|item| item.value == *val).map(|item| item.label.clone())
        })
    }

    /// Returns the index of the selected item, or `None` when nothing is selected.
    ///
    /// The index addresses the item list as a whole, so it matches the index of
    /// the entry whose value is currently selected in
    /// [`selected_value`](Self::selected_value).
    pub fn selected_index(&self) -> Option<usize> {
        let selected = self.selected_value.as_deref()?;
        self.items.iter().position(|item| item.value == selected)
    }

    /// Selects the item at `index`.
    ///
    /// Mirrors what a click on that row does: an in-range, enabled item becomes
    /// the selection and emits [`item_selected`](Self::item_selected); an
    /// out-of-range index or a disabled item leaves the selection untouched.
    /// Returns whether the selection changed as a result.
    pub fn set_selected_index(&mut self, index: usize) -> Result<(), CapabilityAccessError> {
        let Some(item) = self.items.get(index) else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if !item.enabled {
            // Disabled rows are presented but deliberately not selectable.
            return Ok(());
        }
        let value = item.value.clone();
        if self.selected_value.as_deref() == Some(value.as_str()) {
            return Ok(());
        }

        self.selected_value = Some(value.clone());
        self.item_selected.emit(value);
        self.base.request_redraw();
        Ok(())
    }

    /// Adds an item to the dropdown.
    pub fn add_item(&mut self, item: DropdownItem) {
        // A re-added `value` must not inherit the previous item's icon: the override is keyed by
        // identity, so a stale entry would silently attach the old glyph to a new row.
        self.item_icon_sources.retain(|(value, _)| *value != item.value);
        self.items.push(item);
        self.base.request_redraw();
    }

    /// Assigns a typed icon source to an item by its **value**.
    ///
    /// A typed source takes precedence while set, without discarding the item's legacy text icon.
    /// Passing `None` restores that text icon.
    ///
    /// # Why the item's value and not its index
    ///
    /// `DropdownItem::value` is documented as a *unique* identifier, so it is the identity the
    /// collection really has, and §19.6 requires the typed channel to be keyed on real identity
    /// rather than a position: an index association silently reattaches to whatever row later moves
    /// into that slot. The index-keyed form this replaced was justified by a claim that the
    /// collection had only positional identity, which its own item type disproves.
    pub fn set_item_icon_source(
        &mut self,
        value: &str,
        source: Option<IconSource>,
    ) -> Result<(), crate::widget::IconItemError> {
        match self.items.iter().filter(|item| item.value == value).count() {
            0 => {
                return Err(crate::widget::IconItemError::UnknownItemId {
                    id: crate::compat::String::from(value),
                })
            }
            1 => {}
            // Two rows sharing a value cannot be addressed apart, and picking the first would be a
            // silent coin flip (the same reasoning `IconItemError::AmbiguousItemId` records).
            _ => {
                return Err(crate::widget::IconItemError::DuplicateItemId {
                    id: crate::compat::String::from(value),
                })
            }
        }
        self.item_icon_sources.retain(|(existing, _)| existing != value);
        if let Some(source) = source {
            self.item_icon_sources.push((crate::compat::String::from(value), source));
        }
        self.base.request_redraw();
        Ok(())
    }

    /// The typed icon source assigned to `value`, if any.
    pub fn item_icon_source(&self, value: &str) -> Option<IconSource> {
        self.item_icon_sources.iter().find(|(id, _)| id == value).map(|(_, source)| *source)
    }

    /// Removes an item by value. Returns true if an item was removed.
    pub fn remove_item(&mut self, value: &str) -> bool {
        let len_before = self.items.len();
        self.items.retain(|item| item.value != value);
        let removed = self.items.len() < len_before;
        if removed {
            self.item_icon_sources.retain(|(id, _)| id != value);
            // Clear selection if the removed item was selected
            if self.selected_value.as_deref() == Some(value) {
                self.selected_value = None;
            }
            self.base.request_redraw();
        }
        removed
    }

    /// Returns a reference to all items.
    pub fn items(&self) -> &[DropdownItem] {
        &self.items
    }

    /// Returns mutable access to the items, clearing typed icon assignments first.
    ///
    /// A caller can replace an item's `value` through this vector, which would leave an override
    /// pointing at identity the control can no longer verify as the same row; clearing is the honest
    /// answer rather than keeping an association the caller may have broken.
    pub fn items_mut(&mut self) -> &mut Vec<DropdownItem> {
        self.item_icon_sources.clear();
        &mut self.items
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the dropdown is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands the dropdown to show the item list.
    pub fn expand(&mut self) {
        self.expanded = true;
        self.reveal.set_open(true);
        self.scroll_offset = 0;
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Collapses the dropdown to hide the item list.
    pub fn collapse(&mut self) {
        self.expanded = false;
        self.reveal.set_open(false);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.id());
        self.base.request_redraw();
    }

    /// Toggles the expanded state.
    pub fn toggle(&mut self) {
        if self.expanded {
            self.collapse();
        } else {
            self.expand();
        }
    }

    /// How the field's value and each item's label are aligned within their own row.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value and each item's label are aligned within their own row.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored** because a row is centred
    /// vertically in its band by the row's own layout -- the property route refuses it through
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Computes the dropdown list rectangle below the text field.
    fn dropdown_rect(&self) -> Rect {
        let geom = self.geometry();
        let item_count = self.items.len().min(MAX_VISIBLE_ITEMS);
        let height = (item_count as u32 * ITEM_HEIGHT as u32).min(DEFAULT_DROPDOWN_HEIGHT);
        Rect::new(geom.x, geom.y + geom.height as i32, geom.width, height)
    }

    /// Computes the rectangle for an item at the given index.
    fn item_rect(&self, index: usize) -> Rect {
        let drop = self.dropdown_rect();
        Rect::new(
            drop.x,
            drop.y + (index - self.scroll_offset) as i32 * ITEM_HEIGHT,
            drop.width,
            ITEM_HEIGHT as u32,
        )
    }

    /// Finds the item index at a given position, if any.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }
        let drop = self.dropdown_rect();
        if !drop.contains_point(pos) {
            return None;
        }
        let visible_end = (self.scroll_offset + MAX_VISIBLE_ITEMS).min(self.items.len());
        for i in self.scroll_offset..visible_end {
            let ir = self.item_rect(i);
            if ir.contains_point(pos) && self.items[i].enabled {
                return Some(i);
            }
        }
        None
    }
}

impl Widget for DropdownMenu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The field **and** the expanded list accept the pointer.
    ///
    /// The list hangs below the field's own rectangle; without this override a click on a row was
    /// rejected by the hit test and never reached `hit_test_item`.
    fn contains_point(&self, point: crate::core::Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
            || (self.expanded && self.dropdown_rect().contains_point(point))
    }

    fn has_open_popup(&self) -> bool {
        self.expanded
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }

    /// Advances the list's open/close reveal; `true` while it still owes frames.
    ///
    /// Forwarded to the trait so the animation bus drives it: `tick_animations` sweeps every
    /// mounted control and asks `tick`/`is_animating`, so a control that keeps its tick inherent
    /// is a control the host cannot advance — the defect the trait method exists to remove.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating()
    }

    impl_draw_bridge!();
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
            "item_selected" => {
                Some(EventSignalRef::mapped("item_selected", &self.item_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `DropdownMenu`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the item list and has no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for DropdownMenu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "items" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &DROPDOWN_ITEM_SCHEMA,
                    self.items
                        .iter()
                        .map(|item| dropdown_item_record(item, self.item_icon_source(&item.value)))
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => self.set_selected_index(expect_usize(value)?),
            "expanded" => {
                if expect_bool(value)? {
                    self.expand();
                } else {
                    self.collapse();
                }
                Ok(())
            }
            // Derived from the item list, which owns it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // The rows are published so a document can **read** every item's typed icon, but a row
            // cannot express `DropdownItem::children`, so accepting a write would silently discard
            // subtrees. Refusing with the standard read-only error is the non-destructive answer;
            // `set_item_icon_source` and `add_item` are the write paths. See `DROPDOWN_ITEM_SCHEMA`.
            "items" => Err(CapabilityAccessError::ReadOnlyProperty),
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "item_count",
            "selected_index",
            "expanded",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for DropdownMenu {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        // The **effective font** — the resolved theme/caller font — for the field's value;
        // the list rows take the same font below (D09-STYLE-01).
        let font = effective_font(&style);
        let theme = crate::style::resolved_theme_style("dropdown_menu");
        // `dropdown_menu` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — the colour the
        // window paints. The field is therefore a step toward the foreground, so it
        // reads as a field rather than as a hole in the window.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);
        // A disabled field is a chrome state, so it is derived from the resolved
        // colours rather than from a second literal grey.
        let disabled_background = background.blend(&border, 0.25);
        let placeholder_color = text_color.blend(&background, 0.45);
        // Derived from the popup's own fill rather than blended toward it: `text_color.blend(
        // &background, 0.6)` moved the ink 0.6 of the way to the surface it sits on, which is the
        // lowest-contrast point available and read near-invisibly on a light popup. This is the
        // one binding every disabled row, arrow and label below reads, so the recede is one
        // decision rather than six.
        let disabled_text_color = self.base.disabled_ink_on(text_color, background);
        // The selected row and the separators are secondary chrome, derived from the
        // resolved pair so they follow the appearance.
        let item_highlight = background.blend(&text_color, 0.14);
        let separator = background.blend(&text_color, 0.1);

        // ── Draw text field background ──
        let bg_color = if !is_enabled { disabled_background } else { background };
        context.fill_rounded_rect(geom, 4, bg_color);
        context.draw_rounded_rect_stroke(geom, 4, border, 1);

        // ── Draw selected label or placeholder ──
        let display_text = self.selected_label().unwrap_or_else(|| "Select...".to_string());
        let text_color = if !is_enabled {
            disabled_text_color
        } else if self.selected_value.is_some() {
            text_color
        } else {
            placeholder_color
        };
        // `draw_text`'s origin is the glyph box's top-left, so `geom.y + geom.height / 2` put
        // that top edge on the field's middle line and drew the label half a line low. The
        // shared primitive returns the line box itself, centred in the field.
        let line = context.text_line(geom, font);
        // The value is aligned within the field's inner box (its own padding inset at both ends),
        // so a right- or centre-aligned value moves the run while the field it sits in does not.
        // Fitted rather than drawn at a point, because an anchor alone cannot express centre/right
        // over a width.
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        context.draw_text_fitted(
            Rect::new(
                geom.x + PADDING,
                line.y,
                geom.width.saturating_sub(PADDING as u32 * 2),
                line.height,
            ),
            &display_text,
            font,
            text_color,
            value_align,
        );

        // ── Draw dropdown arrow ──
        let arrow_x = geom.x + geom.width as i32 - PADDING - 10;
        let arrow_y = geom.y + geom.height as i32 / 2 - 2;
        let arrow_color =
            if !is_enabled { disabled_text_color } else { text_color.blend(&background, 0.25) };
        if self.expanded {
            // Upward-pointing triangle when expanded
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(arrow_x, arrow_y + 6),
                    Point::new(arrow_x + 10, arrow_y + 6),
                    Point::new(arrow_x + 5, arrow_y),
                ],
                closed: true,
                color: arrow_color,
                filled: true,
                width: 1,
            });
        } else {
            // Downward-pointing triangle when collapsed
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(arrow_x, arrow_y),
                    Point::new(arrow_x + 10, arrow_y),
                    Point::new(arrow_x + 5, arrow_y + 6),
                ],
                closed: true,
                color: arrow_color,
                filled: true,
                width: 1,
            });
        }

        // ── Draw dropdown list when expanded ──
        //
        // # Why the list is drawn clipped to a revealed rectangle
        //
        // The list used to appear at its full height on the frame `expanded` flipped, so opening a
        // dropdown was a hard cut — no sense of the list coming *out of* the field. It is now drawn
        // twice: once to establish the clip at the revealed extent, and once inside it. Clipping is
        // what makes the reveal honest: a list that only translated would, mid-open, still print
        // rows below its own edge, so the user would see items that are not yet reachable.
        //
        // `reveal.is_closed()` is checked rather than just the flag, so a list that is closing
        // keeps painting until it has finished shrinking instead of vanishing on the click.
        if self.expanded && !self.items.is_empty() {
            let drop = self.dropdown_rect();
            let revealed = self.reveal.revealed(
                drop,
                RevealDirection::Down,
                PopupReveal::travel_cap_for(crate::core::Size::new(drop.width, drop.height)),
            );
            if !self.reveal.is_closed() {
                context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
            }
            // Dropdown background with shadow
            context.fill_rounded_rect(drop, 4, background);
            context.draw_rounded_rect_stroke(drop, 4, border, 2);

            // Draw visible items
            let end = (self.scroll_offset + MAX_VISIBLE_ITEMS).min(self.items.len());
            for i in self.scroll_offset..end {
                let item = &self.items[i];
                let ir = self.item_rect(i);

                // Highlight selected item
                let is_selected = self.selected_value.as_deref() == Some(&item.value);
                if is_selected {
                    context.fill_rounded_rect(ir, 2, item_highlight);
                }

                let item_text_color = if !item.enabled { disabled_text_color } else { text_color };
                // The item label takes the **effective font**, so the theme body font and the
                // user's text scale reach the list rows (D09-STYLE-01).
                let item_font = effective_font(&style);
                let mut item_x = ir.x + PADDING;

                // Item icon
                if let Some(source) = self.item_icon_source(&item.value) {
                    let icon_line = context.text_line(ir, item_font);
                    let side = icon_line.height.clamp(1, 16);
                    let icon_rect = Rect::new(item_x, icon_line.y, side, side);
                    if let Err(error) = crate::widget::paint_icon(
                        context,
                        source.into(),
                        icon_rect,
                        item_text_color,
                    ) {
                        log::warn!("dropdown menu item {i} icon failed: {error:?}");
                    }
                    item_x += 20;
                } else if let Some(ref item_icon) = item.icon {
                    let icon_font = Font::simple("sans-serif", 13.0);
                    // The icon and the label share the row, so both take their origin from the
                    // line box centred in it rather than from the row's raw midpoint — that
                    // midpoint is the glyph *top* edge, which sat both half a line low.
                    let icon_line = context.text_line(ir, &icon_font);
                    context.draw_text(
                        Point::new(item_x, icon_line.y),
                        item_icon,
                        &icon_font,
                        item_text_color,
                        HorizontalAlignment::Left,
                    );
                    item_x += 20;
                }

                // Item label
                let item_line = context.text_line(ir, item_font);
                // The label is aligned in the same inner box the field's value uses, so the two
                // columns line up. The band starts where the icon (if any) pushed it, so an icon
                // shifts the whole label run rather than only its left-aligned form. Fitted rather
                // than drawn at a point, so centre/right are expressible over the remaining width.
                let label_band = Rect::new(
                    item_x,
                    item_line.y,
                    (ir.x + ir.width as i32 - PADDING - item_x).max(0) as u32,
                    item_line.height,
                );
                let label_align =
                    self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
                context.draw_text_fitted(
                    label_band,
                    &item.label,
                    item_font,
                    item_text_color,
                    label_align,
                );

                // Submenu indicator if item has children
                if !item.children.is_empty() && item.enabled {
                    let sub_x = ir.x + ir.width as i32 - PADDING - 8;
                    let sub_y = ir.y + ir.height as i32 / 2 - 4;
                    context.execute_command(RenderCommand::DrawPath {
                        points: vec![
                            Point::new(sub_x, sub_y),
                            Point::new(sub_x, sub_y + 8),
                            Point::new(sub_x + 5, sub_y + 4),
                        ],
                        closed: true,
                        color: disabled_text_color,
                        filled: true,
                        width: 1,
                    });
                }

                // Separator line
                if i + 1 < end {
                    context.draw_line(
                        Point::new(ir.x + 4, ir.y + ir.height as i32),
                        Point::new(ir.x + ir.width as i32 - 4, ir.y + ir.height as i32),
                        separator,
                    );
                }
            }

            // Scroll indicators if content is scrollable. Each is a `ChevronUp`/`ChevronDown`
            // **icon outline**, not the `▲`/`▼` text glyph (U+25B2/U+25BC are covered by no
            // bundled face, so those drew as 8x8 bitmap blocks).
            if self.scroll_offset > 0 {
                let box_ = Rect::new(drop.x, drop.y, drop.width.max(1), 12);
                crate::widget::draw_icon_centered(
                    context,
                    box_,
                    10,
                    disabled_text_color,
                    IconName::ChevronUp,
                );
            }
            if end < self.items.len() {
                let box_ =
                    Rect::new(drop.x, drop.y + drop.height as i32 - 12, drop.width.max(1), 12);
                crate::widget::draw_icon_centered(
                    context,
                    box_,
                    10,
                    disabled_text_color,
                    IconName::ChevronDown,
                );
            }
            // The clip is popped on the same path it was pushed, so the reveal cannot leak a clip
            // onto whatever the host draws next.
            if !self.reveal.is_closed() {
                context.pop_clip();
            }
        }
    }
}

impl EventHandler for DropdownMenu {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    if self.geometry().contains_point(*pos) {
                        // Click on the text field — toggle expand
                        self.toggle();
                    } else if self.expanded {
                        // Check if click is on a dropdown item
                        if let Some(idx) = self.hit_test_item(*pos) {
                            let value = self.items[idx].value.clone();
                            self.selected_value = Some(value.clone());
                            self.item_selected.emit(value);
                            self.collapse();
                        } else {
                            // Click outside the dropdown — collapse
                            self.collapse();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == key_codes::ESCAPE && self.expanded {
                    // Escape — collapse
                    self.collapse();
                }
            }
            Event::Wheel { delta, modifiers: _ } => {
                if self.expanded && self.items.len() > MAX_VISIBLE_ITEMS {
                    let max_offset = self.items.len() - MAX_VISIBLE_ITEMS;
                    if delta.y > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    } else if delta.y < 0 {
                        self.scroll_offset = (self.scroll_offset + 1).min(max_offset);
                    }
                    self.base.request_redraw();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    #[test]
    fn dropdown_menu_default_creation() {
        let dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        assert_eq!(dm.selected_value(), None);
        assert_eq!(dm.item_count(), 0);
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_add_remove_items() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("us", "United States"));
        dm.add_item(DropdownItem::new("ca", "Canada"));
        dm.add_item(DropdownItem::new("mx", "Mexico"));
        assert_eq!(dm.item_count(), 3);

        assert!(dm.remove_item("ca"));
        assert_eq!(dm.item_count(), 2);

        assert!(!dm.remove_item("nonexistent"));
        assert_eq!(dm.item_count(), 2);
    }

    #[test]
    fn dropdown_menu_typed_icons_follow_items_and_validate_identity() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("first", "First"));
        dm.add_item(DropdownItem::new("second", "Second").with_icon("legacy"));
        let source = IconSource::Builtin(IconName::Check);

        assert_eq!(
            dm.set_item_icon_source("missing", Some(source)),
            Err(crate::widget::IconItemError::UnknownItemId {
                id: crate::compat::String::from("missing")
            })
        );
        dm.set_item_icon_source("second", Some(source)).unwrap();
        assert_eq!(dm.items()[1].icon.as_deref(), Some("legacy"));
        assert!(dm.remove_item("first"));
        // The override follows the item's identity through the removal of another row.
        assert_eq!(dm.item_icon_source("second"), Some(source));

        dm.expand();
        dm.reveal.jump_to(true);
        let with_typed_source = render_to_svg(&mut dm);
        dm.set_item_icon_source("second", None).unwrap();
        let with_legacy_text = render_to_svg(&mut dm);
        let mut plain = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        plain.add_item(DropdownItem::new("second", "Second"));
        plain.expand();
        plain.reveal.jump_to(true);
        let without_icon = render_to_svg(&mut plain);
        assert_eq!(
            with_typed_source.matches("<path").count(),
            without_icon.matches("<path").count() + 1,
            "the typed source adds one geometry path to the item"
        );
        assert!(with_legacy_text.matches("<path").count() > without_icon.matches("<path").count());

        dm.items_mut().reverse();
        assert_eq!(dm.item_icon_source("second"), None);
    }

    /// The rows publish each item's typed icon, and a source with no document form refuses the read.
    ///
    /// blue27 §19.10 asks for a structured `icon_source` on the entries that carry an icon, and
    /// §19.13 case I13 requires a runtime-only source to be either faithful or refused. The write side
    /// is read-only *on purpose* — see `DROPDOWN_ITEM_SCHEMA` — so both halves are asserted here.
    #[test]
    fn dropdown_menu_publishes_item_icons_as_rows_and_refuses_the_write() {
        use crate::widget::capability::{CapabilityValue, RecordList};
        use crate::widget::display_widgets::icon::IconData;

        static SOLID: IconData =
            IconData { name: "dd-solid", grid: 24, paths: &["M4-4H20V-20H4Z"] };

        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("open", "Open").with_icon("legacy"));
        dm.add_item(DropdownItem::new("close", "Close"));
        dm.set_item_icon_source("open", Some(IconSource::Builtin(IconName::Save))).unwrap();

        let CapabilityValue::Records(list) = dm.get("items").expect("the rows read back") else {
            panic!("`items` must read as a record list");
        };
        assert_eq!(list.len(), 2);
        assert!(matches!(
            list.get_by_id("open").expect("present").get("icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                IconName::Save
            )))
        ));
        assert!(
            list.get_by_id("close").expect("present").get("icon_source").is_none(),
            "a row with no typed icon omits the field rather than inventing one"
        );

        // The row list is a *view*: writing it would have to drop `children`, so it is refused.
        let empty: RecordList = RecordList::new(&super::DROPDOWN_ITEM_SCHEMA);
        assert_eq!(
            dm.set("items", CapabilityValue::Records(empty)),
            Err(crate::widget::capability::CapabilityAccessError::ReadOnlyProperty)
        );
        // ...and a refused write leaves the items exactly as they were.
        assert_eq!(dm.item_count(), 2);
        assert_eq!(dm.item_icon_source("open"), Some(IconSource::Builtin(IconName::Save)));

        // A borrowed source has no document form, so the read refuses instead of reporting "no icon".
        dm.set_item_icon_source("close", Some(IconSource::static_geometry(&SOLID))).unwrap();
        assert_eq!(
            dm.get("items"),
            Err(crate::widget::capability::CapabilityAccessError::IconSourceNotPersistable)
        );
    }

    #[test]
    fn dropdown_menu_select_and_signal() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("us", "United States"));
        dm.add_item(DropdownItem::new("ca", "Canada"));

        let selected = Arc::new(Mutex::new(None));
        let sel = selected.clone();
        dm.item_selected.connect(move |val| {
            *sel.lock().unwrap() = Some(val.to_string());
        });

        // Click on the text field to expand
        dm.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(dm.is_expanded());

        // Click on the first item
        let ir = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(ir.x + 5, ir.y + 5, 1));

        assert_eq!(dm.selected_value(), Some("us".to_string()));
        assert_eq!(selected.lock().unwrap().as_deref(), Some("us"));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_click_outside_collapses() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("a", "Item A"));
        dm.expand();
        assert!(dm.is_expanded());

        dm.handle_event(&Event::mouse_press(500, 500, 1));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_escape_collapses() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.expand();
        assert!(dm.is_expanded());

        dm.handle_event(&Event::key_press(27, 0));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_selected_label() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("fr", "France"));
        dm.select("fr");
        assert_eq!(dm.selected_label(), Some("France".to_string()));
        assert_eq!(dm.selected_value(), Some("fr".to_string()));
    }

    #[test]
    fn dropdown_menu_remove_item_clears_selection() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("x", "Item X"));
        dm.select("x");
        assert_eq!(dm.selected_value(), Some("x".to_string()));

        dm.remove_item("x");
        assert_eq!(dm.selected_value(), None);
    }

    #[test]
    fn dropdown_menu_toggle() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        assert!(!dm.is_expanded());

        dm.toggle();
        assert!(dm.is_expanded());

        dm.toggle();
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_svg_output() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("op1", "Option 1").with_icon("★"));
        dm.add_item(DropdownItem::new("op2", "Option 2").with_enabled(false));
        dm.add_item(DropdownItem::new("op3", "Option 3"));
        dm.select("op1");
        dm.expand();

        let svg = render_to_svg(&mut dm);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }
}
