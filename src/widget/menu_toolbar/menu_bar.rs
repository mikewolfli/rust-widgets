// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu bar widget.
use crate::compat::{lock, Arc, Mutex, String, Vec};
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::TooltipManager;
use crate::widget::menu_toolbar::action::{Action, MenuRole};
use crate::widget::menu_toolbar::menu::Menu;
use crate::widget::menu_toolbar::tool_bar::child_hover_id;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Horizontal gap on each side of a menu-bar title.
///
/// Ten pixels is the macOS menu-bar spacing at body size: enough that adjacent titles read as
/// separate targets, and included in `entry_width` so the hit box and the painted label agree.
const MENU_ENTRY_TEXT_PADDING: u32 = 10;

/// Where a role's actions belong in the platform's menu bar.
///
/// The variants are the *positions* a platform menu bar defines, not a copy of
/// [`MenuRole`]: several roles share one position (About, Preferences and Quit all live in
/// the application menu), and a host merges by position. The order of the variants is the
/// conventional bar order on the platforms that expose these menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeMenuPosition {
    /// The application menu: About, Preferences, Quit, Hide, Hide Others, Show All.
    Application,
    /// The Window menu.
    Window,
    /// The Help menu.
    Help,
    /// The Services submenu inside the application menu.
    Services,
    /// An ordinary top-level entry, in the bar's own order.
    Bar,
}

impl MenuRole {
    /// The platform menu-bar position this role belongs to.
    ///
    /// This is the classification a host uses instead of appending every action as a
    /// duplicate ordinary entry: `Quit` belongs in the application menu, `Window` in the
    /// Window menu, and everything without a role stays in the bar's own order.
    pub const fn native_position(self) -> NativeMenuPosition {
        match self {
            Self::About
            | Self::Preferences
            | Self::Quit
            | Self::Hide
            | Self::HideOthers
            | Self::ShowAll => NativeMenuPosition::Application,
            Self::Services => NativeMenuPosition::Services,
            Self::Window => NativeMenuPosition::Window,
            Self::Help => NativeMenuPosition::Help,
            Self::None => NativeMenuPosition::Bar,
        }
    }
}

/// A host that installs this bar's actions into a platform menu bar.
///
/// The bar owns the actions and their roles, so the bridge only has to translate. It is
/// deliberately one method: a platform menu is rebuilt as a whole when the action set or a
/// role changes, and a granular insert/remove protocol would let the platform copy drift from
/// the bar. The bar calls [`MenuBar::sync_platform_bridge`] after every action it gains.
pub trait NativeMenuBridge {
    /// Rebuilds (or installs) the platform menu bar from `bar`.
    ///
    /// Actions are grouped by [`MenuRole::native_position`]; the bar's own
    /// [`MenuBar::entries`] remain the top-level titles of a drawn bar.
    fn rebuild(&mut self, bar: &MenuBar);
}
/// A top-level menu entry in the menu bar.
///
/// Entries carry only a title and an enabled flag; the menu contents they open
/// live in a separate menu model, not here.
#[derive(Debug, Clone)]
pub struct MenuBarEntry {
    title: String,
    enabled: bool,
    /// Hover hint shown by the tooltip pipeline, or `""` for none.
    tooltip: String,
}

/// A hover source adapter over a menu-bar entry.
///
/// The pipeline reads `WidgetProperties::tooltip`; an entry is presentation data rather than a
/// widget, so this exposes exactly the one property the pipeline consumes. An entry with no hint
/// of its own falls back to the bar's own `tooltip` property.
struct EntryTooltipSource<'a> {
    entry: &'a MenuBarEntry,
    bar_tooltip: &'a str,
}

impl WidgetProperties for EntryTooltipSource<'_> {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tooltip" => {
                let text = if self.entry.tooltip.is_empty() {
                    self.bar_tooltip
                } else {
                    self.entry.tooltip.as_str()
                };
                Ok(CapabilityValue::String(text.to_string()))
            }
            _ => Err(CapabilityAccessError::UnknownProperty),
        }
    }
    fn set(&mut self, _name: &str, _value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        Err(CapabilityAccessError::UnknownProperty)
    }
    fn property_names(&self) -> &'static [&'static str] {
        &["tooltip"]
    }
}

impl MenuBarEntry {
    /// Creates an enabled entry with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), enabled: true, tooltip: String::new() }
    }

    /// The entry's hover hint, or `""` when none was set.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Replaces the entry's hover hint.
    pub fn set_tooltip(&mut self, tooltip: impl Into<String>) {
        self.tooltip = tooltip.into();
    }

    /// Sets the hover hint and returns `self`, for chaining onto [`MenuBarEntry::new`].
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.set_tooltip(tooltip);
        self
    }

    // --- Accessors ---

    /// Returns the label drawn in the bar.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the label. The bar's layout is derived from title width, so
    /// changing this shifts the entries that follow it.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Returns whether the entry accepts mouse input.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the entry.
    ///
    /// A disabled entry is still drawn, in a greyed colour, but click and hover
    /// do not activate it — in particular no `triggered` or `entry_hovered`
    /// signal is emitted for it.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
/// Menu bar widget.
///
/// A horizontal strip of top-level entries, exactly one of which may be active
/// (i.e. its drop-down is considered open). The bar manages only the entries,
/// the pointer cursor, and the signals; it does not own or display drop-down
/// menus, so a consumer is expected to open the matching menu in response to
/// [`MenuBar::triggered`].
///
/// Entry hit areas are estimated from title length rather than measured against
/// real glyph advances, so very wide or narrow fonts can make the clickable
/// regions disagree with the drawn text.
///
pub struct MenuBar {
    base: BaseWidget,
    entries: Vec<MenuBarEntry>,
    active_index: Option<usize>,
    /// The entry under the pointer, or `None` when the bar is not hovered at all.
    ///
    /// # Why this is still a field, and what changed (BLUE23 §93, M1)
    ///
    /// M1's shape is "delete the private state and read `BaseWidget`" — but `BaseWidget` knows
    /// *whether* the pointer is over the control, not **which entry** it is over, and a menu bar is
    /// a row of targets. What was wrong was not the field's existence but its having a second,
    /// independent source: every `MouseMove` wrote it directly, so the bar's idea of "hovered"
    /// could disagree with `BaseWidget`'s — and the theme's `"menu_bar:hover"` key, which the base
    /// flag drives, could be true while no entry was highlighted.
    ///
    /// It is now **derived** from the base flag plus the same `hit_entry` the press path uses:
    /// one pointer fact, one hit test. A move that arrives while the base says "not hovered"
    /// (or a `MouseLeave`) clears it, so the two cannot diverge.
    hovered_index: Option<usize>,
    /// Emitted with the clicked entry's title when an enabled entry is pressed
    /// with the primary mouse button. The payload is the title text, not the
    /// index — duplicate titles are therefore indistinguishable, and renaming
    /// an entry after the fact makes previously received payloads stale.
    pub triggered: Signal1<String>,
    /// Emitted with the title of the entry the pointer newly entered. Not
    /// emitted when the pointer moves to a different disabled entry, and not
    /// re-emitted while the pointer stays on the same entry.
    pub entry_hovered: Signal1<String>,
    /// Emitted with the clicked entry's title **and** the rectangle it occupies.
    ///
    /// # Why the rectangle is part of the event
    ///
    /// A host that opens the matching drop-down needs to place it under the title that was
    /// clicked. Asking the bar for `entry_rect` from inside a `triggered` handler cannot work:
    /// the runtime lends the bar out exclusively for the duration of `handle_event`, so a nested
    /// read answers `None` and the popup would silently open nowhere. Carrying the rectangle in
    /// the event is the same rule as everywhere else — the emitter states the fact while it still
    /// has it, rather than making the receiver guess it afterwards.
    pub triggered_entry: Signal1<(String, Rect)>,
    /// A widget pinned to the bar's trailing edge, right of the last entry.
    ///
    /// The bar owns the box and paints the widget inside it; the widget owns its own
    /// behaviour. A corner slot is what hosts use for an application search field or a
    /// notification indicator that belongs to the chrome rather than to a menu.
    corner: Option<Box<dyn Widget>>,
    /// Drop-down menus attached to entries, keyed by entry index.
    ///
    /// An attached menu is what turns the bar's `triggered_entry` notification into a
    /// working menu bar: the bar places the popup itself, forwards events to it, and
    /// closes it when the entry is chosen again. A bar with no attachments behaves
    /// exactly as before.
    menus: Vec<(usize, Menu)>,
    /// The entry whose attached menu is currently open.
    open_menu_index: Option<usize>,
    /// Whether a host has taken this bar over with the platform's native menu bar.
    ///
    /// Metadata for hosts and designers: the widget still paints and still answers
    /// events, but a host that bridged the entries to the system menu can leave the
    /// control hidden instead of drawing a duplicate.
    native_menu_bar: bool,
    /// Actions this bar exposes to a platform menu bridge.
    ///
    /// A drawn bar owns only titles, which is why the native path needs its own action list:
    /// without it `native_role_items` would have nothing to classify and `set_native(true)`
    /// could only be metadata.
    actions: Vec<Action>,
    /// The host bridge installed by [`MenuBar::set_platform_bridge`], if any.
    platform_bridge: Option<Box<dyn NativeMenuBridge>>,
    /// Live state cells for actions resolved from [`crate::action::ActionManager`].
    ///
    /// The action's `command_id` is the registry id, so one row answers both "which command" and
    /// "which state cell". Keeping the cell here is what makes a registry change visible without
    /// the bar polling the manager.
    live_actions: Vec<(String, Arc<Mutex<crate::action::ActionLiveState>>)>,
    /// The host-side tooltip pipeline for entry hints.
    tooltips: TooltipManager,
    /// The derived id currently registered as hovered, if any.
    tooltip_source: Option<ObjectId>,
    /// Monotonic clock fed to the tooltip pipeline from [`Widget::tick`].
    tooltip_now_ms: u64,
}
impl MenuBar {
    /// Creates an empty bar with no active or hovered entry.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 400x28.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MenuBar, geometry, "MenuBar"),
            entries: Vec::new(),
            active_index: None,
            hovered_index: None,
            triggered: Signal1::new(),
            entry_hovered: Signal1::new(),
            triggered_entry: Signal1::new(),
            corner: None,
            menus: Vec::new(),
            open_menu_index: None,
            native_menu_bar: false,
            actions: Vec::new(),
            platform_bridge: None,
            live_actions: Vec::new(),
            tooltips: TooltipManager::new(geometry),
            tooltip_source: None,
            tooltip_now_ms: 0,
        }
    }

    /// Compatibility spelling of [`Self::entry_hovered`] (W-21): the same emitted signal.
    pub fn hovered_entry(&self) -> &Signal1<String> {
        &self.entry_hovered
    }
    /// Returns the entries in visual order.
    pub fn entries(&self) -> &[MenuBarEntry] {
        &self.entries
    }
    /// Returns the index of the active entry, or `None` when no entry is
    /// active.
    ///
    /// "Active" means the entry was last clicked. It is cleared by pressing
    /// Escape and by [`MenuBar::clear`], but **not** by clicking a different
    /// entry (that entry simply becomes the new active one) nor by
    /// [`MenuBar::remove_menu`].
    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }
    /// Returns the index currently under the pointer, or `None` when the
    /// pointer is outside every entry.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// Registers the hovered entry's hint with the tooltip pipeline.
    ///
    /// An entry without its own hint falls back to the bar's `tooltip` property, so both per-entry
    /// and bar-wide hints share the one text source the issue names.
    fn sync_hover_tooltip(&mut self) {
        let Some(index) = self.hovered_index else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let Some(entry) = self.entries.get(index) else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let bar_tooltip = match WidgetProperties::get(self, "tooltip") {
            Ok(CapabilityValue::String(text)) => text,
            _ => String::new(),
        };
        let text = EntryTooltipSource { entry, bar_tooltip: &bar_tooltip }
            .get("tooltip")
            .ok()
            .and_then(|value| match value {
                CapabilityValue::String(text) if !text.is_empty() => Some(text),
                _ => None,
            });
        match text {
            Some(text) => {
                let id = child_hover_id(self.base.id(), index);
                self.tooltips.register(id, text);
                self.tooltips.note_hover(id);
                self.tooltip_source = Some(id);
            }
            None => {
                if self.tooltip_source.take().is_some() {
                    self.tooltips.note_leave();
                }
            }
        }
    }

    /// Whether the tooltip pipeline still owes frames.
    fn tooltip_owes_frames(&self) -> bool {
        self.tooltip_source.is_some() || self.tooltips.tooltip().is_some()
    }
    /// Appends an entry and returns its index.
    ///
    /// Indices are positions in the list and therefore shift when an earlier
    /// entry is removed.
    pub fn add_menu(&mut self, title: impl Into<String>) -> usize {
        let idx = self.entries.len();
        self.entries.push(MenuBarEntry::new(title));
        idx
    }
    /// Removes the entry at `index`; out-of-range indices are ignored.
    ///
    /// The active and hovered cursors are adjusted with the list: a cursor pointing at the
    /// removed entry is cleared, and one pointing past it moves down by one. Leaving them
    /// untouched made `active_index` name a different title after a removal — or an index
    /// that no longer existed at all — while the bar still reported it as the open menu.
    pub fn remove_menu(&mut self, index: usize) {
        if index >= self.entries.len() {
            return;
        }
        self.entries.remove(index);
        self.active_index = Self::shift_cursor(self.active_index, index);
        self.hovered_index = Self::shift_cursor(self.hovered_index, index);
        self.base.request_redraw();
    }

    /// Shifts one cursor after the entry at `removed` left the list.
    fn shift_cursor(cursor: Option<usize>, removed: usize) -> Option<usize> {
        match cursor {
            Some(at) if at == removed => None,
            Some(at) if at > removed => Some(at - 1),
            other => other,
        }
    }
    /// Enables or disables the entry at `index`. Out-of-range indices are
    /// ignored, but a redraw is still requested in that case.
    pub fn set_menu_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(e) = self.entries.get_mut(index) {
            e.set_enabled(enabled);
        }
        self.base.request_redraw();
    }
    /// Returns whether the entry at `index` is enabled, or `None` when the
    /// index is out of range.
    pub fn menu_enabled(&self, index: usize) -> Option<bool> {
        self.entries.get(index).map(|entry| entry.is_enabled())
    }
    /// Removes every entry and clears both the active and hovered cursors.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.active_index = None;
        self.hovered_index = None;
    }

    /// Installs a widget in the trailing corner slot.
    ///
    /// The bar places it against its right edge and paints it after the entries. Passing
    /// `None` removes it. The slot is opt-in, so a bar without one is unchanged.
    pub fn set_corner_widget(&mut self, widget: Option<Box<dyn Widget>>) {
        self.corner = widget;
        self.base.request_redraw();
    }

    /// The corner widget, if one is installed.
    pub fn corner_widget(&self) -> Option<&dyn Widget> {
        self.corner.as_deref()
    }

    /// The box the corner widget is painted in, when one is installed.
    ///
    /// It is `size_hint().width` wide, capped to the band, and pinned to the bar's
    /// trailing edge. `None` when no widget is installed or the band is too narrow
    /// to give it a positive width.
    pub fn corner_rect(&self) -> Option<Rect> {
        let widget = self.corner.as_deref()?;
        let band = self.band_rect();
        let width = widget.size_hint().width.min(band.width);
        if width == 0 {
            return None;
        }
        Some(Rect::new(band.x + band.width as i32 - width as i32, band.y, width, band.height))
    }

    /// Attaches a drop-down menu to the entry at `index`.
    ///
    /// Returns `false` for an out-of-range index. Attaching replaces any menu already
    /// attached to the same entry. An attached bar places the popup under the entry and
    /// forwards events to it, and it stops emitting `triggered`/`triggered_entry` for
    /// that entry so the host is not asked to open a second copy.
    pub fn attach_menu(&mut self, index: usize, menu: Menu) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        if let Some(slot) = self.menus.iter_mut().find(|(attached, _)| *attached == index) {
            slot.1 = menu;
        } else {
            self.menus.push((index, menu));
        }
        true
    }

    /// Removes and returns the menu attached to `index`, if any.
    pub fn detach_menu(&mut self, index: usize) -> Option<Menu> {
        let slot = self.menus.iter().position(|(attached, _)| *attached == index)?;
        if self.open_menu_index == Some(index) {
            self.open_menu_index = None;
        }
        Some(self.menus.remove(slot).1)
    }

    /// The menu attached to `index`, if any.
    pub fn attached_menu(&self, index: usize) -> Option<&Menu> {
        self.menus.iter().find(|(attached, _)| *attached == index).map(|(_, menu)| menu)
    }

    /// The menu attached to `index`, mutably.
    pub fn attached_menu_mut(&mut self, index: usize) -> Option<&mut Menu> {
        self.menus.iter_mut().find(|(attached, _)| *attached == index).map(|(_, menu)| menu)
    }

    /// The entry whose attached menu is currently open.
    pub fn open_menu_index(&self) -> Option<usize> {
        self.open_menu_index
    }

    /// Opens the menu attached to `index` under its entry.
    ///
    /// Returns `false` when the entry has no attached menu or the entry is disabled.
    pub fn open_attached_menu(&mut self, index: usize) -> bool {
        let Some(rect) = self.entry_rect(index) else {
            return false;
        };
        if !self.entries[index].is_enabled() {
            return false;
        }
        let bar = self.geometry();
        let Some(menu) = self.attached_menu_mut(index) else {
            return false;
        };
        let anchor = Point::new(rect.x, rect.y + rect.height as i32);
        // The bar's own geometry is the popup's viewport unless the host declares a
        // larger one: a menu bar sits at the top of the surface its menus drop into, so
        // its width is the honest horizontal bound.
        let viewport = bar.union(&Rect::new(0, 0, bar.x.max(0) as u32 + bar.width, 4000));
        menu.open_at(anchor, viewport);
        self.open_menu_index = Some(index);
        self.active_index = Some(index);
        self.base.request_redraw();
        true
    }

    /// Closes the open attached menu, if one is open.
    pub fn close_attached_menu(&mut self) {
        if let Some(index) = self.open_menu_index.take() {
            if let Some(menu) = self.attached_menu_mut(index) {
                menu.hide();
            }
            self.active_index = None;
            self.base.request_redraw();
        }
    }

    /// The currently open attached menu, if any.
    pub fn active_menu(&self) -> Option<&Menu> {
        self.open_menu_index.and_then(|index| self.attached_menu(index))
    }

    /// The currently open attached menu, mutably.
    pub fn active_menu_mut(&mut self) -> Option<&mut Menu> {
        let index = self.open_menu_index?;
        self.attached_menu_mut(index)
    }

    /// Declares that the host bridged this bar to the platform's native menu bar.
    ///
    /// Metadata only: the widget still paints and answers events, so a host is free to
    /// hide it or leave it as a fallback. Defaults to `false`.
    pub fn set_native(&mut self, native: bool) {
        self.native_menu_bar = native;
    }

    /// Whether the host declared a native menu-bar bridge.
    pub fn is_native(&self) -> bool {
        self.native_menu_bar
    }

    /// Registers a live action with this bar and returns its index.
    ///
    /// This is the data a platform bridge merges: a drawn bar's [`Self::entries`] are only
    /// titles, so the actions carry the roles, labels and state a native menu needs.
    pub fn add_action(&mut self, action: Action) -> usize {
        let index = self.actions.len();
        self.actions.push(action);
        self.sync_platform_bridge();
        index
    }

    /// Resolves a command from the registry and registers it with this bar.
    ///
    /// This is the menu-bar half of COMP-25: the action carries the registry id, the bar records
    /// the host binding, and the state cell subscribes to the command's signals. A bar built this
    /// way is a view of the registry rather than a second declaration of the same commands.
    pub fn add_action_from_manager(
        &mut self,
        manager: &mut crate::action::ActionManager,
        action_id: &str,
    ) -> Option<usize> {
        let definition = manager.action_definition(action_id)?;
        if !manager.bind_action_to_menu(definition.id.clone(), self.base.id()) {
            return None;
        }
        let action = Action::from_definition(&definition, self.base.geometry());
        let id = definition.id.clone();
        let index = self.add_action(action);
        if let Some(state) = manager.watch_action(&id) {
            self.live_actions.push((id, state));
        }
        Some(index)
    }

    /// The live state cell for `action_id`, if this bar presents it.
    pub fn live_action_state(&self, action_id: &str) -> Option<crate::action::ActionLiveState> {
        self.live_actions
            .iter()
            .find(|(id, _)| id == action_id)
            .map(|(_, state)| lock(state).clone())
    }

    /// Applies registry state to every command this bar presents.
    pub fn refresh_action_states(&mut self, manager: &crate::action::ActionManager) {
        let mut changed = false;
        for action in &mut self.actions {
            let Some(action_id) = action.command_id().map(String::from) else {
                continue;
            };
            let Some(definition) = manager.action_definition(&action_id) else {
                continue;
            };
            let live = self
                .live_actions
                .iter()
                .find(|(id, _)| id == &action_id)
                .map(|(_, state)| lock(state).clone());
            let (text, enabled, checkable, checked) = match live {
                Some(state) => (state.text, state.enabled, state.checkable, state.checked),
                None => {
                    (definition.text, definition.enabled, definition.checkable, definition.checked)
                }
            };
            if action.text() != text {
                action.set_text(text);
                changed = true;
            }
            if action.is_checkable() != checkable {
                action.set_checkable(checkable);
                changed = true;
            }
            if checkable && action.is_checked() != checked {
                action.set_checked(checked);
                changed = true;
            }
            if action.cmd().is_enabled() != enabled {
                action.cmd_mut().set_enabled(enabled);
                changed = true;
            }
        }
        if changed {
            self.base.request_redraw();
            self.sync_platform_bridge();
        }
    }

    /// The live actions registered with this bar, in registration order.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Every action whose [`MenuRole`] is exactly `role`.
    ///
    /// This is the query a bridge uses to find the entries it must merge into one platform
    /// menu instead of appending as duplicates.
    pub fn native_role_items(&self, role: MenuRole) -> Vec<&Action> {
        self.actions.iter().filter(|action| action.menu_role() == role).collect()
    }

    /// Every action classified into `position` by [`MenuRole::native_position`].
    ///
    /// The position, not the role, is what a platform menu is built from: About, Preferences
    /// and Quit are three roles in one application menu.
    pub fn native_position_items(&self, position: NativeMenuPosition) -> Vec<&Action> {
        self.actions
            .iter()
            .filter(|action| action.menu_role().native_position() == position)
            .collect()
    }

    /// Installs the host bridge and immediately gives it the current action set.
    ///
    /// The bridge is notified again by [`Self::sync_platform_bridge`] after every action the
    /// bar gains, so the platform copy cannot silently fall behind the model.
    pub fn set_platform_bridge(&mut self, bridge: impl NativeMenuBridge + 'static) {
        self.platform_bridge = Some(Box::new(bridge));
        self.sync_platform_bridge();
    }

    /// The installed host bridge, if any.
    pub fn platform_bridge(&self) -> Option<&dyn NativeMenuBridge> {
        self.platform_bridge.as_deref()
    }

    /// Rebuilds the platform menu through the installed bridge.
    ///
    /// The bridge is temporarily taken out of the field so it can be handed an immutable
    /// borrow of the bar: `rebuild` needs both `&mut self` (the bridge) and `&MenuBar`, and
    /// both would otherwise come from the same struct.
    pub fn sync_platform_bridge(&mut self) {
        let Some(mut bridge) = self.platform_bridge.take() else {
            return;
        };
        bridge.rebuild(self);
        self.platform_bridge = Some(bridge);
    }

    /// Activates one entry: opens its attached menu, or reports the click.
    fn activate_entry(&mut self, index: usize) {
        if !self.entries.get(index).is_some_and(MenuBarEntry::is_enabled) {
            return;
        }
        if self.menus.iter().any(|(attached, _)| *attached == index) {
            self.open_attached_menu(index);
            return;
        }
        self.active_index = Some(index);
        let title = self.entries[index].title().to_string();
        let entry = self.entry_rect(index);
        self.triggered.emit(title.clone());
        if let Some(entry) = entry {
            self.triggered_entry.emit((title, entry));
        }
    }

    /// Moves the active cursor by `delta` over enabled entries, wrapping at the ends.
    fn move_active_entry(&mut self, delta: i32) {
        let count = self.entries.len();
        if count == 0 || delta == 0 {
            return;
        }
        let mut index = self.active_index.unwrap_or(if delta > 0 { count - 1 } else { 0 });
        for _ in 0..count {
            index = (index as i32 + delta).rem_euclid(count as i32) as usize;
            if self.entries[index].is_enabled() {
                if self.open_menu_index.is_some() {
                    // In the open state the cursor and the open menu move together: a
                    // menu bar that left the old menu up while the highlight moved
                    // would show two active entries.
                    if self.attached_menu(index).is_none() {
                        self.close_attached_menu();
                    } else {
                        self.open_attached_menu(index);
                    }
                } else {
                    self.active_index = Some(index);
                }
                self.base.request_redraw();
                return;
            }
        }
    }

    /// The entry whose title starts with `letter`, ignoring case and `&` markers.
    fn mnemonic_entry(&self, letter: char) -> Option<usize> {
        let wanted = letter.to_ascii_lowercase();
        self.entries.iter().enumerate().position(|(_, entry)| {
            entry
                .title()
                .trim_start_matches('&')
                .chars()
                .next()
                .is_some_and(|first| first.to_ascii_lowercase() == wanted && entry.is_enabled())
        })
    }
    /// The strip the bar actually paints: a full-width band
    /// [`dimensions::MENU_BAR_HEIGHT`] tall, pinned to the **top** of the area the control was
    /// given.
    ///
    /// # Why the bar is a top band and not a centred one
    ///
    /// A menu bar is not centred chrome: the menu contents it opens begin *below* it, so
    /// whatever the layout handed the bar, the strip belongs on the control's top edge.
    /// `draw` painted `geometry()` while `size_hint` reported 28 — the two disagreed about how
    /// thick a menu bar is, and the 240x120 census cell drew a 120 px band, four times the
    /// strip a layout was told to expect (`menu_bar.svg` was a full-canvas fill). The two ends
    /// now share one derivation. This is also what `entry_width` and `hit_entry` read, so the
    /// clickable strip is the painted strip.
    fn band_rect(&self) -> Rect {
        ControlMetrics::top_band(self.geometry(), dimensions::MENU_BAR_HEIGHT)
    }

    fn entry_width(&self, title: &str) -> f32 {
        // Measured with the **effective font** through the shared ruler rather than the old
        // "8 px per byte + 16" guess (D09-STYLE-01 / D09-STYLE-02): the byte count over-measured
        // non-ASCII titles and ignored the font entirely, so a scaled theme font did not widen an
        // entry.
        //
        // The padding must cover what `draw_text_fitted` insets: it reserves
        // `TEXT_FIT_MARGIN` at **each** end, so an entry that only added
        // `TOOLBAR_ITEM_INSET * 2` (4 px) gave the painter 2 px less than the measured title
        // and truncated every label. Measured on the GTK demo before this: "File" painted as
        // "…", "Edit" as "E…". The entry now reserves both the fit margin and a real gap.
        let font = effective_font(self.style());
        (estimate_text_width(title, font, 1.0)
            + 2 * (crate::render::TEXT_FIT_MARGIN + MENU_ENTRY_TEXT_PADDING)) as f32
    }
    /// The rectangle of the entry at `index`, in the coordinate space the bar's own geometry
    /// uses — client coordinates for a bar placed in a window.
    ///
    /// This is the box `draw` paints and `hit_entry` tests, so a consumer that opens the
    /// matching menu from a `triggered` title can place the popup flush under the entry it
    /// came from instead of guessing an offset. Returns `None` for an out-of-range index.
    pub fn entry_rect(&self, index: usize) -> Option<Rect> {
        let rect = self.band_rect();
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = self.entry_width(entry.title()) as i32;
            if i == index {
                return Some(Rect { x, y: rect.y, width: w as u32, height: rect.height });
            }
            x += w;
        }
        None
    }
    fn hit_entry(&self, pos: Point) -> Option<usize> {
        // The **painted band**, so a click below a 28 px strip is not a click on the bar.
        let rect = self.band_rect();
        if pos.y < rect.y || pos.y > rect.y + rect.height as f32 as i32 {
            return None;
        }
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = self.entry_width(entry.title()) as i32;
            if pos.x >= x && pos.x < x + w {
                return Some(i);
            }
            x += w;
        }
        None
    }
}
impl Widget for MenuBar {
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
        crate::core::Size::new(400, dimensions::MENU_BAR_HEIGHT)
    }

    fn has_open_popup(&self) -> bool {
        self.open_menu_index.is_some()
    }

    /// Advances the tooltip delay/duration clocks.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.tooltip_now_ms = self.tooltip_now_ms.saturating_add(u64::from(delta_ms));
        let changed = self.tooltips.tick(u64::from(delta_ms), self.tooltip_now_ms);
        changed || self.tooltip_owes_frames()
    }

    fn is_animating(&self) -> bool {
        self.tooltip_owes_frames()
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
            "triggered" => Some(EventSignalRef::mapped("triggered", &self.triggered, |v| {
                CapabilityValue::String(v.clone())
            })),
            "triggered_entry" => {
                Some(EventSignalRef::mapped("triggered_entry", &self.triggered_entry, |v| {
                    // The published shape is `Mixed`, so it must arrive as a structured carrier:
                    // a subscriber reads the title and the rectangle as their own typed values
                    // rather than re-parsing a debug spelling.
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(v.0.clone()),
                        CapabilityValue::Rect(v.1),
                    ]))
                }))
            }
            "entry_hovered" => {
                Some(EventSignalRef::mapped("entry_hovered", &self.entry_hovered, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `MenuBar`'s property contract.
///
/// Every published property is a read-only projection of the bar's current
/// contents and pointer state; the schema advertises none of them as writable,
/// so the write path forwards straight to the base helpers.
impl WidgetProperties for MenuBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entry_count" => Ok(CapabilityValue::UInt(self.entries().len() as u64)),
            "active_index" => match self.active_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "hovered_index" => match self.hovered_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The entry vector and the hover/active cursors are owned by the
            // control's own input handling, so they exist but refuse writes.
            "entry_count" | "active_index" | "hovered_index" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["entry_count", "active_index", "hovered_index", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `menu_bar` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every entry
    /// and resets both cursors, which is exactly what [`MenuBar::clear`] does.
    /// `add_menu` takes the entry's title and `remove_menu` takes the index to
    /// remove, so a payload-less invocation of either is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the names are right and the argument
    /// is what is missing, which is not `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_menu" | "remove_menu" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MenuBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // An open drop-down owns every event that is not aimed at the bar itself. The bar
        // still takes presses and moves on its own band, because that is how an entry
        // switches or closes the open menu.
        if let Some(index) = self.open_menu_index {
            let on_bar = match event {
                Event::MousePress { pos, .. } | Event::MouseMove { pos } => {
                    self.hit_entry(*pos).is_some()
                }
                _ => false,
            };
            if !on_bar {
                let mut open = true;
                if let Some(menu) = self.attached_menu_mut(index) {
                    menu.handle_event(event);
                    open = menu.is_visible();
                }
                if !open {
                    self.open_menu_index = None;
                    self.active_index = None;
                }
                return;
            }
        }
        match event {
            Event::MouseMove { pos } => {
                // Derived rather than assigned: the entry under the pointer is only meaningful while
                // `BaseWidget` agrees the pointer is on the bar, and re-testing on every move is
                // what keeps this from going stale when the pointer leaves without a `MouseLeave`
                // (a host that stopped forwarding moves, a window that lost focus).
                let next = if self.base.is_hovered() { self.hit_entry(*pos) } else { None };
                let prev = self.hovered_index;
                self.hovered_index = next;
                self.tooltips.note_pointer(*pos);
                self.sync_hover_tooltip();
                if self.hovered_index != prev {
                    if let Some(idx) = self.hovered_index {
                        if self.entries[idx].is_enabled() {
                            let title = self.entries[idx].title().to_string();
                            self.entry_hovered.emit(title);
                        }
                    }
                }
                // While a menu is open the highlight follows the pointer: hovering another
                // entry switches the open drop-down instead of leaving the old one up. A bar
                // whose entries have no attached menu keeps its original behaviour.
                if let Some(open) = self.open_menu_index {
                    if let Some(hovered) = self.hovered_index {
                        if hovered != open && self.attached_menu(hovered).is_some() {
                            self.open_attached_menu(hovered);
                        }
                    }
                }
            }
            // The bar's own pointer fact is the source, so leaving it must clear the entry too —
            // otherwise the last entry stays highlighted on a bar the pointer has left.
            Event::MouseLeave { .. } => {
                self.hovered_index = None;
                self.tooltip_source = None;
                self.tooltips.note_leave();
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(idx) = self.hit_entry(*pos) {
                    if !self.entries[idx].is_enabled() {
                        return;
                    }
                    // Pressing the entry whose menu is open closes it; pressing any other
                    // entry activates that one. This is a menu bar's toggle, not a second
                    // independent way to open a popup.
                    if self.open_menu_index == Some(idx) {
                        self.close_attached_menu();
                    } else {
                        self.activate_entry(idx);
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                // Alt+letter is the mnemonic path: it activates the entry whose title
                // starts with that letter without moving through the bar first.
                if modifiers & 0b0100 != 0 {
                    if let Some(letter) = char::from_u32(*key).filter(char::is_ascii_alphabetic) {
                        if let Some(index) = self.mnemonic_entry(letter) {
                            self.activate_entry(index);
                        }
                    }
                    return;
                }
                match *key {
                    key_codes::ESCAPE => {
                        // Escape closes the open menu first; with no menu it leaves the
                        // active state, exactly as before.
                        if self.open_menu_index.is_some() {
                            self.close_attached_menu();
                        } else {
                            self.active_index = None;
                        }
                    }
                    // F10 is the platform convention for entering the menu bar; 18 is the
                    // Alt key itself, which many hosts report as a bare press.
                    key_codes::F10 | 18 => {
                        if self.open_menu_index.is_some() || self.active_index.is_some() {
                            self.close_attached_menu();
                            self.active_index = None;
                        } else {
                            let first = self.entries.iter().position(MenuBarEntry::is_enabled);
                            if let Some(index) = first {
                                if self.attached_menu(index).is_some() {
                                    self.open_attached_menu(index);
                                } else {
                                    self.active_index = Some(index);
                                }
                            }
                        }
                    }
                    key_codes::LEFT => self.move_active_entry(-1),
                    key_codes::RIGHT => self.move_active_entry(1),
                    key_codes::DOWN => {
                        if let Some(index) = self.active_index {
                            self.activate_entry(index);
                        } else if let Some(first) =
                            self.entries.iter().position(MenuBarEntry::is_enabled)
                        {
                            self.activate_entry(first);
                        }
                    }
                    _ => {}
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for MenuBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **band**, not the control's rectangle: see `band_rect`. The fill, the bottom
        // rule, the entry highlights and the labels are all placed from this one box, so a
        // 240x120 census cell draws a 28 px strip rather than a 120 px slab.
        let rect = self.band_rect();

        // A menu bar is very nearly all chrome — its fill, its separators, its entry
        // highlights and its labels. Every one of those was a literal, so a light/dark
        // switch repainted nothing and the rendering census reported the control as
        // theme-blind.
        //
        // Chrome colours resolve the explicit style first, then the theme's resolved style
        // for this control, and only then the original literal. The literal stays as the
        // fallback so an inactive theme still has a defined appearance. The theme read is a
        // separate manager lock, taken and released inside `resolved_theme_style`, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let themed = crate::style::resolved_theme_style_shared("menu_bar");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        let themed_border = themed.as_ref().and_then(|r| r.border_color);
        let themed_text = themed.as_ref().and_then(|r| r.text_color);
        // `menu_bar` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A bar
        // painted in that colour is byte-identical to the frame behind it, so the resolved
        // surface is re-derived a visible step towards the theme's ink; a colour the caller
        // set still wins.
        let base = style.background_color.or(themed_bg).unwrap_or(Color::rgb(240, 240, 240));
        let ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(0, 0, 0));
        let background = base.blend(&ink, 0.06);
        // The bottom rule is chrome, and its literal was darker than the bar; deriving it
        // from the resolved pair keeps that relationship in either appearance.
        let separator =
            style.border_color.or(themed_border).unwrap_or_else(|| background.blend(&ink, 0.32));
        // The active entry is filled with the accent token (the "open menu" indicator),
        // the hovered one with a lighter step of the same family, and the label over each
        // is picked for contrast rather than hardcoded white-on-accent.
        let active_fill = crate::style::theme_manager()
            .current_theme()
            .map(|theme| theme.colors.primary)
            .unwrap_or(Color::rgb(0, 120, 215));
        let hover_fill = background.blend(&active_fill, 0.25);
        let disabled_ink = ink.blend(&background, 0.5);

        // Menu bar background
        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_line(
            Point::new(rect.x, rect.y + rect.height as i32 - 1),
            Point::new(rect.x + rect.width as i32, rect.y + rect.height as i32 - 1),
            separator,
        );
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = self.entry_width(entry.title()) as i32;
            let is_hovered = self.hovered_index == Some(i);
            let is_active = self.active_index == Some(i);
            let entry_rect = Rect { x, y: rect.y, width: w as u32, height: rect.height };
            if is_active {
                context.fill_rect(entry_rect, active_fill);
            } else if is_hovered {
                context.fill_rect(entry_rect, hover_fill);
            }
            let fg = if !entry.is_enabled() {
                disabled_ink
            } else if is_active {
                active_fill.contrast_color()
            } else {
                ink
            };
            // Centre the label inside its own entry **on both axes**.
            //
            // The previous form was `Point::new(x + w / 2, rect.y + rect.height / 2)` with
            // `HorizontalAlignment::Left`. Two separate errors came out of that one line: the
            // horizontal term computed the entry's *midpoint* and then used it as a left-hand
            // origin, and the vertical term put the glyph box's top edge on the bar's middle
            // line. `entry_width` is `len * 8 + 16`, so a four-character title in a 48 px entry
            // advanced 32 px from `x + 24` — past the end of its own entry and 9.6 px over the
            // next title. Passing the entry rectangle and asking for `Center` states the
            // intent, and there is no midpoint left to misuse.
            let font = effective_font(self.style());
            let line = context.text_line(entry_rect, font);
            context.draw_text_fitted_cow(
                line,
                entry.title(),
                font,
                fg,
                HorizontalAlignment::Center,
            );
            x += w;
        }

        // The corner widget is painted last, inside the box `corner_rect` reports, so the
        // entry run and the corner slot cannot overlap.
        let corner_rect = self.corner_rect();
        if let (Some(rect), Some(corner)) = (corner_rect, self.corner.as_mut()) {
            corner.set_geometry(rect);
            if let Some(drawable) = crate::widget::draw_bridge::draw_of(corner.as_mut()) {
                drawable.draw(context);
            }
        }

        // The open drop-down is drawn after the bar, so its popup sits above the chrome.
        if let Some(index) = self.open_menu_index {
            if let Some(menu) = self.attached_menu_mut(index) {
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(menu) {
                    drawable.draw(context);
                }
            }
        }

        // The hover hint is owned by the bar's tooltip pipeline, drawn last so it is never under
        // the drop-down it explains.
        let geometry = self.geometry();
        self.tooltips.set_geometry(geometry);
        self.tooltips.draw(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menubar_menu_enabled_accessor_handles_valid_and_oob_indices() {
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        let idx = menu_bar.add_menu("File");

        assert_eq!(menu_bar.menu_enabled(idx), Some(true));
        menu_bar.set_menu_enabled(idx, false);
        assert_eq!(menu_bar.menu_enabled(idx), Some(false));
        assert_eq!(menu_bar.menu_enabled(99), None);
    }

    #[test]
    fn removing_an_entry_keeps_the_cursors_pointing_at_the_same_titles() {
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        menu_bar.add_menu("File");
        menu_bar.add_menu("Edit");
        menu_bar.add_menu("View");
        menu_bar.active_index = Some(2);
        menu_bar.hovered_index = Some(1);

        // Removing "File" shifts both titles down by one...
        menu_bar.remove_menu(0);
        assert_eq!(menu_bar.entries()[menu_bar.active_index().unwrap()].title(), "View");
        assert_eq!(menu_bar.entries()[menu_bar.hovered_index().unwrap()].title(), "Edit");

        // ...and removing the entry a cursor was on clears that cursor rather than
        // leaving it naming the title that took its place.
        menu_bar.hovered_index = Some(1);
        menu_bar.remove_menu(1);
        assert_eq!(menu_bar.active_index(), None);
        assert_eq!(menu_bar.hovered_index(), None);
    }

    #[test]
    fn entry_rect_is_none_past_the_last_entry() {
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        menu_bar.add_menu("File");
        assert!(menu_bar.entry_rect(0).is_some());
        assert!(menu_bar.entry_rect(1).is_none());
    }

    /// An attached menu opens under its own entry, draws, and toggles closed.
    #[test]
    fn an_attached_menu_opens_under_its_entry_and_toggles() {
        let mut bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        bar.add_menu("File");
        let mut menu = Menu::new("File", Rect::new(0, 0, 160, 100));
        menu.add_action("Open");
        assert!(bar.attach_menu(0, menu));

        let triggered = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = triggered.clone();
        bar.triggered.connect(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        let entry = bar.entry_rect(0).expect("the entry has a box");
        bar.handle_event(&Event::mouse_press(entry.x + 4, entry.y + 4, 1));
        assert_eq!(bar.open_menu_index(), Some(0));
        assert!(bar.active_menu().expect("an attached menu").is_visible());
        assert_eq!(
            triggered.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "an attached menu replaces the host notification, it does not duplicate it"
        );
        let menu_geometry = bar.active_menu().expect("an attached menu").geometry();
        assert_eq!(menu_geometry.x, entry.x);
        assert_eq!(menu_geometry.y, entry.y + entry.height as i32);

        // `has_open_popup` is what the runtime uses to route outside presses here.
        assert!(bar.has_open_popup());

        // Pressing the same entry again closes it.
        bar.handle_event(&Event::mouse_press(entry.x + 4, entry.y + 4, 1));
        assert_eq!(bar.open_menu_index(), None);
        assert!(!bar.has_open_popup());
    }

    /// F10 enters the bar, the arrows walk it, and Alt+letter jumps by title.
    #[test]
    fn keyboard_enters_walks_and_uses_mnemonics() {
        let mut bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        bar.add_menu("File");
        bar.add_menu("Edit");
        bar.add_menu("View");

        let triggered = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = triggered.clone();
        bar.triggered.connect(move |title| {
            sink.lock().unwrap().push(title.to_string());
        });

        bar.handle_event(&Event::key_press(key_codes::F10, 0));
        assert_eq!(bar.active_index(), Some(0), "F10 enters at the first entry");
        bar.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        assert_eq!(bar.active_index(), Some(1));
        bar.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        assert_eq!(bar.active_index(), Some(2));
        bar.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        assert_eq!(bar.active_index(), Some(0), "the cursor wraps");

        // Alt is bit 2 of the event modifier mask.
        bar.handle_event(&Event::key_press(86, 0b0100));
        assert_eq!(bar.active_index(), Some(2));
        assert_eq!(&*triggered.lock().unwrap(), &["View".to_string()]);

        bar.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert_eq!(bar.active_index(), None);
    }

    /// The corner widget gets a box pinned to the bar's trailing edge.
    #[test]
    fn the_corner_widget_owns_the_trailing_box() {
        let mut bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        bar.add_menu("File");
        assert!(bar.corner_rect().is_none(), "no widget, no slot");

        bar.set_corner_widget(Some(Box::new(crate::widget::Label::new(
            "status".to_string(),
            Rect::new(0, 0, 80, 24),
        ))));
        let rect = bar.corner_rect().expect("the slot exists once a widget is installed");
        let wanted = bar.corner_widget().expect("installed").size_hint().width;
        assert_eq!(rect.width, wanted, "the slot is the widget's own size hint");
        assert_eq!(rect.x + rect.width as i32, 300, "pinned to the trailing edge");
        assert!(bar.corner_widget().is_some());
    }

    /// `native` is a host declaration, not a repaint switch.
    #[test]
    fn the_native_flag_is_metadata() {
        let mut bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        assert!(!bar.is_native(), "a drawn bar is the default");
        bar.set_native(true);
        assert!(bar.is_native());
    }

    /// A role classifies an action into the platform position a bridge installs it at.
    #[test]
    fn menu_roles_classify_into_platform_positions() {
        assert_eq!(MenuRole::None.native_position(), NativeMenuPosition::Bar);
        assert_eq!(MenuRole::About.native_position(), NativeMenuPosition::Application);
        assert_eq!(MenuRole::Preferences.native_position(), NativeMenuPosition::Application);
        assert_eq!(MenuRole::Quit.native_position(), NativeMenuPosition::Application);
        assert_eq!(MenuRole::Services.native_position(), NativeMenuPosition::Services);
        assert_eq!(MenuRole::Window.native_position(), NativeMenuPosition::Window);
        assert_eq!(MenuRole::Help.native_position(), NativeMenuPosition::Help);

        let mut bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        let mut about = Action::new("About", Rect::new(0, 0, 100, 24));
        about.set_menu_role(MenuRole::About);
        let mut quit = Action::new("Quit", Rect::new(0, 0, 100, 24));
        quit.set_menu_role(MenuRole::Quit);
        let ordinary = Action::new("Open", Rect::new(0, 0, 100, 24));
        bar.add_action(about);
        bar.add_action(quit);
        bar.add_action(ordinary);

        assert_eq!(bar.native_role_items(MenuRole::Quit).len(), 1);
        assert_eq!(bar.native_role_items(MenuRole::Quit)[0].text(), "Quit");
        assert_eq!(bar.native_role_items(MenuRole::About).len(), 1);
        assert_eq!(bar.native_position_items(NativeMenuPosition::Application).len(), 2);
        assert_eq!(bar.native_position_items(NativeMenuPosition::Bar).len(), 1);
    }

    /// Installing a bridge hands it the bar and re-notifies it when actions arrive.
    #[test]
    fn a_platform_bridge_receives_the_classified_actions() {
        use std::sync::{Arc, Mutex};

        struct RecordingBridge {
            calls: Arc<Mutex<Vec<Vec<String>>>>,
        }
        impl NativeMenuBridge for RecordingBridge {
            fn rebuild(&mut self, bar: &MenuBar) {
                let names = bar
                    .native_position_items(NativeMenuPosition::Application)
                    .into_iter()
                    .map(|action| action.text().to_string())
                    .collect();
                self.calls.lock().unwrap().push(names);
            }
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut bar = MenuBar::new(Rect::new(0, 0, 300, 24));
        bar.set_platform_bridge(RecordingBridge { calls: Arc::clone(&calls) });
        assert_eq!(calls.lock().unwrap().len(), 1, "installation is the first sync");

        let mut quit = Action::new("Quit", Rect::new(0, 0, 100, 24));
        quit.set_menu_role(MenuRole::Quit);
        bar.add_action(quit);

        assert!(bar.platform_bridge().is_some());
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "adding an action rebuilds the platform menu");
        assert_eq!(calls[1], vec!["Quit".to_string()], "and the bridge sees the role group");
    }
}
