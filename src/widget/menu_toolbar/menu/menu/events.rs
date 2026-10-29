// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The menu's `Widget`, `WidgetProperties` and `EventHandler` implementations.
//!
//! These are the runtime-facing seams: lifecycle, the published property/command
//! surface, and the pointer/keyboard state machine.

use crate::compat::Vec;
use crate::core::Point;
use crate::event::{Event, EventHandler};
use crate::widget::capability::coercion::{
    expect_bool, expect_string, expect_text_direction, expect_u32, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::effects;
use crate::widget::{BaseWidget, Widget};
use crate::{impl_widget_property_hooks, property_names_of};

use super::model::{
    menu_entry_from_record, menu_entry_record, mnemonic_letter, Menu, MenuEntry, MENU_ITEM_SCHEMA,
};

impl Widget for Menu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        // The cached heading line height is a function of the resolved font; a theme switch
        // can change the family/weight at the same size, so the cache is dropped with it.
        self.heading_line_cache.set(None);
        self.popup_metrics_cache.set(None);
        self.trailing_width_cache.set(None);
        *self.mnemonic_cache.borrow_mut() = None;
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn has_open_popup(&self) -> bool {
        self.is_visible()
    }

    /// The popup and every open nested panel accept the pointer.
    ///
    /// The panels hang to the right of the popup, outside its geometry; the default
    /// implementation tests only the widget rectangle, so the runtime's hit test rejected
    /// every press on a child row before the control could see it.
    fn contains_point(&self, point: Point) -> bool {
        Self::contains_point(self, point)
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }

    fn show(&mut self) {
        // Every open path starts at the top of the list. A menu bar reopens with `show()`
        // rather than `open_at`, so this is the one place both paths share; resetting on
        // close instead would jump the rows while the closing animation is still painting.
        self.scroll_offset = 0;
        self.clamp_scroll();
        self.about_to_show.emit();
        self.reveal.set_open(true);
        self.base.show();
    }
    fn hide(&mut self) {
        // The reveal is aimed at closed **before** the flag flips, so the closing transition is
        // still animated: `draw` paints the body while the reveal has anything left to show, and
        // only a fully collapsed reveal stops it.
        self.reveal.set_open(false);
        self.tooltip_source = None;
        self.tooltips.note_leave();
        self.base.hide();
        // Clear the invoker position with the popup: it describes an open menu, and
        // leaving it set would make a reopened menu report a stale origin.
        self.invoker_position = None;
        self.viewport = None;
        self.hovered_index = None;
        self.clear_hover_branch();
        self.submenu_path.clear();
        self.about_to_hide.emit();
    }

    /// Advances the popup's open/close reveal; `true` while it still owes frames.
    ///
    /// Forwarded to the trait so the animation bus drives it — `tick_animations` sweeps every
    /// mounted control, so a menu that kept its tick inherent would be one the host cannot
    /// advance.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.tooltip_now_ms = self.tooltip_now_ms.saturating_add(u64::from(delta_ms));
        let tooltip_changed = self.tooltips.tick(u64::from(delta_ms), self.tooltip_now_ms);
        // A branch the pointer is resting on opens once the shared delay elapses. The pending
        // pair is what makes this a delay rather than an immediate open on every pass-over.
        let mut hover_changed = false;
        if let Some((level, index)) = self.hover_open_pending {
            self.hover_open_elapsed_ms = self.hover_open_elapsed_ms.saturating_add(delta_ms);
            if self.hover_open_elapsed_ms >= effects::SUBMENU_HOVER_DELAY_MS {
                self.hover_open_pending = None;
                self.hover_open_elapsed_ms = 0;
                hover_changed = self.open_submenu_at(level, index);
            } else {
                hover_changed = true;
            }
        }
        self.reveal.tick(delta_ms) || tooltip_changed || self.tooltip_owes_frames() || hover_changed
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating()
            || self.tooltip_owes_frames()
            || self.hover_open_pending.is_some()
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
            "triggered" => Some(EventSignalRef::mapped("triggered", &self.triggered, |v| {
                CapabilityValue::String(v.clone())
            })),
            "triggered_index" => {
                Some(EventSignalRef::mapped("triggered_index", &self.triggered_index, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "about_to_show" => Some(EventSignalRef::unit("about_to_show", &self.about_to_show)),
            "submenu_requested" => {
                Some(EventSignalRef::mapped("submenu_requested", &self.submenu_requested, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "about_to_hide" => Some(EventSignalRef::unit("about_to_hide", &self.about_to_hide)),
            "tearoff_requested" => {
                Some(EventSignalRef::unit("tearoff_requested", &self.tearoff_requested))
            }
            "action_triggered" => {
                Some(EventSignalRef::mapped("action_triggered", &self.action_triggered, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `Menu`'s property contract.
///
/// Only `title` is writable; `item_count` and `hovered_index` describe the menu's
/// current contents and pointer state, so they are read-only. `hovered_index` is
/// optional and published as `Null` when nothing is hovered.
impl WidgetProperties for Menu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The whole entry list as records (BLUE26 B26.09): `add_action`/`add_separator` take
            // arguments the bare command channel cannot carry, so the list is the property and
            // `set_items` is its published command form.
            "items" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &MENU_ITEM_SCHEMA,
                    self.items
                        .iter()
                        .map(menu_entry_record)
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "hovered_index" => match self.hovered_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            "show_heading" => Ok(CapabilityValue::Bool(self.shows_heading())),
            "column_count" => Ok(CapabilityValue::UInt(self.columns() as u64)),
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
            "show_heading" => {
                self.set_show_heading(expect_bool(value)?);
                Ok(())
            }
            "column_count" => {
                let columns = expect_u32(value)?.clamp(1, u8::MAX as u32) as u8;
                self.set_column_count(columns);
                Ok(())
            }
            "hovered_index" => Err(CapabilityAccessError::ReadOnlyProperty),
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // A whole-menu replacement. Checked in full before anything is replaced, then rebuilt
            // through the control's own `clear` + `add_item`, so the row-touch bookkeeping that
            // every mutation needs is kept in one place.
            "items" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&MENU_ITEM_SCHEMA).map_err(|violation| {
                    log::warn!("menu.items refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("items"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let mut entries = Vec::with_capacity(list.len());
                for record in list.items() {
                    entries.push(menu_entry_from_record(record)?);
                }
                self.clear();
                for entry in entries {
                    self.add_item(entry);
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "title",
            "item_count",
            "hovered_index",
            "direction",
            "show_heading",
            "column_count",
            BASE_PROPERTY_NAMES
        ]
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
            "add_action" => Err(CapabilityAccessError::OutOfRange),
            "set_items" => Err(CapabilityAccessError::OutOfRange),
            // `add_separator` takes no argument, so the command appends one through the
            // control's own builder instead of refusing a call it can honor.
            "add_separator" => {
                self.add_separator();
                Ok(())
            }
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
                self.tooltips.note_pointer(*pos);
                if let Some((level, index)) = self.hit_test_submenu(*pos) {
                    if self
                        .level_entries(level)
                        .and_then(|entries| entries.get(index))
                        .is_some_and(MenuEntry::is_selectable)
                    {
                        self.set_level_hovered(level, Some(index));
                        let is_branch = self
                            .level_entries(level)
                            .and_then(|entries| entries.get(index))
                            .is_some_and(MenuEntry::has_submenu);
                        if is_branch {
                            self.note_hover_branch(level, index);
                        } else {
                            self.clear_hover_branch();
                        }
                        self.base.request_redraw();
                    }
                } else if let Some(index) = self.item_index_at(*pos) {
                    if self.items[index].is_selectable() {
                        self.hovered_index = Some(index);
                        // Hovering a branch opens its panel after a short rest (M-01); a
                        // leaf cancels any pending branch the pointer just left.
                        if self.items[index].has_submenu() {
                            self.note_hover_branch(0, index);
                        } else {
                            self.clear_hover_branch();
                        }
                    }
                } else {
                    self.clear_hover_branch();
                }
                self.sync_hover_tooltip();
            }
            Event::MouseLeave { .. } => {
                self.hovered_index = None;
                self.tooltip_source = None;
                self.clear_hover_branch();
                self.tooltips.note_leave();
            }
            Event::MousePress { pos, button: 1, .. } => {
                // The nested panels are outside the parent's own rectangle, so they are
                // hit tested first: otherwise a click on a child row would read as
                // "outside the popup" and dismiss the whole menu.
                if let Some((level, index)) = self.hit_test_submenu(*pos) {
                    if self
                        .level_entries(level)
                        .and_then(|entries| entries.get(index))
                        .is_some_and(MenuEntry::is_selectable)
                    {
                        self.set_level_hovered(level, Some(index));
                        self.activate_level_hovered(level);
                    }
                    return;
                }
                let tearoff_band = self.tearoff_band_height();
                if tearoff_band > 0 {
                    let top = self.body_top();
                    if pos.y >= top && pos.y < top + tearoff_band as i32 {
                        self.tearoff_requested.emit();
                        return;
                    }
                }
                if let Some(index) = self.item_index_at(*pos) {
                    let item = &self.items[index];
                    if item.is_selectable() {
                        self.hovered_index = Some(index);
                        // A submenu parent is a branch, not a leaf: report it as one and leave the
                        // popup open rather than firing a command the user did not choose (G-3).
                        // A click is the *first* way a user meets this defect -- they click `Cut >`
                        // and the `Cut` action runs -- so this arm matters more than the key one.
                        self.activate_hovered();
                    }
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
                if let Some((level, index)) = self.hit_test_submenu(*pos) {
                    if self
                        .level_entries(level)
                        .and_then(|entries| entries.get(index))
                        .is_some_and(MenuEntry::is_selectable)
                    {
                        self.set_level_hovered(level, Some(index));
                        self.activate_level_hovered(level);
                    }
                } else if let Some(index) = self.item_index_at(*pos) {
                    if self.items[index].is_selectable() {
                        self.hovered_index = Some(index);
                        self.activate_hovered();
                    }
                }
                // Touch has no outside-press event: a tap outside the popup is the
                // dismissal gesture, and it is not routed to this widget at all once
                // the menu is no longer topmost, so nothing further is needed here.
            }
            Event::Wheel { delta, .. } => {
                let capacity = Self::visible_capacity();
                if capacity < self.items.len() {
                    let max_offset = self.items.len() - capacity;
                    if delta.y > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    } else if delta.y < 0 {
                        self.scroll_offset = (self.scroll_offset + 1).min(max_offset);
                    }
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                // Key codes are the framework convention (see `Key::from_key_code`): 27 Escape,
                // 13/10 Enter, 38 Up, 40 Down, 37 Left, 39 Right, 36 Home, 35 End.
                // Alt+letter is the mnemonic path; bit 2 is Alt in the event convention.
                if modifiers & 0b0100 != 0 {
                    if let Some(letter) = mnemonic_letter(*key) {
                        self.activate_mnemonic(letter);
                        return;
                    }
                }
                // Type-ahead: a bare letter or digit jumps to the next row starting with
                // it. Chorded presses belong to the application's shortcuts, not to the
                // menu, which is the same rule `DropdownMenu` follows (M-005).
                if *modifiers == 0 {
                    if let Some(letter) =
                        char::from_u32(*key).filter(|letter| letter.is_ascii_alphanumeric())
                    {
                        self.type_ahead_at(self.focus_level(), letter);
                        return;
                    }
                }
                match *key {
                    27 => {
                        // Escape unwinds one level: an open nested panel closes before the
                        // menu itself does, which is the only order that makes Escape feel
                        // like "back" instead of "dismiss".
                        if self.close_submenu() {
                            return;
                        }
                        self.hide();
                    }
                    10 | 13 => {
                        // Enter chooses the highlighted entry. An empty highlight (or one that
                        // cannot be chosen) leaves the menu open, which is what makes a stray Enter
                        // harmless rather than a dismissal with no selection.
                        self.activate_level_hovered(self.focus_level());
                    }
                    38 => {
                        let level = self.focus_level();
                        // A grid's vertical step is one row, i.e. `columns` entries; a
                        // single-column menu keeps the historical ±1.
                        let stride = if level == 0 { self.column_count as i32 } else { 1 };
                        self.move_level_hovered(level, -stride);
                    }
                    40 => {
                        let level = self.focus_level();
                        let stride = if level == 0 { self.column_count as i32 } else { 1 };
                        self.move_level_hovered(level, stride);
                    }
                    // The horizontal pair is the only part of the navigation that follows the
                    // writing direction; see the `direction` field.
                    39 => {
                        // Right opens a branch. It only falls back to stepping the current
                        // level when the highlighted row is not a branch, so a menu of leaves
                        // keeps its original behaviour.
                        let level = self.focus_level();
                        let is_branch = self.level_hovered(level).is_some_and(|index| {
                            self.level_entries(level)
                                .and_then(|entries| entries.get(index))
                                .is_some_and(MenuEntry::has_submenu)
                        });
                        if is_branch {
                            self.activate_level_hovered(level);
                        } else if level == 0 {
                            // One entry to the right is one cell in a grid and the next row
                            // entry in a single-column list.
                            self.move_hovered_horizontally(true);
                        } else {
                            self.move_level_hovered(level, 1);
                        }
                    }
                    37 => {
                        if !self.close_submenu() {
                            self.move_hovered_horizontally(false);
                        }
                    }
                    36 => {
                        // Home/End jump to the ends of the *current* level, which is a
                        // vertical walk, so they are not mirrored either.
                        let level = self.focus_level();
                        let count = self.level_entries(level).map_or(0, |entries| entries.len());
                        self.move_level_hovered(level, -(count as i32));
                    }
                    35 => {
                        let level = self.focus_level();
                        let count = self.level_entries(level).map_or(0, |entries| entries.len());
                        self.move_level_hovered(level, count as i32);
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
