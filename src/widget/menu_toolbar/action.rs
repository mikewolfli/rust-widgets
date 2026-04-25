//! Action widget — represents a command or toggle that can be placed in menus and toolbars.
//!
//! This widget **internally wraps** [`crate::action::Action`] to eliminate duplicate
//! `checkable`/`checked`/`triggered`/`toggled` logic. All command state lives in
//! the inner `action::Action`, while this struct adds widget-only fields
//! (`icon_text`, `shortcut`, `separator`) and `BaseWidget` integration.
use crate::action::Action as CmdAction;
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionHandle, ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
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
    /// Display shortcut string.
    shortcut: String,
    /// Whether this action is a visual separator.
    separator: bool,
    pub triggered: Signal1<bool>,
    pub toggled: Signal1<bool>,
    pub hovered: GenericSignal,
    pub changed: GenericSignal,
    // Connection handles to keep inner cmd signals wired.
    _toggled_handle: Option<ConnectionHandle>,
    _enabled_handle: Option<ConnectionHandle>,
}
impl Action {
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        let text = text.into();
        let mut action = Self {
            base: BaseWidget::new(WidgetKind::Action, geometry, "Action"),
            cmd: CmdAction::new("", &text),
            text: text.clone(),
            icon_text: String::new(),
            shortcut: String::new(),
            separator: false,
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
    pub fn separator(geometry: Rect) -> Self {
        let mut a = Self::new("", geometry);
        a.separator = true;
        a
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn icon_text(&self) -> &str {
        &self.icon_text
    }
    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }
    /// Delegates to inner [`CmdAction::is_checkable`].
    pub fn is_checkable(&self) -> bool {
        self.cmd.is_checkable()
    }
    /// Delegates to inner [`CmdAction::is_checked`].
    pub fn is_checked(&self) -> bool {
        self.cmd.is_checked()
    }
    pub fn is_separator(&self) -> bool {
        self.separator
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
    pub fn set_command_id(&mut self, id: impl Into<String>) {
        self.cmd.id = id.into();
    }
    /// Clears the link to the command action.
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
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cmd.text = self.text.clone();
        self.changed.emit();
    }
    pub fn set_icon_text(&mut self, text: impl Into<String>) {
        self.icon_text = text.into();
        self.changed.emit();
    }
    pub fn set_shortcut(&mut self, shortcut: impl Into<String>) {
        self.shortcut = shortcut.into();
        self.changed.emit();
    }
    /// Delegates to inner [`CmdAction::set_checkable`].
    pub fn set_checkable(&mut self, checkable: bool) {
        self.cmd.set_checkable(checkable);
        self.changed.emit();
    }
    /// Delegates to inner [`CmdAction::set_checked`].
    pub fn set_checked(&mut self, checked: bool) {
        let old = self.cmd.is_checked();
        self.cmd.set_checked(checked);
        if self.cmd.is_checked() != old {
            self.toggled.emit(self.cmd.is_checked());
            self.changed.emit();
        }
    }
    /// Delegates to inner [`CmdAction::trigger`] and emits widget `triggered`.
    pub fn trigger(&mut self) {
        if self.cmd.trigger() {
            // cmd.trigger() already handles checkable toggle + emits its own triggered/toggled.
            // We emit widget-level triggered with current checked state.
            self.triggered.emit(self.cmd.is_checked());
        }
    }
}
impl Widget for Action {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
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
            _ => {}
        }
    }
}
impl Draw for Action {
    fn draw(&mut self, _context: &mut RenderContext) {
        // Actions are drawn by their parent menu/toolbar, not directly.
    }
}
