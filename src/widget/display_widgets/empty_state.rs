// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! EmptyState widget — a placeholder shown when a view has no content.
//!
//! The EmptyState widget displays a large icon (emoji/symbol), title, descriptive
//! message, and an optional action button. It is commonly used in list views,
//! search results, inboxes, and dashboards to communicate "no data" states
//! rather than showing a blank screen.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::text_utils::wrap_to_width;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Fallback icon used when no icon has been explicitly set.
const DEFAULT_EMPTY_ICON: &str = "📭";
/// Default empty title text.
const DEFAULT_TITLE: &str = "Nothing here";
/// Default empty message text.
const DEFAULT_MESSAGE: &str = "There are no items to display yet.";
/// Height of bottom action button area.
const ACTION_BUTTON_HEIGHT: u32 = 36;
/// Padding between layout sections.
const SECTION_GAP: i32 = 12;

/// EmptyState widget — a placeholder shown when a view has no content.
///
/// Displays a centered layout consisting of a large icon, a title label,
/// a descriptive message, and an optional action button. The action button
/// is only drawn when `action_text` is non-empty, and emits `action_pressed`
/// when clicked.
pub struct EmptyState {
    base: BaseWidget,
    icon: String,
    title: String,
    message: String,
    action_text: String,
    /// The secondary action's label, empty when there is none.
    secondary_action_text: String,
    /// Whether the action button is currently armed by a left press.
    ///
    /// A press arms the button and paints it pressed; the release decides whether
    /// the action fires. This is the same press/release pair the crate's binary
    /// controls use, so a press alone never commits.
    pressed: bool,
    /// Whether the secondary action is armed by a left press.
    secondary_pressed: bool,
    /// Emitted when the action button is clicked.
    pub action_pressed: GenericSignal,
    /// Emitted when the secondary action is clicked.
    pub secondary_action_pressed: GenericSignal,
    /// The wrapped message and each line's measured width, rebuilt only when the message,
    /// the face, the available width or the device scale changes.
    message_wrap: Option<MessageWrapCache>,
}

/// Cached message layout: the wrapped lines and the width each one was measured at.
struct MessageWrapCache {
    message: String,
    font: Font,
    available_width: u32,
    dpi_scale_bits: u32,
    lines: Vec<(String, u32)>,
}

/// The colours and interaction state shared by both emptiness action buttons.
struct ActionButtonPaint {
    fill: Color,
    pressed: bool,
    is_enabled: bool,
    ink: Color,
    surface: Color,
}

impl EmptyState {
    /// Creates a new EmptyState widget with the given geometry.
    ///
    /// The widget is initialized with default placeholder values:
    /// - Icon: 📭 (empty mailbox)
    /// - Title: "Nothing here"
    /// - Message: "There are no items to display yet."
    /// - Action text: empty (no action button shown)
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::EmptyState, geometry, "EmptyState"),
            icon: DEFAULT_EMPTY_ICON.to_string(),
            title: DEFAULT_TITLE.to_string(),
            message: DEFAULT_MESSAGE.to_string(),
            action_text: String::new(),
            secondary_action_text: String::new(),
            pressed: false,
            secondary_pressed: false,
            action_pressed: GenericSignal::new(),
            secondary_action_pressed: GenericSignal::new(),
            message_wrap: None,
        }
    }

    /// Sets the icon displayed at the top of the empty state.
    ///
    /// This is typically an emoji or a short symbol string (1–2 characters).
    pub fn set_icon(&mut self, icon: &str) {
        if self.icon == icon {
            return;
        }
        self.icon = icon.to_string();
        self.base.request_redraw();
    }

    /// Returns the current icon string.
    pub fn icon(&self) -> &str {
        &self.icon
    }

    /// Sets the title text displayed below the icon.
    pub fn set_title(&mut self, title: &str) {
        if self.title == title {
            return;
        }
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the current title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the descriptive message displayed below the title.
    pub fn set_message(&mut self, message: &str) {
        if self.message == message {
            return;
        }
        self.message = message.to_string();
        self.base.request_redraw();
    }

    /// Returns the current message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Wraps the message and measures each line, reusing the previous result while the
    /// text, the face, the available width and the device scale all stay the same.
    fn refresh_message_wrap(&mut self, context: &RenderContext, font: &Font, available_width: u32) {
        let dpi_scale_bits = context.dpi_scale().to_bits();
        let unchanged = self.message_wrap.as_ref().is_some_and(|cache| {
            cache.message == self.message
                && cache.font == *font
                && cache.available_width == available_width
                && cache.dpi_scale_bits == dpi_scale_bits
        });
        if unchanged {
            return;
        }

        let lines: Vec<(String, u32)> =
            wrap_text(context, &self.message, font, available_width as usize)
                .into_iter()
                .map(|line| {
                    let width = context.measure_text(&line, font).width;
                    (line, width)
                })
                .collect();
        self.message_wrap = Some(MessageWrapCache {
            message: self.message.clone(),
            font: font.clone(),
            available_width,
            dpi_scale_bits,
            lines,
        });
    }

    /// Sets the action button label text.
    ///
    /// When set to a non-empty string, an action button is rendered at the
    /// bottom of the empty state. Clicking the button emits `action_pressed`.
    /// When set to an empty string, the action button is hidden.
    pub fn set_action_text(&mut self, action_text: &str) {
        if self.action_text == action_text {
            return;
        }
        self.action_text = action_text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current action button label text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Returns `true` if the action button is visible (non-empty text).
    pub fn has_action(&self) -> bool {
        !self.action_text.is_empty()
    }

    /// The secondary action's label.
    pub fn secondary_action_text(&self) -> &str {
        &self.secondary_action_text
    }

    /// Sets the secondary action's label, or clears it with an empty string.
    ///
    /// The secondary action is the quieter companion to the primary one ("retry" beside "view
    /// details"). It is drawn to the primary button's right with a weaker fill, and a control with
    /// only a primary action keeps exactly the picture it had.
    pub fn set_secondary_action_text(&mut self, text: &str) {
        if self.secondary_action_text == text {
            return;
        }
        self.secondary_action_text = text.to_string();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns `true` if the secondary action is visible.
    pub fn has_secondary_action(&self) -> bool {
        !self.secondary_action_text.is_empty()
    }

    /// Returns the descriptive message shown below the title.
    ///
    /// The `description` property reads this rather than [`EmptyState::message`]
    /// because the schema publishes `message` as the widget's title line and
    /// `description` as the explanatory sentence below it.
    pub fn description(&self) -> &str {
        &self.message
    }

    /// Sets the descriptive message shown below the title.
    ///
    /// The `description` property writes through here, mirroring
    /// [`EmptyState::set_message`].
    pub fn set_description(&mut self, description: &str) {
        self.set_message(description);
    }

    /// Paints one action button in `fill`, with the shared pressed and disabled treatments.
    fn paint_action_button(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        text: &str,
        font: &Font,
        paint: ActionButtonPaint,
    ) {
        let ActionButtonPaint { fill, pressed, is_enabled, ink, surface } = paint;
        let corner_radius = rect.height / 2;
        // A pressed action is a step toward the ink, the same direction every other
        // pressed surface in the crate moves. The resting fill is unchanged, so a
        // control that is never pressed paints exactly as it did.
        let btn_bg = if !is_enabled {
            ink.blend(&surface, 0.8)
        } else if pressed {
            fill.blend(&ink, 0.18)
        } else {
            fill
        };
        context.fill_rounded_rect(rect, corner_radius, btn_bg);

        let btn_text_color = if !is_enabled {
            // The button's label sits on the button's fill, not on the page.
            self.base.disabled_ink_on(ink, btn_bg)
        } else {
            btn_bg.contrast_color()
        };
        // Horizontal centring is measured; vertical centring comes from the shared line
        // box. The previous `btn_rect.y + btn_rect.height / 2` put the glyph box's top edge
        // on the button's middle line and drew the label half a line low.
        let btn_metrics = context.measure_text(text, font);
        let btn_line = context.text_line(rect, font);
        let btn_origin =
            Point::new(rect.x + (rect.width as i32 - btn_metrics.width as i32) / 2, btn_line.y);
        context.draw_text(btn_origin, text, font, btn_text_color, HorizontalAlignment::Left);
    }

    /// Computes the action buttons' rectangles: the primary, and the secondary to its right.
    ///
    /// With no secondary action the primary keeps the historical centred rectangle, so a
    /// single-action control paints byte-identical chrome. With one, the pair shares that
    /// width and the primary is the leading half.
    fn action_button_rects(&self) -> Option<(Rect, Option<Rect>)> {
        if !self.has_action() {
            return None;
        }
        let rect = self.geometry();
        let btn_y = rect.y + rect.height as i32 - ACTION_BUTTON_HEIGHT as i32 - SECTION_GAP;
        if !self.has_secondary_action() {
            let btn_width = rect.width.clamp(80, 200);
            let btn_x = rect.x + (rect.width as i32 - btn_width as i32) / 2;
            return Some((Rect::new(btn_x, btn_y, btn_width, ACTION_BUTTON_HEIGHT), None));
        }
        let pair_width = rect.width.clamp(160, 400);
        let gap = SECTION_GAP as u32;
        let btn_width = pair_width.saturating_sub(gap) / 2;
        let start_x = rect.x + (rect.width as i32 - pair_width as i32) / 2;
        let primary = Rect::new(start_x, btn_y, btn_width, ACTION_BUTTON_HEIGHT);
        let secondary = Rect::new(
            start_x + btn_width as i32 + gap as i32,
            btn_y,
            btn_width,
            ACTION_BUTTON_HEIGHT,
        );
        Some((primary, Some(secondary)))
    }

    /// The primary action's rectangle, for the single-action geometry tests.
    #[cfg(test)]
    fn action_button_rect(&self) -> Option<Rect> {
        self.action_button_rects().map(|(primary, _)| primary)
    }
}

impl Widget for EmptyState {
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
        crate::core::Size::new(300, 200)
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
            "action_pressed" => Some(EventSignalRef::unit("action_pressed", &self.action_pressed)),
            "secondary_action_pressed" => Some(EventSignalRef::unit(
                "secondary_action_pressed",
                &self.secondary_action_pressed,
            )),
            _ => None,
        }
    }
}

/// `EmptyState`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The legacy dispatch served
/// `message` / `description` for another `WidgetKind` entirely, so neither name was
/// ever routed here; both are now backed by this control's real state, with
/// `message` mapped to the title line and `description` to the sentence beneath it.
impl WidgetProperties for EmptyState {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" => Ok(CapabilityValue::String(self.title().to_string())),
            "description" => Ok(CapabilityValue::String(self.description().to_string())),
            "secondary_action_text" => {
                Ok(CapabilityValue::String(self.secondary_action_text().to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" => {
                self.set_title(&expect_string(value)?);
                Ok(())
            }
            "description" => {
                self.set_description(&expect_string(value)?);
                Ok(())
            }
            "secondary_action_text" => {
                self.set_secondary_action_text(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["message", "description", "secondary_action_text", BASE_PROPERTY_NAMES]
    }
}

impl Draw for EmptyState {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the surface and
        // every piece of text on it were previously hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("empty_state");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgba(245, 245, 250, 200));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The call to action is an accented affordance on this surface: derived by
        // tinting the resolved ink back toward the surface, so it stays a distinct
        // button in either appearance.
        let action_fill = ink.blend(&surface, 0.35);

        // ── Background ──
        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        let center_x = rect.x + rect.width as i32 / 2;

        // Rows are addressed by the **top edge of their glyph box**, which is what the
        // renderer's `origin.y` means (see `RenderContext::draw_text`). The original helper
        // took a `y` and used it as the box's top while its own doc-comment called it "a given
        // Y baseline", and the two conventions were then mixed in the same stack: the icon was
        // passed `icon_y + icon_size` while the title was passed `icon_y + icon_size + gap`,
        // i.e. 12 px below the icon's *bottom* under one reading and 28 px inside its 48 px box
        // under the other. `snapshots/svg/empty_state.svg` showed the \"Sample\" caption painted
        // across the mailbox glyph because of it. One convention, stated once, is the fix.
        let draw_centered =
            |ctx: &mut RenderContext, top_y: i32, text: &str, font: &Font, color: Color| {
                let metrics = ctx.measure_text(text, font);
                let origin = Point::new(center_x - (metrics.width as i32 / 2), top_y);
                ctx.draw_text(origin, text, font, color, HorizontalAlignment::Left);
            };

        // ── Icon ──
        //
        // The stack is centred as a whole rather than started from a fixed top gap. The
        // fixed gap made the content taller than the control at the sizes a control is
        // actually given (48 + 20 + 14 + button against a 120 px box), so the message ran
        // past the bottom edge and the action button overlapped it. Centring means a tall
        // empty state fills the box and a short one sits in the middle, and it is the
        // layout the control's name implies.
        let icon_size: i32 = 48;
        let title_font_size: i32 = 20;
        let message_font_size: i32 = 14;
        // One line box, measured through the renderer rather than assumed from the font
        // size, is the unit every row below is placed with. A row's *pitch* is that box
        // plus the inter-row gap; the header's contribution to the stack is the icon box
        // and the title box in sequence, so the message's rows begin below both.
        // Each label's face comes from the **effective font** at its role size and weight, so the
        // theme family and the user's text scale reach the empty-state copy (D09-STYLE-01).
        let title_font = role_font(style, title_font_size as f32).with_weight_value(600);
        let icon_font = role_font(style, icon_size as f32).with_weight_value(400);
        let message_font = role_font(style, message_font_size as f32).with_weight_value(400);
        // Resolved here, while the style borrow is live: `refresh_message_wrap` below needs
        // `&mut self`, so every style-derived value has to be computed first.
        let action_font = role_font(style, 14.0).with_weight_value(600);
        let icon_height = context.measure_text("M", &icon_font).height.max(1) as i32;
        let title_height = context.measure_text("M", &title_font).height.max(1) as i32;
        let line_height = context.measure_text("M", &message_font).height.max(1) as i32;
        let line_step = line_height + 4;
        // The message band is what is left of the box after the icon and the title have
        // taken their rows and their two gaps — derived from the same three heights the
        // draws below use, so the band and the rows cannot disagree.
        let message_top_offset = icon_height + SECTION_GAP + title_height + 6;
        let message_band = (rect.height as i32 - message_top_offset).max(0);
        // `1 + (band - line_height) / step` is the number of whole lines that fit when the
        // first one consumes `line_height` and each subsequent one `line_step`.
        let message_line_capacity = (1 + (message_band - line_height) / line_step).max(0) as usize;
        let action_extra =
            if self.action_text.is_empty() { 0 } else { ACTION_BUTTON_HEIGHT as i32 + SECTION_GAP };
        // The stack is measured from the band the message may use, so
        // `icon_height + gap + title_height + 6 + the centred band` is exactly the control's
        // height. A tall box therefore fills it, while a short one starts its first row
        // inside the frame via the `max(rect.y)` floor below.
        let stack_height = message_top_offset + message_band.max(line_height) + action_extra;
        // `(rect.height - stack_height) / 2` is negative for a short box, which pushes the
        // stack above the top edge; `max(rect.y)` keeps the first row inside instead.
        let icon_y = (rect.y + (rect.height as i32 - stack_height) / 2).max(rect.y);
        // The icon is the palest ink on the surface; disabled fades it further.
        let icon_color =
            if is_enabled { ink.blend(&surface, 0.35) } else { ink.blend(&surface, 0.75) };
        // A name this crate ships as an icon is drawn as **geometry** in the box the stack already
        // reserved for it; anything else -- the default mailbox emoji, or a symbol a host still
        // passes -- keeps the text path it always had, so an emoji is not silently swallowed. The
        // reserved `icon_height` is unchanged either way, so the rest of the stack cannot shift.
        let icon_side = icon_height.max(1);
        let drawn_as_glyph = crate::widget::icon_token_is_drawable(&self.icon)
            && crate::widget::draw_icon_named(
                context,
                Rect::new(center_x - icon_side / 2, icon_y, icon_side as u32, icon_side as u32),
                icon_color,
                &self.icon,
            );
        if !drawn_as_glyph {
            draw_centered(context, icon_y, &self.icon, &icon_font, icon_color);
        }

        // ── Title ──
        // One gap below the icon's *box*, which is `icon_y + icon_height` — the same sum the
        // stack height above was built from, so the drawn title and the reserved space agree.
        let title_y = icon_y + icon_height + SECTION_GAP;
        let title_color = self.base.disabled_ink_on(ink, surface);
        let title_metrics = context.measure_text(&self.title, &title_font);
        let title_origin = Point::new(center_x - (title_metrics.width as i32 / 2), title_y);
        context.draw_text(
            title_origin,
            &self.title,
            &title_font,
            title_color,
            HorizontalAlignment::Left,
        );

        // ── Message ──
        let message_y = title_y + title_height + 6;
        // The message is the title's ink, one step closer to the surface.
        let message_color =
            if is_enabled { ink.blend(&surface, 0.25) } else { ink.blend(&surface, 0.7) };

        // Wrap message text if it's wider than the available width, then draw only the
        // lines the band can hold. The wrap itself was never the defect — the message was
        // being *drawn* one step past its last measured row — so the fix is the capacity
        // above plus this explicit stop, which keeps the block inside the control on a
        // short box and leaves the normal case unchanged.
        let available_width = rect.width.max(50) - 20;
        self.refresh_message_wrap(context, &message_font, available_width);
        if let Some(cache) = self.message_wrap.as_ref() {
            for (i, (line, line_width)) in cache.lines.iter().enumerate() {
                if i >= message_line_capacity {
                    break;
                }
                let line_y = message_y + i as i32 * line_step;
                let line_origin = Point::new(center_x - (*line_width as i32 / 2), line_y);
                context.draw_text(
                    line_origin,
                    line,
                    &message_font,
                    message_color,
                    HorizontalAlignment::Left,
                );
            }
        }

        // ── Action Button ──
        if let Some((primary, secondary)) = self.action_button_rects() {
            self.paint_action_button(
                context,
                primary,
                &self.action_text,
                &action_font,
                ActionButtonPaint {
                    fill: action_fill,
                    pressed: self.pressed,
                    is_enabled,
                    ink,
                    surface,
                },
            );
            if let Some(secondary) = secondary {
                // The secondary action is deliberately weaker: it is the alternative, not the
                // call to action the control exists for.
                self.paint_action_button(
                    context,
                    secondary,
                    &self.secondary_action_text,
                    &action_font,
                    ActionButtonPaint {
                        fill: action_fill.blend(&surface, 0.55),
                        pressed: self.secondary_pressed,
                        is_enabled,
                        ink,
                        surface,
                    },
                );
            }
        }
    }
}

impl EventHandler for EmptyState {
    /// Arms the action button on a press and commits it on a release inside.
    ///
    /// The action used to fire on the press, so a gesture that began on the button
    /// and was dragged off still committed, and no release was ever consulted. A
    /// press now only arms (and paints) the button; a release inside it emits
    /// [`Self::action_pressed`], while a release outside clears the arm and cancels.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    // Check the primary and secondary action buttons.
                    if let Some((primary, secondary)) = self.action_button_rects() {
                        if primary.contains_point(*pos) {
                            if !self.pressed {
                                self.pressed = true;
                                self.base.request_redraw();
                            }
                            return;
                        }
                        if secondary.is_some_and(|rect| rect.contains_point(*pos)) {
                            if !self.secondary_pressed {
                                self.secondary_pressed = true;
                                self.base.request_redraw();
                            }
                            return;
                        }
                    }
                    // Click on empty state body (outside action button)
                    if self.geometry().contains_point(*pos) {
                        self.base.clicked.emit();
                    }
                }
            }
            Event::MouseRelease { pos, button } => {
                if *button == 1 {
                    if self.pressed {
                        self.pressed = false;
                        // A release inside commits; anywhere else abandons the press, which is
                        // the platform convention for a button the pointer was dragged off.
                        if let Some((primary, _)) = self.action_button_rects() {
                            if primary.contains_point(*pos) {
                                self.action_pressed.emit();
                            }
                        }
                        self.base.request_redraw();
                    } else if self.secondary_pressed {
                        self.secondary_pressed = false;
                        if let Some((_, Some(secondary))) = self.action_button_rects() {
                            if secondary.contains_point(*pos) {
                                self.secondary_action_pressed.emit();
                            }
                        }
                        self.base.request_redraw();
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

/// Wraps `text` into lines no wider than `max_width` pixels.
///
/// Delegates to the shared [`wrap_to_width`], so the message breaks by exactly the rule a
/// label, a tooltip body or a dialog paragraph breaks by: a word that does not fit moves to
/// the next line whole, a single word wider than the line is split on character boundaries
/// instead of running past the control, and the trailing space at a break is not part of the
/// emitted range. Returns the line strings, each copied once out of the caller's text.
fn wrap_text(context: &RenderContext, text: &str, font: &Font, max_width: usize) -> Vec<String> {
    let mut measure = |candidate: &str| context.measure_text(candidate, font).width;
    let max_width = max_width.min(u32::MAX as usize) as u32;
    wrap_to_width(text, max_width, &mut measure)
        .into_iter()
        .map(|range| text[range].to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Font, Size};
    use std::sync::{Arc, Mutex};

    #[test]
    fn empty_state_default_creation() {
        let es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert_eq!(es.icon(), "📭");
        assert_eq!(es.title(), "Nothing here");
        assert_eq!(es.message(), "There are no items to display yet.");
        assert_eq!(es.action_text(), "");
        assert!(!es.has_action());
        assert_eq!(es.kind(), WidgetKind::EmptyState);
    }

    #[test]
    fn empty_state_set_icon() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_icon("🔍");
        assert_eq!(es.icon(), "🔍");
        es.set_icon("📦");
        assert_eq!(es.icon(), "📦");
    }

    #[test]
    fn empty_state_set_title() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert_eq!(es.title(), "Nothing here");
        es.set_title("No results found");
        assert_eq!(es.title(), "No results found");
        es.set_title("");
        assert_eq!(es.title(), "");
    }

    #[test]
    fn empty_state_set_message() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_message("Your search did not match any items.");
        assert_eq!(es.message(), "Your search did not match any items.");
    }

    #[test]
    fn the_message_wraps_at_the_shared_word_boundaries() {
        let mut backend = crate::render::SvgPaintBackend::new(Size::new(240, 120));
        let context = RenderContext::new(&mut backend);
        let font = Font::with_weight("Sans", 14.0, 400, false);

        // A budget one word wide puts each word on its own line, without inventing breaks.
        // The widest word sets the budget, so every word fits on a line by itself.
        let one_word = context.measure_text("gamma", &font).width as usize;
        assert_eq!(
            wrap_text(&context, "alpha beta gamma", &font, one_word),
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
        );

        // A word wider than the budget is split on character boundaries rather than left to
        // run past the control: every emitted line either fits or is a single glyph.
        let narrow = (context.measure_text("MM", &font).width / 2) as usize;
        let lines = wrap_text(&context, "abcdefgh", &font, narrow);
        assert!(lines.len() > 1, "an over-long word must be split: {lines:?}");
        for line in &lines {
            assert!(
                context.measure_text(line, &font).width as usize <= narrow
                    || line.chars().count() == 1,
                "{line:?} neither fits nor is a single glyph"
            );
        }
    }

    #[test]
    fn empty_state_set_action_text() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert_eq!(es.action_text(), "");
        assert!(!es.has_action());

        es.set_action_text("Retry");
        assert_eq!(es.action_text(), "Retry");
        assert!(es.has_action());

        es.set_action_text("");
        assert_eq!(es.action_text(), "");
        assert!(!es.has_action());
    }

    #[test]
    fn empty_state_action_button_rect_none_when_no_action() {
        let es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert!(es.action_button_rect().is_none());
    }

    #[test]
    fn empty_state_action_button_rect_some_when_action() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Refresh");
        let btn_rect = es.action_button_rect();
        assert!(btn_rect.is_some());
        let rect = btn_rect.unwrap();
        // Button should be within the widget bounds
        assert!(rect.width <= 200);
        assert_eq!(rect.height, 36);
    }

    #[test]
    fn empty_state_action_pressed_signal() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Retry");
        let fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        // Compute action button rect and click on it
        let btn_rect = es.action_button_rect().unwrap();
        let click_x = btn_rect.x + btn_rect.width as i32 / 2;
        let click_y = btn_rect.y + btn_rect.height as i32 / 2;
        es.handle_event(&Event::mouse_press(click_x, click_y, 1));
        assert!(!*fired.lock().unwrap(), "a press arms the button but does not commit");
        es.handle_event(&Event::mouse_release(click_x, click_y, 1));
        assert!(*fired.lock().unwrap(), "a release inside commits the action");
    }

    /// A release outside the button cancels the armed press.
    #[test]
    fn empty_state_release_outside_cancels_the_action() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Retry");
        let fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        let btn_rect = es.action_button_rect().unwrap();
        let inside_x = btn_rect.x + btn_rect.width as i32 / 2;
        let inside_y = btn_rect.y + btn_rect.height as i32 / 2;
        es.handle_event(&Event::mouse_press(inside_x, inside_y, 1));
        // Dragged off the button before release: the gesture is abandoned.
        es.handle_event(&Event::mouse_release(1, 1, 1));
        assert!(!*fired.lock().unwrap(), "a release outside must not commit");
        assert!(!es.pressed, "the arm is cleared either way");

        // A release that was never armed does not commit either.
        es.handle_event(&Event::mouse_release(inside_x, inside_y, 1));
        assert!(!*fired.lock().unwrap());
    }

    /// A press paints a pressed button; committing restores the resting drawing.
    #[test]
    fn empty_state_press_paints_a_pressed_button() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Retry");
        let resting = crate::widget::svg::render_to_svg(&mut es);

        let btn_rect = es.action_button_rect().unwrap();
        let click_x = btn_rect.x + btn_rect.width as i32 / 2;
        let click_y = btn_rect.y + btn_rect.height as i32 / 2;
        es.handle_event(&Event::mouse_press(click_x, click_y, 1));
        let pressed = crate::widget::svg::render_to_svg(&mut es);
        assert_ne!(resting, pressed, "a press must paint a pressed state");

        es.handle_event(&Event::mouse_release(click_x, click_y, 1));
        let released = crate::widget::svg::render_to_svg(&mut es);
        assert_eq!(resting, released, "the resting drawing returns after the release");
    }

    #[test]
    fn empty_state_action_pressed_not_emitted_when_no_action() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        // No action text set
        let fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        // Click somewhere in the middle of the widget
        es.handle_event(&Event::mouse_press(150, 100, 1));
        assert!(!*fired.lock().unwrap());
    }

    #[test]
    fn empty_state_click_outside_action_emits_base_clicked() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Go");

        let base_clicked = Arc::new(Mutex::new(false));
        es.base.clicked.connect({
            let base_clicked = Arc::clone(&base_clicked);
            move || {
                *base_clicked.lock().unwrap() = true;
            }
        });

        let action_fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let action_fired = Arc::clone(&action_fired);
            move || {
                *action_fired.lock().unwrap() = true;
            }
        });

        // Click in the upper icon area (not on action button)
        es.handle_event(&Event::mouse_press(150, 30, 1));
        assert!(!*action_fired.lock().unwrap());
        assert!(*base_clicked.lock().unwrap());
    }

    #[test]
    fn empty_state_disabled_blocks_events() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Press Me");
        es.set_enabled(false);

        let action_fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let action_fired = Arc::clone(&action_fired);
            move || {
                *action_fired.lock().unwrap() = true;
            }
        });

        let btn_rect = es.action_button_rect().unwrap();
        let click_x = btn_rect.x + btn_rect.width as i32 / 2;
        let click_y = btn_rect.y + btn_rect.height as i32 / 2;
        es.handle_event(&Event::mouse_press(click_x, click_y, 1));
        assert!(!*action_fired.lock().unwrap());
    }

    #[test]
    fn empty_state_svg_output() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_icon("📦");
        es.set_title("No items");
        es.set_message("Your list is empty.");
        es.set_action_text("Add Item");
        let svg = crate::widget::svg::render_to_svg(&mut es);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"200\""));
        assert!(svg.ends_with("</svg>"));
    }

    // ── Icon resolution ──────────────────────────────────────────────────────

    /// A name this crate ships as an icon is drawn as geometry; the emoji default is not.
    #[test]
    fn a_known_empty_state_icon_is_drawn_as_geometry() {
        let build = |icon: &str| {
            let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
            es.set_icon(icon);
            es.set_title("No items");
            es
        };
        let emoji = crate::widget::svg::render_to_svg(&mut build("📦"));
        let token = crate::widget::svg::render_to_svg(&mut build("info"));

        assert!(
            crate::widget::icon_token_is_drawable("info"),
            "the fixture must use a token this crate ships"
        );
        assert!(!crate::widget::icon_token_is_drawable("📦"));
        // The default emoji keeps the text path it always had, so the two are not the same
        // rendering; the token adds real outline geometry.
        assert_ne!(emoji, token, "a known token must render differently from an emoji");
    }

    #[test]
    fn empty_state_setters_chain() {
        let mut es = EmptyState::new(Rect::new(0, 0, 400, 250));
        es.set_icon("⭐");
        es.set_title("Not found");
        es.set_message("Try adjusting your filters.");
        es.set_action_text("Clear Filters");
        assert_eq!(es.icon(), "⭐");
        assert_eq!(es.title(), "Not found");
        assert_eq!(es.message(), "Try adjusting your filters.");
        assert_eq!(es.action_text(), "Clear Filters");
    }

    #[test]
    fn empty_state_right_mouse_button_ignored() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Click");

        let action_fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let action_fired = Arc::clone(&action_fired);
            move || {
                *action_fired.lock().unwrap() = true;
            }
        });

        // Right click should be ignored
        let btn_rect = es.action_button_rect().unwrap();
        es.handle_event(&Event::mouse_press(
            btn_rect.x + btn_rect.width as i32 / 2,
            btn_rect.y + btn_rect.height as i32 / 2,
            2,
        ));
        assert!(!*action_fired.lock().unwrap());
    }

    #[test]
    fn empty_state_kind() {
        let es = EmptyState::new(Rect::new(0, 0, 200, 150));
        assert_eq!(es.kind(), WidgetKind::EmptyState);
    }

    /// Parses the **text runs** out of a rendered SVG, one ink box per `<path>`.
    ///
    /// # Why the ink box and not the string
    ///
    /// The backend no longer emits a `<text>` element: a run is the glyph geometry the backend
    /// fills, one subpath per ink region inside a single `<path>` (see
    /// `crate::widget::svg::text_ink_boxes`). The string is therefore absent from the document in
    /// every form — `svg.contains("Sample")` can never be true — and a run is located by *where
    /// it is* rather than by *what it says*.
    ///
    /// # Why this delegates instead of parsing `d` itself
    ///
    /// The first form read the `font8x8` rectangle spelling directly (`M{x} {y}h{w}v{h}`), which
    /// is the **bitmap** path's grammar and only its grammar. An outline face writes
    /// `M21.45 30.24L...Z` with fractional vertices, so that reader took `21` as `x` and `45` as
    /// `y` — not ink, and not where the glyph is — and every lookup missed. It also read each
    /// glyph rectangle as its own run rather than unioning a `<path>`'s subpaths, so a bitmap
    /// glyph came back as one box *per set bit*. The crate's own reader knows both grammars and
    /// unions correctly; one reader, so the two cannot drift apart.
    fn ink_runs(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        crate::widget::svg::text_ink_boxes(svg)
    }

    /// The stack's rows do not overlap: each begins at or below the previous row's bottom edge.
    ///
    /// The icon was passed a `y` that the helper treated as the glyph box's top while every
    /// other row was positioned as though the same number were the box's bottom — so the
    /// 48 px icon box covered y 48..96 and the title was drawn at y = 60, **28 px inside it**.
    /// `snapshots/svg/empty_state.svg` showed the caption painted across the mailbox glyph.
    ///
    /// # What is asserted, and why it is the ink
    ///
    /// Each row is checked against the *drawing* rather than against an element attribute: a run's
    /// ink box is where the row was painted, and the top of that box is the quantity the defect
    /// moved. A row's height is the font's own `measure_text("M", font).height`, the renderer's
    /// measurement contract, and the order of `ink_runs` is document order — the order the rows are
    /// drawn in.
    ///
    /// # Why the bound is a `>=` and not an equality
    ///
    /// The check is that the next row's ink starts at or below the previous row's box bottom, not
    /// that it starts exactly there. Under the bitmap face a row's ink top *is* its box top, so the
    /// two coincide; under an outline face the ink is inset a pixel or two inside the box, which is
    /// painting *further* from the overlap rather than toward it. `>=` is therefore the property
    /// the defect violated (the title's ink started *inside* the icon's box) and it holds for both
    /// faces without having to pin which one drew the run.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_stack_rows_do_not_overlap() {
        let mut es = EmptyState::new(Rect::new(0, 0, 240, 120));
        let svg = crate::widget::svg::render_to_svg(&mut es);
        let rendered = ink_runs(&svg);
        assert!(rendered.len() >= 3, "the fixture must render the icon, title and message");

        // Each row's glyph box is `font.size()` tall — the renderer's measurement contract
        // (`measure_text("M", font).height == font.size()`) — and the rows are drawn in a known
        // order: the 48 px icon, the 20 px title, then the 14 px message lines. That order is
        // what identifies a run, because the string each run spells is no longer in the
        // document; a row drawn out of order would fail on the size it was given rather than
        // on its content.
        let mut backend = crate::render::SvgPaintBackend::new(Size::new(240, 120));
        let context = RenderContext::new(&mut backend);
        let height_of = |size: f32| -> i32 {
            context.measure_text("M", &Font::with_weight("Sans", size, 400, false)).height as i32
        };
        let sizes: Vec<i32> = [48.0f32, 20.0, 14.0, 14.0].iter().map(|s| height_of(*s)).collect();

        for (index, window) in rendered.windows(2).enumerate() {
            let (_, top_a, _, _) = &window[0];
            let (_, top_b, _, _) = &window[1];
            let bottom_a = top_a + sizes[index];
            assert!(
                *top_b >= bottom_a,
                "run {} starts at y={top_b}, inside run {index}'s box ({top_a}..{bottom_a})",
                index + 1
            );
        }
    }

    /// Every row is centred on the control's own vertical axis.
    ///
    /// The rows are the icon, the title and the message, all drawn through the same centred
    /// helper in `draw`, so their — different — string widths all put the same midpoint on
    /// `rect.x + rect.width / 2`. Checking the *ink's* midpoint is what makes this a test of
    /// the drawing: the helper places the pen at `center - measured_width / 2`, and the glyphs'
    /// own blank columns are inside that measured width, so the ink's midpoint lies on the
    /// control's axis for every row independently of what any row says.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn every_stack_row_is_centred_horizontally() {
        let mut es = EmptyState::new(Rect::new(0, 0, 240, 120));
        let svg = crate::widget::svg::render_to_svg(&mut es);
        let runs = ink_runs(&svg);
        assert!(runs.len() >= 3, "the fixture must render the icon, title and message");
        let center = 120;
        for (left, _, right, _) in runs {
            // Each row was placed from its *measured* width, so the run's own midpoint is on
            // the axis; the ink can only fall inside that measured width, which is why the
            // bound is one glyph of slack rather than an equality.
            assert!(
                ((left + right) as f32 / 2.0 - center as f32).abs() <= 8.0,
                "a row spans {left}..{right}, whose midpoint must be near the control's axis"
            );
        }
    }

    /// A secondary action sits to the primary's right with weaker emphasis, and the
    /// single-action geometry is untouched.
    #[test]
    fn the_secondary_action_joins_the_primary_on_the_same_row() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Retry");
        let (_primary, secondary) = es.action_button_rects().expect("the primary exists");
        assert!(secondary.is_none(), "no secondary by default");

        es.set_secondary_action_text("Details");
        let (primary, secondary) = es.action_button_rects().expect("both exist");
        let secondary = secondary.expect("the secondary exists");
        assert_eq!(primary.y, secondary.y, "the pair shares a row");
        assert!(secondary.x > primary.x, "the secondary follows the primary");
        assert_eq!(primary.height, secondary.height);

        // Both buttons are inert until the release lands inside them.
        let pressed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = std::sync::Arc::clone(&pressed);
        es.secondary_action_pressed.connect(move || {
            sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let at = Point::new(secondary.x + 4, secondary.y + 4);
        es.handle_event(&Event::MousePress { pos: at, button: 1, modifiers: 0 });
        assert_eq!(pressed.load(std::sync::atomic::Ordering::SeqCst), 0, "a press only arms");
        es.handle_event(&Event::MouseRelease { pos: at, button: 1 });
        assert_eq!(pressed.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Clearing the secondary restores the single-action picture.
        es.set_secondary_action_text("");
        assert!(es.action_button_rects().unwrap().1.is_none());
    }

    /// The secondary label is a writable property.
    #[test]
    fn the_secondary_action_label_round_trips() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set("secondary_action_text", CapabilityValue::String("Details".into()))
            .expect("writable");
        assert_eq!(es.secondary_action_text(), "Details");
        assert_eq!(
            es.get("secondary_action_text").unwrap(),
            CapabilityValue::String("Details".into())
        );
    }
}
