// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{focus_ring_color, role_font, FocusRing, FOCUS_RING_WIDTH};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Width of the leading glyph gutter, in logical pixels.
///
/// The gutter is reserved only while [`CommandLink::shield_visible`] is set, so a link that
/// never asked for a glyph paints its label exactly where it always did.
const GLYPH_GUTTER: u32 = 22;
/// Half of the shield glyph's top edge, in logical pixels.
const SHIELD_HALF_WIDTH: i32 = 7;
/// Height of the shield glyph, in logical pixels.
const SHIELD_HEIGHT: i32 = 16;

/// The keys that activate a command link from the keyboard.
///
/// Enter, line feed and space are the same three codes every other activatable control in
/// the crate accepts, so a link is reachable with the key the user already expects.
fn is_keyboard_activation(key: u32) -> bool {
    matches!(key, 13 | 10 | 32)
}

/// Command link widget for command link buttons.
///
/// A two-line button: a title line plus a secondary description, typically used
/// to present a small set of mutually exclusive options (for example in a
/// wizard). The widget renders both strings but does not draw the usual arrow
/// glyph itself.
///
pub struct CommandLink {
    base: BaseWidget,
    text: String,
    description: String,
    /// Whether the leading shield glyph is painted.
    ///
    /// Opt-in because it is a visible change to the link's own layout: the reference
    /// vocabulary uses the shield to mark a command that needs elevation, and a plain
    /// command link must keep its unadorned label.
    shield_visible: bool,
    /// Emitted when command link is clicked.
    ///
    /// Fires from [`CommandLink::click`], from the keyboard activation keys, and from a
    /// pointer gesture that is **completed** on the link — a press that is dragged off
    /// and released elsewhere cancels. Carries no payload.
    pub clicked: GenericSignal,
    /// Emitted with the new hover flag when the pointer enters or leaves the
    /// widget. Not emitted when the pointer moves within the widget.
    pub hovered: Signal1<bool>,
}
impl CommandLink {
    /// Creates an enabled command link whose title is `"Command"` and whose
    /// description is empty, with no hover state.
    ///
    /// `geometry` is in parent-relative logical pixels.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CommandLink, geometry, "CommandLink"),
            text: "Command".to_string(),
            description: "".to_string(),
            shield_visible: false,
            clicked: GenericSignal::new(),
            hovered: Signal1::new(),
        }
    }
    /// Returns the title line.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns the secondary description line; empty when none was set.
    pub fn description(&self) -> &str {
        &self.description
    }
    /// Returns whether this widget accepts input.
    ///
    /// Shadows the inherited [`Widget::is_enabled`] with an identical result;
    /// both read the same base flag.
    pub fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    /// Replaces the title line and requests a redraw. The description is
    /// unaffected. A same-value write is a no-op (WDGT-12).
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
    }
    /// Replaces the description line and requests a redraw. An empty string
    /// removes the second line. A same-value write is a no-op.
    pub fn set_description(&mut self, description: String) {
        if self.description == description {
            return;
        }
        self.description = description;
        self.base.request_redraw();
    }
    /// Enables or disables the widget and requests a redraw. A disabled link is
    /// still drawn but ignores clicks and does not report hover.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
        self.base.request_redraw();
    }

    /// Whether the leading shield glyph is painted.
    pub fn shield_visible(&self) -> bool {
        self.shield_visible
    }

    /// Shows or hides the leading shield glyph and requests a redraw.
    ///
    /// The glyph occupies a reserved gutter, so the title and description shift right by
    /// the same amount and neither is ever painted under it. A same-value write is a no-op.
    pub fn set_shield_visible(&mut self, visible: bool) {
        if self.shield_visible == visible {
            return;
        }
        self.shield_visible = visible;
        self.base.request_redraw();
    }
    /// Emits `clicked` if the widget is enabled; a no-op otherwise.
    ///
    /// Takes `&self` because the click carries no state: unlike a button, there
    /// is no pressed state to update.
    pub fn click(&self) {
        if self.base.is_enabled() {
            self.clicked.emit();
        }
    }
}
impl Widget for CommandLink {
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
        // The gutter is part of the link's content when the glyph is shown, so the implicit
        // width grows with it rather than squeezing the label into the same box.
        let gutter = if self.shield_visible { GLYPH_GUTTER } else { 0 };
        crate::core::Size::new(300 + gutter, 40)
    }

    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
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
            "clicked" => Some(EventSignalRef::unit("clicked", &self.clicked)),
            "hovered" => Some(EventSignalRef::mapped("hovered", &self.hovered, |v| {
                CapabilityValue::Bool(*v)
            })),
            _ => None,
        }
    }
}

/// `CommandLink`'s property contract.
///
/// `enabled` is listed here because `COMMAND_LINK_PROPERTIES` publishes it as a
/// property of this control; it is answered by the control's own accessor and
/// writer, which is the same pair the old arm called.
impl WidgetProperties for CommandLink {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "description" => Ok(CapabilityValue::String(self.description().to_string())),
            "shield" => Ok(CapabilityValue::Bool(self.shield_visible())),
            "enabled" => Ok(CapabilityValue::Bool(self.is_enabled())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "description" => {
                self.set_description(expect_string(value)?);
                Ok(())
            }
            "shield" => {
                self.set_shield_visible(expect_bool(value)?);
                Ok(())
            }
            "enabled" => {
                self.set_enabled(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `COMMAND_LINK_PROPERTIES`.
        property_names_of!["text", "description", "shield", "enabled", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `command_link` publishes.
    ///
    /// `click` is the zero-argument action: it emits `clicked` when the link is
    /// enabled and does nothing when it is not, which is exactly what a pointer
    /// activation does — so a programmatic click cannot fire a disabled link. It needs
    /// no `&mut self`, but the trait's signature supplies one and the borrow does not
    /// change the effect. The other three assign state and need a payload, so they are
    /// answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "click" => {
                self.click();
                Ok(())
            }
            "set_text" | "set_description" | "set_enabled" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl CommandLink {
    /// Arms the gesture for a press that resolves to this control.
    fn press_at(&mut self, pos: Point) {
        // Only a press that resolves to this control arms it: a direct dispatch (a
        // test, a designer preview) may not have been hit-tested by the runtime.
        if self.base.contains_point_with_touch_expansion(pos) {
            self.base.set_grabbed(true);
            self.base.set_pressed(true);
        }
    }

    /// Completes the gesture, activating only when the release is still over the link.
    fn release_at(&mut self, pos: Point) {
        let inside = self.base.contains_point_with_touch_expansion(pos);
        self.base.set_grabbed(false);
        self.base.set_pressed(false);
        if inside {
            self.click();
        }
    }
}

impl EventHandler for CommandLink {
    /// The activation contract: a press arms, a release decides, the keyboard commits.
    ///
    /// # Why the commit moved to the release
    ///
    /// `clicked` used to fire on `MousePress`, so a press that immediately slid off the
    /// link and was released somewhere else still activated it. Every other activatable
    /// control in the crate treats the release as the decision: inside commits, outside
    /// cancels. A link that fires on the press cannot be slid out of, which is exactly the
    /// gesture a user makes when they change their mind.
    ///
    /// The gesture runs **before** `base.handle_event` because the base clears `pressed`
    /// on the release; reading it afterwards would no longer tell this handler whether the
    /// release belonged to its own gesture.
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 && self.base.is_enabled() => {
                self.press_at(*pos);
            }
            // A drag off the link clears the painted pressed state; a drag back restores it.
            Event::MouseMove { pos } if self.base.is_grabbed() => {
                self.base.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            Event::MouseRelease { pos, button, .. } if *button == 1 && self.base.is_grabbed() => {
                self.release_at(*pos);
            }
            // Touch has no translated press/release here, so contact arms the same gesture and
            // the completed tap completes it through the same path.
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                self.press_at(*pos);
            }
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.base.set_grabbed(false);
                self.base.set_pressed(false);
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                self.release_at(*pos);
            }
            Event::KeyPress { key, .. }
                if is_keyboard_activation(*key) && self.base.is_enabled() =>
            {
                self.click();
            }
            // A focus change mid-press abandons the gesture: the release belongs to whatever
            // the window switch moved to.
            Event::FocusLost => {
                self.base.set_grabbed(false);
                self.base.set_pressed(false);
            }
            Event::MouseEnter { .. } => {
                self.hovered.emit(true);
            }
            Event::MouseLeave { .. } => {
                self.hovered.emit(false);
            }
            _ => { /* Other events are not relevant */ }
        }
        self.base.handle_event(event);
    }
}
impl Draw for CommandLink {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::TRANSPARENT);
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 102, 204));
        // The hover ink is the theme's `primary`, the hue a theme is expected to vary most.
        //
        // It used to be a literal `rgb(0, 0, 255)`, applied unconditionally on hover — so the
        // themed `text_color` above was thrown away the moment the pointer arrived, and a
        // command link was the one control in its group that ignored the appearance in exactly
        // the state a user is most likely to be looking at. Deriving it means light and dark
        // differ on hover rather than both snapping to the same blue.
        let hover_color =
            crate::style::palette_shared().map(|colors| colors.primary).unwrap_or(text_color);
        // The disabled label steps off the *surface it is painted on*, like the description below,
        // rather than substituting a fixed grey. `Color::GRAY` measured 2.48:1 on a light panel —
        // under the 4.5:1 body-text floor — so a disabled command link read as *less* legible than
        // an enabled one, which is the opposite of what "disabled" should say. The shared
        // `disabled_ink_on` reads the control's `enabled` flag itself, so the two spellings of
        // "is this enabled" cannot disagree.
        let disabled_color = self.base.disabled_ink_on(text_color, bg_color);
        let is_hovered = self.base.is_hovered();
        let is_enabled = self.base.is_enabled();
        // Draw background (transparent by default)
        if bg_color != Color::TRANSPARENT {
            context.face_with_gradient_corners(
                rect,
                bg_color,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().resolved_corner_radii().to_array(),
                Color::BLACK,
            );
        }
        // Determine text color based on state
        let current_text_color = if !is_enabled {
            disabled_color
        } else if is_hovered {
            hover_color
        } else {
            text_color
        };
        // Draw main text
        let padding = &style.padding;
        let text_font = role_font(style, 12.0).with_italic(true);
        // The glyph gutter is reserved **between** the padding and the label, so the shield
        // never shares a box with the text and the two lines shift together.
        let gutter = if self.shield_visible { GLYPH_GUTTER as i32 } else { 0 };
        let leading_x = rect.x + padding.left as i32;
        let text_x = leading_x + gutter;
        // The two lines are stacked from their **measured** line boxes rather than from the
        // literals `+ 12` and `+ 16`. Those constants pinned the layout to one particular font
        // size: the label's offset had no relation to its own line height, so a theme or a
        // caller passing a larger font moved the description into the label. The label sits in
        // the top half of the content box and the description directly below it, each in its own
        // measured line, which is the same reading a stacked link has in every toolkit.
        let content_top = rect.y + padding.top as i32;
        let text_band = Rect {
            x: text_x,
            y: content_top,
            width: rect.width.saturating_sub(padding.left + padding.right + gutter as u32),
            height: rect.height.saturating_sub(padding.top + padding.bottom),
        };
        let line = context.text_line(text_band, &text_font);
        let text_y = line.y;
        // ── The leading glyph slot ──
        //
        // A shield outline drawn from line segments rather than an icon-registry lookup: the
        // glyph is seven short strokes at any scale, so it needs no asset and follows the same
        // hover/disabled ink as the label it marks. It is centred on the title's own line box,
        // which keeps it attached to the text when the theme grows the font.
        if self.shield_visible {
            let center_x = leading_x + GLYPH_GUTTER as i32 / 2;
            let center_y = text_y + line.height as i32 / 2;
            let top = center_y - SHIELD_HEIGHT / 2;
            let bottom = top + SHIELD_HEIGHT;
            let left = center_x - SHIELD_HALF_WIDTH;
            let right = center_x + SHIELD_HALF_WIDTH;
            let shoulder_y = top + SHIELD_HEIGHT * 2 / 5;
            let stroke = current_text_color;
            context.draw_line(Point::new(left, top), Point::new(right, top), stroke);
            context.draw_line(Point::new(left, top), Point::new(left, shoulder_y), stroke);
            context.draw_line(Point::new(right, top), Point::new(right, shoulder_y), stroke);
            context.draw_line(Point::new(left, shoulder_y), Point::new(center_x, bottom), stroke);
            context.draw_line(Point::new(right, shoulder_y), Point::new(center_x, bottom), stroke);
            // The check inside says the command is the sanctioned one; a disabled link draws
            // it in the same receded ink as the outline, so the two cannot disagree.
            let check_y = top + SHIELD_HEIGHT / 2;
            context.draw_line(
                Point::new(center_x - 3, check_y),
                Point::new(center_x - 1, check_y + 2),
                stroke,
            );
            context.draw_line(
                Point::new(center_x - 1, check_y + 2),
                Point::new(center_x + 3, check_y - 3),
                stroke,
            );
        }
        context.draw_text_fitted_cow(
            line,
            &self.text,
            &text_font,
            current_text_color,
            HorizontalAlignment::Left,
        );
        // Draw description if present
        if !self.description.is_empty() {
            let desc_font = role_font(style, 10.0);
            // `Color::GRAY` was a literal that never moved with the appearance; the disabled
            // ink already follows it, so an enabled description is the same ink damped toward
            // whatever the control was actually given to paint on. The disabled ink is derived
            // from the same surface by the shared weight, so both halves of this branch agree
            // about which surface the text sits on.
            let desc_color = if !is_enabled {
                self.base.disabled_ink_on(current_text_color, bg_color)
            } else {
                current_text_color.blend(&bg_color, 0.35)
            };
            let desc_line =
                context.text_line(Rect { y: text_y + line.height as i32, ..text_band }, &desc_font);
            context.draw_text_fitted_cow(
                desc_line,
                &self.description,
                &desc_font,
                desc_color,
                HorizontalAlignment::Left,
            );
        }
        // Draw underline for hover state
        if is_hovered && is_enabled {
            let text_metrics = context.measure_text(&self.text, &text_font);
            // The rule sits on the label's own line-box bottom edge, so it tracks the glyphs it
            // underlines instead of a second hand-tuned offset.
            let underline_y = text_y + line.height as i32;
            context.draw_line(
                Point::new(text_x, underline_y),
                Point::new(text_x + text_metrics.width as i32, underline_y),
                current_text_color,
            );
        }

        // ── Keyboard focus ring ──
        //
        // Only a keyboard focus draws it; a pointer press focuses the link without painting a
        // ring under the cursor, which is the rule every other control in the crate follows.
        if self.base.draws_focus_ring() {
            let ring = FocusRing::for_control(rect, 0);
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(bg_color.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect};
    use crate::style::WidgetStyle;

    #[test]
    fn commandlink_creation_defaults() {
        let cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert_eq!(cl.text(), "Command");
        assert!(cl.description().is_empty());
        assert!(cl.is_enabled());
    }

    #[test]
    fn commandlink_set_text() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.set_text("Save".to_string());
        assert_eq!(cl.text(), "Save");
    }

    #[test]
    fn commandlink_set_description() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.set_description("Save the current document".to_string());
        assert_eq!(cl.description(), "Save the current document");
        cl.set_description(String::new());
        assert!(cl.description().is_empty());
    }

    #[test]
    fn commandlink_set_enabled() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.is_enabled());
        cl.set_enabled(false);
        assert!(!cl.is_enabled());
        cl.set_enabled(true);
        assert!(cl.is_enabled());
    }

    #[test]
    fn commandlink_set_enabled_updates_base_state() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.base().is_enabled());
        cl.set_enabled(false);
        assert!(!cl.base().is_enabled());
        cl.set_enabled(true);
        assert!(cl.base().is_enabled());
    }

    #[test]
    fn commandlink_click() {
        let cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.click(); // Should not panic
    }

    #[test]
    fn commandlink_geometry_delegation() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.set_geometry(Rect::new(10, 10, 400, 80));
        assert_eq!(cl.geometry(), Rect::new(10, 10, 400, 80));
    }

    #[test]
    fn commandlink_visibility() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.is_visible());
        cl.hide();
        assert!(!cl.is_visible());
        cl.show();
        assert!(cl.is_visible());
    }

    #[test]
    fn commandlink_tooltip_roundtrip() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.tooltip().is_empty());
        cl.set_tooltip("Click here".to_string());
        assert_eq!(cl.tooltip(), "Click here");
        cl.set_tooltip(String::new());
        assert!(cl.tooltip().is_empty());
    }

    #[test]
    fn commandlink_style_roundtrip() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert_eq!(*cl.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(240, 240, 240));
        cl.set_style(custom.clone());
        assert_eq!(*cl.style(), custom);
    }

    #[test]
    fn commandlink_id_kind() {
        let cl_a = CommandLink::new(Rect::new(0, 0, 100, 50));
        let cl_b = CommandLink::new(Rect::new(0, 0, 100, 50));
        assert_ne!(cl_a.id(), cl_b.id());
        assert_eq!(cl_a.kind(), WidgetKind::CommandLink);
        assert_eq!(cl_b.kind(), WidgetKind::CommandLink);
    }

    #[test]
    fn commandlink_signal_accessors() {
        let cl = CommandLink::new(Rect::new(0, 0, 100, 50));
        let _clicked = &cl.clicked;
        let _hovered = &cl.hovered;
    }

    /// The hover underline follows the appearance rather than a fixed blue.
    ///
    /// # The defect this pins
    ///
    /// On hover the ink was a literal `rgb(0, 0, 255)`, applied unconditionally to the label
    /// **and** to the underline. The themed `text_color` the control had just resolved was
    /// therefore discarded in the one state a user is most likely to be looking at, so a light
    /// and a dark build drew the same blue under a link whose resting ink differed.
    ///
    /// # Why the underline, and not the whole document
    ///
    /// A first instinct is to compare the two documents wholesale, but a document-wide
    /// comparison is satisfied by any colour that differs — including ones this test is not
    /// about. The hover underline is emitted as a single `<line ... stroke="rgba(...)" />`,
    /// which names the exact element the fix moved, so that is what the assertion reads.
    #[test]
    #[cfg(feature = "desktop")]
    fn the_hover_ink_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 300, 60);

        let hover_stroke = |appearance| -> String {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut cl = CommandLink::new(rect);
            cl.set_text("Open".to_string());
            // The `MouseEnter` the base records is what puts the control in its hover state, so
            // the underline is drawn at all.
            cl.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
            crate::theme::apply_theme_to_widget(&mut cl);
            let svg = crate::widget::svg::render_widget_to_svg(&mut cl, rect);
            underline_stroke(&svg).unwrap_or_else(|| {
                panic!("a hovered command link must underline itself; svg was {svg}")
            })
        };

        let dark = hover_stroke(crate::theme::AppearanceMode::Dark);
        let light = hover_stroke(crate::theme::AppearanceMode::Light);
        assert_ne!(dark, light, "the hover ink must follow the appearance; both were {dark}");
        assert_ne!(
            dark, "rgba(0,0,255,255)",
            "the hover ink must not be the fixed blue the defect used"
        );
    }

    /// A press only arms the link; the release decides whether it activated.
    ///
    /// # The defect this pins
    ///
    /// `clicked` used to fire on `MousePress`, so sliding off the link before releasing —
    /// the standard "changed my mind" gesture — still ran the command. The fix gives the
    /// link the same four-part contract every other activatable control has: press arms,
    /// move follows, release inside commits, release outside (or focus loss) cancels.
    #[test]
    fn a_completed_press_activates_and_a_drag_off_cancels() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let inside = Point::new(10, 10);
        let outside = Point::new(9000, 9000);
        let run = |events: &[Event]| {
            let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
            let clicks = Arc::new(AtomicUsize::new(0));
            let counter = clicks.clone();
            cl.clicked.connect(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
            for event in events {
                cl.handle_event(event);
            }
            clicks.load(Ordering::SeqCst)
        };

        // A bare press is not an activation.
        assert_eq!(run(&[Event::MousePress { pos: inside, button: 1, modifiers: 0 }]), 0);
        // Press inside and release inside is one activation.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1, modifiers: 0 },
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            1
        );
        // Press inside and release outside is a cancellation.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1, modifiers: 0 },
                Event::MouseRelease { pos: outside, button: 1 },
            ]),
            0
        );
        // A release with no press is not an activation either.
        assert_eq!(run(&[Event::MouseRelease { pos: inside, button: 1 }]), 0);
        // Dragging off and back restores the armed state.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1, modifiers: 0 },
                Event::MouseMove { pos: outside },
                Event::MouseMove { pos: inside },
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            1
        );
        // Focus loss abandons a held press.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1, modifiers: 0 },
                Event::FocusLost,
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            0
        );
    }

    /// Enter, line feed and space activate the link; other keys do not.
    #[test]
    fn enter_and_space_activate_the_link() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let clicks_for = |key: u32| {
            let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
            let clicks = Arc::new(AtomicUsize::new(0));
            let counter = clicks.clone();
            cl.clicked.connect(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
            cl.handle_event(&Event::KeyPress { key, modifiers: 0 });
            clicks.load(Ordering::SeqCst)
        };

        assert_eq!(clicks_for(13), 1, "Enter activates");
        assert_eq!(clicks_for(10), 1, "line feed activates");
        assert_eq!(clicks_for(32), 1, "space activates");
        assert_eq!(clicks_for(9), 0, "tab is not an activation");
    }

    /// The shield slot is opt-in, round-trips through the property route, and widens the hint.
    #[test]
    fn the_shield_slot_is_opt_in_and_shifts_the_label() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(!cl.shield_visible(), "a plain command link has no glyph");
        assert_eq!(cl.size_hint(), crate::core::Size::new(300, 40));

        cl.set_shield_visible(true);
        assert!(cl.shield_visible());
        assert_eq!(
            cl.size_hint(),
            crate::core::Size::new(300 + GLYPH_GUTTER, 40),
            "the reserved gutter is part of the implicit width"
        );

        assert_eq!(cl.get("shield").unwrap(), CapabilityValue::Bool(true));
        cl.set("shield", CapabilityValue::Bool(false)).unwrap();
        assert!(!cl.shield_visible());
        assert_eq!(cl.get("shield").unwrap(), CapabilityValue::Bool(false));
    }

    /// The `stroke` of the SVG `<line>` element, if the document has one.
    #[cfg(feature = "desktop")]
    fn underline_stroke(svg: &str) -> Option<String> {
        let at = svg.find("<line ")?;
        let rest = &svg[at..];
        let end = rest.find("/>")? + 2;
        let element = &rest[..end];
        let key = "stroke=\"";
        let from = element.find(key)? + key.len();
        let to = element[from..].find('"')? + from;
        Some(element[from..to].to_string())
    }
}
