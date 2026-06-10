//! Button widget implementation.
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
#[cfg(not(feature = "mini"))]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Button interaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Pressed,
    Disabled,
}
/// Button widget for clickable actions.
pub struct Button {
    base: BaseWidget,
    text: String,
    #[cfg(not(feature = "mini"))]
    icon: Option<Image>,
    pressed: bool,
    default_button: bool,
    focused: bool,
    hovered: bool,
    pub pressed_signal: GenericSignal,
    pub released_signal: GenericSignal,
    pub state_changed: Signal1<ButtonState>,
}
impl Button {
    /// Creates a button with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Button, geometry, "Button"),
            text,
            #[cfg(not(feature = "mini"))]
            icon: None,
            pressed: false,
            default_button: false,
            focused: false,
            hovered: false,
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
        }
    }
    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns current button interaction state.
    pub fn state(&self) -> ButtonState {
        if !self.base.is_enabled() {
            ButtonState::Disabled
        } else if self.pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Normal
        }
    }
    /// Returns whether button is in pressed state.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }
    /// Sets pressed state and emits transition signals when changed.
    pub fn set_pressed(&mut self, pressed: bool) {
        if !self.base.is_enabled() {
            return;
        }
        if self.pressed == pressed {
            return;
        }
        self.pressed = pressed;
        if pressed {
            self.pressed_signal.emit();
        } else {
            self.released_signal.emit();
        }
        self.state_changed.emit(self.state());
    }
    pub fn press(&mut self) {
        self.set_pressed(true);
    }
    pub fn release(&mut self) {
        self.set_pressed(false);
    }
    /// Enables/disables button while preserving deterministic state transitions.
    pub fn set_enabled_state(&mut self, enabled: bool) {
        let previous = self.state();
        self.base.set_enabled(enabled);
        if !enabled {
            self.pressed = false;
        }
        let current = self.state();
        if previous != current {
            self.state_changed.emit(current);
        }
    }
    /// Sets button text.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.base.request_redraw();
    }
    /// Sets the icon displayed on the button.
    #[cfg(not(feature = "mini"))]
    pub fn set_icon(&mut self, icon: Image) {
        self.icon = Some(icon);
        self.base.request_redraw();
    }
    /// Returns a reference to the button icon, if set.
    #[cfg(not(feature = "mini"))]
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    /// Sets whether this button is the default button.
    pub fn set_default(&mut self, default: bool) {
        self.default_button = default;
        self.base.request_redraw();
    }

    /// Returns whether this button is the default button.
    pub fn is_default(&self) -> bool {
        self.default_button
    }

    /// Programmatically clicks the button (press, release, emit clicked signal).
    pub fn click(&mut self) {
        if !self.base.is_enabled() {
            return;
        }
        self.press();
        self.release();
        self.base.clicked.emit();
    }
}
impl Widget for Button {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.set_enabled_state(enabled);
    }

    fn size_hint(&self) -> Size {
        // Approximate: text length * ~8px + padding, minimum 75x28
        let text_w = self.text().len() as u32 * 8 + 20;
        Size::new(text_w.max(75), 28)
    }
}
impl EventHandler for Button {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MouseDown((_, _)) if self.base.is_enabled() => {
                self.press();
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } if self.base.is_enabled() => {
                self.press();
            }
            Event::MouseUp((_, _)) if self.pressed => {
                self.release();
                self.base.clicked.emit();
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } if self.pressed => {
                self.release();
                self.base.clicked.emit();
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } if self.base.is_enabled() => {
                self.base.clicked.emit();
                self.state_changed.emit(self.state());
            }
            Event::FocusGained => {
                self.focused = true;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.base.request_redraw();
            }
            Event::KeyPress { key, .. } if (*key == 13 || *key == 32) && self.base.is_enabled() => {
                self.click();
            }
            Event::MouseEnter { .. } => {
                self.hovered = true;
                self.base.request_redraw();
            }
            Event::MouseLeave { .. } => {
                self.hovered = false;
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Button {
    fn draw(&mut self, context: &mut RenderContext) {
        // Button rendering with style integration (BLUE13 R1.3)
        // Reads style fields (background_color, text_color, border_color, etc.)
        // falling back to state-based defaults when style values are not set.
        let rect = self.geometry();
        let state = self.state();
        let style = self.style();

        // ── Background ──
        let bg = style.background_color.unwrap_or_else(|| match state {
            ButtonState::Normal => Color::from_rgb(240, 240, 240),
            ButtonState::Pressed => Color::from_rgb(200, 200, 200),
            ButtonState::Disabled => Color::from_rgb(220, 220, 220),
        });
        if style.border_radius > 0 {
            context.fill_rounded_rect(rect, style.border_radius, bg);
        } else {
            context.fill_rect(rect, bg);
        }

        // ── Border ──
        if let Some(border_color) = style.border_color {
            if style.border_radius > 0 && style.border_width > 0 {
                context.draw_rounded_rect_stroke(
                    rect,
                    style.border_radius,
                    border_color,
                    style.border_width,
                );
            } else if style.border_width > 0 {
                context.draw_rect_stroke(rect, border_color, style.border_width);
            }
        }

        // ── Text ──
        if !self.text.is_empty() {
            let text_color = style.text_color.unwrap_or_else(|| {
                if state == ButtonState::Disabled {
                    Color::from_rgb(150, 150, 150)
                } else {
                    Color::from_rgb(0, 0, 0)
                }
            });
            let font = style.font.clone().unwrap_or_default();
            context.draw_text(
                Point { x: rect.x + 6, y: rect.y + rect.height as i32 / 2 },
                &self.text,
                &font,
                text_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    #[cfg(not(feature = "mini"))]
    use crate::widget::Image;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // ── Helper ─────────────────────────────────────────────────────────
    fn make_button() -> Button {
        Button::new("Click".into(), Rect::new(10, 20, 120, 36))
    }

    #[cfg(not(feature = "mini"))]
    fn make_image() -> Image {
        Image {
            data: vec![0u8; 64],
            format: crate::widget::ImageFormat::Rgba8,
            width: 8,
            height: 8,
        }
    }

    fn rect() -> Rect {
        Rect::new(10, 20, 120, 36)
    }

    // ── 1. Button creation ─────────────────────────────────────────────
    #[cfg(not(feature = "mini"))]
    #[test]
    fn button_creation_text_geometry_defaults_icon() {
        let b = make_button();
        assert_eq!(b.text(), "Click");
        assert_eq!(b.geometry(), rect());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());
        assert!(!b.is_default());
        assert!(b.icon().is_none());
    }

    // ── 2. State transitions ───────────────────────────────────────────
    #[test]
    fn state_transition_normal_pressed_released() {
        let mut b = make_button();
        assert_eq!(b.state(), ButtonState::Normal);

        b.press();
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);

        b.release();
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    #[test]
    fn state_transition_idempotent_press_release_noop() {
        let mut b = make_button();
        // Already Normal → release is a no-op
        b.release();
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());

        // Press once
        b.press();
        // Second press is a no-op
        b.press();
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);

        // Release once
        b.release();
        // Second release is a no-op
        b.release();
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    // ── 3. Signal emission ─────────────────────────────────────────────
    #[test]
    fn signal_press_emits_pressed_and_state_changed() {
        let mut b = make_button();
        let pressed_fired = Arc::new(AtomicBool::new(false));
        let changed_fired = Arc::new(AtomicBool::new(false));
        b.pressed_signal.connect({
            let flag = Arc::clone(&pressed_fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&changed_fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press();

        assert!(pressed_fired.load(Ordering::SeqCst));
        assert!(changed_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn signal_release_emits_released_and_state_changed() {
        let mut b = make_button();
        let released_fired = Arc::new(AtomicBool::new(false));
        let changed_fired = Arc::new(AtomicBool::new(false));
        b.released_signal.connect({
            let flag = Arc::clone(&released_fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&changed_fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press(); // Press first
        released_fired.store(false, Ordering::SeqCst);
        changed_fired.store(false, Ordering::SeqCst);

        b.release(); // Then release

        assert!(released_fired.load(Ordering::SeqCst));
        assert!(changed_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn signal_no_emission_on_noop_transition() {
        let mut b = make_button();
        let fired = Arc::new(AtomicBool::new(false));
        b.pressed_signal.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.released_signal.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        // No-op: Normal → Normal
        b.release();
        assert!(!fired.load(Ordering::SeqCst));

        // Press → Pressed
        b.press();
        fired.store(false, Ordering::SeqCst);

        // No-op: Pressed → Pressed
        b.press();
        assert!(!fired.load(Ordering::SeqCst));
    }

    // ── 4. Disabled state ──────────────────────────────────────────────
    #[test]
    fn disabled_prevents_transitions() {
        let mut b = make_button();
        b.set_enabled_state(false);
        assert_eq!(b.state(), ButtonState::Disabled);

        // Press is ignored when disabled
        b.press();
        assert_eq!(b.state(), ButtonState::Disabled);
        assert!(!b.is_pressed());

        // Release is ignored when disabled
        b.release();
        assert_eq!(b.state(), ButtonState::Disabled);
    }

    #[test]
    fn re_enable_restores_normal_state() {
        let mut b = make_button();
        b.set_enabled_state(false);
        assert_eq!(b.state(), ButtonState::Disabled);

        b.set_enabled_state(true);
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());
    }

    // ── 5. Text update and icon ────────────────────────────────────────
    #[test]
    fn set_text_updates_text() {
        let mut b = make_button();
        assert_eq!(b.text(), "Click");

        b.set_text("OK".into());
        assert_eq!(b.text(), "OK");
    }

    #[test]
    fn set_empty_text() {
        let mut b = make_button();
        b.set_text("".into());
        assert_eq!(b.text(), "");
        assert!(b.text().is_empty());
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn set_icon_and_default_icon() {
        let mut b = make_button();
        assert!(b.icon().is_none());

        let img = make_image();
        b.set_icon(img);
        assert!(b.icon().is_some());
    }

    // ── 6. Default property ────────────────────────────────────────────
    #[test]
    fn default_property() {
        let mut b = make_button();
        assert!(!b.is_default());

        b.set_default(true);
        assert!(b.is_default());

        b.set_default(false);
        assert!(!b.is_default());
    }

    // ── 7. Event handling ──────────────────────────────────────────────
    #[test]
    fn focus_gained_sets_focused_flag() {
        let mut btn = make_button();
        assert!(!btn.focused, "Button should not be focused by default");
        btn.handle_event(&Event::FocusGained);
        assert!(btn.focused, "Button should be focused after FocusGained");
    }

    #[test]
    fn focus_lost_clears_focused_flag() {
        let mut btn = make_button();
        btn.handle_event(&Event::FocusGained);
        assert!(btn.focused);
        btn.handle_event(&Event::FocusLost);
        assert!(!btn.focused, "Button should not be focused after FocusLost");
    }

    #[test]
    fn key_press_enter_triggers_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst), "Enter key should trigger click");
    }

    #[test]
    fn key_press_space_triggers_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst), "Space key should trigger click");
    }

    #[test]
    fn key_press_other_key_does_not_trigger_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        assert!(!clicked.load(Ordering::SeqCst), "Non-Enter/Space key should NOT trigger click");
    }

    #[test]
    fn mouse_enter_sets_hovered() {
        let mut btn = make_button();
        assert!(!btn.hovered);
        btn.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(btn.hovered);
    }

    #[test]
    fn mouse_leave_clears_hovered() {
        let mut btn = make_button();
        btn.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(btn.hovered);
        btn.handle_event(&Event::MouseLeave { pos: Point::new(10, 10) });
        assert!(!btn.hovered);
    }

    #[test]
    fn event_mouse_down_presses_button() {
        let mut b = make_button();
        let event = Event::MouseDown((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);
    }

    #[test]
    fn event_mouse_up_releases_and_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        // Must be pressed first
        b.press();
        assert!(b.is_pressed());

        let event = Event::MouseUp((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_begin_presses() {
        let mut b = make_button();
        let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_end_releases_and_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press();
        let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_tap_triggers_click() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        let event = Event::Tap { pos: Point::new(15, 25) };
        b.handle_event(&event);
        assert!(clicked.load(Ordering::SeqCst));
        // Tap does not change pressed state
        assert!(!b.is_pressed());
    }

    #[test]
    fn event_disabled_ignores_mouse_down() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::MouseDown((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Disabled);
    }

    #[test]
    fn event_disabled_ignores_mouse_up() {
        let mut b = make_button();
        b.set_enabled_state(false);
        // Even if somehow pressed, disabled handle_event should not process MouseUp
        let event = Event::MouseUp((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_tap() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        let event = Event::Tap { pos: Point::new(15, 25) };
        b.handle_event(&event);
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_touch_begin() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_touch_end() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    // ── 8. Widget trait delegation ─────────────────────────────────────
    #[test]
    fn widget_trait_id_and_kind() {
        let b = make_button();
        assert!(b.id() != 0);
        assert_eq!(b.kind(), WidgetKind::Button);
    }

    #[test]
    fn widget_trait_geometry() {
        let mut b = make_button();
        assert_eq!(b.geometry(), rect());
        let new_rect = Rect::new(0, 0, 200, 50);
        b.set_geometry(new_rect);
        assert_eq!(b.geometry(), new_rect);
    }

    #[test]
    fn widget_trait_visibility() {
        let mut b = make_button();
        assert!(b.is_visible());
        b.hide();
        assert!(!b.is_visible());
        b.show();
        assert!(b.is_visible());
    }

    #[test]
    fn widget_trait_enabled() {
        let mut b = make_button();
        assert!(b.is_enabled());
        b.set_enabled(false);
        assert!(!b.is_enabled());
        assert_eq!(b.state(), ButtonState::Disabled);
        b.set_enabled(true);
        assert!(b.is_enabled());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    #[test]
    fn widget_trait_parent_and_children() {
        let mut b = make_button();
        assert!(b.parent().is_none());
        assert!(b.children().is_empty());

        let child: crate::core::ObjectId = 42;
        let parent: crate::core::ObjectId = 99;

        b.set_parent(Some(parent));
        assert_eq!(b.parent(), Some(parent));

        b.add_child(child);
        assert_eq!(b.children(), &[child]);

        b.remove_child(child);
        assert!(b.children().is_empty());
    }

    #[test]
    fn widget_trait_min_max_size() {
        let mut b = make_button();
        assert!(b.min_size().is_none());
        assert!(b.max_size().is_none());

        b.set_min_size(Some(Size::new(80, 24)));
        assert_eq!(b.min_size(), Some(Size::new(80, 24)));

        b.set_max_size(Some(Size::new(400, 200)));
        assert_eq!(b.max_size(), Some(Size::new(400, 200)));
    }

    #[test]
    fn widget_trait_tooltip() {
        let mut b = make_button();
        assert_eq!(b.tooltip(), "");
        b.set_tooltip("Save".into());
        assert_eq!(b.tooltip(), "Save");
    }

    #[test]
    fn widget_trait_style() {
        let mut b = make_button();
        let default_style = b.style().clone();
        // Verify we can set a modified style via builder pattern.
        let new_style = default_style.clone().with_background(Color::from_rgb(255, 0, 0));
        b.set_style(new_style.clone());
        assert_eq!(b.style().background_color, new_style.background_color);
    }

    #[test]
    fn widget_trait_signals() {
        let b = make_button();
        // Verify signals are accessible via Widget trait
        let _ = b.hover_signal();
        let _ = b.mouse_down_signal();
        let _ = b.mouse_up_signal();
        let _ = b.key_down_signal();
        let _ = b.key_up_signal();
        let _ = b.focus_gained_signal();
        let _ = b.focus_lost_signal();
        let _ = b.redraw_requested_signal();
        let _ = b.layout_requested_signal();
    }

    #[test]
    fn widget_trait_connection_scope() {
        let b = make_button();
        let _scope = b.connection_scope();
    }

    // ── 11. Visibility toggling ────────────────────────────────────────
    #[test]
    fn test_button_visibility_toggle() {
        let mut b = make_button();
        assert!(b.is_visible());

        // Single hide
        b.hide();
        assert!(!b.is_visible());

        // Double hide is idempotent
        b.hide();
        assert!(!b.is_visible());

        // Show restores
        b.show();
        assert!(b.is_visible());

        // Double show is idempotent
        b.show();
        assert!(b.is_visible());

        // Multiple toggle cycles
        for _ in 0..3 {
            b.hide();
            assert!(!b.is_visible());
            b.show();
            assert!(b.is_visible());
        }
    }

    #[test]
    fn test_button_zero_geometry() {
        // Zero width/height — should not panic
        let _b = Button::new("Zero".into(), Rect::new(0, 0, 0, 0));

        // Negative position — should not panic
        let _b = Button::new("Neg".into(), Rect::new(-10, -20, 100, 30));

        // Zero geometry with negative position — should not panic
        let _b = Button::new("All".into(), Rect::new(-5, -5, 0, 0));
    }

    #[test]
    fn test_button_signal_not_emitted_on_same_value() {
        let mut b = make_button();
        let changed_count = Arc::new(AtomicBool::new(false));
        let c = Arc::clone(&changed_count);
        b.base.changed.connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        // set_text does NOT emit base.changed — it only calls request_redraw()
        b.set_text("Click".into());
        assert!(
            !changed_count.load(Ordering::SeqCst),
            "set_text with same value should not emit changed"
        );

        // Even setting different text does not emit base.changed
        b.set_text("Different".into());
        assert!(
            !changed_count.load(Ordering::SeqCst),
            "set_text with different value should not emit changed"
        );
    }
}
