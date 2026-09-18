// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
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
    fn separator_height() -> f32 {
        6.0
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
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "hovered_index" => Err(CapabilityAccessError::ReadOnlyProperty),
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "item_count", "hovered_index", BASE_PROPERTY_NAMES]
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
                if *key == 27 {
                    self.hide();
                }
                // Escape
                else if *key == 13 {
                    // Enter — trigger hovered
                    if let Some(idx) = self.hovered_index {
                        if let Some(item) = self.items.get(idx) {
                            if !item.is_separator() && item.is_enabled() {
                                let text = item.text().to_string();
                                self.triggered.emit(text);
                                self.hide();
                            }
                        }
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Menu {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.is_visible() {
            return;
        }
        let rect = self.geometry();
        let popup_h = self.popup_height();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, popup_h as u32),
            Color::rgb(250, 250, 250),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, popup_h as u32),
            Color::rgb(160, 160, 160),
        );
        let mut y = rect.y as f32 + 2.0;
        for (i, item) in self.items.iter().enumerate() {
            if item.is_separator() {
                let sep_y = y + Self::separator_height() / 2.0;
                context.draw_line(
                    Point::new(rect.x + 4, sep_y as i32),
                    Point::new(rect.x + rect.width as i32 - 4, sep_y as i32),
                    Color::rgb(200, 200, 200),
                );
                y += Self::separator_height();
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            if is_hovered {
                context.fill_rect(
                    Rect::new(
                        rect.x + 2,
                        y as i32,
                        rect.width.saturating_sub(4),
                        Self::item_height() as u32,
                    ),
                    Color::rgb(0, 120, 215),
                );
            }
            let fg = if !item.is_enabled() {
                Color::rgb(150, 150, 150)
            } else if is_hovered {
                Color::rgb(255, 255, 255)
            } else {
                Color::rgb(0, 0, 0)
            };
            if item.is_checkable() {
                let check_sym = if item.is_checked() { "✓" } else { " " };
                context.draw_text(
                    Point::from_f32(rect.x as f32 + 8.0, y + Self::item_height() / 2.0),
                    check_sym,
                    &Font::default(),
                    fg,
                    HorizontalAlignment::Left,
                );
            }
            context.draw_text(
                Point::from_f32(rect.x as f32 + 28.0, y + Self::item_height() / 2.0),
                item.text(),
                &Font::default(),
                fg,
                HorizontalAlignment::Left,
            );
            if !item.shortcut().is_empty() {
                context.draw_text(
                    Point::new(
                        rect.x + rect.width as i32 - 8,
                        (y + Self::item_height() / 2.0) as i32,
                    ),
                    item.shortcut(),
                    &Font::default(),
                    fg,
                    HorizontalAlignment::Left,
                );
            }
            if item.has_submenu() {
                context.draw_text(
                    Point::new(
                        rect.x + rect.width as i32 - 4,
                        (y + Self::item_height() / 2.0) as i32,
                    ),
                    "▶",
                    &Font::default(),
                    fg,
                    HorizontalAlignment::Left,
                );
            }
            y += Self::item_height();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
