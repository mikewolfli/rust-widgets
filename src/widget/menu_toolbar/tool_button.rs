// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool button widget.
use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Rect};
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
/// centred and fades toward the fill while the button is disabled. The icon is loaded from the path
/// [`ToolButton::set_icon`] stores, once per path, and is laid out by [`ToolButtonStyle`].
/// Horizontal padding between the button's edge and its label.
const BUTTON_PADDING: i32 = 4;

/// The square an icon occupies when the style asks for one, in logical pixels.
///
/// The same size a `Button`'s own icon uses, so two toolbar-shaped controls do not disagree about
/// how big an icon is.
const ICON_SIZE: u32 = 16;

/// The gap between the icon and the label when the style puts them on one line.
const ICON_SPACING: u32 = 4;

/// Width reserved at the trailing edge for the popup indicator arrow.
const POPUP_ARROW_RESERVE: i32 = 12;

/// Tool button widget.
///
/// A `ToolButton` is a compact icon-or-text button for a toolbar strip: it can be
/// checkable, can carry a popup-menu indicator, and can opt into `auto_raise` so it
/// stays flat until the pointer is over it. Chrome resolves the explicit style first,
/// then the theme, so its fill, ink and accent all move with the active appearance.
pub struct ToolButton {
    base: BaseWidget,
    text: String,
    icon: Option<PathBuf>,
    /// RGBA8 pixels for [`Self::icon`], decoded once per path.
    ///
    /// `None` is cached too, so an icon path that cannot be read is not re-read every frame. The
    /// pixels are square RGBA8 (width * height * 4 bytes), which is what `draw_image` takes.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    icon_pixels: Option<Vec<u8>>,
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
    /// style hides the label even though one is set.
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
    /// The path is stored verbatim and decoded lazily by [`Self::resolved_icon`] on the first
    /// paint -- not here, because a caller that sets an icon on a control it never shows should not
    /// pay for the decode, and because a decode at set time would have to run inside the event
    /// handler that called it.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// The path was stored, published (getter, setter, schema row, round-trip test) and read by
    /// nothing: `draw` never looked at it, and the control's own doc said so -- "the icon path is
    /// stored but not decoded or painted by this widget".
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

    /// The RGBA8 pixels for the current icon path, decoding once per path.
    ///
    /// Returns `None` when there is no icon, when the file cannot be read, or when the bytes do not
    /// decode -- in which case the button simply shows no icon rather than a broken-image glyph, the
    /// same fallback `avatar` uses for the same reason: a toolbar's shape must not change because
    /// one of its images is missing.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    fn resolved_icon(&mut self) -> Option<Vec<u8>> {
        let path = self.icon.as_ref()?;
        if self.icon_pixels_key.as_ref() != Some(path) {
            let decoded =
                std::fs::read(path).map_err(|error| error.to_string()).and_then(|bytes| {
                    crate::image::decoder::decode_to_rgba8(&bytes)
                        .map_err(|error| error.to_string())
                });
            match decoded {
                Ok(image) => {
                    if let crate::image::ImageData::Rgba8(pixels) = image.data {
                        // The decoded extent is dropped: the icon is scaled to the slot the style
                        // derivation reserved, so a non-square source would otherwise change the
                        // button's layout. Only the pixels are kept.
                        self.icon_pixels = Some(pixels);
                    } else {
                        // `decode_to_rgba8` guarantees the `Rgba8` variant; this arm is written out
                        // rather than `unwrap`ed because an unreachable panic inside `draw` is a
                        // worse failure than an icon that does not show.
                        self.icon_pixels = None;
                    }
                }
                Err(reason) => {
                    log::warn!(
                        "tool button icon {path:?} could not be loaded ({reason}); no icon is drawn"
                    );
                    self.icon_pixels = None;
                }
            }
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

    /// The button's content box: its own frame inset by [`BUTTON_PADDING`], less the room the popup
    /// arrow reserves at the trailing edge when one is drawn.
    ///
    /// One derivation for both content slots and the arrow, so the three cannot disagree about
    /// where the button's interior ends.
    fn content_box(&self) -> Rect {
        let rect = self.geometry();
        let has_popup = self.popup_mode == ToolButtonPopupMode::MenuButtonPopup
            || self.popup_mode == ToolButtonPopupMode::InstantPopup;
        let left = rect.x + BUTTON_PADDING;
        let right = rect.x + rect.width as i32
            - if has_popup { POPUP_ARROW_RESERVE } else { BUTTON_PADDING };
        Rect::new(left, rect.y, (right - left).max(0) as u32, rect.height)
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
        let size = ICON_SIZE.min(content.width).min(band.height);
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
                let advance = (ICON_SIZE + ICON_SPACING).min(content.width);
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
    /// that leaves less than [`ICON_SIZE`] would squeeze the icon into a sliver, which is what a split
    /// driven by whole lines alone produced in a short button. Whatever is left after the icon and its
    /// gap is the label's, floored to whole lines so the band never clips the glyphs it draws.
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
        let reserved = (ICON_SIZE + ICON_SPACING / 2).min(content.height);
        let remainder = content.height.saturating_sub(reserved);
        // A remainder shorter than one line is still given to the label rather than discarded: a
        // button with no room for a line is better drawn with a cramped one than with the icon on its
        // own, which would make the style's instruction unobservable.
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
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        let side = crate::widget::metrics::dimensions::TOOL_BUTTON_SIZE;
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
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            // A disabled button is inert, but the base still records the pointer facts so
            // hover keeps working once it is re-enabled.
            self.base.handle_event(event);
            return;
        }
        // The gesture is resolved **before** the base records the event: the base clears
        // `pressed`/`grabbed` on the release, so a control that asked afterwards could no
        // longer tell whether the release belonged to its own press.
        match event {
            Event::MouseEnter { pos: _ } => {}
            Event::MouseLeave { pos: _ } => {
                // Crossing the edge abandons the press; the grab is kept by the base, which is
                // what lets a drag that returns still complete.
                self.base.set_pressed(false);
            }
            Event::MousePress { button: 1, .. } => {
                self.base.set_pressed(true);
            }
            Event::MouseRelease { button: 1, .. } if self.base.is_pressed() => {
                self.base.set_pressed(false);
                self.click();
            }
            Event::KeyPress { key: 13, .. } | Event::KeyPress { key: 32, .. } => {
                self.click();
            }
            _ => { /* Other events are not relevant */ }
        }
        // Record the primitive facts after the gesture, so `widget_state`/hover stay right.
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
        //
        // The surface literal is the *last* step, not the whole answer: it used to be read
        // unconditionally, which left `themed_bg` resolved and then discarded — the four
        // state fills were therefore constant across appearances and the census reported
        // the control as theme-blind. Reading the resolved surface here is what makes the
        // states move when the theme does.
        let base = style.background_color.or(themed_bg).unwrap_or(Color::rgb(240, 240, 240));
        let accent = themed_border.unwrap_or(Color::rgb(0, 120, 215));
        // The interaction states are **derived from the base**, not collapsed into one
        // colour, so the four states stay distinguishable on any theme: a press is a step
        // toward the accent, a hover is a step toward white, and a toggled-on button keeps
        // the previous checked/hover ordering.
        let bg = if self.base.is_pressed() {
            base.blend(&accent, 0.45)
        } else if self.checked {
            base.blend(&accent, 0.25)
        } else if self.base.is_hovered() && !self.auto_raise {
            base.blend(&Color::WHITE, 0.55)
        } else if self.auto_raise && !self.base.is_hovered() {
            Color::rgba(0, 0, 0, 0) // transparent
        } else {
            base
        };
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        if self.base.is_hovered() || self.base.is_pressed() || self.checked {
            context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), accent);
        }
        // A disabled label is the ink faded toward the fill behind it, which keeps it
        // readable-but-muted on a dark theme as well as a light one; the literal is only
        // the fallback for a control whose ink the theme does not supply.
        let ink = style.text_color.or(themed_text).unwrap_or_else(|| bg.contrast_color());
        let fg = if !self.base.is_enabled() { ink.blend(&base, 0.45) } else { ink };
        // Popup arrow indicator
        let has_popup = self.popup_mode == ToolButtonPopupMode::MenuButtonPopup
            || self.popup_mode == ToolButtonPopupMode::InstantPopup;

        let font = Font::default();
        let line = context.text_line(rect, &font);

        // ── The icon ──
        //
        // The box comes from `icon_rect`, the same derivation that decides whether the label gets
        // the content box or only part of it -- so a style change moves the two together rather than
        // letting a label overlap the icon it was told to sit beside. The path is decoded once per
        // path by `resolved_icon`, so a paint does not read the file.
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        if let Some(box_) = self.icon_rect() {
            if let Some(pixels) = self.resolved_icon() {
                context.draw_image(box_.x, box_.y, box_.width, box_.height, &pixels);
            }
        }

        // The label is **centred in its own content box**. The previous form put the glyph
        // origin — which is the box's top-left — at `rect.x + (content - rect.x) / 2`, i.e. the
        // horizontal midpoint of the content, and then asked for `HorizontalAlignment::Left`.
        // The result was a label that began at the middle of the button and ran off its right
        // edge, with its top edge on the vertical midline: the signature of "computed a centre,
        // drew from it as an origin".
        //
        // The box is now the *trailing slot* of the content box, from `label_rect`, so it is the
        // room after the icon when one is beside the label and the whole box otherwise.
        if let Some(label_box) = self.label_rect() {
            // A label under an icon is centred in the lower half; a label beside one is centred in
            // the space that is left, which is what `text_line` already does for its own band.
            let band = Rect::new(
                label_box.x,
                context.text_line(label_box, &font).y,
                label_box.width,
                line.height,
            );
            context.draw_text_fitted(band, &self.text, &font, fg, HorizontalAlignment::Center);
        }

        if has_popup {
            // The arrow sits on the same line box as the label and is centred on it, instead
            // of being pinned to a hardcoded 6 px above the button's bottom edge.
            context.draw_text_fitted(
                Rect {
                    x: rect.x + rect.width as i32 - POPUP_ARROW_RESERVE,
                    y: line.y,
                    width: POPUP_ARROW_RESERVE as u32,
                    height: line.height,
                },
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
    use super::{ToolButton, ToolButtonPopupMode, ToolButtonStyle, ICON_SPACING};
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    use crate::event::EventHandler;
    use crate::render::svg::SvgPaintBackend;
    use crate::render::PaintBackend;
    use crate::render::RenderContext;
    use crate::widget::{Draw, Widget, WidgetKind};
    use std::path::{Path, PathBuf};
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

    // ── 16. `icon` and `button_style` decide the content layout ──

    /// A style change moves the two content slots, and the label is *not* drawn where the icon is.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `button_style` was stored, published and read by a `match` whose four arms all returned
    /// `&self.text` -- a four-way branch with one answer. The label was drawn in the content box
    /// whichever style was set, so `IconOnly` and `TextBesideIcon` rendered identically. The
    /// assertion is on the *boxes*, because that is what a paint has to obey.
    #[test]
    fn the_style_decides_where_the_label_and_the_icon_go() {
        let build = |style, with_icon: bool| {
            let mut btn = ToolButton::new("Go", rect());
            btn.set_button_style(style);
            if with_icon {
                btn.set_icon(Some(PathBuf::from("icons/go.png")));
            }
            btn
        };

        // Without an icon, every style falls back to the label filling the content box. A blank
        // button is a state the user cannot act on, so this is the whole answer for `IconOnly` too.
        for style in [
            ToolButtonStyle::IconOnly,
            ToolButtonStyle::TextOnly,
            ToolButtonStyle::TextBesideIcon,
            ToolButtonStyle::TextUnderIcon,
            ToolButtonStyle::FollowStyle,
        ] {
            let btn = build(style, false);
            assert!(!btn.wants_icon(), "{style:?} has no icon to want");
            assert!(btn.wants_label(), "{style:?} must still show its label");
            assert!(btn.icon_rect().is_none());
            assert_eq!(
                btn.label_rect(),
                Some(btn.content_box()),
                "{style:?} gives the label the whole content box without an icon"
            );
        }

        // Icon-only with an icon draws no label, which is what the variant's name means.
        let icon_only = build(ToolButtonStyle::IconOnly, true);
        assert!(icon_only.wants_icon());
        assert!(!icon_only.wants_label(), "icon-only draws no label");
        assert!(icon_only.label_rect().is_none());
        assert!(icon_only.icon_rect().is_some());

        // `TextOnly` with an icon set still draws no icon: the style is the caller's instruction.
        let text_only = build(ToolButtonStyle::TextOnly, true);
        assert!(!text_only.wants_icon(), "text-only ignores the icon");
        assert_eq!(text_only.label_rect(), Some(text_only.content_box()));

        // Beside: the label starts after the icon and its gap, so the two cannot overlap.
        let beside = build(ToolButtonStyle::TextBesideIcon, true);
        let icon = beside.icon_rect().expect("beside shows an icon");
        let label = beside.label_rect().expect("beside shows a label");
        assert_eq!(icon.x, beside.content_box().x, "the icon is the leading slot");
        assert_eq!(
            label.x,
            icon.x + icon.width as i32 + ICON_SPACING as i32,
            "the label begins after the icon and its gap"
        );
        assert!(
            label.x + label.width as i32
                <= beside.content_box().x + beside.content_box().width as i32,
            "and the label stays inside the content box"
        );

        // Under: the label takes the lower half, below the icon rather than across it.
        let under = build(ToolButtonStyle::TextUnderIcon, true);
        let icon = under.icon_rect().expect("under shows an icon");
        let label = under.label_rect().expect("under shows a label");
        assert!(
            label.y >= icon.y + icon.height as i32,
            "the label is below the icon: {label:?} vs {icon:?}"
        );
        assert!(
            icon.x + icon.width as i32 <= under.content_box().x + under.content_box().width as i32,
            "and the icon stays inside the content box"
        );
    }

    /// A real icon file is decoded, painted, and cached per path.
    ///
    /// The layout assertions above say where the icon's box is; this says the pixels reach the
    /// document. Without it, an `icon_rect` nothing drew would satisfy every number.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn a_real_icon_path_is_loaded_and_painted() {
        let path = std::env::temp_dir().join("rw_tool_button_icon_test.png");
        std::fs::write(&path, MINIMAL_PNG).expect("the fixture is writable");

        let render = |source: Option<&Path>| {
            let mut btn = ToolButton::new("Go", Rect::new(0, 0, 60, 24));
            btn.set_icon(source.map(Path::to_path_buf));
            let (w, h) = (btn.geometry().width, btn.geometry().height);
            let mut backend = SvgPaintBackend::new(Size::new(w, h));
            backend.begin_frame(Color::WHITE);
            let mut ctx = RenderContext::new(&mut backend);
            btn.draw(&mut ctx);
            backend.end_frame();
            backend.finish()
        };

        let without = render(None);
        let with = render(Some(&path));
        assert_ne!(
            without, with,
            "a real icon must change the rendering -- that was the dead state"
        );
        assert!(with.contains("<image"), "the decoded pixels reach the document");
        assert!(!without.contains("<image"), "and nothing is embedded without an icon");

        let _ = std::fs::remove_file(&path);
    }

    /// A path that will not load leaves the button intact rather than blanking or panicking it.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn an_unloadable_icon_falls_back_to_the_label() {
        let mut btn = ToolButton::new("Go", rect());
        btn.set_button_style(ToolButtonStyle::IconOnly);
        btn.set_icon(Some(PathBuf::from("/definitely/not/a/real/icon.png")));
        // The style asks for an icon and the path exists as a *path*, so the box is reserved ...
        assert!(btn.icon_rect().is_some(), "the style still reserves the icon's slot");
        // ... but nothing decodes, so no pixels are handed to the paint.
        assert!(btn.resolved_icon().is_none(), "an unreadable path decodes to nothing");
        // The label is *not* brought back: `IconOnly` is the caller's instruction, and silently
        // switching layouts because a file is missing would move the button's content under a user
        // who did not ask for it.
        assert!(btn.label_rect().is_none());

        // The failure is cached, so a paint does not re-read the file every frame.
        assert_eq!(
            btn.icon_pixels_key.as_deref(),
            Some(Path::new("/definitely/not/a/real/icon.png")),
            "the failure is recorded against the path"
        );
        assert!(btn.resolved_icon().is_none());
    }

    /// Changing the path drops the cached pixels, so a new icon cannot paint the old one.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn changing_the_icon_path_invalidates_the_decoded_pixels() {
        let path = std::env::temp_dir().join("rw_tool_button_cache_test.png");
        std::fs::write(&path, MINIMAL_PNG).expect("the fixture is writable");

        let mut btn = ToolButton::new("Go", rect());
        btn.set_icon(Some(path.clone()));
        assert!(btn.resolved_icon().is_some(), "the fixture decodes");
        assert_eq!(btn.icon_pixels_key.as_deref(), Some(path.as_path()));

        btn.set_icon(Some(PathBuf::from("/somewhere/else.png")));
        assert!(btn.icon_pixels.is_none(), "the pixels are dropped with the path");
        assert!(btn.icon_pixels_key.is_none(), "and so is the key they belong to");

        // Clearing the icon drops them too.
        btn.set_icon(Some(path.clone()));
        btn.resolved_icon();
        btn.set_icon(None);
        assert!(btn.icon_pixels.is_none());
        assert!(btn.icon_pixels_key.is_none());

        let _ = std::fs::remove_file(&path);
    }

    /// The smallest valid PNG: a 2x2 truecolour-with-alpha image, first pixel opaque red.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    const MINIMAL_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR
        0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, // width 2, height 2
        0x08, 0x06, 0x00, 0x00, 0x00, 0x72, 0xb6, 0x0d, // 8-bit, RGBA
        0x24, 0x00, 0x00, 0x00, 0x0f, 0x49, 0x44, 0x41, // IDAT
        0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00, // zlib stream
        0x44, 0x48, 0x00, 0x00, 0x1e, 0xf3, 0x01, 0xff, // ...
        0x6a, 0x37, 0x5d, 0xad, 0x00, 0x00, 0x00, 0x00, // IDAT CRC
        0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82, // IEND
    ];
}
