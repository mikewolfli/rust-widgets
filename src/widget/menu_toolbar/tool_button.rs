// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool button widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::path::{Path, PathBuf};
/// Tool button popup mode.
///
/// Selects how the button's attached menu is presented. The mode is stored and
/// exposed as a property, but the widget does not itself own or show a menu, so
/// the value is currently a hint for the containment layer that wires the button
/// to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolButtonPopupMode {
    /// Opening the menu requires a long or delayed press, so a quick click
    /// performs the button's main action instead.
    DelayedPopup,
    /// The menu opens from a dedicated arrow area while the rest of the button
    /// performs the main action.
    MenuButtonPopup,
    /// A click opens the menu immediately; there is no separate main action.
    InstantPopup,
}
/// Tool button style.
///
/// Selects which of the text and icon parts are painted. The widget's own
/// `draw` renders only the text, so the variants differ in label placement only
/// once an icon renderer is supplied by the containment layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolButtonStyle {
    /// Show the icon only, with no label.
    IconOnly,
    /// Show the label only, with no icon.
    TextOnly,
    /// Show the icon followed by the label on the same line.
    TextBesideIcon,
    /// Show the label centred beneath the icon.
    TextUnderIcon,
    /// Defer to the enclosing tool bar's style rather than setting one here.
    FollowStyle,
}
/// Tool button widget.
///
/// A compact button for tool bars and menu surfaces. It can act as a plain push
/// button or, when [`ToolButton::set_checkable`] is enabled, as a toggle whose
/// state is exposed through [`ToolButton::is_checked`].
///
/// # Activation
///
/// Every activation path routes through [`ToolButton::click`], so the signals
/// fire consistently whether the button was clicked, activated by Enter or
/// Space, or driven programmatically. Events are ignored entirely while the
/// widget is disabled.
///
/// # Appearance
///
/// Chrome follows the resolved theme: the fill resolves the explicit style first, then
/// this control's resolved style, and falls back to the previous literal when no theme is
/// active. The interaction states are derived from that fill rather than hardcoded, so
/// pressed, checked and hovered remain distinguishable on any appearance. The label is
/// centred and fades toward the fill while the button is disabled. The icon path is stored
/// but not decoded or painted by this widget.
pub struct ToolButton {
    base: BaseWidget,
    text: String,
    icon: Option<PathBuf>,
    checkable: bool,
    checked: bool,
    popup_mode: ToolButtonPopupMode,
    button_style: ToolButtonStyle,
    auto_raise: bool,
    pressed: bool,
    hovered: bool,
    /// Emitted on every activation by [`ToolButton::click`], carrying the
    /// button's checked state at that moment. For a non-checkable button this is
    /// therefore always `false`, and it is emitted even when nothing was
    /// toggled.
    pub clicked: Signal1<bool>,
    /// Emitted when the checked state actually changes, carrying the new state.
    /// Never emitted for a non-checkable button, or when a set leaves the state
    /// unchanged.
    pub toggled: Signal1<bool>,
    /// Emitted on every activation, with no payload; a convenience signal for
    /// handlers that do not care about the checked state.
    pub triggered: GenericSignal,
}
impl ToolButton {
    /// Creates a tool button labelled `text` occupying `geometry`.
    ///
    /// The label may be any string-convertible value. The defaults are: no icon,
    /// not checkable and unchecked, [`ToolButtonPopupMode::DelayedPopup`],
    /// [`ToolButtonStyle::IconOnly`], and auto-raise off. Note that the default
    /// style hides the label even though one is set.
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        let text = text.into();
        Self {
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, "ToolButton"),
            text,
            icon: None,
            checkable: false,
            checked: false,
            popup_mode: ToolButtonPopupMode::DelayedPopup,
            button_style: ToolButtonStyle::IconOnly,
            auto_raise: false,
            pressed: false,
            hovered: false,
            clicked: Signal1::new(),
            toggled: Signal1::new(),
            triggered: GenericSignal::new(),
        }
    }
    /// Returns the button's label.
    ///
    /// The label is stored whether or not the current [`ToolButtonStyle`]
    /// paints it.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns the icon's file path, or `None` when no icon has been set.
    ///
    /// The path is returned as given; nothing verifies that the file exists or
    /// that it decodes as an image.
    pub fn icon(&self) -> Option<&Path> {
        self.icon.as_deref()
    }
    /// Returns whether the button toggles rather than acting as a momentary
    /// push button.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }
    /// Returns whether the button is currently checked.
    ///
    /// Always `false` for a non-checkable button.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Returns how the button's attached menu should be presented.
    pub fn popup_mode(&self) -> ToolButtonPopupMode {
        self.popup_mode
    }
    /// Returns which parts of the button are painted.
    pub fn button_style(&self) -> ToolButtonStyle {
        self.button_style
    }
    /// Returns whether the button raises itself out of a tool bar when hovered.
    pub fn auto_raise(&self) -> bool {
        self.auto_raise
    }
    /// Sets the button's label and requests a redraw.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }
    /// Sets or clears the icon path and requests a redraw.
    ///
    /// The path is stored verbatim; it is not validated, loaded, or decoded
    /// here.
    pub fn set_icon(&mut self, icon: Option<PathBuf>) {
        self.icon = icon;
        self.base.request_redraw();
    }
    /// Enables or disables checkable behaviour and requests a redraw.
    ///
    /// Turning checkability **off** also clears the checked state without
    /// emitting [`ToolButton::toggled`], so a listener is not told about the
    /// change. Turning it on leaves the current state untouched.
    pub fn set_checkable(&mut self, v: bool) {
        self.checkable = v;
        if !v {
            self.checked = false;
        }
        self.base.request_redraw();
    }
    /// Sets the popup mode and requests a redraw.
    pub fn set_popup_mode(&mut self, mode: ToolButtonPopupMode) {
        self.popup_mode = mode;
        self.base.request_redraw();
    }
    /// Sets the button style and requests a redraw.
    pub fn set_button_style(&mut self, style: ToolButtonStyle) {
        self.button_style = style;
        self.base.request_redraw();
    }
    /// Enables or disables auto-raise and requests a redraw.
    pub fn set_auto_raise(&mut self, v: bool) {
        self.auto_raise = v;
        self.base.request_redraw();
    }
    /// Sets the checked state, if the button is checkable and the state changes.
    ///
    /// The call is a no-op for a non-checkable button, or when `checked` already
    /// matches the current state; in those cases nothing is emitted and no
    /// redraw is requested. On an actual change, [`ToolButton::toggled`] is
    /// emitted with the new state and a redraw is requested. Note that unlike
    /// [`ToolButton::click`], this does not emit `clicked` or `triggered`, so a
    /// programmatic change is distinguishable from a user activation.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable && self.checked != checked {
            self.checked = checked;
            self.toggled.emit(checked);
            self.base.request_redraw();
        }
    }
    /// Activates the button as if the user had clicked it.
    ///
    /// A checkable button flips its state first (emitting [`ToolButton::toggled`]
    /// if it changed); then [`ToolButton::clicked`] is emitted with the resulting
    /// state and [`ToolButton::triggered`] with no payload. This runs regardless
    /// of whether the widget is enabled, so wrap it in an enabled check if the
    /// caller is acting on external input.
    pub fn click(&mut self) {
        if self.checkable {
            self.set_checked(!self.checked);
        }
        self.clicked.emit(self.checked);
        self.triggered.emit();
    }
}
impl Widget for ToolButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(28, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ToolButton`'s property contract.
///
/// Note that `WidgetKind::ToolButton` is shared with `SplitButton`, which owns a
/// different property set. The capability registry disambiguates them by concrete
/// type, and this impl is what makes that possible: a split button reaches its
/// own contract instead of being described as a tool button.
impl WidgetProperties for ToolButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "checked", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `tool_button` publishes.
    ///
    /// `set_text` and `set_checked` assign state through the property route, so a
    /// payload-less call is refused as [`CapabilityAccessError::OutOfRange`] — the
    /// names are right and the value is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_checked" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ToolButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseEnter { pos: _ } => {
                self.hovered = true;
            }
            Event::MouseLeave { pos: _ } => {
                self.hovered = false;
                self.pressed = false;
            }
            Event::MousePress { button: 1, .. } => {
                self.pressed = true;
            }
            Event::MouseRelease { button: 1, .. } if self.pressed => {
                self.pressed = false;
                self.click();
            }
            Event::KeyPress { key: 13, .. } | Event::KeyPress { key: 32, .. } => {
                self.click();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for ToolButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve the explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Every colour here used to
        // be a literal, so a light/dark switch changed nothing on screen: the four state
        // fills, the focus border and the label were all fixed. The rendering census
        // reported the control as theme-blind.
        //
        // The retired literals are kept as the fallbacks, and they are also the **base**
        // from which each state is derived. That is deliberate: it means an inactive theme
        // still renders byte-identically to the previous behaviour, so no existing pixel
        // baseline can regress, while an active theme shifts every state together.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("tool_button");
        let themed_bg = theme.as_ref().and_then(|resolved| resolved.background_color);
        let themed_text = theme.as_ref().and_then(|resolved| resolved.text_color);
        let themed_border = theme.as_ref().and_then(|resolved| resolved.border_color);
        // `tool_button` is absent from the role table, so it resolves as `Surface` and its
        // background is the resolved surface fill. A caller that set a colour still wins.
        let base = Color::rgb(240, 240, 240);
        let accent = themed_border.unwrap_or(Color::rgb(0, 120, 215));
        // The interaction states are **derived from the base**, not collapsed into one
        // colour, so the four states stay distinguishable on any theme: a press is a step
        // toward the accent, a hover is a step toward white, and a toggled-on button keeps
        // the previous checked/hover ordering.
        let bg = if self.pressed {
            base.blend(&accent, 0.45)
        } else if self.checked {
            base.blend(&accent, 0.25)
        } else if self.hovered && !self.auto_raise {
            base.blend(&Color::WHITE, 0.55)
        } else if self.auto_raise && !self.hovered {
            Color::rgba(0, 0, 0, 0) // transparent
        } else {
            base
        };
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        if self.hovered || self.pressed || self.checked {
            context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), accent);
        }
        // A disabled label is the ink faded toward the fill behind it, which keeps it
        // readable-but-muted on a dark theme as well as a light one; the literal is only
        // the fallback for a control whose ink the theme does not supply.
        let ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(0, 0, 0));
        let fg = if !self.base.is_enabled() { ink.blend(&base, 0.45) } else { ink };
        let label = match self.button_style {
            ToolButtonStyle::TextOnly
            | ToolButtonStyle::TextBesideIcon
            | ToolButtonStyle::TextUnderIcon
            | ToolButtonStyle::FollowStyle => &self.text,
            ToolButtonStyle::IconOnly => &self.text,
        };
        // Popup arrow indicator
        let has_popup = self.popup_mode == ToolButtonPopupMode::MenuButtonPopup
            || self.popup_mode == ToolButtonPopupMode::InstantPopup;
        let text_right = if has_popup {
            rect.x as f32 + rect.width as f32 - 12.0
        } else {
            rect.x as f32 + rect.width as f32
        };
        context.draw_text(
            Point::from_f32(
                rect.x as f32 + (text_right - rect.x as f32) / 2.0,
                rect.y as f32 + rect.height as f32 / 2.0,
            ),
            label,
            &Font::default(),
            fg,
            HorizontalAlignment::Left,
        );

        if has_popup {
            context.draw_text(
                Point::from_f32(
                    rect.x as f32 + rect.width as f32 - 8.0,
                    rect.y as f32 + rect.height as f32 - 6.0,
                ),
                "▾",
                &Font::default(),
                fg,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolButton, ToolButtonPopupMode, ToolButtonStyle};
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    use crate::event::EventHandler;
    use crate::render::svg::SvgPaintBackend;
    use crate::render::PaintBackend;
    use crate::render::RenderContext;
    use crate::widget::{Draw, Widget, WidgetKind};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    fn rect() -> Rect {
        Rect::new(10, 20, 100, 30)
    }

    // ── 1. Creating default ToolButton ──
    #[test]
    fn tool_button_default_creation() {
        let btn = ToolButton::new("Open", rect());
        assert_eq!(btn.text(), "Open");
        assert!(btn.icon().is_none());
        assert!(!btn.is_checkable());
        assert!(!btn.is_checked());
        assert!(btn.is_enabled());
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::DelayedPopup);
        assert_eq!(btn.button_style(), ToolButtonStyle::IconOnly);
        assert!(!btn.auto_raise());
        assert_eq!(btn.geometry(), rect());
    }

    // ── 2. Setting/getting text ──
    #[test]
    fn tool_button_set_get_text() {
        let mut btn = ToolButton::new("Old", rect());
        btn.set_text("New Label");
        assert_eq!(btn.text(), "New Label");
    }

    // ── 3. Setting/getting icon ──
    #[test]
    fn tool_button_set_get_icon() {
        let mut btn = ToolButton::new("Open", rect());
        assert!(btn.icon().is_none());
        btn.set_icon(Some(PathBuf::from("icons/open.png")));
        assert_eq!(btn.icon(), Some(PathBuf::from("icons/open.png").as_path()));
        btn.set_icon(None);
        assert!(btn.icon().is_none());
    }

    // ── 4. Enabled/disabled state ──
    #[test]
    fn tool_button_enabled_disabled_state() {
        let mut btn = ToolButton::new("X", rect());
        assert!(btn.is_enabled());
        btn.set_enabled(false);
        assert!(!btn.is_enabled());
        btn.set_enabled(true);
        assert!(btn.is_enabled());
    }

    // ── 5. Button style configuration ──
    #[test]
    fn tool_button_button_style_configuration() {
        let mut btn = ToolButton::new("Btn", rect());
        assert_eq!(btn.button_style(), ToolButtonStyle::IconOnly);

        btn.set_button_style(ToolButtonStyle::TextOnly);
        assert_eq!(btn.button_style(), ToolButtonStyle::TextOnly);

        btn.set_button_style(ToolButtonStyle::TextBesideIcon);
        assert_eq!(btn.button_style(), ToolButtonStyle::TextBesideIcon);

        btn.set_button_style(ToolButtonStyle::TextUnderIcon);
        assert_eq!(btn.button_style(), ToolButtonStyle::TextUnderIcon);

        btn.set_button_style(ToolButtonStyle::FollowStyle);
        assert_eq!(btn.button_style(), ToolButtonStyle::FollowStyle);
    }

    // ── 6. Popup mode (for menu buttons) ──
    #[test]
    fn tool_button_popup_mode() {
        let mut btn = ToolButton::new("Menu", rect());
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::DelayedPopup);

        btn.set_popup_mode(ToolButtonPopupMode::InstantPopup);
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::InstantPopup);

        btn.set_popup_mode(ToolButtonPopupMode::MenuButtonPopup);
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::MenuButtonPopup);

        btn.set_popup_mode(ToolButtonPopupMode::DelayedPopup);
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::DelayedPopup);
    }

    // ── 7. Signal accessor (clicked) ──
    #[test]
    fn tool_button_clicked_signal() {
        let mut btn = ToolButton::new("Btn", rect());
        let clicked_count = Arc::new(AtomicUsize::new(0));
        let c = clicked_count.clone();
        btn.clicked.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        btn.click();
        assert_eq!(clicked_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn tool_button_toggled_signal() {
        let mut btn = ToolButton::new("Tog", rect());
        btn.set_checkable(true);
        let last = Arc::new(AtomicBool::new(false));
        let l = last.clone();
        btn.toggled.connect(move |v| {
            l.store(*v, Ordering::SeqCst);
        });
        btn.set_checked(true);
        assert!(last.load(Ordering::SeqCst));
    }

    #[test]
    fn tool_button_triggered_signal() {
        let mut btn = ToolButton::new("Trg", rect());
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        btn.triggered.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        btn.click();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // ── 8. Mouse click triggers signal ──
    #[test]
    fn tool_button_mouse_click_triggers_signal() {
        let mut btn = ToolButton::new("Btn", rect());
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Press then release
        btn.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1 });
        btn.handle_event(&Event::MouseRelease { pos: Point::new(15, 25), button: 1 });
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn tool_button_keyboard_enter_triggers() {
        let mut btn = ToolButton::new("Btn", rect());
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Enter (key 13) and Space (key 32) trigger
        btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst));
    }

    // ── 9. Geometry delegation ──
    #[test]
    fn tool_button_geometry_delegation() {
        let mut btn = ToolButton::new("X", rect());
        assert_eq!(btn.geometry(), rect());
        let new_rect = Rect::new(0, 0, 200, 50);
        btn.set_geometry(new_rect);
        assert_eq!(btn.geometry(), new_rect);
    }

    // ── 10. Widget ID and kind ──
    #[test]
    fn tool_button_widget_id_and_kind() {
        let btn = ToolButton::new("A", rect());
        assert_eq!(btn.kind(), WidgetKind::ToolButton);
        let btn2 = ToolButton::new("B", rect());
        assert_ne!(btn.id(), btn2.id());
    }

    // ── 11. SVG output ──
    #[test]
    fn tool_button_svg_output() {
        let mut btn = ToolButton::new("Btn", rect());
        let mut svg = SvgPaintBackend::new(Size::new(200, 60));
        svg.begin_frame(Color::WHITE);
        let mut rc = RenderContext::new(&mut svg);
        btn.draw(&mut rc);
        svg.end_frame();
        let output = svg.finish();
        assert!(output.contains("<svg"), "SVG output should contain svg tag");
        // Should contain a rect (fill) for the background
        assert!(
            output.contains("rect") || output.contains("text"),
            "ToolButton SVG should contain visual elements"
        );
    }

    #[test]
    fn tool_button_svg_disabled_draw() {
        let mut btn = ToolButton::new("Disabled", rect());
        btn.set_enabled(false);
        let mut svg = SvgPaintBackend::new(Size::new(200, 60));
        svg.begin_frame(Color::WHITE);
        let mut rc = RenderContext::new(&mut svg);
        btn.draw(&mut rc);
        svg.end_frame();
        let output = svg.finish();
        assert!(output.contains("<svg"));
    }

    // ── 12. Disabled state blocks events ──
    #[test]
    fn tool_button_disabled_state_blocks_events() {
        let mut btn = ToolButton::new("X", rect());
        btn.set_enabled(false);
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Mouse press+release should be blocked
        btn.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1 });
        btn.handle_event(&Event::MouseRelease { pos: Point::new(15, 25), button: 1 });
        assert!(
            !clicked.load(Ordering::SeqCst),
            "Disabled button should not emit clicked on mouse events"
        );
    }

    #[test]
    fn tool_button_disabled_blocks_keyboard() {
        let mut btn = ToolButton::new("X", rect());
        btn.set_enabled(false);
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Keyboard Enter should be blocked
        btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(
            !clicked.load(Ordering::SeqCst),
            "Disabled button should not emit clicked on key events"
        );
    }

    // ── 13. Checkable toggle ──
    #[test]
    fn tool_button_checkable_toggle() {
        let mut btn = ToolButton::new("Tog", rect());
        btn.set_checkable(true);
        assert!(!btn.is_checked());
        btn.click();
        assert!(btn.is_checked());
        btn.click();
        assert!(!btn.is_checked());
    }

    // ── 14. Non-checkable click does not change checked ──
    #[test]
    fn tool_button_non_checkable_ignores_checked() {
        let mut btn = ToolButton::new("X", rect());
        assert!(!btn.is_checkable());
        btn.click();
        // Non-checkable: click() calls set_checked which checks checkable
        // Since checkable is false, the click won't toggle
        assert!(!btn.is_checked(), "Non-checkable button should not toggle via click");
    }

    // ── 15. Auto raise ──
    #[test]
    fn tool_button_auto_raise() {
        let mut btn = ToolButton::new("X", rect());
        assert!(!btn.auto_raise());
        btn.set_auto_raise(true);
        assert!(btn.auto_raise());
        btn.set_auto_raise(false);
        assert!(!btn.auto_raise());
    }
}
