// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool button widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::path::{Path, PathBuf};

/// Tool button popup mode.
///
/// Selects how the button's attached menu is presented. The mode is stored and
/// exposed as a property, but the widget does not itself own or show a menu: it
/// paints the popup indicator arrow and reserves its room, and the value tells
/// the containment layer that wires the button to a menu how to present it.
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
/// Selects which of the text and icon parts are painted, and how they are arranged when both are
/// shown. Read by [`ToolButton::icon_rect`] and [`ToolButton::label_rect`], which are the single
/// derivation of the two content slots, so the paint and the layout cannot disagree about where a
/// part goes.
///
/// With no icon, `IconOnly` and every other variant fall back to the label: a button with nothing to
/// show would otherwise be a blank rectangle, and "the style says icon-only but there is no icon" is
/// a state the user cannot act on.
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
/// A compact icon-or-text button for tool bars and menu surfaces. It can act as a plain push
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
/// centred and fades toward the fill while the button is disabled. The icon is loaded from the
/// path [`ToolButton::set_icon`] stores, once per path, and is laid out by [`ToolButtonStyle`].
pub struct ToolButton {
    base: BaseWidget,
    text: String,
    icon: Option<PathBuf>,
    /// RGBA8 pixels for [`Self::icon`], decoded once per path.
    ///
    /// `None` is cached too, so an icon path that cannot be read is not re-read every frame. The
    /// pixels are square RGBA8 (width * height * 4 bytes), which is what `draw_image` takes.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    icon_pixels: Option<alloc::sync::Arc<Vec<u8>>>,
    /// The path [`Self::icon_pixels`] was produced for, so changing the path invalidates the pixels
    /// without the caller having to clear them.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    icon_pixels_key: Option<PathBuf>,
    checkable: bool,
    checked: bool,
    popup_mode: ToolButtonPopupMode,
    button_style: ToolButtonStyle,
    auto_raise: bool,
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
    /// style hides the label even though one is set, but only once an icon is
    /// available to replace it -- see [`ToolButton::wants_label`].
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        let text = text.into();
        Self {
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, "ToolButton"),
            text,
            icon: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            icon_pixels: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            icon_pixels_key: None,
            checkable: false,
            checked: false,
            popup_mode: ToolButtonPopupMode::DelayedPopup,
            button_style: ToolButtonStyle::IconOnly,
            auto_raise: false,
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
    /// that it decodes as an image. The decode happens lazily on the first paint,
    /// so a caller that sets an icon on a control it never shows pays nothing.
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
    /// The path is stored verbatim and decoded lazily by [`Self::resolved_icon`] on the first
    /// paint -- not here, because a caller that sets an icon on a control it never shows should not
    /// pay for the decode, and because a decode at set time would have to run inside the event
    /// handler that called it.
    pub fn set_icon(&mut self, icon: Option<PathBuf>) {
        if self.icon == icon {
            return;
        }
        self.icon = icon;
        // Both halves of the cache drop together. Leaving the key would make the next paint believe
        // the pixels belonged to the new path -- a worse failure than having no cache at all.
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        {
            self.icon_pixels = None;
            self.icon_pixels_key = None;
        }
        self.base.request_redraw();
    }

    /// The RGBA8 pixels for the current icon path, shared process-wide.
    ///
    /// Returns `None` when there is no icon, when the file cannot be read, or when the bytes do not
    /// decode -- in which case the button simply shows no icon rather than a broken-image glyph, the
    /// same fallback `avatar` uses for the same reason: a toolbar's shape must not change because
    /// one of its images is missing.
    ///
    /// # Two layers, as in `avatar`
    ///
    /// `icon_pixels_key` is the per-instance string compare that keeps the per-frame path cheap, and
    /// [`crate::image::cache`] is what makes **twelve buttons on one icon** decode it once.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    fn resolved_icon(&mut self) -> Option<alloc::sync::Arc<Vec<u8>>> {
        let path = self.icon.as_ref()?;
        if self.icon_pixels_key.as_ref() != Some(path) {
            // The decoded extent is dropped: the icon is scaled to the slot the style derivation
            // reserved, so a non-square source would otherwise change the button's layout. Only the
            // pixels are kept -- and they are the cache's allocation, not this button's.
            self.icon_pixels = crate::image::cache::file_rgba8_or_none(path);
            // Set whether or not it loaded, so a missing icon is not re-read every frame.
            self.icon_pixels_key = Some(path.clone());
        }
        self.icon_pixels.clone()
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

    /// Whether the current style asks for the icon, and one is actually available to draw.
    ///
    /// The two halves of the question are answered together because they have one answer: a style
    /// that asks for an icon the control does not have is a button with nothing to show, so every
    /// caller that would ask "icon only?" has to re-ask "but is there an icon?". Answering once
    /// keeps a style change and a `set_icon(None)` from disagreeing about the layout.
    ///
    /// # Why the answer is a *place*, not a *path*
    ///
    /// "There is an icon path" is not the same as "there is an icon to draw": the file may not
    /// exist, or may not decode. The layout is therefore derived from where a *set* icon path
    /// lands, not from whether its bytes loaded, so a missing file cannot shift the label (that is
    /// [`Self::pixels_available`]'s separate question, and it only decides whether pixels are
    /// painted).
    ///
    /// # Why `IconOnly` still reports `false` without an icon
    ///
    /// "Icon-only with no icon" would be a blank rectangle, which is a state a user cannot act on.
    /// Falling back to the label is what `ToolButtonStyle`'s own doc promises.
    fn wants_icon(&self) -> bool {
        if self.icon.is_none() {
            return false;
        }
        !matches!(self.button_style, ToolButtonStyle::TextOnly)
    }

    /// Whether the label is drawn.
    ///
    /// False only for `IconOnly` *with* an icon: with the icon showing, "icon only" means exactly
    /// that. Every other case draws the label, including `IconOnly` without an icon -- see
    /// [`Self::wants_icon`].
    fn wants_label(&self) -> bool {
        match self.button_style {
            ToolButtonStyle::IconOnly => !self.wants_icon(),
            _ => true,
        }
    }

    /// The button's content box: its own frame inset by [`dimensions::TOOL_BUTTON_PADDING`], less
    /// the room the popup arrow reserves at the trailing edge when one is drawn.
    ///
    /// One derivation for both content slots and the arrow, so the three cannot disagree about
    /// where the button's interior ends.
    fn content_box(&self) -> Rect {
        let rect = self.geometry();
        let has_popup = matches!(
            self.popup_mode,
            ToolButtonPopupMode::MenuButtonPopup | ToolButtonPopupMode::InstantPopup
        );
        let padding = dimensions::TOOL_BUTTON_PADDING;
        let trailing =
            if has_popup { dimensions::TOOL_BUTTON_POPUP_ARROW_RESERVE } else { padding };
        ControlMetrics::content_box(
            rect,
            EdgeOffsets { top: padding, right: trailing, bottom: padding, left: padding },
        )
    }

    /// The box the icon occupies, or `None` when the style does not ask for one.
    ///
    /// The icon is square. Its position depends on where the label went, which is what
    /// [`Self::content_bands`] decides *when the style stacks them*; with the label beside it the two
    /// share one full-height row, which is why the band split is not applied there -- splitting a
    /// one-row box by a whole line's height would leave the icon a two-pixel sliver.
    pub fn icon_rect(&self) -> Option<Rect> {
        if !self.wants_icon() {
            return None;
        }
        let content = self.content_box();
        let band = match self.button_style {
            ToolButtonStyle::TextUnderIcon if self.wants_label() => self.content_bands(content).0,
            _ => content,
        };
        let size = dimensions::TOOL_BUTTON_ICON_SIZE.min(content.width).min(band.height);
        if size == 0 {
            return None;
        }
        let y = band.y + (band.height.saturating_sub(size)) as i32 / 2;
        let x = match self.button_style {
            // With the label beside it, the icon is the leading slot, so it sits at the leading edge
            // of the *whole* content box rather than centred across it.
            ToolButtonStyle::TextBesideIcon => content.x,
            // Under the label, or alone, the icon is centred in its band.
            _ => content.x + (content.width.saturating_sub(size)) as i32 / 2,
        };
        Some(Rect::new(x, y, size, size))
    }

    /// The box the label occupies, or `None` when the style draws no label.
    ///
    /// The trailing slot of the same two-part content box: with an icon beside it the label begins
    /// after the icon and its gap, so a wider icon pushes the label along instead of overlapping it.
    /// With the icon above, the label takes the lower band; with no icon it takes the whole box,
    /// which is the pre-existing single-label layout.
    pub fn label_rect(&self) -> Option<Rect> {
        if !self.wants_label() {
            return None;
        }
        let content = self.content_box();
        match self.button_style {
            ToolButtonStyle::TextBesideIcon if self.wants_icon() => {
                let advance = (dimensions::TOOL_BUTTON_ICON_SIZE
                    + dimensions::TOOL_BUTTON_ICON_SPACING)
                    .min(content.width);
                Some(Rect::new(
                    content.x + advance as i32,
                    content.y,
                    content.width.saturating_sub(advance),
                    content.height,
                ))
            }
            ToolButtonStyle::TextUnderIcon if self.wants_icon() => {
                Some(self.content_bands(content).1)
            }
            _ => Some(content),
        }
    }

    /// Splits the content box into the icon's band and the label's band for the stacked style.
    ///
    /// # Why the icon is reserved first
    ///
    /// The icon is a fixed square and the label is `n` whole lines, so the split has to start from
    /// one of them. The icon goes first because its size is the *constraint* -- a label given a band
    /// that leaves less than [`dimensions::TOOL_BUTTON_ICON_SIZE`] would squeeze the icon into a
    /// sliver, which is what a split driven by whole lines alone produced in a short button.
    /// Whatever is left after the icon and its gap is the label's, floored to whole lines so the
    /// band never clips the glyphs it draws.
    ///
    /// # Why the two bands always sum to the content box
    ///
    /// The label takes exactly the remainder, so no strip of nothing can appear between them -- a
    /// layout whose halves do not add up reads as a gap in the button.
    fn content_bands(&self, content: Rect) -> (Rect, Rect) {
        // The label's unit is a *line*, and the line height comes from the shared estimate rather
        // than a literal: a control that floors to a number the renderer does not agree with would
        // give its label a band that clips the glyphs it draws.
        let line = crate::widget::metrics::estimate_line_height(&Font::default(), 1.0).max(1);
        let reserved = (dimensions::TOOL_BUTTON_ICON_SIZE
            + dimensions::TOOL_BUTTON_ICON_SPACING / 2)
            .min(content.height);
        let remainder = content.height.saturating_sub(reserved);
        // A remainder shorter than one line is still given to the label rather than discarded: a
        // button with no room for a line is better drawn with a cramped one than with the icon on
        // its own, which would make the style's instruction unobservable.
        let label_height =
            if remainder == 0 { 0 } else { (remainder / line).max(1) * line }.min(remainder);
        let icon_height = content.height.saturating_sub(label_height);
        let icon = Rect::new(content.x, content.y, content.width, icon_height);
        let label =
            Rect::new(content.x, content.y + icon_height as i32, content.width, label_height);
        (icon, label)
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The square this button claims when nothing constrains it.
    ///
    /// [`dimensions::TOOL_BUTTON_SIZE`] rather than a literal, so the tool bar that packs a row of
    /// these and the button's own snapshot agree on the same size (rule #101).
    fn size_hint(&self) -> crate::core::Size {
        let side = dimensions::TOOL_BUTTON_SIZE;
        crate::core::Size::new(side, side)
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
    /// Tool button activation, in the four-segment form `Button` established.
    ///
    /// # Why the gesture runs **before** the base records the event
    ///
    /// The base both records and paints from the same primitive facts, and it clears
    /// `grabbed`/`pressed` on the release. A control that read them *after* delegating could no
    /// longer tell whether the release belonged to its own gesture — this handler used to have
    /// exactly that shape, and it fired `clicked` for a press that started elsewhere and merely
    /// ended here. Running the gesture first keeps the grab's guard meaningful; the base then
    /// records the same outcome from the same facts.
    ///
    /// | event | result |
    /// |---|---|
    /// | press inside | armed + `pressed` |
    /// | move | `pressed` follows whether the pointer is still inside |
    /// | release inside | `clicked` + `triggered` |
    /// | release outside | nothing (the base clears the state) |
    /// | Enter / Space | `clicked` + `triggered` |
    /// | disabled | every arm is skipped |
    fn handle_event(&mut self, event: &Event) {
        match event {
            // Only a press that resolves to this control arms it. The runtime hit-tests before
            // delivery, but a direct dispatch (a test, a host with its own routing, a designer
            // preview) does not, and an unguarded press would leave the latch armed for a release
            // that belongs elsewhere. Both halves of "resolves to this control" are in the guard,
            // so the arm either arms the gesture or does nothing at all.
            Event::MousePress { pos, button }
                if self.base.is_enabled()
                    && *button == crate::event::mouse_button::PRIMARY
                    && self.base.contains_point_with_touch_expansion(*pos) =>
            {
                self.base.set_pressed(true);
            }
            // A press that wanders off cancels, and one that wanders back re-arms: `pressed` is a
            // continuous quantity, not an edge. Guarded on the grab rather than on `pressed` so
            // that the move which should restore the state is not the one that exits early.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.base.is_grabbed() => {
                self.base.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            // `is_pressed` still answers for the gesture that is ending, because the base has not
            // yet cleared it (the base records after this match). A release inside is an
            // activation; a release outside was already cancelled by the move arm, so the guard
            // reads the same fact the arm would.
            Event::MouseRelease { .. } | Event::PointerRelease { .. }
                if self.base.is_grabbed() && self.base.is_pressed() =>
            {
                self.click();
            }
            Event::KeyPress { key: 13 | 32, .. } if self.base.is_enabled() => {
                self.click();
            }
            _ => { /* Other events are not relevant */ }
        }
        // After the control's own gesture: let the base record the primitive facts (hover, press,
        // grab, focus reason) for the state channel.
        self.base.handle_event(event);
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
        let base = style.background_color.or(themed_bg).unwrap_or(Color::rgb(240, 240, 240));
        let accent = themed_border.unwrap_or(Color::rgb(0, 120, 215));
        // The interaction states are **derived from the base**, not collapsed into one
        // colour, so the four states stay distinguishable on any theme: a press is a step
        // toward the accent, a hover is a step toward white, and a toggled-on button keeps
        // the previous checked/hover ordering.
        let pressed = self.base.is_pressed();
        let hovered = self.base.is_hovered();
        let bg = if pressed {
            base.blend(&accent, 0.45)
        } else if self.checked {
            base.blend(&accent, 0.25)
        } else if hovered && !self.auto_raise {
            base.blend(&Color::WHITE, 0.55)
        } else if self.auto_raise && !hovered {
            Color::rgba(0, 0, 0, 0) // transparent
        } else {
            base
        };
        // The face, not a bare rectangle: `tool_button` classifies as pressable chrome in
        // `render::surface`'s role table, so its `SurfaceStyle` carries a raised bevel (and level 0,
        // i.e. no shadow). Painting through `face` is what makes that declaration reach the screen --
        // a raw `fill_rect` here would leave the role table's answer for this kind unread, which is
        // the "declared but nobody consumes it" shape the census exists to catch. `tool_bar`,
        // `menu_bar` and `status_bar` are the sibling call sites in this module and all read their
        // own surface the same way.
        context.face(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            bg,
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        if hovered || pressed || self.checked {
            context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), accent);
        }
        // A disabled label is the ink faded toward the fill behind it, which keeps it
        // readable-but-muted on a dark theme as well as a light one; the literal is only
        // the fallback for a control whose ink the theme does not supply.
        let ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(0, 0, 0));
        let fg = if !self.base.is_enabled() { ink.blend(&base, 0.45) } else { ink };

        // ── Icon ──
        //
        // The icon is the leading slot (or the upper band), and its box comes from `icon_rect` --
        // the same derivation `ToolButtonStyle` documents -- so the room the style reserves and the
        // pixels drawn cannot disagree. Pixels are painted only when the path actually decoded:
        // a missing file leaves the slot empty instead of shifting the label onto the icon.
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        if let (Some(pixels), Some(box_)) = (self.resolved_icon(), self.icon_rect()) {
            if !pixels.is_empty() {
                context.draw_image(box_.x, box_.y, box_.width, box_.height, &pixels);
            }
        }

        // ── Label and popup arrow ──
        //
        // Both are `RenderContext::text_line`-free here: the label's box is centred on its own
        // middle line, and the arrow sits in the room `content_box` reserved, so neither can drift
        // from the band that was laid out for it.
        let font = Font::default();
        let has_popup = matches!(
            self.popup_mode,
            ToolButtonPopupMode::MenuButtonPopup | ToolButtonPopupMode::InstantPopup
        );
        if let Some(box_) = self.label_rect() {
            // `draw_text_line` is the primitive that both fits the label to the box and centres it
            // **vertically** in it -- `draw_text_fitted` alone leaves the origin at `bounds.y`, so a
            // full-height box would pin the label to its top edge. `Button` centres its own label on
            // `context.text_line(rect, font)` for the same reason.
            //
            // The box is handed over whole, with `Center` doing the horizontal centring. Computing
            // an `x` here *and* asking for `Center` would centre twice: the renderer offsets by
            // `(free + 1) / 2` from the origin it is given, so a pre-shifted origin lands the ink
            // half a box to the leading side.
            context.draw_text_line(box_, &self.text, &font, fg, HorizontalAlignment::Center);
        }
        if has_popup {
            // The arrow lives in the trailing strip `content_box` already removed from the content,
            // so its x is derived from that strip rather than from another literal. It is centred on
            // the button's middle line like the label, which is what makes a menu button read as one
            // row rather than two baselines.
            let strip = dimensions::TOOL_BUTTON_POPUP_ARROW_RESERVE as i32;
            let arrow_x = rect.x + rect.width as i32 - strip;
            // The origin is a glyph box's **top** edge, so `rect.y + height / 2` would put that edge
            // on the middle line and draw the arrow half a line low (the defect
            // `tools/check_text_vertically_centred.sh` names). `text_line` answers the question
            // directly: it is the band's own line box, centred, measured from the same font the ink
            // is drawn with — which is what the sibling label above already uses.
            let arrow_box =
                context.text_line(Rect::new(arrow_x, rect.y, strip as u32, rect.height), &font);
            context.draw_text(
                Point::new(arrow_box.x + strip / 2, arrow_box.y),
                "▾",
                &font,
                fg,
                HorizontalAlignment::Center,
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

    /// The popup arrow is centred on the button's middle line, not half a line below it.
    ///
    /// # The defect this pins
    ///
    /// The arrow was drawn at `rect.y + rect.height / 2`. A text origin is a glyph box's **top**
    /// edge, so that put the top edge on the middle line and the ink half a line low — the exact
    /// shape `tools/check_text_vertically_centred.sh` reports, and which that gate did report at
    /// this file's line until it was fixed.
    ///
    /// The assertion is on the **drawn geometry**, not on a helper's return value: the snapshot set
    /// cannot see this (the default tool button has no popup, so `tool_button.svg` never draws an
    /// arrow), which is why a unit test is the only thing standing between this and a regression.
    #[test]
    fn tool_button_popup_arrow_is_vertically_centred() {
        let mut btn = ToolButton::new("Menu", Rect::new(0, 0, 120, 40));
        btn.set_popup_mode(ToolButtonPopupMode::MenuButtonPopup);
        let svg = crate::widget::svg::render_widget_to_svg(&mut btn, Rect::new(0, 0, 120, 40));

        // An arrow glyph reaches the renderer as a text path; take its ink box.
        // An arrow glyph reaches the renderer as a text path; take its ink box.
        let ink = crate::widget::svg::text_ink_boxes(&svg);
        assert!(
            !ink.is_empty(),
            "the popup mode must draw the arrow, or this test asserts nothing"
        );
        // The **rightmost** box is the arrow: the label is drawn first and lies to its left. Reading
        // `ink[0]` instead picks up the label, which is centred in both the fixed and the broken
        // code — so a test written that way passes either way and pins nothing. (It did, and this
        // is the correction.)
        let (_, top, _, bottom) =
            *ink.iter().max_by_key(|(left, ..)| *left).expect("boxes were checked non-empty");
        let mid = 20i32; // half of 40
        let ink_mid = (top + bottom) / 2;
        assert!(
            (ink_mid - mid).abs() <= 2,
            "the arrow's ink must straddle the button's middle line ({mid}); it was {top}..{bottom} \
             (mid {ink_mid}) — half a line low is the defect this pins"
        );
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

    // ── 16. `button_style` decides where the label and the icon go ──
    #[test]
    fn the_style_decides_where_the_label_and_the_icon_go() {
        let mut btn = ToolButton::new("Open", rect());
        // Without an icon every style falls back to a full-box label: a button with nothing to
        // show is not a state a user can act on.
        for style in [
            ToolButtonStyle::IconOnly,
            ToolButtonStyle::TextOnly,
            ToolButtonStyle::TextBesideIcon,
            ToolButtonStyle::TextUnderIcon,
            ToolButtonStyle::FollowStyle,
        ] {
            btn.set_button_style(style);
            assert!(btn.icon_rect().is_none(), "{style:?} without an icon has no icon box");
            assert_eq!(btn.label_rect(), Some(btn.content_box()), "{style:?} label takes the box");
        }

        btn.set_icon(Some(PathBuf::from("icons/open.png")));

        // `IconOnly` with an icon draws exactly that, so there is no label box at all.
        btn.set_button_style(ToolButtonStyle::IconOnly);
        assert!(btn.icon_rect().is_some());
        assert!(btn.label_rect().is_none(), "icon-only hides the label");

        // `TextOnly` is the caller's instruction, not "draw whatever there is".
        btn.set_button_style(ToolButtonStyle::TextOnly);
        assert!(btn.icon_rect().is_none(), "text-only hides the icon");
        assert_eq!(btn.label_rect(), Some(btn.content_box()));

        // Beside: the label begins after the icon and its gap, so a wider icon pushes it along.
        btn.set_button_style(ToolButtonStyle::TextBesideIcon);
        let icon = btn.icon_rect().expect("beside has an icon");
        let label = btn.label_rect().expect("beside has a label");
        assert_eq!(
            label.x,
            icon.x
                + icon.width as i32
                + crate::widget::metrics::dimensions::TOOL_BUTTON_ICON_SPACING as i32,
            "the label starts after the icon's own advance"
        );
        assert!(label.width < btn.content_box().width, "the icon took room from the label");

        // Under: the label takes the lower band, so its top is at or below the icon's bottom.
        btn.set_button_style(ToolButtonStyle::TextUnderIcon);
        let icon = btn.icon_rect().expect("under has an icon");
        let label = btn.label_rect().expect("under has a label");
        assert!(
            label.y >= icon.y + icon.height as i32,
            "the stacked label sits below the icon (label.y={}, icon bottom={})",
            label.y,
            icon.y + icon.height as i32
        );
    }

    // ── 17. A real icon path is loaded and painted ──
    ///
    /// # Why a real file rather than injected pixels
    ///
    /// The claim is that *an icon path becomes a picture*, so the test writes the smallest real PNG
    /// it can and points the button at it. Injecting RGBA bytes would test the draw and skip the
    /// loader, which is the half that was dead.
    ///
    /// # Why the filename carries a per-test suffix
    ///
    /// [`crate::image::cache`] is process-wide and keyed on content, so two tests that write the
    /// *same path* with *different bytes* can observe each other. A name no other test uses is what
    /// makes each test's I/O its own. The bytes are the fixture `avatar` uses, so the two controls
    /// are proven against the same "a real image decodes" baseline rather than two hand-written
    /// PNGs that could each be wrong in their own way.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn a_real_icon_path_is_loaded_and_painted() {
        use crate::widget::svg::render_to_svg;

        let path = std::env::temp_dir().join("rw_tool_button_icon_real.png");
        std::fs::write(&path, MINIMAL_PNG).expect("the fixture is writable");

        let mut with_icon = ToolButton::new("Open", Rect::new(0, 0, 80, 30));
        with_icon.set_icon(Some(path.clone()));
        with_icon.set_button_style(ToolButtonStyle::IconOnly);
        let icon_svg = render_to_svg(&mut with_icon);

        let mut without_icon = ToolButton::new("Open", Rect::new(0, 0, 80, 30));
        without_icon.set_button_style(ToolButtonStyle::IconOnly);
        let plain_svg = render_to_svg(&mut without_icon);

        assert_ne!(icon_svg, plain_svg, "a real icon must change the rendering");
        // The SVG backend emits `<image>` for a non-empty `draw_image`, and nothing else does.
        assert!(icon_svg.contains("<image"), "the icon is painted as an image: {icon_svg}");
        assert!(!plain_svg.contains("<image"), "without a path nothing is painted");

        let _ = std::fs::remove_file(&path);
    }

    // ── 18. An unloadable icon leaves the layout alone ──
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn an_unloadable_icon_keeps_the_layout_and_paints_nothing() {
        use crate::widget::svg::render_to_svg;

        // The bytes are not a PNG, so the file reads but the decode fails -- the IO-succeeds path
        // rather than the IO-fails one.
        let path = std::env::temp_dir().join("rw_tool_button_icon_bad_bytes.png");
        std::fs::write(&path, b"this is not a png at all").expect("the fixture is writable");

        let mut btn = ToolButton::new("Open", Rect::new(0, 0, 80, 30));
        btn.set_icon(Some(path.clone()));
        btn.set_button_style(ToolButtonStyle::IconOnly);

        // The *layout* still reserves the icon (a path was set), so a failure cannot move the
        // label; only the pixels are absent. This is the deliberate non-degradation.
        assert!(btn.icon_rect().is_some(), "the slot is reserved by the path, not by the bytes");
        assert!(btn.label_rect().is_none(), "icon-only with an icon still hides the label");
        assert!(btn.resolved_icon().is_none(), "undecodable bytes produce no pixels");

        let svg = render_to_svg(&mut btn);
        assert!(!svg.contains("<image"), "nothing is painted for a failed decode: {svg}");

        // The failure is cached: the path is remembered, so the next frame does not re-read it.
        assert_eq!(btn.icon_pixels_key.as_ref(), Some(&path));

        let _ = std::fs::remove_file(&path);
    }

    // ── 19. Changing the icon path invalidates the decoded pixels ──
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn changing_the_icon_path_invalidates_the_decoded_pixels() {
        let path = std::env::temp_dir().join("rw_tool_button_icon_cache_a.png");
        std::fs::write(&path, b"not a png").expect("the fixture is writable");

        let mut btn = ToolButton::new("Open", Rect::new(0, 0, 80, 30));
        btn.set_icon(Some(path.clone()));
        let _ = btn.resolved_icon();
        // Even a failure is cached, so the key is populated.
        assert!(btn.icon_pixels_key.is_some());

        // A new path drops both halves together: leaving the key would make the next paint believe
        // the old pixels belonged to the new path.
        btn.set_icon(Some(std::env::temp_dir().join("rw_tool_button_icon_cache_b.png")));
        assert!(btn.icon_pixels_key.is_none());
        assert!(btn.icon_pixels.is_none());

        // Clearing the icon does the same, and removes the slot with it.
        let _ = btn.resolved_icon();
        btn.set_icon(None);
        assert!(btn.icon_pixels_key.is_none());
        assert!(btn.icon_pixels.is_none());
        assert!(btn.icon_rect().is_none(), "no path, no slot");

        // Re-setting the *same* path is a no-op, so the cache is not needlessly dropped.
        btn.set_icon(Some(path.clone()));
        let _ = btn.resolved_icon();
        let cached = btn.icon_pixels_key.clone();
        btn.set_icon(Some(path.clone()));
        assert_eq!(btn.icon_pixels_key, cached, "an unchanged path keeps its decoded pixels");

        let _ = std::fs::remove_file(&path);
    }

    /// The smallest valid PNG: a 2x2 truecolour-with-alpha image whose first pixel is opaque red.
    ///
    /// Written out as bytes rather than produced by an encoder so the fixture works wherever the
    /// `image` feature is on, including builds without the encoder side. The same fixture `avatar`
    /// uses, so both controls are held to one "a real image decodes" baseline.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    const MINIMAL_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR, 13 data bytes
        0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, // width 2, height 2
        0x08, 0x06, 0x00, 0x00, 0x00, 0x72, 0xb6, 0x0d, // 8-bit, colour type 6 (RGBA), CRCs
        0x24, 0x00, 0x00, 0x00, 0x0f, 0x49, 0x44, 0x41, // IDAT, 15 data bytes
        0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00, // zlib stream
        0x44, 0x48, 0x00, 0x00, 0x1e, 0xf3, 0x01, 0xff, // ...
        0x6a, 0x37, 0x5d, 0xad, 0x00, 0x00, 0x00, 0x00, // IDAT CRC
        0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82, // IEND
    ];
}
