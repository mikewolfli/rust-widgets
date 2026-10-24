// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool button widget.
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::menu_button::{
    draw_menu_panel, menu_item_rect, menu_panel_rect, MenuItem,
};
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
use crate::widget::metrics::{dimensions, effective_font, estimate_line_height, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::path::{Path, PathBuf};

/// How long a [`ToolButtonPopupMode::DelayedPopup`] press must be held before the menu opens.
///
/// Five hundred milliseconds is the threshold the popup family has always described in words
/// ("long or delayed press"); keeping it a named constant is what makes it observable to
/// `Widget::tick` and to the tests that pin the boundary.
const DELAYED_POPUP_MS: u32 = 500;

/// Tool button popup mode.
///
/// Selects how the button's attached menu is presented. A menu set with
/// [`ToolButton::set_menu`] is owned and painted by the button itself: the mode only decides
/// which gesture opens it. A button with no menu keeps its plain push-button behaviour, so
/// the default mode cannot turn every tool-bar button into a popup.
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

/// Which way the popup indicator arrow of a tool button points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolButtonArrowType {
    /// A downward chevron, the historical indicator.
    #[default]
    Down,
    /// An upward chevron.
    Up,
    /// A leftward chevron.
    Left,
    /// A rightward chevron.
    Right,
    /// No indicator; the whole button becomes the popup target.
    None,
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
    icon_source: Option<IconSource>,
    /// RGBA8 pixels for [`Self::icon`], decoded once per path.
    ///
    /// `None` is cached too, so an icon path that cannot be read is not re-read every frame. The
    /// handle is the decode cache's own allocation (see
    /// [`SharedIconPixels::from_cached_file`]), so the pixels are shared process-wide rather than
    /// copied per button, and the decoded extent is kept rather than dropped — the draw needs it to
    /// fit the image instead of stretching it to the slot.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    icon_pixels: Option<crate::widget::icon_source::SharedIconPixels>,
    /// The path [`Self::icon_pixels`] was produced for, so changing the path invalidates the pixels
    /// without the caller having to clear them.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    icon_pixels_key: Option<PathBuf>,
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    shared_icon_pixels: Option<crate::widget::icon_source::SharedIconPixels>,
    checkable: bool,
    checked: bool,
    popup_mode: ToolButtonPopupMode,
    /// Which direction the popup indicator points.
    arrow_type: ToolButtonArrowType,
    button_style: ToolButtonStyle,
    auto_raise: bool,
    /// The rows of the menu this button owns. Empty means the button has no menu, and every
    /// popup mode then behaves as a plain push button.
    menu_items: Vec<MenuItem>,
    /// Whether that menu is currently showing.
    menu_open: bool,
    /// The menu's open/close reveal, so the panel grows out of the button rather than appearing.
    reveal: PopupReveal,
    /// Whether a `DelayedPopup` press is being held, waiting to reach
    /// [`DELAYED_POPUP_MS`].
    delayed_press: bool,
    /// How long the current held press has been accumulating, in milliseconds.
    delayed_elapsed_ms: u32,
    /// Set by a press that the open menu consumed (or that opened it), so the matching release
    /// does not also fire the button's main action.
    suppress_click: bool,
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
            icon_source: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            icon_pixels: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            icon_pixels_key: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            shared_icon_pixels: None,
            checkable: false,
            checked: false,
            popup_mode: ToolButtonPopupMode::DelayedPopup,
            arrow_type: ToolButtonArrowType::Down,
            button_style: ToolButtonStyle::IconOnly,
            auto_raise: false,
            menu_items: Vec::new(),
            menu_open: false,
            reveal: PopupReveal::new(false),
            delayed_press: false,
            delayed_elapsed_ms: 0,
            suppress_click: false,
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

    /// Returns the legacy icon file path, or `None` when no path is active.
    ///
    /// This returns `None` while a typed icon source is active; use
    /// [`Self::icon_source`] to inspect that source.
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
        let text = text.into();
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
    }

    /// Sets or clears the icon path and requests a redraw.
    ///
    /// The path is stored verbatim and decoded lazily by [`Self::resolved_icon`] on the first
    /// paint -- not here, because a caller that sets an icon on a control it never shows should not
    /// pay for the decode, and because a decode at set time would have to run inside the event
    /// handler that called it.
    pub fn set_icon(&mut self, icon: Option<PathBuf>) {
        if self.icon == icon && self.icon_source.is_none() && {
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            {
                self.shared_icon_pixels.is_none()
            }
            #[cfg(not(all(feature = "image", not(alloc_frugal))))]
            {
                true
            }
        } {
            return;
        }
        self.icon = icon;
        self.icon_source = None;
        // Both halves of the cache drop together. Leaving the key would make the next paint believe
        // the pixels belonged to the new path -- a worse failure than having no cache at all.
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        {
            self.icon_pixels = None;
            self.icon_pixels_key = None;
            self.shared_icon_pixels = None;
        }
        self.base.request_redraw();
    }

    /// Sets a built-in or static-geometry icon, replacing any legacy file path.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        if self.icon_source == source && self.icon.is_none() && {
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            {
                self.shared_icon_pixels.is_none()
            }
            #[cfg(not(all(feature = "image", not(alloc_frugal))))]
            {
                true
            }
        } {
            return;
        }
        self.icon_source = source;
        self.icon = None;
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        {
            self.icon_pixels = None;
            self.icon_pixels_key = None;
            self.shared_icon_pixels = None;
        }
        self.base.request_redraw();
    }

    /// Sets immutable RGBA8 icon pixels shared with other controls.
    ///
    /// The handle clone is constant-time; use
    /// [`crate::widget::icon_source::SharedIconPixels::from_image`] to move a decoded RGBA8 image
    /// into shared storage without copying its pixel vector.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn set_shared_icon_pixels(
        &mut self,
        pixels: Option<crate::widget::icon_source::SharedIconPixels>,
    ) {
        self.shared_icon_pixels = pixels;
        self.icon = None;
        self.icon_source = None;
        self.icon_pixels = None;
        self.icon_pixels_key = None;
        self.base.request_redraw();
    }

    /// Returns the shared pixel handle, if one is active.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn shared_icon_pixels(&self) -> Option<&crate::widget::icon_source::SharedIconPixels> {
        self.shared_icon_pixels.as_ref()
    }

    /// Returns the typed icon source, if one was most recently assigned.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
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
    /// [`crate::image::cache`] is what makes **twelve buttons on one icon** decode it once. The
    /// handle is the cache's allocation, so the pixels are not copied into this button either.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    fn resolved_icon(&mut self) -> Option<crate::widget::icon_source::SharedIconPixels> {
        let path = self.icon.as_ref()?;
        if self.icon_pixels_key.as_ref() != Some(path) {
            self.icon_pixels = crate::widget::SharedIconPixels::from_cached_file(path);
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
        if self.checkable == v {
            return;
        }
        self.checkable = v;
        if !v {
            self.checked = false;
        }
        self.base.request_redraw();
    }

    /// Sets the popup mode and requests a redraw.
    pub fn set_popup_mode(&mut self, mode: ToolButtonPopupMode) {
        if self.popup_mode == mode {
            return;
        }
        self.popup_mode = mode;
        self.base.request_redraw();
    }

    /// Which way the popup indicator points.
    pub fn arrow_type(&self) -> ToolButtonArrowType {
        self.arrow_type
    }

    /// Sets the indicator's direction and requests a redraw.
    ///
    /// [`ToolButtonArrowType::None`] removes the indicator and gives its trailing strip back
    /// to the label, which also makes the whole button the popup target.
    pub fn set_arrow_type(&mut self, arrow: ToolButtonArrowType) {
        if self.arrow_type == arrow {
            return;
        }
        self.arrow_type = arrow;
        self.base.request_redraw();
    }

    /// Replaces the button's menu rows and requests a redraw.
    ///
    /// An empty list removes the menu: every popup mode then keeps the plain push-button
    /// gesture, so a tool-bar button that was never given a menu cannot open an empty panel.
    pub fn set_menu(&mut self, items: Vec<MenuItem>) {
        self.menu_items = items;
        if self.menu_items.is_empty() {
            self.close_menu();
        }
        self.base.request_redraw();
    }

    /// Returns the menu rows this button owns.
    pub fn menu_items(&self) -> &[MenuItem] {
        &self.menu_items
    }

    /// Returns whether the button's menu is currently showing.
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// Opens the menu and takes the popup grab, so a press outside it is routed here.
    pub fn open_menu(&mut self) {
        self.menu_open = true;
        self.delayed_press = false;
        self.delayed_elapsed_ms = 0;
        self.reveal.set_open(true);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Closes the menu and releases the popup grab.
    pub fn close_menu(&mut self) {
        self.menu_open = false;
        self.delayed_press = false;
        self.delayed_elapsed_ms = 0;
        self.reveal.set_open(false);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.id());
        self.base.request_redraw();
    }

    /// Whether this mode paints an indicator arrow and reserves room for it.
    fn popup_capable(&self) -> bool {
        self.arrow_type != ToolButtonArrowType::None
            && matches!(
                self.popup_mode,
                ToolButtonPopupMode::MenuButtonPopup | ToolButtonPopupMode::InstantPopup
            )
    }

    /// The trailing strip the indicator arrow occupies, or `None` when no arrow is drawn.
    ///
    /// This is the single derivation of the arrow's rectangle: the paint and the arrow's hit
    /// test must agree, or a button would react to a click beside the glyph.
    fn popup_arrow_rect(&self) -> Option<Rect> {
        if !self.popup_capable() {
            return None;
        }
        let rect = self.geometry();
        let strip = dimensions::TOOL_BUTTON_POPUP_ARROW_RESERVE as i32;
        Some(Rect::new(
            rect.x + rect.width as i32 - strip,
            rect.y,
            strip.max(1) as u32,
            rect.height.max(1),
        ))
    }

    /// The menu panel's rectangle, hung below the button.
    fn menu_rect(&self) -> Rect {
        menu_panel_rect(self.geometry(), self.menu_items.len())
    }

    /// The enabled menu row at `pos`, when the menu is open.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let panel = self.menu_rect();
        if !panel.contains_point(pos) {
            return None;
        }
        self.menu_items.iter().enumerate().find_map(|(index, item)| {
            let row = menu_item_rect(panel, index);
            (item.enabled && row.contains_point(pos)).then_some(index)
        })
    }

    /// Sets the button style and requests a redraw.
    pub fn set_button_style(&mut self, style: ToolButtonStyle) {
        if self.button_style == style {
            return;
        }
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
        if self.icon.is_none() && self.icon_source.is_none() && {
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            {
                self.shared_icon_pixels.is_none()
            }
            #[cfg(not(all(feature = "image", not(alloc_frugal))))]
            {
                true
            }
        } {
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
        let has_popup = self.popup_capable();
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
        // give its label a band that clips the glyphs it draws. The line is measured with the
        // **effective font** so the band matches the text the draw path paints (D09-STYLE-01).
        let line = estimate_line_height(effective_font(self.style()), 1.0).max(1);
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
        if self.auto_raise == v {
            return;
        }
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

    /// The button and its open menu accept the pointer.
    ///
    /// The panel hangs below the button's rectangle; without this override a press on a menu row
    /// is rejected by the hit test and never reaches `hit_test_item`.
    fn contains_point(&self, point: Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
            || (self.menu_open && self.menu_rect().contains_point(point))
    }

    fn has_open_popup(&self) -> bool {
        self.menu_open
    }

    /// Advances the reveal and the delayed-popup timer.
    ///
    /// A held `DelayedPopup` press is a timed gesture, not an event: nothing else would ever
    /// turn the press into an open menu, so the accumulation happens here and reports frames
    /// for as long as the hold is pending.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut animating = self.reveal.tick(delta_ms);
        if self.delayed_press && !self.menu_open && self.base.is_enabled() {
            self.delayed_elapsed_ms = self.delayed_elapsed_ms.saturating_add(delta_ms);
            if self.delayed_elapsed_ms >= DELAYED_POPUP_MS {
                self.delayed_press = false;
                if !self.menu_items.is_empty() {
                    self.open_menu();
                    self.suppress_click = true;
                }
            }
            // The opening itself started a reveal, so this call still owes a frame either way.
            animating = true;
        }
        animating
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating() || (self.delayed_press && !self.menu_open)
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
            "clicked" => Some(EventSignalRef::mapped("clicked", &self.clicked, |v| {
                CapabilityValue::Bool(*v)
            })),
            "toggled" => Some(EventSignalRef::mapped("toggled", &self.toggled, |v| {
                CapabilityValue::Bool(*v)
            })),
            "triggered" => Some(EventSignalRef::unit("triggered", &self.triggered)),
            _ => None,
        }
    }
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
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
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
        property_names_of!["text", "checked", "icon_source", BASE_PROPERTY_NAMES]
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
    ///
    /// # Popup modes
    ///
    /// When a menu is set, the three modes differ only in the gesture that opens it:
    ///
    /// | mode | opening gesture |
    /// |---|---|
    /// | `MenuButtonPopup` | press on the trailing arrow zone |
    /// | `InstantPopup` | any press on the button |
    /// | `DelayedPopup` | a press held for `DELAYED_POPUP_MS`, accumulated by [`Widget::tick`] |
    ///
    /// A press the open menu consumes (a row, the button that toggles it closed, or the press
    /// that opened it) suppresses the release's main action, so a menu interaction never doubles
    /// as a click.
    fn handle_event(&mut self, event: &Event) {
        match event {
            // Only a press that resolves to this control (or to its open panel) arms a gesture.
            // The runtime hit-tests before delivery, but a direct dispatch does not, and an
            // unguarded press would leave the latch armed for a release that belongs elsewhere.
            Event::MousePress { pos, button, .. }
                if self.base.is_enabled()
                    && *button == crate::event::mouse_button::PRIMARY
                    && (self.base.contains_point_with_touch_expansion(*pos)
                        || (self.menu_open && self.menu_rect().contains_point(*pos))) =>
            {
                if self.menu_open && self.menu_rect().contains_point(*pos) {
                    // A press on the open panel selects a row; it is not a new button gesture.
                    if self.hit_test_item(*pos).is_some() {
                        self.triggered.emit();
                    }
                    self.suppress_click = true;
                    self.close_menu();
                } else if self.menu_open {
                    // A press on the button while its menu is open closes it, the toggle a menu
                    // button performs, instead of also performing the main action.
                    self.suppress_click = true;
                    self.close_menu();
                } else {
                    self.delayed_press = false;
                    self.delayed_elapsed_ms = 0;
                    self.suppress_click = false;
                    match self.popup_mode {
                        ToolButtonPopupMode::InstantPopup if !self.menu_items.is_empty() => {
                            self.open_menu();
                            self.suppress_click = true;
                        }
                        ToolButtonPopupMode::MenuButtonPopup
                            if !self.menu_items.is_empty()
                                && self
                                    .popup_arrow_rect()
                                    .is_some_and(|zone| zone.contains_point(*pos)) =>
                        {
                            self.open_menu();
                            self.suppress_click = true;
                        }
                        ToolButtonPopupMode::DelayedPopup => {
                            self.delayed_press = true;
                            self.base.set_pressed(true);
                        }
                        _ => {
                            self.base.set_pressed(true);
                        }
                    }
                }
            }
            // A press outside the button and its panel closes the menu. The runtime's popup grab
            // routes such a press here even though it hit nothing, so this arm is what makes the
            // grab's "outside press dismisses" contract observable.
            Event::MousePress { button, .. }
                if self.base.is_enabled()
                    && *button == crate::event::mouse_button::PRIMARY
                    && self.menu_open =>
            {
                self.suppress_click = true;
                self.close_menu();
            }
            // A press that wanders off cancels, and one that wanders back re-arms: `pressed` is a
            // continuous quantity, not an edge. Guarded on the grab rather than on `pressed` so
            // that the move which should restore the state is not the one that exits early.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.base.is_grabbed() => {
                let inside = self.base.contains_point_with_touch_expansion(*pos);
                self.base.set_pressed(inside);
                if !inside {
                    // Dragging off a held press abandons the delayed-popup gesture, the same way
                    // it abandons an ordinary press-to-click gesture.
                    self.delayed_press = false;
                    self.delayed_elapsed_ms = 0;
                }
            }
            // `is_pressed` still answers for the gesture that is ending, because the base has not
            // yet cleared it (the base records after this match). A release inside is an
            // activation; a release outside was already cancelled by the move arm.
            Event::MouseRelease { .. } | Event::PointerRelease { .. }
                if self.base.is_grabbed() && self.base.is_pressed() =>
            {
                if !self.suppress_click {
                    self.click();
                }
                self.suppress_click = false;
                self.delayed_press = false;
                self.delayed_elapsed_ms = 0;
            }
            Event::MouseRelease { .. } | Event::PointerRelease { .. } => {
                self.suppress_click = false;
                self.delayed_press = false;
                self.delayed_elapsed_ms = 0;
            }
            Event::KeyPress { key, .. } if *key == key_codes::ESCAPE && self.menu_open => {
                self.close_menu();
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
        // The style is cloned rather than borrowed because the icon decode below (`resolved_icon`)
        // needs `&mut self`; a borrow of the base would keep the whole widget borrowed through it.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style_shared("tool_button");
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
        context.face_with_gradient(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            bg,
            style.background_gradient.as_ref(),
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
        let fg = self.base.disabled_ink_on(ink, base);

        // ── Icon ──
        //
        // The icon is the leading slot (or the upper band), and its box comes from `icon_rect` --
        // the same derivation `ToolButtonStyle` documents -- so the room the style reserves and the
        // pixels drawn cannot disagree. Pixels are painted only when the path actually decoded:
        // a missing file leaves the slot empty instead of shifting the label onto the icon.
        if let (Some(source), Some(box_)) = (self.icon_source, self.icon_rect()) {
            if let Err(error) = crate::widget::paint_icon(context, source.into(), box_, fg) {
                log::warn!("tool button icon could not be painted: {error:?}");
            }
        }
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        if self.icon_source.is_none() && self.shared_icon_pixels.is_some() {
            if let (Some(pixels), Some(box_)) = (self.shared_icon_pixels.as_ref(), self.icon_rect())
            {
                let fitted = pixels.fit_into(box_);
                context.draw_image(
                    fitted.x,
                    fitted.y,
                    fitted.width,
                    fitted.height,
                    pixels.pixels(),
                );
            }
        }
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        if self.icon_source.is_none() && self.shared_icon_pixels.is_none() {
            if let (Some(pixels), Some(box_)) = (self.resolved_icon(), self.icon_rect()) {
                if !pixels.pixels().is_empty() {
                    let fitted = pixels.fit_into(box_);
                    context.draw_image(
                        fitted.x,
                        fitted.y,
                        fitted.width,
                        fitted.height,
                        pixels.pixels(),
                    );
                }
            }
        }

        // ── Label and popup arrow ──
        //
        // Both are `RenderContext::text_line`-free here: the label's box is centred on its own
        // middle line, and the arrow sits in the room `content_box` reserved, so neither can drift
        // from the band that was laid out for it.
        // The **effective font** — the resolved theme/caller font — not a hardcoded default, so
        // the label honours the theme body font and the user's text scale (D09-STYLE-01).
        let font = effective_font(self.style());
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
            context.draw_text_line_cow(box_, &self.text, font, fg, HorizontalAlignment::Center);
        }
        if let Some(arrow_box) = self.popup_arrow_rect() {
            // The arrow lives in the trailing strip `content_box` already removed from the content,
            // so its rectangle comes from that one derivation rather than from another literal. It
            // is a `ChevronDown` **icon outline**, not the `▾` text glyph (U+25BE is covered by no
            // bundled face, so the glyph drew as an 8x8 bitmap block), centred in the strip so it
            // reads on the button's middle line like the label.
            let side = dimensions::TOOL_BUTTON_POPUP_ARROW_RESERVE;
            let chevron = match self.arrow_type {
                ToolButtonArrowType::Down | ToolButtonArrowType::None => IconName::ChevronDown,
                ToolButtonArrowType::Up => IconName::ChevronUp,
                ToolButtonArrowType::Left => IconName::ChevronLeft,
                ToolButtonArrowType::Right => IconName::ChevronRight,
            };
            crate::widget::draw_icon_centered(
                context,
                arrow_box,
                side.min(arrow_box.height),
                fg,
                chevron,
            );
        }

        // ── Draw the menu ──
        //
        // Same shared painter as `MenuButton`, and the same reveal rule: the panel is painted
        // while the reveal has anything left to show, so a *closing* popup animates out instead
        // of being cut off on its first frame.
        if self.menu_open || !self.reveal.is_closed() {
            let panel = self.menu_rect();
            let revealed = self.reveal.revealed(
                panel,
                RevealDirection::Down,
                PopupReveal::travel_cap_for(Size::new(panel.width, panel.height)),
            );
            let clipped = !self.reveal.is_closed();
            if clipped {
                context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
            }
            draw_menu_panel(context, panel, &self.menu_items, &[], &style, accent);
            if clipped {
                context.pop_clip();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolButton, ToolButtonArrowType, ToolButtonPopupMode, ToolButtonStyle};
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    use crate::event::EventHandler;
    use crate::render::svg::SvgPaintBackend;
    use crate::render::PaintBackend;
    use crate::render::RenderContext;
    use crate::widget::capability::{CapabilityAccessError, CapabilityValue, WidgetProperties};
    use crate::widget::menu_toolbar::menu_button::MenuItem;
    use crate::widget::{Draw, IconName, IconSource, Widget, WidgetKind};
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

    #[test]
    fn tool_button_typed_source_replaces_path_and_reserves_the_icon_slot() {
        let mut button = ToolButton::new("Open", rect());
        assert!(button.icon_rect().is_none());
        let source = IconSource::builtin(IconName::Info);
        button.set_icon_source(Some(source));
        assert_eq!(button.icon_source(), Some(source));
        assert!(button.icon().is_none(), "typed source is not exposed as a legacy path");
        assert!(button.icon_rect().is_some());

        button.set_icon(Some(PathBuf::from("icons/open.png")));
        assert_eq!(button.icon_source(), None, "legacy setter replaces typed content");
        assert!(button.icon_rect().is_some());
        button.set_icon(None);
        assert!(button.icon_rect().is_none());
    }

    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn shared_icon_pixels_are_reused_and_legacy_setter_clears_them() {
        let shared = crate::widget::SharedIconPixels::from_rgba8(vec![64; 4 * 4 * 4], 4, 4)
            .expect("valid RGBA8 data");
        let mut button = ToolButton::new("Open", rect());
        button.set_shared_icon_pixels(Some(shared.clone()));
        let active = button.shared_icon_pixels().expect("shared source is active");
        assert!(active.shares_allocation_with(&shared));
        assert!(button.icon_rect().is_some());
        assert!(button.icon().is_none());
        let svg = crate::widget::svg::render_widget_to_svg(&mut button, rect());
        assert!(svg.contains("<image "), "shared in-memory pixels must reach the paint backend");

        button.set_icon_source(None);
        assert!(button.shared_icon_pixels().is_none(), "explicit clear must clear shared pixels");
        button.set_shared_icon_pixels(Some(shared.clone()));
        button.set_icon_source(Some(IconSource::builtin(IconName::Info)));
        assert!(button.shared_icon_pixels().is_none());
        button.set_shared_icon_pixels(Some(shared.clone()));
        button.set_icon(Some(PathBuf::from("icons/open.png")));
        assert!(button.shared_icon_pixels().is_none());
        assert_eq!(button.icon(), Some(PathBuf::from("icons/open.png").as_path()));
        assert!(button.icon_rect().is_some());

        button.set_shared_icon_pixels(None);
        assert!(button.icon().is_none());
        assert!(button.icon_rect().is_none());
    }

    /// A non-square icon keeps its aspect ratio instead of being stretched into the slot.
    ///
    /// The legacy path used to drop the decoded extent, so a wide image was drawn at the slot's
    /// aspect ratio — a distorted icon. The handle now carries the extent, and the draw fits it.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn a_non_square_icon_is_fitted_not_stretched() {
        let wide = crate::widget::SharedIconPixels::from_rgba8(vec![64; 8 * 4 * 4], 8, 4)
            .expect("valid RGBA8 data");
        let mut button = ToolButton::new("Open", rect());
        button.set_button_style(ToolButtonStyle::IconOnly);
        button.set_shared_icon_pixels(Some(wide));
        let slot = button.icon_rect().expect("shared pixels reserve the slot");

        let svg = crate::widget::svg::render_widget_to_svg(&mut button, rect());
        let image_tag = svg.split("<image ").nth(1).expect("the icon is painted as an image");
        let attribute = |name: &str| {
            let rest = image_tag.split(&format!("{name}=\"")).nth(1).expect("attribute is emitted");
            rest.split('"')
                .next()
                .expect("the attribute is closed")
                .parse::<u32>()
                .expect("a number")
        };
        let (width, height) = (attribute("width"), attribute("height"));
        assert_eq!((width, height), (2 * height, height), "8x4 must stay 2:1 inside the slot");
        assert!(width <= slot.width && height <= slot.height, "the fit must stay inside the slot");
    }

    #[test]
    fn icon_source_property_round_trips_only_persistable_sources() {
        let mut button = ToolButton::new("Open", rect());
        let spec = crate::widget::icon_source::IconSpec::Builtin(IconName::Info);
        button
            .set("icon_source", CapabilityValue::IconSpec(spec.clone()))
            .expect("a built-in descriptor is directly resolvable");
        assert_eq!(button.get("icon_source"), Ok(CapabilityValue::IconSpec(spec)));
        button.set("icon_source", CapabilityValue::Null).expect("null clears the source");
        assert_eq!(button.get("icon_source"), Ok(CapabilityValue::Null));
        assert_eq!(
            button.set(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                    id: "project/icons/open".into(),
                    color_mode: crate::widget::icon_source::IconColorMode::Original,
                }),
            ),
            Err(CapabilityAccessError::ResourceResolutionRequired)
        );
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

    // ── 6b. Popup behaviour ──

    fn two_rows() -> Vec<MenuItem> {
        vec![MenuItem::new(1, "Open"), MenuItem::new(2, "Save")]
    }

    /// An `InstantPopup` press opens the menu and takes the popup grab; a button with no menu
    /// keeps its plain push-button behaviour even in that mode.
    #[test]
    fn tool_button_instant_popup_press_opens_and_takes_the_grab() {
        let mut btn = ToolButton::new("Menu", rect());
        btn.set_popup_mode(ToolButtonPopupMode::InstantPopup);
        btn.set_menu(two_rows());
        btn.handle_event(&Event::mouse_press(15, 25, 1));

        assert!(btn.is_menu_open());
        assert!(btn.has_open_popup());
        assert!(btn.is_animating(), "the opening reveal owes frames");
        assert_eq!(btn.menu_items().len(), 2);
        assert!(!btn.geometry().contains_point(Point::new(500, 500)));

        let mut plain = ToolButton::new("Plain", rect());
        plain.set_popup_mode(ToolButtonPopupMode::InstantPopup);
        plain.handle_event(&Event::mouse_press(15, 25, 1));
        assert!(!plain.is_menu_open(), "an empty menu must not turn a push button into a popup");
    }

    /// A press outside the popup closes it, which is what the runtime's grab route delivers.
    #[test]
    fn tool_button_outside_press_closes_the_popup() {
        let mut btn = ToolButton::new("Menu", rect());
        btn.set_popup_mode(ToolButtonPopupMode::InstantPopup);
        btn.set_menu(two_rows());
        btn.open_menu();
        assert!(btn.is_menu_open());

        btn.handle_event(&Event::mouse_press(500, 500, 1));
        assert!(!btn.is_menu_open());
        assert!(!btn.has_open_popup());
    }

    /// A `MenuButtonPopup` opens from its arrow strip while its body still fires `triggered`.
    #[test]
    fn tool_button_menu_button_popup_arrow_opens_and_the_body_still_triggers() {
        let mut btn = ToolButton::new("Menu", Rect::new(0, 0, 120, 40));
        btn.set_popup_mode(ToolButtonPopupMode::MenuButtonPopup);
        btn.set_menu(two_rows());

        let arrow = btn.popup_arrow_rect().expect("the arrow strip is reserved");
        btn.handle_event(&Event::mouse_press(arrow.x + 1, arrow.y + 1, 1));
        assert!(btn.is_menu_open());
        assert!(btn.has_open_popup());

        btn.handle_event(&Event::mouse_release(arrow.x + 1, arrow.y + 1, 1));
        btn.close_menu();

        let triggered = Arc::new(AtomicUsize::new(0));
        let count = triggered.clone();
        btn.triggered.connect(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        btn.handle_event(&Event::mouse_press(5, 5, 1));
        btn.handle_event(&Event::mouse_release(5, 5, 1));
        assert_eq!(triggered.load(Ordering::SeqCst), 1);
        assert!(!btn.is_menu_open(), "the body performs the main action, not the popup");
    }

    /// A `DelayedPopup` opens only after the hold threshold, accumulated by `tick`.
    #[test]
    fn tool_button_delayed_popup_opens_after_the_hold_threshold() {
        let mut btn = ToolButton::new("Menu", rect());
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::DelayedPopup);
        btn.set_menu(vec![MenuItem::new(1, "Open")]);

        btn.handle_event(&Event::mouse_press(15, 25, 1));
        assert!(!btn.is_menu_open());
        assert!(btn.is_animating(), "a pending held press owes frames");

        assert!(btn.tick(499), "one millisecond short of the threshold");
        assert!(!btn.is_menu_open());
        btn.tick(1);
        assert!(btn.is_menu_open());
        assert!(btn.has_open_popup());

        // The release ends the held gesture; it must not also fire the main action or close
        // the menu it just produced.
        btn.handle_event(&Event::mouse_release(15, 25, 1));
        assert!(btn.is_menu_open());
    }

    /// Every mode keeps the capability contract it had before the popup existed.
    #[test]
    fn tool_button_popup_modes_keep_the_capability_write_back() {
        let modes = [
            ToolButtonPopupMode::DelayedPopup,
            ToolButtonPopupMode::MenuButtonPopup,
            ToolButtonPopupMode::InstantPopup,
        ];
        for mode in modes {
            let mut btn = ToolButton::new("Menu", rect());
            btn.set_popup_mode(mode);
            btn.set("text", CapabilityValue::String(String::from("Renamed")))
                .expect("text is writable");
            assert_eq!(btn.get("text"), Ok(CapabilityValue::String(String::from("Renamed"))));
            btn.set_checkable(true);
            btn.set("checked", CapabilityValue::Bool(true)).expect("checked is writable");
            assert_eq!(btn.get("checked"), Ok(CapabilityValue::Bool(true)));
            assert!(btn.menu_items().is_empty());
            assert!(!btn.is_menu_open());
        }
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
        btn.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1, modifiers: 0 });
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
        btn.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1, modifiers: 0 });
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

    // ── D09-STYLE-01: the label consumes the resolved theme/caller font ──

    /// A caller-authored `style.font` must change the label the tool button paints.
    ///
    /// # The defect this pins
    ///
    /// `draw` built a `Font::default()` by hand, so the theme body font and the user's text scale
    /// never reached a text-mode tool button's label. It now reads `metrics::effective_font`.
    #[test]
    fn a_custom_font_changes_the_tool_button_label_ink() {
        let render = |font: Option<crate::core::Font>| {
            let mut btn = ToolButton::new("Label", Rect::new(0, 0, 160, 60));
            btn.set_button_style(ToolButtonStyle::TextOnly);
            if let Some(font) = font {
                btn.set_style(crate::style::WidgetStyle::default().with_font(font));
            }
            let svg = crate::widget::svg::render_widget_to_svg(&mut btn, Rect::new(0, 0, 160, 60));
            crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
        };

        let base_ink = render(None);
        let big_ink = render(Some(crate::core::Font::simple("Test", 34.0)));
        assert!(base_ink > 0, "the default label must paint ink for this test to mean anything");
        assert!(
            big_ink > base_ink,
            "a larger font must widen the painted label: {big_ink} vs {base_ink}"
        );
    }

    /// The indicator direction reaches the painter, and `None` frees its strip.
    #[test]
    fn the_popup_arrow_direction_is_configurable() {
        let mut btn = ToolButton::new("More", rect());
        btn.set_menu(two_rows());
        btn.set_popup_mode(ToolButtonPopupMode::InstantPopup);
        assert_eq!(btn.arrow_type(), ToolButtonArrowType::Down);
        let down = btn.popup_arrow_rect().expect("the default indicator reserves a strip");
        let down_svg = crate::widget::svg::render_to_svg(&mut btn);

        btn.set_arrow_type(ToolButtonArrowType::Up);
        let up_svg = crate::widget::svg::render_to_svg(&mut btn);
        assert_ne!(down_svg, up_svg, "the direction must reach the painter");

        btn.set_arrow_type(ToolButtonArrowType::None);
        assert!(btn.popup_arrow_rect().is_none(), "no indicator, no strip");
        let none_svg = crate::widget::svg::render_to_svg(&mut btn);
        assert_ne!(up_svg, none_svg);
        assert!(
            btn.label_rect().is_some_and(|label| label.width > 0),
            "the label keeps a box when the strip is gone"
        );
        let _ = down;
    }

    /// The smallest valid PNG: a 2x2 truecolour-with-alpha image whose first pixel is opaque red.
    ///
    /// Written out as bytes rather than produced by an encoder so the fixture works wherever the
    /// `image` feature is on, including builds without the encoder side. The same fixture `avatar`
    /// uses, so both controls are held to one "a real image decodes" baseline.
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
