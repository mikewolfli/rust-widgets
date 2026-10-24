// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MenuButton widget — a button that shows a dropdown menu when clicked.
//!
//! The MenuButton displays as a button with text and an optional icon. When
//! clicked, it toggles a dropdown menu containing items. Each item supports
//! an icon, enabled/disabled state, checked state, and optional submenu items.
//! Selecting a menu item emits an `item_triggered` signal with the item's ID.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
pub use crate::widget::menu_toolbar::MAX_SUBMENU_DEPTH;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Default width of the dropdown menu panel.
const DEFAULT_MENU_WIDTH: u32 = 180;
/// Height of each menu item row.
const ITEM_HEIGHT: i32 = 28;
/// Padding inside the button and menu.
const PADDING: i32 = 8;

/// The record form of a `MenuButton`'s `items` property: one row per **top-level** item.
///
/// # Why the write side is refused
///
/// `MenuItem` is a public-field struct whose `submenu` holds a nested `Vec<MenuItem>`, and a record row
/// cannot carry a subtree. A writable `items` property would have to invent the children or drop them,
/// and a write that loses a cascading submenu is the silent-loss shape §19.6 forbids. The rows
/// therefore publish what a document can hold faithfully (id, text, enabled, checked, typed icon) and
/// a write is refused with the standard read-only error; `add_item` / `set_item_icon_source` /
/// `clear_items` remain the write paths.
pub const MENU_BUTTON_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::UInt,
            ),
            crate::widget::capability::RecordField::required(
                "text",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "checked",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`MENU_BUTTON_ITEM_SCHEMA`]; refuses a source with no document form.
fn menu_item_record(
    item: &MenuItem,
    icon_source: Option<IconSource>,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::UInt(item.id))
        .with("text", CapabilityValue::String(item.text.clone()))
        .with("enabled", CapabilityValue::Bool(item.enabled))
        .with("checked", CapabilityValue::Bool(item.checked));
    match icon_source {
        // No typed override: the field is omitted, which is how a document says "no typed icon"
        // (the item's legacy `icon` text icon is a different channel and stays where it is).
        None => {}
        Some(IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry has no document form, so the read refuses rather than reporting
        // the row as icon-less — the rule the control-level `icon_source` owners already follow.
        Some(IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    Ok(record)
}

/// A single item inside a MenuButton dropdown.
///
/// Each item has a unique ID, display text, optional icon, and flags for
/// enabled/disabled and checked state. Items can also have an optional
/// submenu (nested `MenuItem` list) for cascading menus.
///
/// `Clone` is what lets an open child panel hold its own rows while the item tree it came from
/// stays owned by the control: a submenu that borrowed the tree could not outlive a mutation of
/// the parent list.
#[derive(Clone, Debug)]
pub struct MenuItem {
    /// Unique identifier for this item.
    pub id: u64,
    /// Display text.
    pub text: String,
    /// Optional icon character/emoji.
    pub icon: Option<String>,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Whether this item is checked (shows a checkmark).
    pub checked: bool,
    /// Optional submenu items for cascading menus.
    pub submenu: Option<Vec<MenuItem>>,
}

impl MenuItem {
    /// Creates a new MenuItem with the given id and text.
    pub fn new(id: u64, text: &str) -> Self {
        Self {
            id,
            text: text.to_string(),
            icon: None,
            enabled: true,
            checked: false,
            submenu: None,
        }
    }

    /// Sets the icon for this menu item.
    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Sets the enabled state for this menu item.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the checked state for this menu item.
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Sets the submenu items for this menu item.
    pub fn with_submenu(mut self, items: Vec<MenuItem>) -> Self {
        self.submenu = Some(items);
        self
    }
}

/// The panel rectangle a menu anchored at `anchor` occupies.
///
/// Shared by this control, its child panels and `ToolButton`, so every surface that hangs a
/// menu below a button derives the same extent instead of repeating the literals.
pub(crate) fn menu_panel_rect(anchor: Rect, item_count: usize) -> Rect {
    let menu_width = anchor.width.max(DEFAULT_MENU_WIDTH);
    let menu_height = (item_count as u32 * ITEM_HEIGHT as u32).max(1);
    Rect::new(anchor.x, anchor.y + anchor.height as i32, menu_width, menu_height)
}

/// Row `index` inside `panel`.
pub(crate) fn menu_item_rect(panel: Rect, index: usize) -> Rect {
    Rect::new(panel.x, panel.y + index as i32 * ITEM_HEIGHT, panel.width, ITEM_HEIGHT as u32)
}

/// The right-hand child panel of a branch row.
pub(crate) fn menu_submenu_panel_rect(parent: Rect, anchor_row: Rect, item_count: usize) -> Rect {
    let width = parent.width.max(DEFAULT_MENU_WIDTH);
    let height = (item_count as u32 * ITEM_HEIGHT as u32).max(1);
    Rect::new(parent.x + parent.width as i32, anchor_row.y, width, height)
}

/// One open child panel: the rows it shows and the rectangle it occupies.
///
/// The stack keeps one of these per open level, which is what makes a four-deep cascade one
/// control state instead of a chain of mutually exclusive `Option` fields.
struct SubmenuPanel {
    items: Vec<MenuItem>,
    rect: Rect,
}

/// Paints one menu list and the panel it sits in.
///
/// This is the single painter for `MenuButton`'s menu, its child panels and `ToolButton`'s
/// popup: the geometry comes from [`menu_panel_rect`]/[`menu_item_rect`], so a row can never be
/// hit-tested somewhere the paint did not put it.
pub(crate) fn draw_menu_panel(
    context: &mut RenderContext,
    panel: Rect,
    items: &[MenuItem],
    icon_sources: &[(u64, IconSource)],
    style: &crate::style::WidgetStyle,
    accent: Color,
) {
    draw_menu_panel_with_branches(context, panel, items, icon_sources, style, accent, true);
}

/// The shared menu painter, with the disclosure arrow made conditional.
///
/// `branches_open` says whether a row with children may open another panel. A one-level menu
/// passes `true`; the deepest level of a cascade passes `false`, because an arrow that cannot
/// open anything would promise a level the control refuses to build.
pub(crate) fn draw_menu_panel_with_branches(
    context: &mut RenderContext,
    panel: Rect,
    items: &[MenuItem],
    icon_sources: &[(u64, IconSource)],
    style: &crate::style::WidgetStyle,
    accent: Color,
    branches_open: bool,
) {
    // The popup panel is chrome: a popup that stayed white in a dark theme would be the
    // brightest object on screen. It reads the resolved surface, and the literal it used to be
    // stays as the fallback.
    let themed = crate::style::resolved_theme_style_shared("menu_button");
    let panel_fill =
        themed.as_ref().and_then(|resolved| resolved.background_color).unwrap_or(Color::WHITE);
    let panel_border = style
        .border_color
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.border_color))
        .unwrap_or(Color::rgba(190, 190, 200, 200));
    let item_ink = style
        .text_color
        .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
        .unwrap_or(Color::rgb(33, 33, 33));
    // A selected row's highlight is chrome, not data, so it is resolved from the theme's
    // accent rather than from a literal tint of blue.
    let item_highlight = panel_fill.blend(&accent, 0.3);
    let item_rule = panel_fill.blend(&item_ink, 0.08);

    context.fill_rounded_rect(panel, 4, panel_fill);
    context.draw_rounded_rect_stroke(panel, 4, panel_border, 1);

    // One resolved font for every row: it used to be rebuilt per item.
    let item_font = effective_font(style);
    for (i, item) in items.iter().enumerate() {
        let item_rect = menu_item_rect(panel, i);

        // Highlight on checked rows
        if item.checked {
            context.fill_rounded_rect(item_rect, 2, item_highlight);
        }

        let item_text_color =
            if !item.enabled { item_ink.blend(&panel_fill, 0.5) } else { item_ink };
        let mut item_x = item_rect.x + PADDING;

        // Item icon
        let item_source =
            icon_sources.iter().find(|(id, _)| *id == item.id).map(|(_, source)| *source);
        if let Some(source) = item_source {
            let icon_line = context.text_line(item_rect, item_font);
            let side = icon_line.height.clamp(1, 16);
            let icon_box = Rect::new(item_x, icon_line.y, side, side.min(item_rect.height));
            if let Err(error) =
                crate::widget::paint_icon(context, source.into(), icon_box, item_text_color)
            {
                log::warn!("menu item {id} icon could not be painted: {error:?}", id = item.id);
            }
            item_x += 20;
        } else if let Some(ref item_icon) = item.icon {
            let icon_font = Font::simple("sans-serif", 13.0);
            // The icon and the label occupy the same row, so both take their origin from the
            // line box centred in it: the row midpoint is the glyph top edge, and using it drew
            // them half a line low.
            let icon_line = context.text_line(item_rect, &icon_font);
            context.draw_text(
                Point::new(item_x, icon_line.y),
                item_icon,
                &icon_font,
                item_text_color,
                HorizontalAlignment::Left,
            );
            item_x += 20;
        }

        let item_line = context.text_line(item_rect, item_font);
        // Fitted between the label start and the checkmark zone: an unbounded label ran under
        // the check glyph on checked rows (and past the popup edge on long ones).
        let item_text_right = if item.checked {
            item_rect.x + item_rect.width as i32 - PADDING - 14
        } else {
            item_rect.x + item_rect.width as i32 - PADDING
        };
        let item_text_width = (item_text_right - item_x).max(0) as u32;
        context.draw_text_fitted_cow(
            Rect::new(item_x, item_line.y, item_text_width, item_line.height.max(1)),
            &item.text,
            item_font,
            item_text_color,
            HorizontalAlignment::Left,
        );

        // Checkmark for checked items. Drawn as an `IconName` outline rather than the `✓`
        // (U+2713) character: no bundled face covers the dingbats block, so the character fell
        // back to an 8x8 bitmap and read as a blob.
        if item.checked {
            let check_x = item_rect.x + item_rect.width as i32 - PADDING - 12;
            let check_line = context.text_line(item_rect, item_font);
            let side = check_line.height.max(1);
            let check_rect = Rect::new(
                check_x,
                check_line.y + (check_line.height as i32 - side as i32) / 2,
                side,
                side,
            );
            crate::widget::draw_icon_at(
                context,
                check_rect,
                accent.contrast_color(),
                IconName::Check,
            );
        }

        // Submenu indicator
        if branches_open && item.submenu.is_some() && item.enabled {
            let sub_x = item_rect.x + item_rect.width as i32 - PADDING - 8;
            let sub_y = item_rect.y + item_rect.height as i32 / 2 - 4;
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(sub_x, sub_y),
                    Point::new(sub_x, sub_y + 8),
                    Point::new(sub_x + 5, sub_y + 4),
                ],
                closed: true,
                color: item_ink.blend(&panel_fill, 0.45),
                filled: true,
                width: 1,
            });
        }

        // Separator between rows (except the last)
        if i + 1 < items.len() {
            context.draw_line(
                Point::new(item_rect.x + 4, item_rect.y + item_rect.height as i32),
                Point::new(
                    item_rect.x + item_rect.width as i32 - 4,
                    item_rect.y + item_rect.height as i32,
                ),
                item_rule,
            );
        }
    }
}

/// How a menu button splits between its face and its disclosure arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuButtonPopup {
    /// A press anywhere on the button opens the menu. This is the historical behaviour
    /// and stays the default, so an unconfigured button is unchanged.
    #[default]
    InstantPopup,
    /// The face reports [`MenuButton::clicked`]; only the arrow segment opens the menu.
    MenuButtonPopup,
    /// A press on the face starts a timer; holding still for the configured delay opens
    /// the menu. Releasing before the delay fires nothing.
    DelayedPopup,
}

/// Which way a menu button's disclosure arrow points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuArrowType {
    /// No arrow is painted, and the label may use the full button width.
    None,
    /// A downward triangle: the menu opens below the button.
    #[default]
    Down,
    /// An upward triangle.
    Up,
    /// A leftward triangle.
    Left,
    /// A rightward triangle.
    Right,
}

/// MenuButton widget — a button that opens a dropdown menu on click.
///
/// Displays a button with text and an optional icon. When clicked, it toggles
/// a dropdown menu positioned below the button. Selecting a menu item emits
/// the `item_triggered` signal with the item's ID; activating a branch item
/// opens its child panel instead and emits `submenu_requested`.
pub struct MenuButton {
    base: BaseWidget,
    text: String,
    menu_items: Vec<MenuItem>,
    item_icon_sources: Vec<(u64, IconSource)>,
    icon: Option<String>,
    icon_source: Option<IconSource>,
    /// Whether the dropdown menu is currently visible.
    menu_open: bool,
    /// The menu's open/close reveal; see `menu::Menu` for why it is separate from `menu_open`.
    reveal: PopupReveal,
    /// The open child panels, deepest last. One panel per level, up to [`MAX_SUBMENU_DEPTH`].
    submenus: Vec<SubmenuPanel>,
    /// The top-level row the pointer last addressed, so Right can open its submenu.
    active_index: Option<usize>,
    /// How the button's face and arrow divide the press.
    popup_mode: MenuButtonPopup,
    /// Which direction the disclosure arrow points.
    arrow_type: MenuArrowType,
    /// How long a DelayedPopup press must be held before the menu opens.
    popup_delay_ms: u32,
    /// Elapsed milliseconds of the current held press, when one is timing.
    press_elapsed: Option<u32>,
    /// Emitted when the face of a [`MenuButtonPopup`]-mode button is pressed.
    pub clicked: GenericSignal,
    /// Emitted just before the menu opens.
    pub about_to_show: GenericSignal,
    /// Emitted just after the menu closes.
    pub about_to_hide: GenericSignal,
    /// Emitted when a menu item is clicked, providing the item ID.
    pub item_triggered: Signal1<u64>,
    /// Emitted when a branch row opens its child panel, carrying the branch's text.
    pub submenu_requested: Signal1<String>,
}

impl MenuButton {
    /// Creates a new MenuButton widget with the given geometry.
    pub fn new(text: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MenuButton, geometry, "MenuButton"),
            text: text.to_string(),
            menu_items: Vec::new(),
            item_icon_sources: Vec::new(),
            icon: None,
            icon_source: None,
            menu_open: false,
            reveal: PopupReveal::new(false),
            submenus: Vec::new(),
            active_index: None,
            popup_mode: MenuButtonPopup::InstantPopup,
            arrow_type: MenuArrowType::Down,
            popup_delay_ms: 300,
            press_elapsed: None,
            clicked: GenericSignal::new(),
            about_to_show: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
            item_triggered: Signal1::new(),
            submenu_requested: Signal1::new(),
        }
    }

    /// Returns the button text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the button text.
    pub fn set_text(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        self.text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the optional icon.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// Sets the button icon.
    pub fn set_icon(&mut self, icon: &str) {
        if self.icon.as_deref() == Some(icon) && self.icon_source.is_none() {
            return;
        }
        self.icon = Some(icon.to_string());
        self.icon_source = None;
        self.base.request_redraw();
    }

    /// Sets a built-in or static-geometry icon, replacing the legacy text icon.
    ///
    /// `icon()` returns `None` while this typed source is active.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon = None;
        self.icon_source = source;
        self.base.request_redraw();
    }

    /// Returns the typed icon source, if one is active.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Clears the icon.
    pub fn clear_icon(&mut self) {
        self.icon = None;
        self.icon_source = None;
        self.base.request_redraw();
    }

    /// Adds a menu item to the dropdown.
    pub fn add_item(&mut self, item: MenuItem) {
        self.item_icon_sources.retain(|(id, _)| *id != item.id);
        self.menu_items.push(item);
        self.base.request_redraw();
    }

    /// Assigns a typed icon source to the uniquely identified top-level menu item.
    ///
    /// The legacy [`MenuItem::icon`] remains text. When a typed source is active it occupies that
    /// item's icon slot instead; setting `None` restores the legacy text icon. Duplicate IDs are
    /// refused because they cannot identify one item unambiguously.
    pub fn set_item_icon_source(
        &mut self,
        id: u64,
        source: Option<IconSource>,
    ) -> Result<(), crate::widget::IconItemError> {
        let matches = self.menu_items.iter().filter(|item| item.id == id).count();
        if matches == 0 {
            return Err(crate::widget::IconItemError::ItemNotFound { id });
        }
        if matches > 1 {
            return Err(crate::widget::IconItemError::AmbiguousItemId { id });
        }

        self.item_icon_sources.retain(|(stored_id, _)| *stored_id != id);
        if let Some(source) = source {
            self.item_icon_sources.push((id, source));
        }
        self.base.request_redraw();
        Ok(())
    }

    /// The typed icon source assigned to the item with `id`, if any.
    pub fn item_icon_source(&self, id: u64) -> Option<IconSource> {
        self.item_icon_sources.iter().find(|(stored_id, _)| *stored_id == id).map(|(_, s)| *s)
    }

    /// Removes a menu item by its ID.
    pub fn remove_item(&mut self, id: u64) -> bool {
        let len_before = self.menu_items.len();
        self.menu_items.retain(|item| item.id != id);
        let removed = self.menu_items.len() < len_before;
        if removed {
            self.item_icon_sources.retain(|(item_id, _)| *item_id != id);
            // Every open panel holds a snapshot of the removed row's subtree, so none of
            // them can be trusted to describe the menu that is still there.
            self.close_all_submenus();
            self.base.request_redraw();
        }
        removed
    }

    /// Removes all menu items.
    pub fn clear_items(&mut self) {
        self.menu_items.clear();
        self.item_icon_sources.clear();
        self.close_all_submenus();
        self.base.request_redraw();
    }

    /// Returns the number of menu items.
    pub fn item_count(&self) -> usize {
        self.menu_items.len()
    }

    /// Returns a reference to the menu items.
    pub fn items(&self) -> &[MenuItem] {
        &self.menu_items
    }

    /// Returns a mutable reference to the menu items.
    pub fn items_mut(&mut self) -> &mut Vec<MenuItem> {
        self.item_icon_sources.clear();
        self.close_all_submenus();
        &mut self.menu_items
    }

    /// Returns whether the dropdown menu is currently open.
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// Whether a branch's child panel is currently showing.
    pub fn is_submenu_open(&self) -> bool {
        !self.submenus.is_empty()
    }

    /// How many nested panels are open below the menu's own list.
    ///
    /// `0` when nothing is open and at most [`MAX_SUBMENU_DEPTH`], because a branch on the
    /// deepest level cannot open another panel.
    pub fn submenu_depth(&self) -> usize {
        self.submenus.len()
    }

    /// How the face and the disclosure arrow divide a press.
    pub fn popup_mode(&self) -> MenuButtonPopup {
        self.popup_mode
    }

    /// Changes the press split. See [`MenuButtonPopup`] for the three modes.
    pub fn set_popup_mode(&mut self, mode: MenuButtonPopup) {
        if self.popup_mode == mode {
            return;
        }
        self.popup_mode = mode;
        self.press_elapsed = None;
        self.base.request_redraw();
    }

    /// Which way the disclosure arrow points.
    pub fn arrow_type(&self) -> MenuArrowType {
        self.arrow_type
    }

    /// Sets the disclosure arrow's direction. [`MenuArrowType::None`] removes it and
    /// hands its width back to the label.
    pub fn set_arrow_type(&mut self, arrow: MenuArrowType) {
        if self.arrow_type == arrow {
            return;
        }
        self.arrow_type = arrow;
        self.base.request_redraw();
    }

    /// How long a [`MenuButtonPopup::DelayedPopup`] press must be held.
    pub fn popup_delay_ms(&self) -> u32 {
        self.popup_delay_ms
    }

    /// Sets the delayed-popup hold time. Zero opens on the next tick.
    pub fn set_popup_delay_ms(&mut self, delay_ms: u32) {
        if self.popup_delay_ms == delay_ms {
            return;
        }
        self.popup_delay_ms = delay_ms;
    }

    /// The segment of the button that opens the menu directly.
    ///
    /// The trailing 20 px, or the whole button when the arrow is hidden.
    pub fn arrow_rect(&self) -> Rect {
        let geom = self.geometry();
        if self.arrow_type == MenuArrowType::None {
            return geom;
        }
        let width = geom.width.min(20);
        Rect::new(geom.x + geom.width as i32 - width as i32, geom.y, width, geom.height)
    }

    /// Whether a delayed press is currently counting down.
    pub fn is_press_pending(&self) -> bool {
        self.press_elapsed.is_some()
    }

    /// Toggles the dropdown menu visibility.
    pub fn toggle_menu(&mut self) {
        if self.menu_open {
            self.close_menu();
        } else {
            self.open_menu();
        }
    }

    /// Opens the dropdown menu.
    pub fn open_menu(&mut self) {
        if self.menu_open {
            return;
        }
        self.about_to_show.emit();
        self.menu_open = true;
        self.reveal.set_open(true);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Closes the dropdown menu and every open child panel.
    pub fn close_menu(&mut self) {
        if !self.menu_open && self.submenus.is_empty() {
            return;
        }
        self.menu_open = false;
        self.reveal.set_open(false);
        self.submenus.clear();
        self.active_index = None;
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.id());
        self.base.request_redraw();
        self.about_to_hide.emit();
    }

    /// Collapses the deepest open child panel, leaving its parent level open.
    fn close_submenu(&mut self) {
        if self.submenus.pop().is_none() {
            return;
        }
        // The outside press that collapsed the child also cleared the runtime grab. The
        // parent level is still open, so re-arm it; otherwise the next outside press would
        // be delivered to whatever is behind this control.
        #[cfg(not(alloc_frugal))]
        if self.menu_open {
            crate::widget::runtime::set_popup_grab(self.id());
        }
        self.base.request_redraw();
    }

    /// Drops every open child panel, leaving the menu itself open.
    fn close_all_submenus(&mut self) {
        if self.submenus.is_empty() {
            return;
        }
        self.submenus.clear();
        #[cfg(not(alloc_frugal))]
        if self.menu_open {
            crate::widget::runtime::set_popup_grab(self.id());
        }
        self.base.request_redraw();
    }

    /// The rows shown at `level`: the menu's own list at level 0, a panel's rows below it.
    fn items_at(&self, level: usize) -> Option<&[MenuItem]> {
        if level == 0 {
            Some(&self.menu_items)
        } else {
            self.submenus.get(level - 1).map(|panel| panel.items.as_slice())
        }
    }

    /// The panel rectangle at `level`: level 0 is the menu's own list.
    fn panel_rect_at(&self, level: usize) -> Option<Rect> {
        if level == 0 {
            Some(self.menu_rect())
        } else {
            self.submenus.get(level - 1).map(|panel| panel.rect)
        }
    }

    /// Row `index` of `level`.
    fn row_rect_at(&self, level: usize, index: usize) -> Option<Rect> {
        let panel = self.panel_rect_at(level)?;
        let len = self.items_at(level)?.len();
        (index < len).then(|| menu_item_rect(panel, index))
    }

    /// Opens the child panel of the row at `index` in `level` to the right of that level.
    ///
    /// Activating a branch is not an activation of the branch's own action: no
    /// [`Self::item_triggered`] fires and the parent menu stays open, which is the difference
    /// between a row that leads somewhere and a row that does something.
    ///
    /// Opening replaces `level` and every deeper level, so picking a sibling branch closes
    /// the branch that was open beside it. A `level` at or beyond [`MAX_SUBMENU_DEPTH`] is
    /// refused, which is the depth contract this control shares with the rest of the family.
    fn open_submenu_at(&mut self, level: usize, index: usize) {
        if level >= MAX_SUBMENU_DEPTH {
            return;
        }
        let Some(item) = self.items_at(level).and_then(|items| items.get(index)) else {
            return;
        };
        if !item.enabled {
            return;
        }
        let Some(children) = item.submenu.clone().filter(|children| !children.is_empty()) else {
            return;
        };
        let Some(parent) = self.panel_rect_at(level) else {
            return;
        };
        let Some(anchor_row) = self.row_rect_at(level, index) else {
            return;
        };
        let text = item.text.clone();
        let count = children.len();
        self.submenu_requested.emit(text);
        self.submenus.truncate(level);
        self.submenus.push(SubmenuPanel {
            items: children,
            rect: menu_submenu_panel_rect(parent, anchor_row, count),
        });
        if level == 0 {
            self.active_index = Some(index);
        }
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Activates the row at `index` in `level`: a branch opens the next panel, a leaf emits
    /// and closes the whole popup.
    fn activate_row_at(&mut self, level: usize, index: usize) {
        let Some(item) = self.items_at(level).and_then(|items| items.get(index)) else {
            return;
        };
        if !item.enabled {
            return;
        }
        let id = item.id;
        let text = item.text.clone();
        let is_branch = item.submenu.as_ref().is_some_and(|children| !children.is_empty());
        if is_branch {
            if level >= MAX_SUBMENU_DEPTH {
                // Depth contract: announce the branch, but a fifth panel is refused.
                self.submenu_requested.emit(text);
            } else {
                self.open_submenu_at(level, index);
            }
            return;
        }
        self.item_triggered.emit(id);
        self.close_menu();
    }

    /// Activates a top-level row: a branch opens its child panel, a leaf emits and closes.
    fn activate_top_level(&mut self, index: usize) {
        self.activate_row_at(0, index);
    }

    /// The index of the enabled row in `level` that a Right key should open.
    fn keyboard_branch_index_at(&self, level: usize) -> Option<usize> {
        let is_branch = |index: usize| {
            self.items_at(level).and_then(|items| items.get(index)).is_some_and(|item| {
                item.enabled && item.submenu.as_ref().is_some_and(|c| !c.is_empty())
            })
        };
        if level == 0 {
            if let Some(index) = self.active_index.filter(|index| is_branch(*index)) {
                return Some(index);
            }
        }
        (0..self.items_at(level)?.len()).find(|index| is_branch(*index))
    }

    /// The deepest enabled submenu row at `pos`, as `(level, index)`.
    fn hit_test_submenu(&self, pos: Point) -> Option<(usize, usize)> {
        for (offset, panel) in self.submenus.iter().enumerate().rev() {
            if !panel.rect.contains_point(pos) {
                continue;
            }
            let level = offset + 1;
            for (index, item) in panel.items.iter().enumerate() {
                let row = menu_item_rect(panel.rect, index);
                if item.enabled && row.contains_point(pos) {
                    return Some((level, index));
                }
            }
        }
        None
    }

    /// Computes the menu rectangle positioned below the button.
    fn menu_rect(&self) -> Rect {
        menu_panel_rect(self.geometry(), self.menu_items.len())
    }

    /// Computes the rectangle for a specific menu item at the given index.
    fn item_rect(&self, index: usize) -> Rect {
        menu_item_rect(self.menu_rect(), index)
    }

    /// Finds the index of the menu item at the given position, if any.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let menu = self.menu_rect();
        if !menu.contains_point(pos) {
            return None;
        }
        for (i, item) in self.menu_items.iter().enumerate() {
            let ir = self.item_rect(i);
            if ir.contains_point(pos) && item.enabled {
                return Some(i);
            }
        }
        None
    }
}

impl Widget for MenuButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The button, its open menu, and every child panel accept the pointer.
    ///
    /// The menu is painted below the button's rectangle and each panel sits to the right of
    /// the one that opened it; without this override every click on any of them was rejected
    /// before `hit_test_item` could see it.
    fn contains_point(&self, point: crate::core::Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
            || (self.menu_open && self.menu_rect().contains_point(point))
            || self.submenus.iter().any(|panel| panel.rect.contains_point(point))
    }

    fn has_open_popup(&self) -> bool {
        self.menu_open
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(120, 28)
    }

    /// Advances the menu's open/close reveal; `true` while it still owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut animating = self.reveal.tick(delta_ms);
        if let Some(elapsed) = self.press_elapsed.as_mut() {
            *elapsed = elapsed.saturating_add(delta_ms);
            if *elapsed >= self.popup_delay_ms {
                self.press_elapsed = None;
                self.open_menu();
                animating = true;
            } else {
                animating = true;
            }
        }
        animating
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating() || self.press_elapsed.is_some()
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
            "item_triggered" => {
                Some(EventSignalRef::mapped("item_triggered", &self.item_triggered, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            "submenu_requested" => {
                Some(EventSignalRef::mapped("submenu_requested", &self.submenu_requested, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "about_to_hide" => Some(EventSignalRef::unit("about_to_hide", &self.about_to_hide)),
            "about_to_show" => Some(EventSignalRef::unit("about_to_show", &self.about_to_show)),
            "clicked" => Some(EventSignalRef::unit("clicked", &self.clicked)),
            _ => None,
        }
    }
}

/// `MenuButton`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `expanded` is the published
/// name for the open/closed state [`MenuButton::is_menu_open`] reports, because
/// that is the name the schema declares. `item_count` is derived from the menu
/// item list and has no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for MenuButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "items" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &MENU_BUTTON_ITEM_SCHEMA,
                    self.menu_items
                        .iter()
                        .map(|item| menu_item_record(item, self.item_icon_source(item.id)))
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_menu_open())),
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                // A borrowed static-geometry source has no document representation, so reading it
                // back as one would be a lie; the caller is told instead.
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "expanded" => {
                if expect_bool(value)? {
                    self.open_menu();
                } else {
                    self.close_menu();
                }
                Ok(())
            }
            "icon_source" => {
                match value {
                    CapabilityValue::Null => self.set_icon_source(None),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                        name,
                    )) => self.set_icon_source(Some(IconSource::Builtin(name))),
                    // A resource ID needs a host resolver; turning it into a path here would be
                    // guessing at the host's resource policy.
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                        ..
                    }) => return Err(CapabilityAccessError::ResourceResolutionRequired),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            // Derived from the menu item list, which owns it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // A row cannot express `MenuItem::submenu`, so a write would silently discard a cascading
            // menu. Refused with the standard read-only error; `add_item` / `set_item_icon_source`
            // are the write paths. See `MENU_BUTTON_ITEM_SCHEMA`.
            "items" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn set_resolved_icon_source(
        &mut self,
        source: Option<IconSource>,
    ) -> Result<(), CapabilityAccessError> {
        self.set_icon_source(source);
        Ok(())
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "icon_source",
            "items",
            "item_count",
            "expanded",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for MenuButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve the explicit style first, then the theme's resolved style
        // for this control, and only then the original literal. The literal stays as the
        // fallback so an inactive theme still has a defined appearance, and because keeping
        // the old value means the existing pixel baselines cannot regress. The theme read is
        // a separate manager lock, taken and released inside `resolved_theme_style`, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        // The **effective font** — the resolved theme/caller font — for the button's own label;
        // the popup rows take the same font below (D09-STYLE-01).
        let font = effective_font(style);
        let themed = crate::style::resolved_theme_style_shared("menu_button");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        let themed_border = themed.as_ref().and_then(|r| r.border_color);
        let themed_text = themed.as_ref().and_then(|r| r.text_color);
        // `menu_button` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background`; the fallback is the button's
        // own resting grey rather than that surface, because the stroke below needs a fill to
        // stand against.
        let background =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(235, 235, 240, 200));
        let border =
            style.border_color.or(themed_border).unwrap_or(Color::rgba(180, 180, 190, 200));
        let ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(33, 33, 33));
        // The three button states are derived from the resolved pair instead of from three
        // separate literals, so they keep their relationship in either appearance: a disabled
        // button recedes towards the surface, an open-menu button steps towards the accent.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|theme| theme.colors.accent)
            .unwrap_or(Color::rgba(200, 200, 220, 200));
        let disabled_ink = self.base.disabled_ink_on(ink, background);

        // ── Draw button background ──
        let bg_color = if !is_enabled {
            background.blend(&ink, 0.08)
        } else if self.menu_open {
            background.blend(&accent, 0.25)
        } else {
            background
        };
        context.fill_rounded_rect(geom, 4, bg_color);
        context.draw_rounded_rect_stroke(geom, 4, border, 1);

        // ── Draw icon if present ──
        let mut text_offset_x = geom.x + PADDING;
        if let Some(source) = self.icon_source {
            let side = geom.height.saturating_sub((PADDING * 2) as u32).clamp(1, 16);
            let line = context.text_line(geom, font);
            let icon_rect = Rect::new(text_offset_x, line.y, side, side);
            let icon_color = if is_enabled { ink } else { disabled_ink };
            if let Err(error) =
                crate::widget::paint_icon(context, source.into(), icon_rect, icon_color)
            {
                log::warn!("menu button icon could not be painted: {error:?}");
            }
            text_offset_x += 22;
        } else if let Some(ref icon_str) = self.icon {
            let icon_font = Font::simple("sans-serif", 14.0);
            // The icon is a glyph too, so its origin is a top edge: taking the button's raw
            // midpoint as the origin drew it half a line low. The line box supplies the
            // correctly centred top edge, and icon plus label share it.
            let icon_line = context.text_line(geom, &icon_font);
            context.draw_text(
                Point::new(text_offset_x, icon_line.y),
                icon_str,
                &icon_font,
                if is_enabled { ink } else { disabled_ink },
                HorizontalAlignment::Left,
            );
            text_offset_x += 22; // space for icon
        }

        // ── Draw button text, fitted to the space left of the disclosure arrow: an
        // unbounded label ran underneath it. ──
        let text_color = if !is_enabled { disabled_ink } else { ink };
        let line = context.text_line(geom, font);
        let arrow_x = if self.arrow_type == MenuArrowType::None {
            geom.x + geom.width as i32 - PADDING
        } else {
            geom.x + geom.width as i32 - PADDING - 8
        };
        let text_band_width = ((arrow_x - 4) - text_offset_x).max(0) as u32;
        context.draw_text_fitted_cow(
            Rect::new(text_offset_x, line.y, text_band_width, line.height.max(1)),
            &self.text,
            font,
            text_color,
            HorizontalAlignment::Left,
        );

        // ── Draw dropdown arrow ──
        let arrow_y = geom.y + geom.height as i32 / 2 - 2;
        let arrow_color = if !is_enabled { disabled_ink } else { ink };
        if self.arrow_type != MenuArrowType::None {
            // Triangle vertices per direction. `Down` is the historical shape, so the
            // default rendering is unchanged; the other three are opt-in.
            let points = match self.arrow_type {
                MenuArrowType::Down | MenuArrowType::None => vec![
                    Point::new(arrow_x, arrow_y),
                    Point::new(arrow_x + 8, arrow_y),
                    Point::new(arrow_x + 4, arrow_y + 6),
                ],
                MenuArrowType::Up => vec![
                    Point::new(arrow_x, arrow_y + 6),
                    Point::new(arrow_x + 8, arrow_y + 6),
                    Point::new(arrow_x + 4, arrow_y),
                ],
                MenuArrowType::Left => vec![
                    Point::new(arrow_x + 6, arrow_y),
                    Point::new(arrow_x + 6, arrow_y + 6),
                    Point::new(arrow_x, arrow_y + 3),
                ],
                MenuArrowType::Right => vec![
                    Point::new(arrow_x, arrow_y),
                    Point::new(arrow_x, arrow_y + 6),
                    Point::new(arrow_x + 6, arrow_y + 3),
                ],
            };
            context.execute_command(RenderCommand::DrawPath {
                points,
                closed: true,
                color: arrow_color,
                filled: true,
                width: 1,
            });
        }
        // A split button states the boundary between its two targets.
        if self.popup_mode == MenuButtonPopup::MenuButtonPopup && is_enabled {
            let divider_x = self.arrow_rect().x;
            context.draw_line(
                Point::new(divider_x, geom.y + 4),
                Point::new(divider_x, geom.y + geom.height as i32 - 4),
                border,
            );
        }

        // ── Draw dropdown menu ──
        //
        // Painted while the reveal has anything left to show, so a *closing* menu animates too:
        // gating on `menu_open` alone would cut the close off at its first frame.
        if self.menu_open || !self.reveal.is_closed() {
            let menu = self.menu_rect();
            let revealed = self.reveal.revealed(
                menu,
                RevealDirection::Down,
                PopupReveal::travel_cap_for(crate::core::Size::new(menu.width, menu.height)),
            );
            let clipped = !self.reveal.is_closed();
            if clipped {
                context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
            }
            draw_menu_panel(
                context,
                menu,
                &self.menu_items,
                &self.item_icon_sources,
                style,
                accent,
            );
            if clipped {
                context.pop_clip();
            }
        }

        // The child panels hang to the right of the menu, outside the menu's reveal clip, so
        // they are painted after the clip is popped. One panel per open level, deepest last:
        // opening another branch replaces that level and everything below it.
        for (offset, panel) in self.submenus.iter().enumerate() {
            draw_menu_panel_with_branches(
                context,
                panel.rect,
                &panel.items,
                &self.item_icon_sources,
                style,
                accent,
                // `offset + 1` is the level these rows sit on; the deepest panel cannot
                // open another, so it paints no arrow.
                offset + 1 < MAX_SUBMENU_DEPTH,
            );
        }
    }
}

impl EventHandler for MenuButton {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                if self.geometry().contains_point(*pos) {
                    // The face and the disclosure arrow are separate targets in
                    // `MenuButtonPopup` mode; every other mode keeps the historical
                    // whole-button toggle.
                    let on_arrow = self.arrow_rect().contains_point(*pos);
                    match self.popup_mode {
                        MenuButtonPopup::InstantPopup => self.toggle_menu(),
                        MenuButtonPopup::MenuButtonPopup => {
                            if on_arrow || self.arrow_type == MenuArrowType::None {
                                self.toggle_menu();
                            } else {
                                self.clicked.emit();
                            }
                        }
                        MenuButtonPopup::DelayedPopup => {
                            if self.menu_open {
                                self.close_menu();
                            } else if on_arrow {
                                self.open_menu();
                            } else {
                                // Start the hold timer; `tick` opens the menu when it fires.
                                self.press_elapsed = Some(0);
                                self.base.request_redraw();
                            }
                        }
                    }
                } else if self.menu_open {
                    if let Some((level, index)) = self.hit_test_submenu(*pos) {
                        self.activate_row_at(level, index);
                    } else if let Some(index) = self.hit_test_item(*pos) {
                        self.active_index = Some(index);
                        self.activate_top_level(index);
                    } else if !self.submenus.is_empty() {
                        // An outside click collapses one level: the deepest panel first, the
                        // menu itself on the click after the last panel is gone.
                        self.close_submenu();
                    } else {
                        self.close_menu();
                    }
                }
            }
            Event::MouseRelease { .. } => {
                // A release before the delay fired cancels the pending popup: a held
                // press that the user let go is not an activation.
                self.press_elapsed = None;
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                key_codes::ESCAPE if !self.submenus.is_empty() => self.close_submenu(),
                key_codes::ESCAPE if self.menu_open => self.close_menu(),
                key_codes::RIGHT if self.menu_open => {
                    // The deepest open panel owns the focus; the depth gate lives in
                    // `open_submenu_at`, so Right on the deepest level opens nothing.
                    let level = self.submenus.len();
                    if let Some(index) = self.keyboard_branch_index_at(level) {
                        self.open_submenu_at(level, index);
                    }
                }
                key_codes::LEFT if !self.submenus.is_empty() => self.close_submenu(),
                key_codes::SPACE | key_codes::ENTER => self.toggle_menu(),
                _ => { /* Other keys are not relevant */ }
            },
            Event::MouseLeave { .. } => {
                // A pointer that left the button during a delayed press is no longer
                // asking for the popup.
                self.press_elapsed = None;
                self.base.handle_event(event);
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// `menu_button`'s own `icon_source` contract: a built-in descriptor round-trips, a resource
    /// needs a host resolver, and a borrowed static geometry is not persistable.
    #[test]
    fn menu_button_icon_source_property_round_trips_and_refuses_the_unresolvable() {
        use crate::widget::capability::types::CapabilityValue as V;
        use crate::widget::icon_source::{IconColorMode, IconSpec};

        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        assert_eq!(mb.get("icon_source"), Ok(V::Null));

        let icon = IconSpec::Builtin(crate::widget::IconName::Info);
        mb.set("icon_source", V::IconSpec(icon.clone()))
            .expect("built-in descriptors apply without a resolver");
        assert_eq!(mb.get("icon_source"), Ok(V::IconSpec(icon)));
        assert_eq!(mb.icon_source(), Some(IconSource::Builtin(crate::widget::IconName::Info)));

        assert_eq!(
            mb.set(
                "icon_source",
                V::IconSpec(IconSpec::Resource {
                    id: "project/icons/save".into(),
                    color_mode: IconColorMode::Monochrome,
                }),
            ),
            Err(CapabilityAccessError::ResourceResolutionRequired)
        );

        mb.set("icon_source", V::Null).expect("null clears the icon");
        assert_eq!(mb.icon_source(), None, "an explicit clear is Empty, not the old icon");

        // The resolver entry point reaches the same active state, which is what lets a document
        // with a resource descriptor land here once the host has resolved it.
        mb.set_resolved_icon_source(Some(IconSource::Builtin(crate::widget::IconName::Save)))
            .expect("a resolved source applies");
        assert_eq!(mb.icon_source(), Some(IconSource::Builtin(crate::widget::IconName::Save)));

        static DATA: crate::widget::display_widgets::icon::IconData =
            crate::widget::display_widgets::icon::IconData {
                name: "runtime",
                grid: 24,
                paths: &[],
            };
        mb.set_icon_source(Some(IconSource::static_geometry(&DATA)));
        assert_eq!(mb.get("icon_source"), Err(CapabilityAccessError::IconSourceNotPersistable));
    }

    #[test]
    fn menu_button_default_creation() {
        let mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        assert_eq!(mb.text(), "File");
        assert_eq!(mb.item_count(), 0);
        assert!(!mb.is_menu_open());
        assert!(mb.icon().is_none());
    }

    #[test]
    fn menu_button_add_remove_items() {
        let mut mb = MenuButton::new("Edit", Rect::new(0, 0, 100, 30));
        assert_eq!(mb.item_count(), 0);

        mb.add_item(MenuItem::new(1, "Cut"));
        mb.add_item(MenuItem::new(2, "Copy"));
        mb.add_item(MenuItem::new(3, "Paste"));
        assert_eq!(mb.item_count(), 3);

        assert!(mb.remove_item(2));
        assert_eq!(mb.item_count(), 2);

        mb.clear_items();
        assert_eq!(mb.item_count(), 0);
    }

    #[test]
    fn menu_button_toggle_menu() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        assert!(!mb.is_menu_open());

        mb.toggle_menu();
        assert!(mb.is_menu_open());

        mb.toggle_menu();
        assert!(!mb.is_menu_open());
    }

    #[test]
    fn menu_button_item_triggered_signal() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.add_item(MenuItem::new(42, "Open"));
        mb.add_item(MenuItem::new(99, "Save"));

        let triggered_id = Arc::new(Mutex::new(0u64));
        let tid = triggered_id.clone();
        mb.item_triggered.connect(move |val| {
            *tid.lock().unwrap() = *val;
        });

        // Open the menu
        mb.open_menu();
        assert!(mb.is_menu_open());

        // Click on the first menu item
        let item_rect = mb.item_rect(0);
        let click_pos = Point::new(item_rect.x + 5, item_rect.y + 5);
        mb.handle_event(&Event::mouse_press(click_pos.x, click_pos.y, 1));

        assert_eq!(*triggered_id.lock().unwrap(), 42);
        assert!(!mb.is_menu_open());
    }

    #[test]
    fn menu_button_click_outside_closes_menu() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.add_item(MenuItem::new(1, "Item"));
        mb.open_menu();
        assert!(mb.is_menu_open());

        // Click far outside
        mb.handle_event(&Event::mouse_press(500, 500, 1));
        assert!(!mb.is_menu_open());
    }

    #[test]
    fn menu_button_escape_closes_menu() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.open_menu();
        assert!(mb.is_menu_open());

        mb.handle_event(&Event::key_press(27, 0));
        assert!(!mb.is_menu_open());
    }

    /// A menu with one branch row ("Recent") and one leaf row ("Exit").
    fn button_with_branch() -> MenuButton {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.add_item(
            MenuItem::new(1, "Recent")
                .with_submenu(vec![MenuItem::new(2, "Doc"), MenuItem::new(3, "Sheet")]),
        );
        mb.add_item(MenuItem::new(4, "Exit"));
        mb
    }

    /// Activating a branch opens its child panel, leaves the parent open, and emits no trigger.
    #[test]
    fn menu_button_branch_opens_its_child_panel_without_triggering() {
        let mut mb = button_with_branch();
        let triggered = Arc::new(AtomicUsize::new(0));
        let count = triggered.clone();
        mb.item_triggered.connect(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });

        mb.open_menu();
        let row = mb.item_rect(0);
        mb.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));

        assert!(mb.is_menu_open(), "a branch activation must not close the parent menu");
        assert_eq!(triggered.load(Ordering::SeqCst), 0, "a branch is not a leaf activation");
        assert_eq!(mb.submenus[0].items.len(), 2);
        let panel = mb.submenus[0].rect;
        assert!(panel.x >= mb.menu_rect().x + mb.menu_rect().width as i32);
        assert!(mb.contains_point(Point::new(panel.x + 5, panel.y + 5)));
    }

    /// The branch's own label is what `submenu_requested` carries.
    #[test]
    fn menu_button_submenu_requested_carries_the_branch_text() {
        let mut mb = button_with_branch();
        let label = Arc::new(Mutex::new(String::new()));
        let captured = label.clone();
        mb.submenu_requested.connect(move |text| {
            *captured.lock().unwrap() = text.to_string();
        });

        mb.open_menu();
        let row = mb.item_rect(0);
        mb.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));
        assert_eq!(*label.lock().unwrap(), "Recent");
    }

    /// Escape collapses one level: the child panel first, then the menu.
    #[test]
    fn menu_button_escape_collapses_the_child_panel_first() {
        let mut mb = button_with_branch();
        mb.open_menu();
        let row = mb.item_rect(0);
        mb.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));
        assert!(mb.is_submenu_open());

        mb.handle_event(&Event::key_press(27, 0));
        assert!(mb.is_menu_open(), "the first Escape returns to the parent menu");
        assert!(!mb.is_submenu_open());

        mb.handle_event(&Event::key_press(27, 0));
        assert!(!mb.is_menu_open());
    }

    /// Choosing a leaf inside the child panel triggers that leaf and closes the whole popup.
    #[test]
    fn menu_button_child_leaf_selection_emits_item_triggered() {
        let mut mb = button_with_branch();
        let chosen = Arc::new(Mutex::new(0u64));
        let captured = chosen.clone();
        mb.item_triggered.connect(move |id| {
            *captured.lock().unwrap() = *id;
        });

        mb.open_menu();
        let branch = mb.item_rect(0);
        mb.handle_event(&Event::mouse_press(branch.x + 5, branch.y + 5, 1));
        let panel = mb.submenus[0].rect;
        mb.handle_event(&Event::mouse_press(panel.x + 5, panel.y + 5, 1));

        assert_eq!(*chosen.lock().unwrap(), 2);
        assert!(!mb.is_menu_open(), "a leaf selection closes the whole popup");
        assert!(!mb.is_submenu_open());
    }

    /// Right opens the branch, Left returns to the parent menu.
    #[test]
    fn menu_button_right_opens_a_branch_and_left_returns_to_the_menu() {
        let mut mb = button_with_branch();
        mb.open_menu();

        mb.handle_event(&Event::key_press(39, 0));
        assert!(mb.is_submenu_open());
        mb.handle_event(&Event::key_press(37, 0));
        assert!(!mb.is_submenu_open());
        assert!(mb.is_menu_open());
    }

    /// A button whose branch chain is deeper than the control may open.
    ///
    /// Every level carries a branch named after the level below it plus a leaf named after
    /// its own level, and a second top-level branch exercises sibling replacement. The
    /// deepest branch ("Level 5") sits on level [`MAX_SUBMENU_DEPTH`], where opening is
    /// refused.
    fn button_with_deep_chain() -> MenuButton {
        let l5 = MenuItem::new(50, "Level 5").with_submenu(vec![MenuItem::new(51, "Leaf 5")]);
        let l4 = MenuItem::new(40, "Level 4").with_submenu(vec![l5, MenuItem::new(41, "Leaf 4")]);
        let l3 = MenuItem::new(30, "Level 3").with_submenu(vec![l4, MenuItem::new(31, "Leaf 3")]);
        let l2 = MenuItem::new(20, "Level 2").with_submenu(vec![l3, MenuItem::new(21, "Leaf 2")]);
        let l1 = MenuItem::new(10, "Level 1").with_submenu(vec![l2, MenuItem::new(11, "Leaf 1")]);
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.add_item(l1);
        mb.add_item(MenuItem::new(60, "Other").with_submenu(vec![MenuItem::new(61, "Other 1")]));
        mb
    }

    /// Clicks row `index` of `level` (0 is the menu's own list).
    fn click_row(mb: &mut MenuButton, level: usize, index: usize) {
        let row = if level == 0 {
            mb.item_rect(index)
        } else {
            menu_item_rect(mb.submenus[level - 1].rect, index)
        };
        mb.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));
    }

    /// Four Rights open four panels, each to the right of the level that opened it.
    #[test]
    fn menu_button_opens_four_nested_child_panels() {
        let mut mb = button_with_deep_chain();
        mb.open_menu();
        for _ in 0..MAX_SUBMENU_DEPTH {
            mb.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        }

        assert_eq!(mb.submenu_depth(), MAX_SUBMENU_DEPTH);
        assert!(mb.is_menu_open(), "opening panels must never close the menu");
        let mut parent = mb.menu_rect();
        for panel in &mb.submenus {
            assert_eq!(panel.rect.x, parent.x + parent.width as i32);
            assert!(
                mb.contains_point(Point::new(panel.rect.x + 2, panel.rect.y + 2)),
                "every open panel accepts the pointer"
            );
            parent = panel.rect;
        }
        assert_eq!(mb.submenus[3].items[0].id, 50, "the fourth panel holds the deepest branch");
    }

    /// The depth contract: a branch on the deepest level announces itself but opens nothing.
    #[test]
    fn menu_button_refuses_a_fifth_panel_but_announces_the_branch() {
        let mut mb = button_with_deep_chain();
        let requested = Arc::new(Mutex::new(Vec::new()));
        let captured = requested.clone();
        mb.submenu_requested.connect(move |text| {
            captured.lock().unwrap().push(text.to_string());
        });
        let triggered = Arc::new(AtomicUsize::new(0));
        let count = triggered.clone();
        mb.item_triggered.connect(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });
        mb.open_menu();
        for _ in 0..MAX_SUBMENU_DEPTH {
            mb.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        }
        let before = requested.lock().unwrap().len();

        // "Level 5" is a branch, but it sits on the deepest allowed level.
        click_row(&mut mb, MAX_SUBMENU_DEPTH, 0);

        assert_eq!(mb.submenu_depth(), MAX_SUBMENU_DEPTH, "no fifth panel may open");
        assert!(mb.is_menu_open());
        assert_eq!(triggered.load(Ordering::SeqCst), 0, "a branch is not a leaf activation");
        let after = requested.lock().unwrap().clone();
        assert_eq!(after.len(), before + 1);
        assert_eq!(after.last().map(String::as_str), Some("Level 5"));
    }

    /// Escape closes one panel per press, deepest first, then the menu itself.
    #[test]
    fn menu_button_escape_unwinds_one_panel_per_press() {
        let mut mb = button_with_deep_chain();
        mb.open_menu();
        for _ in 0..MAX_SUBMENU_DEPTH {
            mb.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        }
        assert_eq!(mb.submenu_depth(), MAX_SUBMENU_DEPTH);

        for expected in (0..MAX_SUBMENU_DEPTH).rev() {
            mb.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
            assert_eq!(mb.submenu_depth(), expected);
            assert!(mb.is_menu_open(), "Escape leaves the menu open until it is last");
        }
        mb.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert!(!mb.is_menu_open());
    }

    /// A leaf in the deepest panel triggers and closes the whole popup.
    #[test]
    fn menu_button_deep_leaf_selection_emits_item_triggered() {
        let mut mb = button_with_deep_chain();
        let chosen = Arc::new(Mutex::new(0u64));
        let captured = chosen.clone();
        mb.item_triggered.connect(move |id| {
            *captured.lock().unwrap() = *id;
        });
        mb.open_menu();
        for level in 0..MAX_SUBMENU_DEPTH {
            click_row(&mut mb, level, 0);
        }
        assert_eq!(mb.submenu_depth(), MAX_SUBMENU_DEPTH);

        click_row(&mut mb, MAX_SUBMENU_DEPTH, 1);

        assert_eq!(*chosen.lock().unwrap(), 41, "Leaf 4 sits on the deepest level");
        assert!(!mb.is_menu_open(), "a leaf selection closes the whole popup");
        assert_eq!(mb.submenu_depth(), 0);
    }

    /// Choosing a sibling branch replaces the deeper panels instead of stacking them.
    #[test]
    fn menu_button_opening_a_sibling_branch_discards_the_deeper_panels() {
        let mut mb = button_with_deep_chain();
        mb.open_menu();
        click_row(&mut mb, 0, 0);
        click_row(&mut mb, 1, 0);
        assert_eq!(mb.submenu_depth(), 2);

        click_row(&mut mb, 0, 1);

        assert_eq!(mb.submenu_depth(), 1);
        assert_eq!(mb.submenus[0].items[0].id, 61);
    }

    #[test]
    fn menu_button_disabled_blocks_events() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.set_enabled(false);

        let triggered_id = Arc::new(Mutex::new(0u64));
        let tid = triggered_id.clone();
        mb.item_triggered.connect(move |val| {
            *tid.lock().unwrap() = *val;
        });

        mb.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(!mb.is_menu_open());
        assert_eq!(*triggered_id.lock().unwrap(), 0);
    }

    #[test]
    fn menu_button_icon() {
        let mut mb = MenuButton::new("Menu", Rect::new(0, 0, 100, 30));
        assert!(mb.icon().is_none());

        mb.set_icon("🔍");
        assert_eq!(mb.icon(), Some("🔍"));

        let source = IconSource::builtin(IconName::Info);
        mb.set_icon_source(Some(source));
        assert_eq!(mb.icon_source(), Some(source));
        assert!(mb.icon().is_none(), "typed geometry is not exposed as legacy text");
        let svg = crate::widget::svg::render_to_svg(&mut mb);
        assert!(svg.contains("<path"), "the typed icon reaches the SVG backend");

        mb.set_icon("★");
        assert_eq!(mb.icon_source(), None, "legacy text replaces typed geometry");
        assert_eq!(mb.icon(), Some("★"));
        mb.clear_icon();
        assert!(mb.icon().is_none());
        assert_eq!(mb.icon_source(), None);
    }

    #[test]
    fn menu_button_item_sources_use_unique_ids_and_follow_mutation_lifecycle() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "menu-button-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut button = MenuButton::new("Menu", Rect::new(0, 0, 140, 32));
        button.add_item(MenuItem::new(7, "Open").with_icon("legacy"));
        assert_eq!(
            button.set_item_icon_source(8, Some(IconSource::static_geometry(&RING))),
            Err(crate::widget::IconItemError::ItemNotFound { id: 8 })
        );
        button
            .set_item_icon_source(7, Some(IconSource::static_geometry(&RING)))
            .expect("unique existing id accepts a typed source");
        assert_eq!(button.items()[0].icon.as_deref(), Some("legacy"));
        button.open_menu();
        let svg = crate::widget::svg::render_to_svg(&mut button);
        assert!(svg.contains("fill-rule=\"nonzero\""), "item geometry reaches the menu painter");

        button.add_item(MenuItem::new(7, "Duplicate"));
        assert_eq!(
            button.set_item_icon_source(7, Some(IconSource::static_geometry(&RING))),
            Err(crate::widget::IconItemError::AmbiguousItemId { id: 7 })
        );
        assert!(button.item_icon_sources.is_empty(), "new duplicate invalidates stale mapping");

        button.items_mut().clear();
        assert!(button.item_icon_sources.is_empty(), "mutable access clears source associations");
    }

    #[test]
    fn menu_button_svg_output() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 120, 30));
        mb.add_item(MenuItem::new(1, "Open").with_icon("📂"));
        mb.add_item(MenuItem::new(2, "Save").with_checked(true));
        mb.open_menu();

        let svg = render_to_svg(&mut mb);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    /// In split mode the face reports a click and only the arrow opens the menu.
    #[test]
    fn split_mode_separates_the_face_from_the_arrow() {
        let mut mb = MenuButton::new("Run", Rect::new(0, 0, 120, 30));
        mb.add_item(MenuItem::new(1, "Open"));
        mb.set_popup_mode(MenuButtonPopup::MenuButtonPopup);

        let clicks = Arc::new(AtomicUsize::new(0));
        let sink = clicks.clone();
        mb.clicked.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        mb.handle_event(&Event::mouse_press(10, 10, 1));
        assert_eq!(clicks.load(Ordering::SeqCst), 1);
        assert!(!mb.is_menu_open(), "the face must not open the menu in split mode");

        let arrow = mb.arrow_rect();
        mb.handle_event(&Event::mouse_press(arrow.x + 4, arrow.y + 4, 1));
        assert!(mb.is_menu_open(), "the arrow segment opens the menu");
        assert_eq!(clicks.load(Ordering::SeqCst), 1, "the arrow is not the face");
    }

    /// A delayed press opens the menu only after the hold time elapses.
    #[test]
    fn a_delayed_press_opens_after_the_hold_time() {
        let mut mb = MenuButton::new("Run", Rect::new(0, 0, 120, 30));
        mb.add_item(MenuItem::new(1, "Open"));
        mb.set_popup_mode(MenuButtonPopup::DelayedPopup);
        mb.set_popup_delay_ms(100);

        mb.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(mb.is_press_pending());
        assert!(!mb.is_menu_open());
        assert!(mb.tick(60));
        assert!(!mb.is_menu_open(), "60 ms is before the 100 ms delay");
        assert!(mb.tick(60));
        assert!(mb.is_menu_open(), "the second tick crosses the delay");
        assert!(!mb.is_press_pending());
    }

    /// Releasing before the delay cancels the pending popup.
    #[test]
    fn releasing_a_delayed_press_cancels_it() {
        let mut mb = MenuButton::new("Run", Rect::new(0, 0, 120, 30));
        mb.set_popup_mode(MenuButtonPopup::DelayedPopup);
        mb.set_popup_delay_ms(500);
        mb.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(mb.is_press_pending());
        mb.handle_event(&Event::mouse_release(10, 10, 1));
        assert!(!mb.is_press_pending());
        mb.tick(1000);
        assert!(!mb.is_menu_open());
    }

    /// The arrow direction is configurable and `None` hands its width back to the label.
    #[test]
    fn arrow_type_is_configurable() {
        let mut mb = MenuButton::new("Run", Rect::new(0, 0, 120, 30));
        assert_eq!(mb.arrow_type(), MenuArrowType::Down);
        let full = mb.arrow_rect();
        assert_eq!(full.width, 20);
        mb.set_arrow_type(MenuArrowType::None);
        assert_eq!(mb.arrow_rect().width, 120, "no arrow means the whole button is the target");
        let without = render_to_svg(&mut mb);
        mb.set_arrow_type(MenuArrowType::Right);
        let with = render_to_svg(&mut mb);
        assert_ne!(without, with, "the arrow direction must reach the painter");
    }

    /// open/close announce their transitions exactly once.
    #[test]
    fn menu_button_announces_show_and_hide() {
        let mut mb = MenuButton::new("Run", Rect::new(0, 0, 120, 30));
        let shown = Arc::new(AtomicUsize::new(0));
        let hidden = Arc::new(AtomicUsize::new(0));
        let s = shown.clone();
        let h = hidden.clone();
        mb.about_to_show.connect(move || {
            s.fetch_add(1, Ordering::SeqCst);
        });
        mb.about_to_hide.connect(move || {
            h.fetch_add(1, Ordering::SeqCst);
        });
        mb.open_menu();
        mb.open_menu();
        assert_eq!(shown.load(Ordering::SeqCst), 1);
        mb.close_menu();
        mb.close_menu();
        assert_eq!(hidden.load(Ordering::SeqCst), 1);
    }
}
