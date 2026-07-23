//! Action widget — represents a command or toggle that can be placed in menus and toolbars.
//!
//! This widget **internally wraps** [`crate::action::Action`] to eliminate duplicate
//! `checkable`/`checked`/`triggered`/`toggled` logic. All command state lives in
//! the inner `action::Action`, while this struct adds widget-only fields
//! (`icon_text`, `shortcut`, `separator`) and `BaseWidget` integration.
use crate::action::Action as CmdAction;
use crate::core::Rect;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionHandle, GenericSignal, Signal1};
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
            text,
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
        self.base.request_redraw();
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
        self.base.request_redraw();
    }
    pub fn set_icon_text(&mut self, text: impl Into<String>) {
        self.icon_text = text.into();
        self.changed.emit();
        self.base.request_redraw();
    }
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
}
impl Widget for Action {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(100, 28)
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
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Action {
    fn draw(&mut self, _context: &mut RenderContext) {
        // Actions are drawn by their parent menu/toolbar, not directly.
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
        a.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1 });
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
}
