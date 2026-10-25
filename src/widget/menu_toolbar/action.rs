// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Action widget — represents a command or toggle that can be placed in menus and toolbars.
//!
//! This widget **internally wraps** [`crate::action::Action`] to eliminate duplicate
//! `checkable`/`checked`/`triggered`/`toggled` logic. All command state lives in
//! the inner `action::Action`, while this struct adds widget-only fields
//! (`icon_text`, `shortcut`, `separator`) and `BaseWidget` integration.
use crate::action::Action as CmdAction;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::{Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutManager};
use crate::signal::{ConnectionHandle, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Where an action belongs in the platform's application menu.
///
/// The role is metadata, not painting: a host that builds a native application menu
/// uses it to merge the action into the system's About/Preferences/Quit items instead
/// of appending a duplicate entry. A host that draws its own menu ignores it, which is
/// why the default [`MenuRole::None`] changes no pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuRole {
    /// No special placement; the action is an ordinary menu entry.
    #[default]
    None,
    /// The application's About item.
    About,
    /// The application's Preferences/Settings item.
    Preferences,
    /// The application's Quit item.
    Quit,
    /// The application's Hide item.
    Hide,
    /// The application's Hide Others item.
    HideOthers,
    /// The application's Show All item.
    ShowAll,
    /// The application's Services submenu placeholder.
    Services,
    /// The application's Window menu placeholder.
    Window,
    /// The application's Help menu placeholder.
    Help,
}

impl MenuRole {
    /// The stable identifier used by hosts and serialized configuration.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::About => "about",
            Self::Preferences => "preferences",
            Self::Quit => "quit",
            Self::Hide => "hide",
            Self::HideOthers => "hide_others",
            Self::ShowAll => "show_all",
            Self::Services => "services",
            Self::Window => "window",
            Self::Help => "help",
        }
    }

    /// Reads the token written by [`Self::as_str`].
    ///
    /// An unknown token is `None` rather than `None` the variant: a typo and the legitimate
    /// "ordinary entry" role must not collapse into the same answer.
    pub fn from_token(token: &str) -> Option<Self> {
        Some(match token {
            "none" => Self::None,
            "about" => Self::About,
            "preferences" => Self::Preferences,
            "quit" => Self::Quit,
            "hide" => Self::Hide,
            "hide_others" => Self::HideOthers,
            "show_all" => Self::ShowAll,
            "services" => Self::Services,
            "window" => Self::Window,
            "help" => Self::Help,
            _ => return None,
        })
    }
}

/// Represents a user action (command, toggle, etc.) used in menus and toolbars.
///
/// The inner [`CmdAction`] owns the canonical command state (`id`, `checkable`,
/// `checked`, `enabled`). Widget-specific presentation fields (`icon_text`,
/// `shortcut` display string, `separator`) live on this struct.
pub struct Action {
    base: BaseWidget,
    /// The canonical command action — owns id, checkable, checked, enabled, triggered/toggled signals.
    cmd: CmdAction,
    /// Display text (label shown in menu/toolbar).
    text: String,
    /// Display icon text.
    icon_text: String,
    /// Typed icon geometry, mutually exclusive with the legacy text stand-in.
    icon_source: Option<IconSource>,
    /// Display shortcut string.
    shortcut: String,
    /// Whether this action is a visual separator.
    separator: bool,
    /// Where the action belongs in the platform's application menu.
    menu_role: MenuRole,
    /// Whether the action's icon is painted in menu surfaces.
    ///
    /// Toolbar surfaces ignore this flag; menus honour it, so an action can show an
    /// icon on a toolbar while keeping the menu row text-only.
    icon_visible_in_menu: bool,
    /// One-line hint a host may show in a status bar while the action is hovered.
    status_tip: String,
    /// The "What's This?" help text, empty when the action has none.
    whats_this: String,
    /// A real keyboard binding, as opposed to the display-only shortcut label.
    shortcut_key: Option<Shortcut>,
    /// Emitted when the action fires, with the checked state *after* the fire.
    ///
    /// Emitted by [`Action::trigger`], which routes through the inner command,
    /// so it does not fire when the action is disabled or is a separator.
    /// Distinct from the inner command's own `triggered` signal.
    pub triggered: Signal1<bool>,
    /// Emitted with the new checked state when it changes, whether the change
    /// came from [`Action::set_checked`] or from a trigger toggling a checkable
    /// action.
    pub toggled: Signal1<bool>,
    /// Emitted when the pointer enters the action's bounds (`Event::MouseEnter`).
    /// Carries no payload; a listener is expected to know the action it attached to.
    pub hovered: GenericSignal,
    /// Emitted whenever any presentation or command state changes — text, icon,
    /// shortcut, checkable, checked, enabled, or a command sync. Carries no
    /// payload, so a listener must re-read whatever it needs; it is a
    /// "something changed" notification, not a diff.
    pub changed: GenericSignal,
    // Connection handles to keep inner cmd signals wired.
    _toggled_handle: Option<ConnectionHandle>,
    _enabled_handle: Option<ConnectionHandle>,
}
impl Action {
    /// Builds a widget action from a registry definition.
    ///
    /// The definition is the data half of a command; this constructor is the one place it becomes
    /// a presentable action, so a menu bar, toolbar and button built from the same registry row
    /// cannot disagree about its text, flags or shortcut. Geometry is the caller's, because a
    /// definition carries no placement.
    pub fn from_definition(definition: &crate::action::ActionDefinition, geometry: Rect) -> Self {
        let mut action = Self::new(definition.text.clone(), geometry);
        action.set_command_id(definition.id.clone());
        action.set_checkable(definition.checkable);
        if definition.checkable {
            action.set_checked(definition.checked);
        }
        action.cmd_mut().set_enabled(definition.enabled);
        if !definition.shortcut.is_empty() {
            action.set_shortcut(definition.shortcut.clone());
        }
        action
    }

    /// Creates a plain (non-checkable, non-separator) action enabled by default.
    ///
    /// The inner command starts with an empty id, so
    /// [`Action::command_id`] returns `None` until
    /// [`Action::set_command_id`] is called. The signal wiring to the inner
    /// command is established here.
    ///
    /// `geometry` is in parent-relative logical pixels.
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        let text = text.into();
        let mut action = Self {
            base: BaseWidget::new(WidgetKind::Action, geometry, "Action"),
            cmd: CmdAction::new("", &text),
            text,
            icon_text: String::new(),
            icon_source: None,
            shortcut: String::new(),
            separator: false,
            menu_role: MenuRole::None,
            icon_visible_in_menu: true,
            status_tip: String::new(),
            whats_this: String::new(),
            shortcut_key: None,
            triggered: Signal1::new(),
            toggled: Signal1::new(),
            hovered: GenericSignal::new(),
            changed: GenericSignal::new(),
            _toggled_handle: None,
            _enabled_handle: None,
        };
        action.wire_signals();
        action
    }
    /// Creates a visual separator: an empty action flagged via
    /// [`Action::is_separator`], which drawing renders as a divider rather than
    /// a clickable item.
    ///
    /// The action still has working command and signal plumbing; the separator
    /// flag is a rendering and layout hint, not a behavioural one.
    pub fn separator(geometry: Rect) -> Self {
        let mut a = Self::new("", geometry);
        a.separator = true;
        a
    }
    /// Returns the label shown in its host menu or toolbar.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns the icon stand-in text, or `""` when none was set.
    /// This is text, not image data.
    pub fn icon_text(&self) -> &str {
        &self.icon_text
    }
    /// Returns the typed icon source, if one is active.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }
    /// Returns the shortcut display string (for example `"Ctrl+S"`).
    ///
    /// This is the label. When a binding was installed through
    /// [`Action::set_shortcut_key`] the label is kept in sync with it, and the
    /// action also responds to the key itself through [`Action::matches_shortcut`].
    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }
    /// Returns the role that places the action in a platform application menu.
    pub fn menu_role(&self) -> MenuRole {
        self.menu_role
    }
    /// Assigns the platform application-menu role. Metadata only: no repaint.
    pub fn set_menu_role(&mut self, role: MenuRole) {
        if self.menu_role == role {
            return;
        }
        self.menu_role = role;
        self.changed.emit();
    }
    /// Whether this action's icon is painted in menu surfaces.
    pub fn is_icon_visible_in_menu(&self) -> bool {
        self.icon_visible_in_menu
    }
    /// Shows or hides the icon in menu surfaces. Requests a repaint when it changes.
    pub fn set_icon_visible_in_menu(&mut self, visible: bool) {
        if self.icon_visible_in_menu == visible {
            return;
        }
        self.icon_visible_in_menu = visible;
        self.changed.emit();
        self.base.request_redraw();
    }
    /// The short hint a host may display while the action is hovered.
    pub fn status_tip(&self) -> &str {
        &self.status_tip
    }
    /// Replaces the status-bar hint. Metadata only: no repaint.
    pub fn set_status_tip(&mut self, tip: impl Into<String>) {
        let tip = tip.into();
        if self.status_tip == tip {
            return;
        }
        self.status_tip = tip;
        self.changed.emit();
    }
    /// The "What's This?" help text, or `""` when the action has none.
    pub fn whats_this(&self) -> &str {
        &self.whats_this
    }
    /// Replaces the "What's This?" help text. Metadata only: no repaint.
    pub fn set_whats_this(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.whats_this == text {
            return;
        }
        self.whats_this = text;
        self.changed.emit();
    }
    /// The installed keyboard binding, if any.
    ///
    /// Unlike the display-only [`Action::shortcut`] string, a binding is matched by
    /// [`Action::matches_shortcut`] and fires the action from
    /// [`EventHandler::handle_event`].
    pub fn shortcut_key(&self) -> Option<&Shortcut> {
        self.shortcut_key.as_ref()
    }
    /// Installs a keyboard binding and refreshes the display label from it.
    ///
    /// The label uses the compile target's modifier notation, so one declaration
    /// reads `⌘S` on macOS and `Ctrl+S` elsewhere. Returns the action so a host can
    /// register the binding with a global shortcut manager as well.
    pub fn set_shortcut_key(&mut self, shortcut: Shortcut) -> Shortcut {
        let label = PlatformShortcutStyle::current().format(&shortcut);
        let changed = self.shortcut_key.as_ref() != Some(&shortcut) || self.shortcut != label;
        self.shortcut_key = Some(shortcut.clone());
        self.shortcut = label;
        if changed {
            self.changed.emit();
            self.base.request_redraw();
        }
        shortcut
    }
    /// Removes the keyboard binding and clears the display label.
    pub fn clear_shortcut_key(&mut self) {
        if self.shortcut_key.is_none() && self.shortcut.is_empty() {
            return;
        }
        self.shortcut_key = None;
        self.shortcut.clear();
        self.changed.emit();
        self.base.request_redraw();
    }
    /// Whether a key event matches the installed binding.
    ///
    /// Both arguments use the same convention as [`Event::KeyPress`]. The platform modifier rule
    /// is **not** re-implemented here: [`ShortcutManager::matches_binding`] is the single decision
    /// point, so a standalone action, a registered menu binding, and the global registry can
    /// never disagree about whether a chord matches.
    pub fn matches_shortcut(&self, key: u32, modifiers: u32) -> bool {
        let Some(binding) = self.shortcut_key.as_ref() else {
            return false;
        };
        let Some(pressed) = Key::from_key_code(key) else {
            return false;
        };
        ShortcutManager::matches_binding(binding, pressed, Modifiers::from_event_bits(modifiers))
    }

    /// Dispatches one key press to this action, returning whether it fired.
    ///
    /// This is the host entry point the generator and the widget event loop share: call it for
    /// every key press and it triggers the action exactly when the binding matches and the
    /// command is enabled. It exists so a real accelerator does not depend on the host routing
    /// `KeyPress` to this particular widget.
    pub fn dispatch_shortcut(&mut self, key: u32, modifiers: u32) -> bool {
        if !self.matches_shortcut(key, modifiers) || !self.cmd.is_enabled() {
            return false;
        }
        self.trigger();
        true
    }

    /// Registers this action's binding in the global shortcut registry.
    ///
    /// Registration is what brings the action under conflict detection and lets a host dispatch
    /// through one table. The action id is explicit because a widget action is not guaranteed to
    /// have a [`Action::command_id`] yet.
    pub fn register_shortcut(
        &self,
        manager: &mut ShortcutManager,
        action_id: impl Into<String>,
    ) -> bool {
        let Some(binding) = self.shortcut_key.as_ref() else {
            return false;
        };
        manager.register(action_id, binding.clone(), self.text.clone())
    }
    /// Delegates to inner [`CmdAction::is_checkable`].
    pub fn is_checkable(&self) -> bool {
        self.cmd.is_checkable()
    }
    /// Delegates to inner [`CmdAction::is_checked`].
    pub fn is_checked(&self) -> bool {
        self.cmd.is_checked()
    }
    /// Returns `true` for a visual separator. See [`Action::separator`].
    pub fn is_separator(&self) -> bool {
        self.separator
    }
    /// Returns whether the pointer is currently over the action.
    ///
    /// Hover is recorded by [`BaseWidget`] from the runtime's synthesised
    /// `MouseEnter`/`MouseLeave` pair, which keeps an action's answer identical to the
    /// menus and tool buttons that draw it.
    pub fn is_pointer_hovered(&self) -> bool {
        self.base.is_hovered()
    }
    /// Returns the inner [`CmdAction`]'s id, if non-empty.
    pub fn command_id(&self) -> Option<&str> {
        let id = &self.cmd.id;
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    }
    /// Links this widget action to a named action command.
    /// Sets the inner [`CmdAction`]'s id.
    /// Sets the inner command's id and requests a redraw. An empty id makes
    /// [`Action::command_id`] report `None` again.
    pub fn set_command_id(&mut self, id: impl Into<String>) {
        self.cmd.id = id.into();
        self.base.request_redraw();
    }
    /// Clears the link to the command action.
    ///
    /// Does not request a redraw, unlike [`Action::set_command_id`].
    pub fn clear_command_id(&mut self) {
        self.cmd.id = String::new();
    }
    /// Provides mutable access to the inner [`CmdAction`] for advanced configuration
    /// (e.g. connecting to `enabled_changed` or `triggered` signals directly).
    pub fn cmd_mut(&mut self) -> &mut CmdAction {
        &mut self.cmd
    }
    /// Provides read-only access to the inner [`CmdAction`].
    pub fn cmd(&self) -> &CmdAction {
        &self.cmd
    }
    /// Syncs this widget action's state from another [`CmdAction`].
    /// Useful when an external `ActionManager` updates the command state.
    pub fn sync_from_command(&mut self, cmd: &CmdAction) {
        self.text = cmd.text.clone();
        self.cmd.set_checkable(cmd.is_checkable());
        if cmd.is_checkable() {
            self.cmd.set_checked(cmd.is_checked());
        }
        self.cmd.set_enabled(cmd.is_enabled());
        self.changed.emit();
    }
    /// Creates a standalone [`CmdAction`] mirroring this widget action's state,
    /// suitable for registration in an `ActionManager`.
    pub fn to_command_action(&self) -> CmdAction {
        let mut cmd = CmdAction::new(&self.cmd.id, &self.text);
        cmd.set_checkable(self.cmd.is_checkable());
        if self.cmd.is_checkable() {
            cmd.set_checked(self.cmd.is_checked());
        }
        cmd
    }
    /// Wires the inner `CmdAction` signals to this widget's signals.
    /// Call once after construction if you want the inner action's
    /// `toggled`/`enabled_changed` to propagate to widget signals.
    ///
    /// Calling it again is safe but leaks the previous connections: the old
    /// handles are overwritten without being disconnected, so each call adds
    /// another forwarding path and a single inner toggle will emit the widget
    /// `toggled`/`changed` signals once per call.
    pub fn wire_signals(&mut self) {
        let toggled_out = self.toggled.clone();
        let changed_out = self.changed.clone();
        let handle = self.cmd.connect_toggled(move |checked| {
            toggled_out.emit(*checked);
            changed_out.emit();
        });
        self._toggled_handle = Some(handle);
        let changed_out2 = self.changed.clone();
        let handle2 = self.cmd.connect_enabled_changed(move |_| {
            changed_out2.emit();
        });
        self._enabled_handle = Some(handle2);
    }
    /// Replaces the label, mirrors it into the inner command, emits `changed`,
    /// and requests a redraw.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cmd.text = self.text.clone();
        self.changed.emit();
        self.base.request_redraw();
    }
    /// Replaces the icon stand-in text, emits `changed`, and requests a redraw.
    pub fn set_icon_text(&mut self, text: impl Into<String>) {
        self.icon_text = text.into();
        self.icon_source = None;
        self.changed.emit();
        self.base.request_redraw();
    }
    /// Sets a built-in or static-geometry icon, replacing the legacy icon text.
    ///
    /// `icon_text()` returns an empty string while a typed source is active. Setting icon text
    /// again switches back to the legacy text presentation.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_text.clear();
        self.icon_source = source;
        self.changed.emit();
        self.base.request_redraw();
    }
    /// Replaces the shortcut *display* string, emits `changed`, and requests a
    /// redraw. No shortcut is registered as a result — see
    /// [`Action::shortcut`].
    pub fn set_shortcut(&mut self, shortcut: impl Into<String>) {
        self.shortcut = shortcut.into();
        self.changed.emit();
        self.base.request_redraw();
    }
    /// Delegates to inner [`CmdAction::set_checkable`].
    pub fn set_checkable(&mut self, checkable: bool) {
        self.cmd.set_checkable(checkable);
        self.changed.emit();
        self.base.request_redraw();
    }
    /// Delegates to inner [`CmdAction::set_checked`].
    pub fn set_checked(&mut self, checked: bool) {
        let old = self.cmd.is_checked();
        self.cmd.set_checked(checked);
        if self.cmd.is_checked() != old {
            self.toggled.emit(self.cmd.is_checked());
            self.changed.emit();
        }
        self.base.request_redraw();
    }
    /// Delegates to inner [`CmdAction::trigger`] and emits widget `triggered`.
    pub fn trigger(&mut self) {
        if self.cmd.trigger() {
            // cmd.trigger() already handles checkable toggle + emits its own triggered/toggled.
            // We emit widget-level triggered with current checked state.
            self.triggered.emit(self.cmd.is_checked());
        }
    }

    /// Serializes this action's presentation and command state.
    ///
    /// Unlike the widget-free [`crate::action::Action`], this type owns a real binding and a menu
    /// role, so both travel here — which is the shape a menu document needs to describe a command
    /// completely. The record is [`crate::action::ActionDefinition`], so the two action types share
    /// one JSON spelling.
    pub fn to_json(&self) -> String {
        let definition = crate::action::ActionDefinition {
            id: crate::compat::String::from(self.command_id().unwrap_or("")),
            text: self.text.clone(),
            checkable: self.cmd.is_checkable(),
            checked: self.cmd.is_checked(),
            enabled: self.base.is_enabled(),
            shortcut: self.shortcut.clone(),
            role: crate::compat::String::from(self.menu_role.as_str()),
        };
        definition.to_json()
    }

    /// Rebuilds an action from [`Self::to_json`].
    ///
    /// Geometry is required because a widget needs one; it is not part of the serialized record.
    /// An unparsable shortcut is reported rather than silently dropped: a document that names an
    /// accelerator the build cannot bind is wrong, not empty.
    pub fn from_json(text: &str, geometry: Rect) -> Result<Self, String> {
        let definition = crate::action::ActionDefinition::from_json(text)?;
        let role = MenuRole::from_token(&definition.role)
            .ok_or_else(|| crate::compat::format!("`{}` is not a menu role", definition.role))?;
        let mut action = Self::new(definition.text, geometry);
        action.set_command_id(definition.id);
        action.set_checkable(definition.checkable);
        action.set_checked(definition.checked);
        action.base.set_enabled(definition.enabled);
        action.set_menu_role(role);
        if !definition.shortcut.is_empty() {
            let binding = Shortcut::from_string(&definition.shortcut).ok_or_else(|| {
                crate::compat::format!(
                    "`{}` is not a shortcut this build understands",
                    definition.shortcut
                )
            })?;
            action.set_shortcut_key(binding);
        }
        Ok(action)
    }
}
/// A named set of actions that can enforce mutual exclusion.
///
/// A group owns its members, because exclusive behaviour needs to reach the
/// siblings of the action that was checked. The alternative — every action
/// holding a shared registry — would make `Action` non-`Send` and would hide the
/// membership in interior mutability; owning the members keeps the rule visible
/// at the call site and testable without a widget tree.
///
/// Members are ordinary [`Action`] values, so they keep their signals and can be
/// hosted by a menu or toolbar through [`ActionGroup::get`]/[`ActionGroup::get_mut`].
#[derive(Debug)]
pub struct ActionGroup {
    name: String,
    exclusive: bool,
    actions: Vec<Action>,
}

impl core::fmt::Debug for Action {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Action")
            .field("text", &self.text)
            .field("separator", &self.separator)
            .field("checkable", &self.cmd.is_checkable())
            .field("checked", &self.cmd.is_checked())
            .finish_non_exhaustive()
    }
}

impl ActionGroup {
    /// Creates an empty non-exclusive group.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), exclusive: false, actions: Vec::new() }
    }
    /// Creates an empty exclusive group: checking one member clears the others.
    pub fn exclusive(name: impl Into<String>) -> Self {
        Self { name: name.into(), exclusive: true, actions: Vec::new() }
    }
    /// The group's name, for diagnostics and configuration keys.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Whether checking one member clears the others.
    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }
    /// Turns mutual exclusion on or off.
    ///
    /// Turning it on leaves at most one member checked: the first checked member in
    /// insertion order keeps its state and the rest are cleared.
    pub fn set_exclusive(&mut self, exclusive: bool) {
        if self.exclusive == exclusive {
            return;
        }
        self.exclusive = exclusive;
        if exclusive {
            let keep = self.actions.iter().position(Action::is_checked);
            for (index, action) in self.actions.iter_mut().enumerate() {
                if Some(index) != keep {
                    action.set_checked(false);
                }
            }
        }
    }
    /// Appends an action and returns its index.
    pub fn add(&mut self, action: Action) -> usize {
        self.actions.push(action);
        self.actions.len() - 1
    }
    /// The number of members.
    pub fn len(&self) -> usize {
        self.actions.len()
    }
    /// Whether the group has no members.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
    /// Reads a member by index.
    pub fn get(&self, index: usize) -> Option<&Action> {
        self.actions.get(index)
    }
    /// Mutates a member by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Action> {
        self.actions.get_mut(index)
    }
    /// All members in insertion order.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }
    /// All members mutably, in insertion order.
    pub fn actions_mut(&mut self) -> &mut [Action] {
        &mut self.actions
    }
    /// The index of the checked member, if any.
    pub fn checked_index(&self) -> Option<usize> {
        self.actions.iter().position(Action::is_checked)
    }
    /// Checks or clears a member, applying the group's exclusion rule.
    ///
    /// An out-of-range index is ignored rather than panicking: hosts commonly
    /// drive groups from tables whose content changed under them.
    pub fn set_checked(&mut self, index: usize, checked: bool) {
        let exclusive = self.exclusive;
        let Some(action) = self.actions.get_mut(index) else {
            return;
        };
        action.set_checked(checked);
        if checked && exclusive {
            for (other, candidate) in self.actions.iter_mut().enumerate() {
                if other != index && candidate.is_checked() {
                    candidate.set_checked(false);
                }
            }
        }
    }
    /// Triggers a member, then re-applies exclusion.
    ///
    /// A checkable member toggles, so triggering the already-checked member clears
    /// it and leaves the group with no selection. That is the behaviour of a
    /// single-selection radio group only when the caller wants it; pass
    /// `checked = true` to [`ActionGroup::set_checked`] for a group that must always
    /// retain a selection.
    pub fn trigger(&mut self, index: usize) {
        let exclusive = self.exclusive;
        let Some(action) = self.actions.get_mut(index) else {
            return;
        };
        action.trigger();
        let now_checked = action.is_checked();
        if now_checked && exclusive {
            for (other, candidate) in self.actions.iter_mut().enumerate() {
                if other != index && candidate.is_checked() {
                    candidate.set_checked(false);
                }
            }
        }
    }
}

impl Widget for Action {
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
        crate::core::Size::new(100, 28)
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
                CapabilityValue::Bool(*v)
            })),
            "toggled" => Some(EventSignalRef::mapped("toggled", &self.toggled, |v| {
                CapabilityValue::Bool(*v)
            })),
            "hovered" => Some(EventSignalRef::unit("hovered", &self.hovered)),
            "changed" => Some(EventSignalRef::unit("changed", &self.changed)),
            _ => None,
        }
    }
}

/// `Action`'s property contract.
///
/// `command_id` is optional, so it is published as `Null` when unset and accepts
/// `Null` on write to clear it — the same optional-value convention the old
/// dispatch used. `separator` is derived from the action's short text and has no
/// setter, so it is read-only.
impl WidgetProperties for Action {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "icon_text" => Ok(CapabilityValue::String(self.icon_text().to_string())),
            "shortcut" => Ok(CapabilityValue::String(self.shortcut().to_string())),
            "checkable" => Ok(CapabilityValue::Bool(self.is_checkable())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
            },
            "separator" => Ok(CapabilityValue::Bool(self.is_separator())),
            "command_id" => match self.command_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "icon_text" => {
                self.set_icon_text(expect_string(value)?);
                Ok(())
            }
            "shortcut" => {
                self.set_shortcut(expect_string(value)?);
                Ok(())
            }
            "checkable" => {
                self.set_checkable(expect_bool(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            "icon_source" => {
                match value {
                    CapabilityValue::Null => self.set_icon_source(None),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                        name,
                    )) => self.set_icon_source(Some(IconSource::Builtin(name))),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                        ..
                    }) => return Err(CapabilityAccessError::ResourceResolutionRequired),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "command_id" => {
                match value {
                    CapabilityValue::Null => self.clear_command_id(),
                    other => self.set_command_id(expect_string(other)?),
                }
                Ok(())
            }
            "separator" => Err(CapabilityAccessError::ReadOnlyProperty),
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
            "icon_text",
            "shortcut",
            "checkable",
            "checked",
            "icon_source",
            "separator",
            "command_id",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `action` publishes.
    ///
    /// `trigger` is the genuine zero-argument action here: it delegates to the
    /// inner command and emits the widget-level `triggered` signal, exactly as a
    /// pointer activation does. `set_text`, `set_checkable` and `set_checked`
    /// assign state through the property route, so a payload-less call is refused as
    /// [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "trigger" => {
                self.trigger();
                Ok(())
            }
            "set_text" | "set_checkable" | "set_checked" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Action {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button, .. } if *button == 1 => self.trigger(),
            Event::KeyPress { key, modifiers } => {
                self.dispatch_shortcut(*key, *modifiers);
            }
            Event::MouseEnter { .. } => {
                // The base already recorded the hover; this arm only repaints and
                // announces it, so the flag cannot be set in two places.
                self.base.request_redraw();
                self.hovered.emit();
            }
            Event::MouseLeave { .. } if self.base.is_hovered() => {
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Action {
    /// Draws the action when it is rendered directly rather than by a host.
    ///
    /// # Why this is not simply a no-op
    ///
    /// A menu or toolbar normally draws its own items, so in that arrangement the
    /// action contributes nothing here and the host's row is what the user sees.
    /// That is the resting case: with no text, no icon and no hover the action
    /// paints nothing, which is unchanged behaviour.
    ///
    /// What must *not* stay unchanged is the case where an action is placed on a
    /// surface of its own — a standalone action, a hover highlight, or a separator.
    /// Those used to be drawn from literals, so a light/dark switch did nothing for
    /// them. They now resolve their chrome from the caller's explicit style first,
    /// then the theme's resolved style for this control, and only then a literal.
    ///
    /// The theme reads are separate manager locks, each taken and released inside
    /// `resolved_theme_style`, so none is held across the draw or across another
    /// accessor — the global manager's mutex is not re-entrant.
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // A bare, unhovered action is still owned by its host menu/toolbar: painting
        // its own surface here would fight the host's row background.
        if !self.base.is_hovered()
            && self.text.is_empty()
            && self.icon_text.is_empty()
            && self.icon_source.is_none()
        {
            return;
        }

        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("action");
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A hover highlight is the resolved ink damped to a faint wash; with no theme
        // resolve at all the fallback is a neutral tint rather than a literal that
        // could never move with the appearance.
        let highlight = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .map(|base| ink.blend(&base, 0.85))
            .unwrap_or(Color::rgba(0, 0, 0, 20));

        // A separator is a rule: the same resolved ink, drawn as a divider.
        if self.separator {
            let y = rect.y + rect.height as i32 / 2;
            context.draw_line(
                Point::new(rect.x + 4, y),
                Point::new(rect.x + rect.width as i32 - 4, y),
                ink.blend(&highlight, 0.6),
            );
            return;
        }

        if self.base.is_hovered() {
            context.face_with_gradient(
                rect,
                highlight,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
        }

        // The **effective font** — the resolved theme/caller font — so an action honours the theme
        // body font and the user's text scale instead of a hardcoded 13 px face (M-09).
        let font = effective_font(style);
        // One line box for the icon, the label, the check mark and the shortcut, all of which
        // share this row. `rect.height / 2` was the row's middle used as a glyph origin, so
        // every one of them sat half a line low (`action.svg` drew its 13 px label at y = 22 in
        // a 45 px row, where 16 is the centred value).
        let line = context.text_line(rect, font);
        let baseline = line.y;
        // The disabled tint is computed **before** any content is painted. It used to be
        // recomputed after the icon and label, so a disabled action kept full-strength text while
        // its check mark and shortcut were dimmed — the same row in two weights.
        let content_ink = self.base.disabled_ink_on(ink, highlight);
        let mut x = rect.x + 8;
        if !self.icon_visible_in_menu {
            // The row keeps the icon slot, so text stays aligned with sibling rows that
            // do show an icon, but the menu does not paint the glyph.
            if self.icon_source.is_some() || !self.icon_text.is_empty() {
                x += 20;
            }
        } else if let Some(source) = self.icon_source {
            let line_height = context.measure_text("M", font).height;
            let side = line_height.min(rect.height).max(1);
            let icon_rect = Rect::new(x, line.y, side, side);
            if let Err(error) =
                crate::widget::paint_icon(context, source.into(), icon_rect, content_ink)
            {
                log::warn!("action icon could not be painted: {error:?}");
            }
            x += 20;
        } else if !self.icon_text.is_empty() {
            context.draw_text(
                Point::new(x, baseline),
                &self.icon_text,
                font,
                content_ink,
                HorizontalAlignment::Left,
            );
            x += 20;
        }
        if !self.text.is_empty() {
            context.draw_text(
                Point::new(x, baseline),
                &self.text,
                font,
                content_ink,
                HorizontalAlignment::Left,
            );
        }
        if self.is_checked() && self.is_checkable() {
            // The tick is an `IconName` outline, not the `✓` (U+2713) character: no bundled face
            // covers the dingbats block, so the character fell back to an 8x8 bitmap and the check
            // read as a blob. An icon is geometry both backends agree on.
            let line_height = context.measure_text("M", font).height;
            let side = line_height.max(1);
            let check_rect = Rect::new(
                rect.x + rect.width as i32 - 16,
                baseline + (line_height as i32 - side as i32) / 2,
                side,
                side,
            );
            crate::widget::draw_icon_at(context, check_rect, content_ink, IconName::Check);
        }
        if !self.shortcut.is_empty() {
            let metrics = context.measure_text(&self.shortcut, font);
            context.draw_text(
                Point::new(rect.x + rect.width as i32 - metrics.width as i32 - 12, baseline),
                &self.shortcut,
                font,
                content_ink.blend(&highlight, 0.4),
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Action;
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    use crate::event::EventHandler;
    use crate::render::svg::SvgPaintBackend;
    use crate::render::PaintBackend;
    use crate::render::RenderContext;
    use crate::widget::{Draw, Widget, WidgetKind};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    fn rect() -> Rect {
        Rect::new(10, 20, 120, 28)
    }

    // ── 1. Creating default Action ──
    #[test]
    fn action_default_creation() {
        let a = Action::new("Hello", rect());
        assert_eq!(a.text(), "Hello");
        assert!(a.icon_text().is_empty());
        assert!(a.shortcut().is_empty());
        assert!(!a.is_separator());
        assert!(a.is_enabled());
        assert!(!a.is_checkable());
        assert!(!a.is_checked());
        assert_eq!(a.geometry(), rect());
    }

    // ── 2. Setting/getting text ──
    #[test]
    fn action_set_get_text() {
        let mut a = Action::new("Old", rect());
        a.set_text("New Text");
        assert_eq!(a.text(), "New Text");
        // Also verify the inner cmd text is updated
        assert_eq!(a.cmd().text, "New Text");
    }

    // ── 3. Setting/getting icon ──
    #[test]
    fn action_set_get_icon_text() {
        let mut a = Action::new("", rect());
        assert!(a.icon_text().is_empty());
        a.set_icon_text("🔍");
        assert_eq!(a.icon_text(), "🔍");
        let source = crate::widget::IconSource::builtin(crate::widget::IconName::Info);
        a.set_icon_source(Some(source));
        assert_eq!(a.icon_source(), Some(source));
        assert!(a.icon_text().is_empty(), "typed geometry does not masquerade as text");
        let svg = crate::widget::svg::render_to_svg(&mut a);
        assert!(svg.contains("<path"), "the typed source must reach the renderer");
        a.set_icon_text("★");
        assert_eq!(a.icon_source(), None, "legacy text setter restores text presentation");
        assert_eq!(a.icon_text(), "★");
    }

    // ── 4. Setting/getting tooltip ──
    #[test]
    fn action_set_get_tooltip() {
        let mut a = Action::new("Save", rect());
        a.set_tooltip("Save the current file".to_string());
        assert_eq!(a.tooltip(), "Save the current file");
    }

    // ── 5. Enabled/disabled state ──
    #[test]
    fn action_enabled_disabled_state() {
        let mut a = Action::new("Cmd", rect());
        assert!(a.is_enabled());
        a.set_enabled(false);
        assert!(!a.is_enabled());
        a.set_enabled(true);
        assert!(a.is_enabled());
    }

    // ── 6. Checkable mode ──
    #[test]
    fn action_checkable_mode() {
        let mut a = Action::new("Toggle", rect());
        assert!(!a.is_checkable());
        a.set_checkable(true);
        assert!(a.is_checkable());
        a.set_checkable(false);
        assert!(!a.is_checkable());
    }

    // ── 7. Checked state and toggling ──
    #[test]
    fn action_checked_state_and_toggling() {
        let mut a = Action::new("Toggle", rect());
        a.set_checkable(true);
        assert!(!a.is_checked());

        a.set_checked(true);
        assert!(a.is_checked());

        a.set_checked(false);
        assert!(!a.is_checked());
    }

    #[test]
    fn action_checkable_not_checkable_ignores_checked() {
        let mut a = Action::new("X", rect());
        // Not checkable; checked should always be false
        a.set_checked(true);
        assert!(!a.is_checked());
    }

    // ── 8. Signal accessors (triggered, toggled, changed) ──
    #[test]
    fn action_triggered_signal_fires() {
        let mut a = Action::new("Fire", rect());
        let triggered_count = Arc::new(AtomicUsize::new(0));
        let c = triggered_count.clone();
        a.triggered.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        a.trigger();
        assert_eq!(triggered_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn action_toggled_signal_fires() {
        let mut a = Action::new("Tog", rect());
        a.set_checkable(true);
        let last_val = Arc::new(AtomicBool::new(false));
        let v = last_val.clone();
        a.toggled.connect(move |checked| {
            v.store(*checked, Ordering::SeqCst);
        });
        a.set_checked(true);
        assert!(last_val.load(Ordering::SeqCst));
    }

    #[test]
    fn action_changed_signal_fires_on_text_change() {
        let mut a = Action::new("X", rect());
        let changed_count = Arc::new(AtomicUsize::new(0));
        let c = changed_count.clone();
        a.changed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        a.set_text("Y");
        assert!(changed_count.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn action_changed_signal_fires_on_checkable_change() {
        let mut a = Action::new("X", rect());
        let changed_count = Arc::new(AtomicUsize::new(0));
        let c = changed_count.clone();
        a.changed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        a.set_checkable(true);
        assert!(changed_count.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn action_hovered_signal_fires_on_mouse_enter() {
        let mut a = Action::new("X", rect());
        let hovered_count = Arc::new(AtomicUsize::new(0));
        let c = hovered_count.clone();
        a.hovered.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        a.handle_event(&Event::mouse_enter(60, 30));
        assert_eq!(hovered_count.load(Ordering::SeqCst), 1);

        // Mouse press must not emit hovered.
        a.handle_event(&Event::mouse_press(60, 30, 1));
        assert_eq!(hovered_count.load(Ordering::SeqCst), 1);
    }

    // ── 9. Widget ID and kind ──
    #[test]
    fn action_widget_id_and_kind() {
        let a = Action::new("X", rect());
        assert_eq!(a.kind(), WidgetKind::Action);
        // Each widget gets a unique id
        let a2 = Action::new("Y", rect());
        assert_ne!(a.id(), a2.id());
    }

    // ── 10. Geometry delegation ──
    #[test]
    fn action_geometry_delegation() {
        let mut a = Action::new("X", rect());
        assert_eq!(a.geometry(), rect());
        let new_rect = Rect::new(5, 5, 200, 40);
        a.set_geometry(new_rect);
        assert_eq!(a.geometry(), new_rect);
    }

    #[test]
    fn action_position_and_size() {
        let mut a = Action::new("X", Rect::new(10, 20, 100, 30));
        assert_eq!(a.position(), Point::new(10, 20));
        assert_eq!(a.size(), Size::new(100, 30));
        a.set_position(Point::new(30, 40));
        assert_eq!(a.position(), Point::new(30, 40));
        a.set_size(Size::new(200, 50));
        assert_eq!(a.size(), Size::new(200, 50));
    }

    // ── 11. SVG output ──
    #[test]
    fn action_svg_output() {
        let mut a = Action::new("Save", rect());
        let mut svg = SvgPaintBackend::new(Size::new(200, 50));
        svg.begin_frame(Color::WHITE);
        let mut rc = RenderContext::new(&mut svg);
        a.draw(&mut rc);
        svg.end_frame();
        let output = svg.finish();
        // Action draw is a no-op but should produce valid SVG
        assert!(output.contains("<svg"), "SVG output should contain svg tag");
    }

    // ── 12. Keyboard shortcut binding ──
    #[test]
    fn action_shortcut_binding() {
        let mut a = Action::new("Copy", rect());
        assert!(a.shortcut().is_empty());
        a.set_shortcut("Ctrl+C");
        assert_eq!(a.shortcut(), "Ctrl+C");
    }

    // ── 13. Separator action ──
    #[test]
    fn action_separator_creation() {
        let a = Action::separator(rect());
        assert!(a.is_separator());
        assert!(a.text().is_empty());
    }

    // ── 14. Command id delegation ──
    #[test]
    fn action_command_id() {
        let mut a = Action::new("X", rect());
        assert!(a.command_id().is_none());
        a.set_command_id("file.save");
        assert_eq!(a.command_id(), Some("file.save"));
        a.clear_command_id();
        assert!(a.command_id().is_none());
    }

    // ── 15. Disabled state blocks event ──
    #[test]
    fn action_disabled_blocks_trigger() {
        let mut a = Action::new("X", rect());
        a.set_enabled(false);
        let triggered = Arc::new(AtomicBool::new(false));
        let t = triggered.clone();
        a.triggered.connect(move |_| {
            t.store(true, Ordering::SeqCst);
        });
        // Manually trigger should still work (trigger is not gated by base)
        // but handle_event should be blocked
        a.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1, modifiers: 0 });
        // When disabled, handle_event should return early before trigger()
        // We check that triggered signal was not emitted
        assert!(
            !triggered.load(Ordering::SeqCst),
            "Disabled action should not trigger on mouse press"
        );
    }

    // ── 16. Trigger on checkable toggles checked state ──
    #[test]
    fn action_trigger_toggles_checkable() {
        let mut a = Action::new("Tog", rect());
        a.set_checkable(true);
        assert!(!a.is_checked());
        a.trigger();
        assert!(a.is_checked());
        a.trigger();
        assert!(!a.is_checked());
    }

    #[test]
    fn menu_role_and_help_metadata_round_trip() {
        use super::MenuRole;
        let mut a = Action::new("About", rect());
        assert_eq!(a.menu_role(), MenuRole::None);
        assert!(a.is_icon_visible_in_menu(), "menus show icons unless told otherwise");
        assert!(a.status_tip().is_empty());
        assert!(a.whats_this().is_empty());

        let changed = Arc::new(AtomicUsize::new(0));
        let c = changed.clone();
        a.changed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        a.set_menu_role(MenuRole::About);
        assert_eq!(a.menu_role(), MenuRole::About);
        a.set_menu_role(MenuRole::About);
        assert_eq!(
            changed.load(Ordering::SeqCst),
            1,
            "an unchanged role must not announce a change"
        );

        a.set_icon_visible_in_menu(false);
        assert!(!a.is_icon_visible_in_menu());
        let before = changed.load(Ordering::SeqCst);
        a.set_icon_visible_in_menu(false);
        assert_eq!(changed.load(Ordering::SeqCst), before, "no-op visibility must stay quiet");

        a.set_status_tip("Version information");
        a.set_whats_this("Shows the application version");
        assert_eq!(a.status_tip(), "Version information");
        assert_eq!(a.whats_this(), "Shows the application version");
    }

    #[test]
    fn a_bound_shortcut_fires_the_action_and_labels_itself() {
        use crate::shortcut::{Key, Shortcut};
        let mut a = Action::new("Save", rect());
        a.set_shortcut_key(Shortcut::primary(Key::S));
        assert!(!a.shortcut().is_empty(), "the label follows the binding");
        assert_eq!(a.shortcut_key(), Some(&Shortcut::primary(Key::S)));

        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        a.triggered.connect(move |_| {
            f.fetch_add(1, Ordering::SeqCst);
        });

        // 83 is 'S'; bit 3 is the primary accelerator in the event convention.
        a.handle_event(&Event::KeyPress { key: 83, modifiers: 0 });
        assert_eq!(fired.load(Ordering::SeqCst), 0, "the modifier is part of the chord");
        a.handle_event(&Event::KeyPress { key: 83, modifiers: 0b1000 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "the primary+S chord must fire");
        a.handle_event(&Event::KeyPress { key: 84, modifiers: 0b1000 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "a different key must not fire");

        a.clear_shortcut_key();
        assert!(a.shortcut_key().is_none());
        assert!(a.shortcut().is_empty());
        a.handle_event(&Event::KeyPress { key: 83, modifiers: 0b1000 });
        assert_eq!(fired.load(Ordering::SeqCst), 1, "a cleared binding stops firing");
    }

    #[test]
    fn an_exclusive_group_keeps_one_member_checked() {
        use super::ActionGroup;
        let mut group = ActionGroup::exclusive("alignment");
        let mut left = Action::new("Left", rect());
        left.set_checkable(true);
        let mut center = Action::new("Center", rect());
        center.set_checkable(true);
        let mut right = Action::new("Right", rect());
        right.set_checkable(true);
        group.add(left);
        group.add(center);
        group.add(right);

        group.set_checked(0, true);
        assert_eq!(group.checked_index(), Some(0));
        group.set_checked(2, true);
        assert_eq!(group.checked_index(), Some(2), "checking a sibling must clear the first");
        assert_eq!(group.get(0).map(Action::is_checked), Some(false));

        group.trigger(2);
        assert_eq!(group.checked_index(), None, "triggering the checked member toggles it off");

        group.set_checked(9, true);
        assert_eq!(group.checked_index(), None, "an out-of-range index is ignored");
    }

    #[test]
    fn a_non_exclusive_group_lets_members_accumulate() {
        use super::ActionGroup;
        let mut group = ActionGroup::new("filters");
        assert!(!group.is_exclusive());
        let mut a = Action::new("A", rect());
        a.set_checkable(true);
        let mut b = Action::new("B", rect());
        b.set_checkable(true);
        group.add(a);
        group.add(b);
        group.set_checked(0, true);
        group.set_checked(1, true);
        assert_eq!(group.get(0).map(Action::is_checked), Some(true));
        assert_eq!(group.get(1).map(Action::is_checked), Some(true));

        group.set_exclusive(true);
        assert_eq!(group.checked_index(), Some(0), "the first checked member keeps its state");
        assert_eq!(group.get(1).map(Action::is_checked), Some(false));
        assert_eq!(group.len(), 2);
    }
}
