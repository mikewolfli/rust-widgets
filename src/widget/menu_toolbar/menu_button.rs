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
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
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

/// MenuButton widget — a button that opens a dropdown menu on click.
///
/// Displays a button with text and an optional icon. When clicked, it toggles
/// a dropdown menu positioned below the button. Selecting a menu item emits
/// the `item_triggered` signal with the item's ID.
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
    /// Emitted when a menu item is clicked, providing the item ID.
    pub item_triggered: Signal1<u64>,
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
            item_triggered: Signal1::new(),
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
            self.base.request_redraw();
        }
        removed
    }

    /// Removes all menu items.
    pub fn clear_items(&mut self) {
        self.menu_items.clear();
        self.item_icon_sources.clear();
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
        &mut self.menu_items
    }

    /// Returns whether the dropdown menu is currently open.
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// Toggles the dropdown menu visibility.
    pub fn toggle_menu(&mut self) {
        self.menu_open = !self.menu_open;
        self.reveal.set_open(self.menu_open);
        self.base.request_redraw();
    }

    /// Opens the dropdown menu.
    pub fn open_menu(&mut self) {
        self.menu_open = true;
        self.reveal.set_open(true);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Closes the dropdown menu.
    pub fn close_menu(&mut self) {
        self.menu_open = false;
        self.reveal.set_open(false);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.id());
        self.base.request_redraw();
    }

    /// Computes the menu rectangle positioned below the button.
    fn menu_rect(&self) -> Rect {
        let geom = self.geometry();
        let menu_width = geom.width.max(DEFAULT_MENU_WIDTH);
        let menu_height = (self.menu_items.len() as u32 * ITEM_HEIGHT as u32).max(1);
        Rect::new(geom.x, geom.y + geom.height as i32, menu_width, menu_height)
    }

    /// Computes the rectangle for a specific menu item at the given index.
    fn item_rect(&self, index: usize) -> Rect {
        let menu_rect = self.menu_rect();
        Rect::new(
            menu_rect.x,
            menu_rect.y + index as i32 * ITEM_HEIGHT,
            menu_rect.width,
            ITEM_HEIGHT as u32,
        )
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

    /// The button **and** its open menu accept the pointer.
    ///
    /// The menu is painted below the button's rectangle; without this override every click on a
    /// menu item was rejected before `hit_test_item` could see it.
    fn contains_point(&self, point: crate::core::Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
            || (self.menu_open && self.menu_rect().contains_point(point))
    }

    fn has_open_popup(&self) -> bool {
        self.menu_open
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(120, 28)
    }

    /// Advances the menu's open/close reveal; `true` while it still owes frames.
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
        let arrow_x = geom.x + geom.width as i32 - PADDING - 8;
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
        // Draw a small triangle pointing down (filled path)
        context.execute_command(RenderCommand::DrawPath {
            points: vec![
                Point::new(arrow_x, arrow_y),
                Point::new(arrow_x + 8, arrow_y),
                Point::new(arrow_x + 4, arrow_y + 6),
            ],
            closed: true,
            color: arrow_color,
            filled: true,
            width: 1,
        });

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
            // The popup panel is chrome: a popup that stayed white in a dark theme would be
            // the brightest object on screen. It reads the resolved surface, and the literal
            // it used to be stays as the fallback.
            let panel = crate::style::resolved_theme_style_shared("menu_button")
                .and_then(|r| r.background_color)
                .or(themed_bg)
                .unwrap_or(Color::WHITE);
            let panel_border =
                style.border_color.or(themed_border).unwrap_or(Color::rgba(190, 190, 200, 200));
            let item_ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(33, 33, 33));
            // A selected row's highlight is chrome, not data, so it is resolved from the
            // theme's accent rather than from a literal tint of blue.
            let item_highlight = panel.blend(&accent, 0.3);
            let item_rule = panel.blend(&item_ink, 0.08);
            // Menu background
            context.fill_rounded_rect(menu, 4, panel);
            context.draw_rounded_rect_stroke(menu, 4, panel_border, 1);

            // One resolved font for every row: it used to be rebuilt per item.
            let item_font = effective_font(style);
            for (i, item) in self.menu_items.iter().enumerate() {
                let item_rect = self.item_rect(i);

                // Highlight on hover/checked
                if item.checked {
                    context.fill_rounded_rect(item_rect, 2, item_highlight);
                }

                // Item text
                let item_text_color =
                    if !item.enabled { item_ink.blend(&panel, 0.5) } else { item_ink };
                let mut item_x = item_rect.x + PADDING;

                // Item icon
                let item_source =
                    self.item_icon_sources.iter().find(|(id, _)| *id == item.id).map(|(_, s)| *s);
                if let Some(source) = item_source {
                    let icon_line = context.text_line(item_rect, item_font);
                    let side = icon_line.height.clamp(1, 16);
                    let icon_box = Rect::new(item_x, icon_line.y, side, side.min(item_rect.height));
                    if let Err(error) =
                        crate::widget::paint_icon(context, source.into(), icon_box, item_text_color)
                    {
                        log::warn!(
                            "menu item {id} icon could not be painted: {error:?}",
                            id = item.id
                        );
                    }
                    item_x += 20;
                } else if let Some(ref item_icon) = item.icon {
                    let icon_font = Font::simple("sans-serif", 13.0);
                    // The icon and the label occupy the same row, so both take their origin
                    // from the line box centred in it: the row midpoint is the glyph top
                    // edge, and using it drew them half a line low.
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
                // Fitted between the label start and the checkmark zone: an unbounded
                // label ran under the check glyph on checked rows (and past the popup
                // edge on long ones).
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

                // Checkmark for checked items. Drawn as an `IconName` outline rather than the
                // `✓` (U+2713) character: no bundled face covers the dingbats block, so the
                // character fell back to an 8x8 bitmap and read as a blob.
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
                if item.submenu.is_some() && item.enabled {
                    let sub_x = item_rect.x + item_rect.width as i32 - PADDING - 8;
                    let sub_y = item_rect.y + item_rect.height as i32 / 2 - 4;
                    context.execute_command(RenderCommand::DrawPath {
                        points: vec![
                            Point::new(sub_x, sub_y),
                            Point::new(sub_x, sub_y + 8),
                            Point::new(sub_x + 5, sub_y + 4),
                        ],
                        closed: true,
                        color: item_ink.blend(&panel, 0.45),
                        filled: true,
                        width: 1,
                    });
                }

                // Separator between items (except last)
                if i + 1 < self.menu_items.len() {
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
            if clipped {
                context.pop_clip();
            }
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
                if *button == 1 {
                    if self.geometry().contains_point(*pos) {
                        // Click on the button itself — toggle menu
                        self.toggle_menu();
                    } else if self.menu_open {
                        // Check if click is on a menu item
                        if let Some(idx) = self.hit_test_item(*pos) {
                            let item_id = self.menu_items[idx].id;
                            self.item_triggered.emit(item_id);
                            self.close_menu();
                        } else {
                            // Click outside menu — close it
                            self.close_menu();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == key_codes::ESCAPE && self.menu_open {
                    // Escape — close menu
                    self.close_menu();
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
}
