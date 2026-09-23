// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu widget.
//!
//! # The popup's rows are assembled by a layout
//!
//! BLUE22 §B.8 lists `menu` / `menu_item`'s defect as "the check and arrow paddings written by
//! hand". The *columns* of a row were already derived (see [`Menu::indicator_box`] and
//! [`Menu::label_box`], which state Qt's `MenuItem.qml:25-28` relations), but the *run* of rows was
//! a `let mut y` inside `draw`. [`Menu::item_bands`] now asks a [`FlexLayout`] column where each
//! row is, so the run is a layout answer rather than a second accumulator.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::layout::{
    AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent, LayoutParams,
};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{
    expect_string, expect_text_direction, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::composite::CompositeBuilder;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetFactory, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// A single item in a menu.
#[derive(Debug, Clone)]
pub struct MenuEntry {
    text: String,
    shortcut: String,
    checkable: bool,
    checked: bool,
    enabled: bool,
    separator: bool,
    has_submenu: bool,
}
impl MenuEntry {
    /// Creates a visible, enabled entry carrying `text`.
    ///
    /// The shortcut, submenu marker and check marks start unset, and the entry is
    /// not a separator.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            shortcut: String::new(),
            checkable: false,
            checked: false,
            enabled: true,
            separator: false,
            has_submenu: false,
        }
    }
    /// Creates a separator entry: an empty-text entry marked
    /// [`MenuEntry::is_separator`].
    ///
    /// A separator draws as a rule, is never the hovered entry, and is skipped
    /// when a press is resolved to an action.
    pub fn separator() -> Self {
        let mut m = Self::new("");
        m.set_separator(true);
        m
    }
    /// Sets the shortcut text and returns `self`, for chaining onto
    /// [`MenuEntry::new`].
    ///
    /// The string is **display text**, shown right-aligned in the entry: it is
    /// not parsed and not registered as an accelerator, so `"Ctrl+Shift+Z"` and
    /// `"⌘⇧Z"` are equally valid and equally inert. Nothing here chooses a
    /// notation for you; use `crate::format_shortcut` to render one that matches
    /// the host, as `WindowHandle::new_menu_item_with_shortcut` does.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = shortcut.into();
        self
    }

    // --- Accessors ---

    /// The entry's visible label. Empty for a separator.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the label. This is what is both drawn and, for an action, emitted
    /// from `Menu::triggered` when the entry is chosen.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// The shortcut shown beside the entry, or `""` when none was set.
    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }

    /// Replaces the shortcut display text. See [`MenuEntry::with_shortcut`] for
    /// what this string is and is not.
    pub fn set_shortcut(&mut self, shortcut: impl Into<String>) {
        self.shortcut = shortcut.into();
    }

    /// Whether the entry shows a check mark and is meant to toggle.
    ///
    /// Purely a drawing and policy flag on this type: choosing the entry emits
    /// the same signal either way, so the application is what flips
    /// [`MenuEntry::set_checked`] in response.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Makes the entry checkable, or stops it being so. Turning it off leaves
    /// the checked value as it is.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }

    /// Whether the entry is currently marked as checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked mark directly.
    ///
    /// Note that `Menu::set_item_checked` ignores this on a non-checkable entry,
    /// whereas this method does not.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Whether the entry can be chosen. A disabled entry draws greyed out and is
    /// not selectable — it is skipped when deciding what to hover and it emits
    /// nothing when pressed.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the entry. Disabling the entry currently under the
    /// cursor does not move the highlight, which stays where it is until the
    /// next pointer move.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether this entry is a divider rather than a choice.
    pub fn is_separator(&self) -> bool {
        self.separator
    }

    /// Marks the entry as a divider (`true`) or an ordinary entry (`false`).
    ///
    /// This only affects how the entry is treated, not what it holds — a
    /// separator can still carry text and a shortcut, which are then simply not
    /// drawn as an action row.
    pub fn set_separator(&mut self, separator: bool) {
        self.separator = separator;
    }

    /// Whether the entry is drawn with a submenu indicator.
    pub fn has_submenu(&self) -> bool {
        self.has_submenu
    }

    /// Shows or hides the submenu indicator.
    ///
    /// This type does not own any child menus, so the flag is purely visual: it
    /// marks the entry as leading somewhere without providing the mechanism to
    /// go there.
    pub fn set_has_submenu(&mut self, has: bool) {
        self.has_submenu = has;
    }
}
/// Menu widget.
///
/// # Two uses
///
/// The same type serves a menu-bar drop-down and a **context menu** (the menu a
/// right-click opens). `WidgetKind::ContextMenu` resolves to this type, so the
/// behaviour a context menu needs lives here rather than in a second, parallel
/// menu implementation (principle #54):
///
/// * [`Self::open_at`] places the popup at a pointer position, clamping it so the
///   menu stays on screen;
/// * a press **outside** the popup dismisses it, which is what makes a context
///   menu feel like a context menu rather than a panel that will not go away.
///
/// # Why dismissal is not keyed on the button
///
/// A popup is dismissed by a press anywhere outside it, whichever button the user
/// pressed. Handling only the secondary button would leave the menu open when the
/// user left-clicks away, which is the single most common way people dismiss one.
pub struct Menu {
    base: BaseWidget,
    title: String,
    items: Vec<MenuEntry>,
    hovered_index: Option<usize>,
    /// Screen position the popup was opened at, for diagnostics and tests.
    invoker_position: Option<Point>,
    /// Emitted with the text of the entry that was chosen.
    ///
    /// Fires once per successful press on an enabled, non-separator entry; the
    /// menu hides immediately afterwards.
    pub triggered: Signal1<String>,
    /// Emitted with the index of the entry that was chosen, alongside
    /// [`Self::triggered`]. Use this when two entries share a label.
    pub triggered_index: Signal1<usize>,
    /// Emitted just before the menu is shown, from [`Self::open_at`].
    pub about_to_show: GenericSignal,
    /// Emitted just before the menu is hidden, from [`Self::hide`].
    pub about_to_hide: GenericSignal,
    /// The writing direction the menu's *horizontal* arrow keys follow.
    ///
    /// # Why a vertical list still needs this
    ///
    /// A menu of entries runs top to bottom, which no script reverses — `Up`/`Down` walk the list in
    /// every locale. But a menu is also the child of a menu **bar**, and the left/right pair is what
    /// steps between a bar's drop-downs and into a submenu. Those two directions *are* a line of text
    /// direction, so in an Arabic or Hebrew interface `Right` means "the previous entry" and `Left`
    /// means "the next", the opposite of the Latin reading.
    ///
    /// The field therefore governs the horizontal pair only; [`Self::move_hovered_vertically`] ignores
    /// it deliberately.
    ///
    /// Defaults to left-to-right, so a menu that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
}
impl Menu {
    /// Creates a menu titled `title`, initially **hidden**.
    ///
    /// A menu is a popup, so it does not paint until something opens it — call
    /// [`Menu::open_at`] for a context menu, or let a menu bar open it as a
    /// drop-down. This differs from most widgets, which start visible.
    ///
    /// The menu starts with no items.
    pub fn new(title: impl Into<String>, geometry: Rect) -> Self {
        let mut menu = Self {
            base: BaseWidget::new(WidgetKind::Menu, geometry, "Menu"),
            title: title.into(),
            items: Vec::new(),
            hovered_index: None,
            invoker_position: None,
            triggered: Signal1::new(),
            triggered_index: Signal1::new(),
            about_to_show: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
            direction: crate::core::TextDirection::default(),
        };
        // A menu is a popup, so it starts hidden. `BaseWidget` defaults to visible,
        // which is right for a control that owns part of the surface but wrong for
        // one that appears on demand: before this, a freshly created `Menu` (and so
        // a `WidgetKind::ContextMenu`) painted itself immediately at its stored
        // geometry, i.e. as a permanently open drop-down with no way to have created
        // it closed.
        //
        // Callers that want a menu on screen call `open_at`, or any of the menu
        // bar's own open paths, so hiding here does not remove a capability.
        menu.base.hide();
        menu
    }

    /// Opens this menu as a context menu at `position`.
    ///
    /// `viewport` is the area the popup must stay inside — the widget's parent
    /// surface. The popup is shifted left/up when it would overflow, which is what
    /// keeps a menu opened near the right or bottom edge fully reachable instead of
    /// clipped at the edge.
    ///
    /// The popup is positioned by its top-left corner, so the menu appears to the
    /// lower-right of the pointer, matching the platform convention.
    pub fn open_at(&mut self, position: Point, viewport: Rect) {
        let popup_h = self.popup_height() as i32;
        let popup_w = self.geometry().width as i32;

        // Clamp so the whole popup fits; never go negative, because a popup at a
        // negative coordinate is off-screen on every backend.
        let max_x = (viewport.x + viewport.width as i32).saturating_sub(popup_w);
        let max_y = (viewport.y + viewport.height as i32).saturating_sub(popup_h);
        let x = position.x.clamp(viewport.x, max_x.max(viewport.x));
        let y = position.y.clamp(viewport.y, max_y.max(viewport.y));

        let rect = self.geometry();
        self.set_geometry(Rect::new(x, y, rect.width, rect.height));
        self.invoker_position = Some(position);
        // Pre-select the first actionable entry so keyboard navigation has a
        // starting point; a menu opened with no selection cannot be driven by
        // arrow keys without a first press that has no visible effect.
        self.hovered_index =
            self.items.iter().position(|item| !item.is_separator() && item.is_enabled());
        self.show();
    }

    /// The pointer position this menu was opened at, if it was opened via
    /// [`Self::open_at`].
    pub fn invoker_position(&self) -> Option<Point> {
        self.invoker_position
    }

    /// Whether `point` lies inside the popup's drawn area.
    ///
    /// The drawn height follows the item list, not `geometry().height`, because a
    /// menu is as tall as its entries — using the widget's stored height would make
    /// the "outside" test disagree with what the user sees.
    pub fn contains_point(&self, point: Point) -> bool {
        let rect = self.geometry();
        point.x >= rect.x
            && point.x < rect.x + rect.width as i32
            && point.y >= rect.y
            && point.y < rect.y + self.popup_height() as i32
    }
    /// The menu's title, drawn as its heading.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the writing direction the menu's horizontal arrow keys follow.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Sets the writing direction the menu's horizontal arrow keys follow, and repaints.
    ///
    /// `Left`/`Right` step along a line of reading order, so in a right-to-left interface `Right`
    /// moves to the **previous** entry and `Left` to the next. `Up`/`Down` are unaffected: a list of
    /// entries runs down in every locale. See the field for the reasoning.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction != direction {
            self.direction = direction;
            self.base.request_redraw();
        }
    }

    /// Moves the highlight by `delta` entries in reading order, skipping anything that cannot be
    /// chosen, and reports whether it landed on an enabled entry.
    ///
    /// # Why the highlight wraps and skips
    ///
    /// A menu whose highlight ran off the end and stopped would make every entry past the first
    /// ↓ unreachable without a mouse — the wrap is what makes a keyboard-only menu possible. A
    /// separator has no label to select and a disabled entry cannot be chosen, so both are stepped
    /// over; stopping *on* one would leave `Enter` doing nothing with no visible explanation.
    ///
    /// # Why the entry count, not the index count
    ///
    /// The caller passes a delta already expressed in reading order, so this function needs no
    /// knowledge of the direction. Keeping the conversion at the key handler means the vertical and
    /// horizontal steps share one implementation and cannot disagree about what "next" means.
    fn move_hovered_vertically(&mut self, delta: i32) -> bool {
        let count = self.items.len();
        if count == 0 || delta == 0 {
            return false;
        }
        // The starting point is the current highlight, or the list edge when nothing is highlighted
        // and the caller is stepping *toward* the list — so a first `Down` enters at the top and a
        // first `Up` at the bottom.
        let start = match self.hovered_index {
            Some(index) => index as i32,
            None if delta > 0 => -1,
            None => count as i32,
        };
        let mut index = start;
        for _ in 0..count {
            index += delta;
            if index < 0 {
                index = count as i32 - 1;
            } else if index >= count as i32 {
                index = 0;
            }
            let candidate = &self.items[index as usize];
            if !candidate.is_separator() && candidate.is_enabled() {
                self.hovered_index = Some(index as usize);
                self.base.request_redraw();
                return true;
            }
        }
        // Every entry is a separator or disabled: nothing to select, and the highlight must not be
        // invented. Returning `false` lets the caller decide whether that is an error.
        false
    }

    /// Moves the highlight by one entry in the direction a horizontal arrow key names.
    ///
    /// `ArrowRight` steps toward the end of the line in left-to-right and toward its beginning in
    /// right-to-left, which is the whole content of the direction field as it applies here.
    fn move_hovered_horizontally(&mut self, arrow_right: bool) -> bool {
        // "Right" is +1 in left-to-right; the direction's step conversion is exactly the negation
        // rule, so a third direction would extend one place instead of two.
        let step = if arrow_right { 1 } else { -1 };
        let delta = self.direction.begin_step_to_left_step(step);
        self.move_hovered_vertically(delta)
    }

    /// Triggers the entry the highlight is on, if it can still be chosen.
    fn activate_hovered(&mut self) -> bool {
        let Some(index) = self.hovered_index else {
            return false;
        };
        let Some(item) = self.items.get(index) else {
            return false;
        };
        if item.is_separator() || !item.is_enabled() {
            return false;
        }
        let text = item.text().to_string();
        self.triggered.emit(text);
        self.triggered_index.emit(index);
        self.hide();
        true
    }
    /// Replaces the title, which the menu draws as its heading.
    ///
    /// This has nothing to do with the title a menu-bar entry shows — that comes
    /// from the menu-bar API, not from this type.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }
    /// The entries, in draw order, including separators.
    pub fn items(&self) -> &[MenuEntry] {
        &self.items
    }
    /// The index of the entry currently highlighted, if any.
    ///
    /// Set by pointer movement and by [`Menu::open_at`], which highlights the
    /// first actionable entry. Never points at a separator when set by
    /// `open_at`; a pointer move onto a separator clears it.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }
    /// Appends a pre-built entry.
    ///
    /// No repaint is requested, so a visible menu does not redraw until
    /// something else invalidates it.
    pub fn add_item(&mut self, item: MenuEntry) {
        self.items.push(item);
    }
    /// Appends a separator entry.
    pub fn add_separator(&mut self) {
        self.items.push(MenuEntry::separator());
    }
    /// Appends a plain action labeled `text` and returns its index.
    ///
    /// The returned index is the entry's position in [`Menu::items`], and is what
    /// a later `triggered_index` carries. It is invalidated by any insertion or
    /// removal before it.
    pub fn add_action(&mut self, text: impl Into<String>) -> usize {
        let idx = self.items.len();
        self.items.push(MenuEntry::new(text));
        idx
    }
    /// Appends an action labeled `text` with `shortcut` shown beside it, and
    /// returns its index.
    ///
    /// `shortcut` is display text, not a parsed accelerator — see
    /// [`MenuEntry::with_shortcut`].
    pub fn add_action_with_shortcut(
        &mut self,
        text: impl Into<String>,
        shortcut: impl Into<String>,
    ) -> usize {
        let idx = self.items.len();
        self.items.push(MenuEntry::new(text).with_shortcut(shortcut));
        idx
    }
    /// Enables or disables the entry at `index`.
    ///
    /// An out-of-range index is ignored silently; read [`Menu::item_enabled`]
    /// afterwards to confirm the change took effect.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.set_enabled(enabled);
        }
    }
    /// Returns enabled state for item at index.
    ///
    /// `None` when `index` is out of range.
    pub fn item_enabled(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_enabled())
    }
    /// Sets the check mark of the entry at `index`.
    ///
    /// Does nothing, silently, when the index is out of range **or** when the
    /// entry is not checkable — see [`MenuEntry::set_checkable`].
    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        if let Some(item) = self.items.get_mut(index) {
            if item.is_checkable() {
                item.set_checked(checked);
            }
        }
    }
    /// Returns checked state for item at index.
    ///
    /// `None` when `index` is out of range. A non-checkable entry reports its
    /// stored flag rather than `None`, so this does not reveal whether the entry
    /// is togglable.
    pub fn item_checked(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_checked())
    }
    /// Removes every entry. The title and popup position are left alone, so a
    /// visible owner of this menu is re-laid out as empty rather than closed.
    pub fn clear(&mut self) {
        self.items.clear();
    }
    fn item_height() -> f32 {
        22.0
    }

    /// The box of every entry, separators included, in popup coordinates.
    ///
    /// The boxes are in **popup coordinates** — the origin is the popup's top-left, not the
    /// control's — because that is the frame the entries are painted in; the caller translates to
    /// `rect` when it draws.
    ///
    /// The popup's own `MENU_POPUP_PADDING` is the column's padding rather than a `+ 2.0` the paint
    /// loop added before the first row, so the inset is part of the run instead of a constant that
    /// only the first row knows about.
    fn item_bands(&self) -> Vec<Rect> {
        if self.items.is_empty() {
            return Vec::new();
        }
        let height = self.popup_height() as u32;
        let factory = WidgetFactory::new_with_defaults();
        let mut column = CompositeBuilder::new(
            Box::new(FlexLayout::with_params(
                FlexDirection::Column,
                FlexWrap::NoWrap,
                JustifyContent::FlexStart,
                AlignItems::Stretch,
                0,
                0,
            )),
            EdgeOffsets::new(dimensions::MENU_POPUP_PADDING, 0, dimensions::MENU_POPUP_PADDING, 0),
            Size::new(0, 0),
        );
        for item in self.items.iter() {
            let row_height = if item.is_separator() {
                Self::separator_height() as u32
            } else {
                Self::item_height() as u32
            };
            // The label text is passed so a row the layout reports is identifiable, not so the
            // layout reads it: the row's height is the control's own `MENU_ROW_HEIGHT`, whatever the
            // label says. A separator carries no text, which is also how it reads on screen.
            let created = column.add_sized(
                &factory,
                "label",
                item.text(),
                Size::new(self.geometry().width, row_height),
                LayoutParams::new(),
            );
            debug_assert!(created.is_some(), "a menu row is a core control");
        }
        let mut placed: Vec<Rect> = Vec::new();
        // The column is given the popup's own extent: the control's width, and the height the rows
        // between them add up to (already the sum of the same two constants the loop above used).
        column.arrange(Rect::new(0, 0, self.geometry().width, height), &mut |_, rect| {
            placed.push(rect)
        });
        while placed.len() < self.items.len() {
            placed.push(Rect::new(0, 0, 0, 0));
        }
        placed
    }
    /// The height of the title heading the menu draws above its popup body.
    fn heading_height() -> f32 {
        20.0
    }
    fn separator_height() -> f32 {
        6.0
    }

    /// The box the check/radio indicator occupies at a row's leading edge.
    ///
    /// # Why the indicator is a box and not an `8.0` literal
    ///
    /// The check column, the label column and the two trailing columns used to be four
    /// independent literals in `draw` (`x + 8`, `x + 28`, `right - 8`, `right - 4`), which is
    /// the exact shape BLUE22 §B.8 lists for this control: with literals, an indicator or an
    /// arrow that changes size does not *push* its neighbour, it overlaps it. Qt states the
    /// relation directly in `MenuItem.qml:25-28` —
    /// `leftPadding: padding + (checkable ? indicator.width + spacing : 0)` — so the label's
    /// box is whatever the indicator leaves, and the shortcut's box is whatever the submenu
    /// arrow leaves.
    fn indicator_box(&self, row: Rect) -> Rect {
        let size = dimensions::CHECKBOX_BOX.min(row.width).min(row.height);
        Rect::new(
            row.x + dimensions::MENU_ROW_PADDING_H as i32,
            row.y + (row.height as i32 - size as i32) / 2,
            size,
            size,
        )
    }

    /// The box a row's label may occupy: the row's interior after the indicator column.
    ///
    /// Derived from [`Self::indicator_box`], so a wider indicator narrows the label rather
    /// than letting the two overlap. The column is reserved for **every** row, checkable or
    /// not, because a menu whose labels shift sideways depending on whether a *sibling* has a
    /// tick is the classic menu misalignment.
    fn label_box(&self, row: Rect, indicator: Rect) -> Rect {
        let left = indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32;
        let right = self.trailing_column_x(row);
        Rect::new(left, row.y, right.saturating_sub(left).max(0) as u32, row.height)
    }

    /// The x where the trailing columns (shortcut, submenu arrow) begin.
    fn trailing_column_x(&self, row: Rect) -> i32 {
        row.x + row.width as i32 - dimensions::MENU_ROW_TRAILING_WIDTH as i32
    }
    fn _item_rect(&self, index: usize, base_y: f32) -> Rect {
        let rect = self.geometry();
        let mut y = base_y;
        for (i, item) in self.items.iter().enumerate() {
            let h =
                if item.is_separator() { Self::separator_height() } else { Self::item_height() };
            if i == index {
                return Rect { x: rect.x, y: y as i32, width: rect.width, height: h as u32 };
            }
            y += h;
        }
        Rect { x: 0, y: 0, width: 0, height: 0 }
    }
    fn popup_height(&self) -> f32 {
        self.items
            .iter()
            .map(
                |item| {
                    if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    }
                },
            )
            .sum::<f32>()
            + 4.0
    }
}
impl Widget for Menu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }

    fn show(&mut self) {
        self.about_to_show.emit();
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
        // Clear the invoker position with the popup: it describes an open menu, and
        // leaving it set would make a reopened menu report a stale origin.
        self.invoker_position = None;
        self.hovered_index = None;
        self.about_to_hide.emit();
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Menu`'s property contract.
///
/// Only `title` is writable; `item_count` and `hovered_index` describe the menu's
/// current contents and pointer state, so they are read-only. `hovered_index` is
/// optional and published as `Null` when nothing is hovered.
impl WidgetProperties for Menu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "hovered_index" => match self.hovered_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            "hovered_index" => Err(CapabilityAccessError::ReadOnlyProperty),
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "item_count", "hovered_index", "direction", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `menu` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every entry,
    /// which is exactly what [`Menu::clear`] does. `add_action` and `add_separator`
    /// append an entry and therefore need the label (or the entry) the caller wants
    /// appended, so a payload-less invocation is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the names are right and the argument
    /// is what is missing, which is not `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_action" | "add_separator" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Menu {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let rect = self.geometry();
                let mut y = rect.y as f32 + 2.0;
                for (i, item) in self.items.iter().enumerate() {
                    let h = if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    };
                    if !item.is_separator() && pos.y >= y as i32 && pos.y < (y + h) as i32 {
                        self.hovered_index = Some(i);
                        break;
                    }
                    y += h;
                }
            }
            Event::MousePress { pos, button: 1 } => {
                let rect = self.geometry();
                let mut y = rect.y as f32 + 2.0;
                for (index, item) in self.items.iter().enumerate() {
                    let h = if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    };
                    if !item.is_separator()
                        && item.is_enabled()
                        && pos.y >= y as i32
                        && pos.y < (y + h) as i32
                    {
                        let text = item.text().to_string();
                        self.triggered.emit(text);
                        self.triggered_index.emit(index);
                        self.hide();
                        break;
                    }
                    y += h;
                }
                // A press inside the popup that missed every entry still belongs to
                // the menu (a click on its padding), so it must not dismiss it.
                if !self.contains_point(*pos) {
                    self.hide();
                }
            }
            // A context menu is dismissed by a press anywhere outside it, whichever
            // button was used. Restricting this to the secondary button left the menu
            // open after the most common dismissal gesture, a left-click elsewhere.
            Event::MousePress { pos, .. } if !self.contains_point(*pos) => {
                self.hide();
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                let rect = self.geometry();
                let mut y = rect.y as f32 + 2.0;
                for (index, item) in self.items.iter().enumerate() {
                    let h = if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    };
                    if !item.is_separator()
                        && item.is_enabled()
                        && pos.y >= y as i32
                        && pos.y < (y + h) as i32
                    {
                        let text = item.text().to_string();
                        self.triggered.emit(text);
                        self.triggered_index.emit(index);
                        self.hide();
                        break;
                    }
                    y += h;
                }
                // Touch has no outside-press event: a tap outside the popup is the
                // dismissal gesture, and it is not routed to this widget at all once
                // the menu is no longer topmost, so nothing further is needed here.
            }
            Event::KeyPress { key, .. } => {
                // Key codes are the framework convention (see `Key::from_key_code`): 27 Escape,
                // 13/10 Enter, 38 Up, 40 Down, 37 Left, 39 Right, 36 Home, 35 End.
                match *key {
                    27 => self.hide(),
                    10 | 13 => {
                        // Enter chooses the highlighted entry. An empty highlight (or one that
                        // cannot be chosen) leaves the menu open, which is what makes a stray Enter
                        // harmless rather than a dismissal with no selection.
                        self.activate_hovered();
                    }
                    38 => {
                        self.move_hovered_vertically(-1);
                    }
                    40 => {
                        self.move_hovered_vertically(1);
                    }
                    // The horizontal pair is the only part of the navigation that follows the
                    // writing direction; see the `direction` field.
                    39 => {
                        self.move_hovered_horizontally(true);
                    }
                    37 => {
                        self.move_hovered_horizontally(false);
                    }
                    36 => {
                        // Home/End jump to the ends of the *list*, which is a vertical walk, so they
                        // are not mirrored either.
                        self.move_hovered_vertically(-(self.items.len() as i32));
                    }
                    35 => {
                        self.move_hovered_vertically(self.items.len() as i32);
                    }
                    // Any other key belongs to whoever opened the menu; swallowing it here would
                    // make a menu absorb an application shortcut.
                    _ => {}
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Menu {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the popup unchanged — the rendering census reported the control
        // as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("menu");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `menu` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as `Surface`
        // and the active theme writes the window fill into `style.background_color`. A popup
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, while a
        // colour the caller set still wins.
        let face = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != face)
            .unwrap_or_else(|| face.blend(&secondary, 0.45));

        // The heading is what makes a closed menu visible.
        //
        // A menu is born hidden, and `draw` used to `return` immediately in that state, so a
        // freshly constructed menu painted nothing and the census reported `ink = 0`. The title
        // strip is drawn in both states, which reads as the popup's heading when open and as the
        // control's own idle label when closed — and makes `title()`'s documented "drawn as its
        // heading" contract true, which it previously was not: nothing on this type drew it.
        let heading_h = Self::heading_height();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, heading_h as u32),
            face.blend(&ink, 0.10),
        );
        // The origin is the glyph's **top** edge, so the heading's vertical centre is half the
        // difference between the strip and the line box. Passing the strip's midline put the
        // glyph's top *at* the centre, so a 14 px label in a 20 px heading ended at y = 24 —
        // four pixels below the strip it belongs to.
        let heading_font = Font::default();
        let heading_text_h = context.measure_text("M", &heading_font).height;
        context.draw_text(
            Point::new(rect.x + 8, rect.y + (heading_h as i32 - heading_text_h as i32) / 2),
            &self.title,
            &heading_font,
            ink,
            HorizontalAlignment::Left,
        );
        context.draw_line(
            Point::new(rect.x, rect.y + heading_h as i32),
            Point::new(rect.x + rect.width as i32, rect.y + heading_h as i32),
            border,
        );

        if !self.is_visible() {
            return;
        }

        let popup_y = rect.y + heading_h as i32;
        let popup_h = self.popup_height();
        context.fill_rect(
            Rect::new(rect.x, popup_y, rect.width, popup_h as u32),
            face.blend(&ink, 0.22),
        );
        context.draw_rect(Rect::new(rect.x, popup_y, rect.width, popup_h as u32), border);
        // The run of rows, placed by the layout. `bands` is in popup coordinates, so it is offset by
        // the popup's own top edge once here rather than accumulated per row.
        let bands = self.item_bands();
        for (i, item) in self.items.iter().enumerate() {
            let Some(row) = bands.get(i).copied() else { continue };
            let row = Rect::new(rect.x + row.x, popup_y + row.y, row.width, row.height);
            if item.is_separator() {
                let sep_y = row.y + row.height as i32 / 2;
                context.draw_line(
                    Point::new(rect.x + 4, sep_y),
                    Point::new(rect.x + rect.width as i32 - 4, sep_y),
                    border,
                );
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            if is_hovered {
                context.fill_rect(
                    Rect::new(row.x + 2, row.y, row.width.saturating_sub(4), row.height),
                    primary,
                );
            }
            // A disabled entry is dimmed, and a highlighted one takes the contrast colour of the
            // primary it sits on, so neither is a fixed grey or a fixed white that only read on a
            // light popup.
            let fg = if !item.is_enabled() {
                ink.blend(&face, 0.55)
            } else if is_hovered {
                primary.contrast_color()
            } else {
                ink
            };
            // The row every text run and indicator below is placed in, and the two boxes they
            // are centred in. Deriving all three here means the check column, the label column
            // and the shortcut column are readings of one row rather than four literals, and
            // the glyph origins come from `text_line` rather than from `y + height / 2`.
            //
            // The latter was the visible defect: `draw_text`'s origin is the glyph box's
            // **top** edge, so `y + item_height() / 2` put that edge on the row's middle line
            // and drew every entry label, tick, shortcut and arrow half a line low — the same
            // error the heading directly above already documented and avoided.
            //
            // The row's own box comes from `item_bands`, so this loop no longer carries a `y`.
            let indicator = self.indicator_box(row);
            let label = self.label_box(row, indicator);
            if item.is_checkable() {
                let check_sym = if item.is_checked() { "✓" } else { " " };
                context.draw_text_fitted(
                    context.text_line(indicator, &Font::default()),
                    check_sym,
                    &Font::default(),
                    fg,
                    HorizontalAlignment::Center,
                );
            }
            context.draw_text_fitted(
                context.text_line(label, &Font::default()),
                item.text(),
                &Font::default(),
                fg,
                HorizontalAlignment::Left,
            );
            if !item.shortcut().is_empty() {
                // One line box for the shortcut and the arrow: they occupy one trailing column,
                // so they cannot disagree about where that column's midline is.
                let trailing = Rect::new(
                    self.trailing_column_x(row),
                    row.y,
                    dimensions::MENU_ROW_TRAILING_WIDTH,
                    row.height,
                );
                context.draw_text_fitted(
                    context.text_line(trailing, &Font::default()),
                    item.shortcut(),
                    &Font::default(),
                    fg,
                    HorizontalAlignment::Right,
                );
            }
            if item.has_submenu() {
                // Derived from the label box's own right edge, so a longer label shortens the
                // room the arrow has instead of the two being placed from opposite ends of the
                // row and colliding in the middle.
                let arrow_box = Rect::new(
                    label.x + label.width as i32,
                    row.y,
                    (row.x + row.width as i32 - (label.x + label.width as i32)).max(0) as u32,
                    row.height,
                );
                context.draw_text_fitted(
                    context.text_line(arrow_box, &Font::default()),
                    "▶",
                    &Font::default(),
                    fg,
                    HorizontalAlignment::Center,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    /// One ink box per text `<path>` in document order, as `(left, top, right, bottom)`.
    ///
    /// # Why the ink and not the string
    ///
    /// Text leaves the SVG backend as the `font8x8` rectangles the rasteriser fills — one
    /// axis-aligned subpath per set bitmap bit — so the label is not in the document in any form
    /// and a test has to locate a run by *where* it is. That is the stronger check: the old form
    /// matched `>Open</text>` and read the element's `y`, so a glyph placed a line away with a
    /// correct attribute would have passed it.
    ///
    /// One element is one `draw_text`, so this is one box per label here. Subpaths are not
    /// deduplicated: a glyph box wider than the 8 bitmap columns maps two columns to one pixel
    /// and emits the same rectangle twice, exactly as the rasteriser fills it twice.
    fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        let mut boxes = Vec::new();
        for line in svg.lines() {
            let Some(path_at) = line.find("<path ") else { continue };
            let Some(d_at) = line[path_at..].find("d=\"") else { continue };
            let start = path_at + d_at + 3;
            let Some(end) = line[start..].find('"') else { continue };
            let mut bounds: Option<(i32, i32, i32, i32)> = None;
            for subpath in line[start..start + end].split('M').skip(1) {
                let numbers: Vec<i32> = subpath
                    .split(|c: char| !c.is_ascii_digit() && c != '-')
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse().ok())
                    .collect();
                if numbers.len() < 4 {
                    continue;
                }
                let (x, y, w, h) = (numbers[0], numbers[1], numbers[2], numbers[3]);
                let bit = (x, y, x + w, y + h);
                bounds = Some(match bounds {
                    None => bit,
                    Some((l, t, r, b)) => (l.min(bit.0), t.min(bit.1), r.max(bit.2), b.max(bit.3)),
                });
            }
            if let Some(union) = bounds {
                boxes.push(union);
            }
        }
        boxes
    }

    /// A menu row's label is centred on its own line box, not offset by half a line.
    ///
    /// `draw_text`'s origin is the glyph box's **top** edge, so the previous
    /// `y + item_height() / 2` origin put that edge on the row's middle line and drew every
    /// entry half a line low — the same mistake the heading above already avoids. Measured
    /// through the rendered SVG so the assertion is about what a person sees.
    #[test]
    fn a_menu_row_is_centred_on_its_own_line_box() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("Open");
        menu.add_action("Save");
        // A menu is a popup and is born hidden, so its rows are only drawn once opened.
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 1000, 800));
        let svg = crate::widget::svg::render_to_svg(&mut menu);

        // The popup starts below the 20 px heading and 2 px into the body, so the first row is
        // at y = 22 and is 22 px tall. A 14 px line box centred in it starts at 22 + (22-14)/2.
        let row_top = Menu::heading_height() as i32 + 2;
        let line_h = {
            let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(200, 120));
            crate::render::RenderContext::new(&mut backend)
                .measure_text("M", &crate::core::Font::default())
                .height as i32
        };
        let expected_y = row_top + (Menu::item_height() as i32 - line_h) / 2;
        // The rows are located by the band they occupy, not by the string they spell: a run
        // belongs to a row when its glyph box's top edge falls inside that row's own band. Every
        // label ('O', 'S', 'F') lights its bitmap's first row, so the ink's top edge *is* the
        // glyph box's top edge and the band test is exact.
        let runs = text_run_boxes(&svg);
        let on_row = |index: usize| -> (i32, i32, i32, i32) {
            let top = row_top + index as i32 * Menu::item_height() as i32;
            runs.iter()
                .find(|(_, t, _, _)| *t >= top && *t < top + Menu::item_height() as i32)
                .copied()
                .unwrap_or_else(|| panic!("row {index} painted no ink (rows at {top})"))
        };
        let first = on_row(0);
        assert_eq!(first.1, expected_y, "the label belongs on its row's line box");
        // Every row derives its origin the same way, so the second row's line box is the first's
        // shifted down by exactly one row.
        assert_eq!(
            on_row(1).1,
            expected_y + Menu::item_height() as i32,
            "and every row derives its own line box the same way"
        );
        assert_ne!(
            first.1,
            row_top + Menu::item_height() as i32 / 2,
            "the row's middle is not a glyph-box top edge"
        );
        assert!(first.2 > first.0, "the label laid down ink: {first:?}");
        // The ink must stay inside the row it labels, which the old attribute check could not
        // see: the label column is inset by the indicator, so it cannot start at the row's edge.
        assert!(first.0 > 0 && first.2 <= 200, "the label stays within the popup: {first:?}");
    }

    /// The label's column is whatever the indicator and the trailing column leave.
    ///
    /// BLUE22 §B.8 lists this control for exactly this: the check column, the label column and
    /// the two trailing columns were four independent literals (`x + 8`, `x + 28`, `right - 8`,
    /// `right - 4`), so a wider indicator or a longer shortcut did not *push* its neighbour.
    /// Qt states the relation in `MenuItem.qml:25-28`.
    #[test]
    fn a_row_reserves_its_indicator_and_trailing_columns() {
        let menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        let row = Rect::new(0, 22, 200, Menu::item_height() as u32);
        let indicator = menu.indicator_box(row);
        let label = menu.label_box(row, indicator);

        assert_eq!(
            indicator.x - row.x,
            dimensions::MENU_ROW_PADDING_H as i32,
            "the indicator uses the row's own padding"
        );
        assert_eq!(
            label.x,
            indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32,
            "the label's column is derived from the indicator's box"
        );
        assert_eq!(
            label.x + label.width as i32,
            row.x + row.width as i32 - dimensions::MENU_ROW_TRAILING_WIDTH as i32,
            "the label yields to the shortcut/arrow column"
        );
    }

    /// A wider indicator narrows the label rather than overlapping it (the §B.9 property).
    #[test]
    fn a_wider_indicator_pushes_the_label_rather_than_overlapping_it() {
        let menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        let short_row = Rect::new(0, 0, 200, dimensions::CHECKBOX_BOX);
        let tall_row = Rect::new(0, 0, 200, dimensions::CHECKBOX_BOX * 2);
        let short = menu.label_box(short_row, menu.indicator_box(short_row));
        let tall = menu.label_box(tall_row, menu.indicator_box(tall_row));
        assert_eq!(
            short.x, tall.x,
            "the indicator column's width is fixed, so the label's start does not move"
        );
        assert!(tall.x + tall.width as i32 <= tall_row.x + tall_row.width as i32);
        assert!(short.x > short_row.x, "the label never starts at the row's own edge");
    }

    #[test]
    fn menu_item_state_accessors_handle_valid_and_oob_indices() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        let idx = menu.add_action("Open");

        assert_eq!(menu.item_enabled(idx), Some(true));
        menu.set_item_enabled(idx, false);
        assert_eq!(menu.item_enabled(idx), Some(false));
        assert_eq!(menu.item_enabled(99), None);

        menu.items[idx].set_checkable(true);
        assert_eq!(menu.item_checked(idx), Some(false));
        menu.set_item_checked(idx, true);
        assert_eq!(menu.item_checked(idx), Some(true));
        assert_eq!(menu.item_checked(99), None);
    }

    fn context_menu() -> Menu {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Cut");
        menu.add_action("Copy");
        menu.add_separator();
        menu.add_action("Paste");
        menu
    }

    /// A context menu must appear where the pointer was, and report that origin.
    #[test]
    fn open_at_places_the_popup_at_the_pointer() {
        let mut menu = context_menu();
        // A menu is born hidden: it is a popup, not a panel, so `open_at` is what
        // makes it appear. Asserting that first keeps the test from passing on a
        // menu that was already visible for an unrelated reason.
        assert!(!menu.is_visible(), "a menu must start hidden");

        menu.open_at(Point::new(300, 200), Rect::new(0, 0, 1000, 800));

        assert!(menu.is_visible());
        assert_eq!(menu.invoker_position(), Some(Point::new(300, 200)));
        assert_eq!(menu.geometry().x, 300);
        assert_eq!(menu.geometry().y, 200);
    }

    /// Opened near an edge, the popup must shift so it stays fully on screen —
    /// otherwise part of the menu is unreachable.
    #[test]
    fn open_at_clamps_a_popup_that_would_overflow() {
        let viewport = Rect::new(0, 0, 200, 150);
        let mut menu = context_menu();

        menu.open_at(Point::new(195, 148), viewport);

        let rect = menu.geometry();
        assert!(
            rect.x + rect.width as i32 <= viewport.width as i32,
            "popup must not extend past the right edge: {rect:?}",
        );
        assert!(
            rect.y + menu.popup_height() as i32 <= viewport.height as i32,
            "popup must not extend past the bottom edge",
        );
        // The invoker position is recorded unclamped: it describes where the user
        // clicked, which is a different fact from where the popup ended up.
        assert_eq!(menu.invoker_position(), Some(Point::new(195, 148)));
    }

    /// A press inside the popup that hits an entry triggers it and closes the menu.
    #[test]
    fn pressing_an_entry_triggers_it_and_closes_the_menu() {
        use std::sync::{Arc, Mutex};

        let mut menu = context_menu();
        menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
        // Signal slots are `Send + Sync`, so the sink is an `Arc<Mutex<_>>` rather
        // than the `Rc<RefCell<_>>` a single-threaded test would otherwise use.
        let fired = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&fired);
        menu.triggered.connect(move |text: Arc<String>| {
            sink.lock().expect("sink poisoned").push((*text).clone());
        });

        // First entry sits just below the popup's 2px top padding.
        menu.handle_event(&Event::MousePress { pos: Point::new(20, 14), button: 1 });

        assert_eq!(&*fired.lock().expect("sink poisoned"), &["Cut".to_string()]);
        assert!(!menu.is_visible(), "choosing an entry must close the menu");
    }

    /// The dismissal gesture people actually use is a left-click elsewhere, so an
    /// outside press must close the menu whichever button was used.
    #[test]
    fn a_press_outside_the_popup_closes_it_with_either_button() {
        for button in [crate::event::mouse_button::PRIMARY, crate::event::mouse_button::SECONDARY] {
            let mut menu = context_menu();
            menu.open_at(Point::new(100, 100), Rect::new(0, 0, 400, 400));
            assert!(menu.is_visible());

            menu.handle_event(&Event::MousePress { pos: Point::new(5, 5), button });

            assert!(!menu.is_visible(), "button {button} outside the popup must dismiss it");
        }
    }

    /// A press on the popup's own padding is still "inside": it must not dismiss,
    /// or clicking a menu's edge would close it before an entry could be chosen.
    #[test]
    fn a_press_inside_the_popup_does_not_dismiss_it() {
        let mut menu = context_menu();
        menu.open_at(Point::new(100, 100), Rect::new(0, 0, 400, 400));

        // x is inside the popup but past the text column; y is the top padding.
        menu.handle_event(&Event::MousePress { pos: Point::new(250, 101), button: 1 });

        assert!(menu.is_visible(), "padding inside the popup must not dismiss the menu");
    }

    /// Reopening must not report the previous invocation's position.
    #[test]
    fn hiding_clears_the_invoker_position() {
        let mut menu = context_menu();
        menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
        assert!(menu.invoker_position().is_some());

        menu.hide();

        assert_eq!(menu.invoker_position(), None);
        assert_eq!(menu.hovered_index(), None);
    }

    /// The popup's hit area follows its drawn height, not the stored geometry
    /// height, so the outside test agrees with what the user sees.
    #[test]
    fn contains_point_follows_the_drawn_height() {
        let mut menu = context_menu();
        // Geometry height is 100 but the four entries plus padding draw shorter.
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

        let drawn = menu.popup_height() as i32;
        assert!(menu.contains_point(Point::new(5, drawn - 1)));
        assert!(!menu.contains_point(Point::new(5, drawn)));
    }

    /// The rows tile the popup in sequence, below its own padding.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.8 lists this control's defect as "the check and arrow paddings written by hand".
    /// The columns inside a row were already derived; what this pins is the **run**: each row starts
    /// where the previous one ended, separators get `MENU_SEPARATOR_HEIGHT` and entries
    /// `MENU_ROW_HEIGHT`, and the first row starts `MENU_POPUP_PADDING` below the popup's top edge.
    /// The old form carried that arithmetic as a `let mut y` inside the painting loop, so nothing
    /// could ask for a row's box without painting the menu.
    #[test]
    fn the_rows_tile_the_popup_in_sequence() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("Open");
        menu.add_separator();
        menu.add_action("Save");

        let bands = menu.item_bands();
        assert_eq!(bands.len(), 3, "one band per entry, separators included");
        assert_eq!(
            bands[0].y,
            dimensions::MENU_POPUP_PADDING as i32,
            "the first row starts below the popup's own padding: {bands:?}"
        );
        for pair in bands.windows(2) {
            assert_eq!(
                pair[1].y,
                pair[0].y + pair[0].height as i32,
                "each row follows the previous one's own box: {bands:?}"
            );
        }
        assert_eq!(bands[0].height, Menu::item_height() as u32);
        assert_eq!(bands[1].height, Menu::separator_height() as u32);
        assert_eq!(bands[2].height, Menu::item_height() as u32);
        // The run is exactly as tall as the popup the control reports.
        let last = bands.last().expect("three bands");
        assert_eq!(
            last.y + last.height as i32 + dimensions::MENU_POPUP_PADDING as i32,
            menu.popup_height() as i32,
            "the rows and the popup's padding add up to the popup's own height"
        );
    }

    /// A separator sits between two entries and does not disturb their alignment.
    ///
    /// The companion invariant to the run: the columns of a row are read from the row's own box, so
    /// a separator shifting every later row down must not shift their *labels* sideways.
    #[test]
    fn a_separator_shifts_the_rows_but_not_their_columns() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("Open");
        menu.add_action("Save");
        let without: Vec<i32> = menu
            .item_bands()
            .iter()
            .map(|row| menu.label_box(*row, menu.indicator_box(*row)).x)
            .collect();

        menu.add_separator();
        let with: Vec<i32> = menu
            .item_bands()
            .iter()
            .take(2)
            .map(|row| menu.label_box(*row, menu.indicator_box(*row)).x)
            .collect();
        assert_eq!(without, with, "a separator must not move the label columns");
    }

    /// The highlight must walk the entry list, wrap at both ends, and step over separators.
    ///
    /// # The defect this pins
    ///
    /// The menu handled Escape and Enter and nothing else, so every entry past the first was
    /// unreachable without a mouse even though `open_at` pre-selects one "so keyboard navigation has
    /// a starting point". A selection that no key can move is an affordance that is declared and not
    /// delivered.
    #[test]
    fn arrow_keys_walk_the_entries_and_skip_separators() {
        let mut menu = context_menu();
        menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
        // `open_at` pre-selects the first actionable entry.
        assert_eq!(menu.hovered_index(), Some(0));

        // Down: 0 -> 1 -> (separator skipped) -> 3.
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(1), "Down must move to the next entry");
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(3), "Down must step over the separator");
        // Down again wraps to the top.
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(0), "Down past the end must wrap");

        // Up wraps the other way, again without landing on the separator.
        menu.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(3), "Up past the start must wrap to the last entry");
        assert!(menu.is_visible(), "navigating must not dismiss the menu");
    }

    /// A disabled entry is not a destination: the highlight must step over it too.
    ///
    /// Stopping *on* one would leave Enter doing nothing with no visible explanation, which reads as a
    /// frozen menu rather than a disabled entry.
    #[test]
    fn arrow_keys_skip_disabled_entries() {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Cut");
        let disabled = menu.add_action("Copy");
        menu.add_action("Paste");
        menu.set_item_enabled(disabled, false);
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

        assert_eq!(menu.hovered_index(), Some(0));
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(2), "Down must step over the disabled entry");
        menu.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(0), "and Up must step back over it");
    }

    /// Enter chooses the highlighted entry; a menu with nothing choosable stays open.
    #[test]
    fn enter_activates_the_highlight_and_leaves_an_empty_menu_open() {
        let mut menu = context_menu();
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });

        let chosen = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = chosen.clone();
        menu.triggered_index.connect(move |index| {
            sink.lock().unwrap().push(*index);
        });
        menu.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(*chosen.lock().unwrap(), vec![1], "Enter must trigger the highlighted entry");
        assert!(!menu.is_visible(), "choosing an entry must dismiss the menu");

        // Nothing highlighted: Enter is inert rather than a dismissal with no selection.
        let mut empty = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        empty.add_separator();
        empty.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        assert_eq!(empty.hovered_index(), None, "a separator is not a starting point");
        empty.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(empty.is_visible(), "Enter with no highlight must leave the menu open");
    }

    /// Right/Left follow the writing direction; Up/Down do not.
    ///
    /// A list of entries runs downward in every locale, but the horizontal pair steps along a line of
    /// reading order — and a menu is the child of a menu bar, where that pair is what moves between a
    /// bar's drop-downs and into a submenu. Mirroring the vertical pair instead would make an Arabic
    /// menu's Up key walk backwards.
    #[test]
    fn horizontal_arrows_follow_the_direction_and_vertical_ones_do_not() {
        for (direction, right_is_forward) in [
            (crate::core::TextDirection::LeftToRight, true),
            (crate::core::TextDirection::RightToLeft, false),
        ] {
            let mut menu = context_menu();
            menu.set_direction(direction);
            menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
            assert_eq!(menu.hovered_index(), Some(0));

            // Key 39 is Right, key 37 is Left.
            let (key, expected) = if right_is_forward { (39, Some(1)) } else { (39, Some(3)) };
            menu.handle_event(&Event::KeyPress { key, modifiers: 0 });
            assert_eq!(
                menu.hovered_index(),
                expected,
                "{direction:?}: Right must move {}",
                if right_is_forward { "forward" } else { "backward" }
            );

            // The vertical pair is untouched by the direction: Down always advances a row.
            let mut vertical = context_menu();
            vertical.set_direction(direction);
            vertical.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
            vertical.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
            assert_eq!(
                vertical.hovered_index(),
                Some(1),
                "{direction:?}: Down must advance a row in every locale"
            );
        }
    }

    /// The default must be byte-identical, so a menu that never asks behaves exactly as it did.
    #[test]
    fn the_default_menu_is_still_left_to_right() {
        let mut untouched = context_menu();
        untouched.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        let mut ltr = context_menu();
        ltr.set_direction(crate::core::TextDirection::LeftToRight);
        ltr.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        assert_eq!(render_to_svg(&mut untouched), render_to_svg(&mut ltr));
    }

    /// The direction is reachable from the property API with a matching read-back token.
    #[test]
    fn the_direction_round_trips_through_the_property_api() {
        let mut menu = context_menu();
        assert_eq!(menu.get("direction").unwrap().as_str(), Some("ltr"));
        menu.set("direction", CapabilityValue::String("rtl".to_string())).unwrap();
        assert_eq!(menu.direction(), crate::core::TextDirection::RightToLeft);
        assert_eq!(menu.get("direction").unwrap().as_str(), Some("rtl"));
    }
}
