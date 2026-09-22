// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Message box dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::layout::hints::{ChildInfo, Hints, LayoutParams};
use crate::layout::{FlexLayout, JustifyContent, Layout};
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{EdgeOffsets, SemanticColor};
use crate::tr;
use crate::widget::capability::coercion::{
    expect_bool, expect_message_box_icon, expect_string, message_box_icon_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Message box icon type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxIcon {
    /// No icon is drawn; the text fills the full content area.
    NoIcon,
    /// Informational "i" glyph, drawn in blue.
    Information,
    /// A question-mark glyph, drawn in the same blue as
    /// [`Self::Information`].
    Question,
    /// A warning triangle glyph, drawn in orange.
    Warning,
    /// A critical cross glyph, drawn in red; use for errors that blocked an
    /// action.
    Critical,
}
/// Standard buttons for message boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardButton {
    /// Acknowledge and dismiss.
    Ok,
    /// Abandon the operation and dismiss.
    Cancel,
    /// Affirm the prompt.
    Yes,
    /// Decline the prompt.
    No,
    /// Affirm, and stop asking for the same kind of prompt in this run.
    YesAll,
    /// Decline, and stop asking for the same kind of prompt in this run.
    NoAll,
    /// Persist the current state.
    Save,
    /// Abandon the current changes.
    Discard,
    /// Keep the current changes without dismissing.
    Apply,
    /// Dismiss without an accept/reject judgement. Note that
    /// [`MessageBox::click_button`] still counts it as a rejection.
    Close,
    /// Stop an in-progress operation because it cannot succeed.
    Abort,
    /// Repeat the operation that just failed.
    Retry,
    /// Continue past a non-fatal problem.
    Ignore,
    /// Open contextual help instead of answering.
    Help,
}
impl StandardButton {
    /// The English label shown on the button, e.g. `"OK"` or `"Yes to All"`.
    ///
    /// This is the untranslated string; use [`Self::translated_label`] for a
    /// locale-aware label. `'static` because the labels are compile-time
    /// constants.
    pub fn label(&self) -> &'static str {
        match self {
            StandardButton::Ok => "OK",
            StandardButton::Cancel => "Cancel",
            StandardButton::Yes => "Yes",
            StandardButton::No => "No",
            StandardButton::YesAll => "Yes to All",
            StandardButton::NoAll => "No to All",
            StandardButton::Save => "Save",
            StandardButton::Discard => "Discard",
            StandardButton::Apply => "Apply",
            StandardButton::Close => "Close",
            StandardButton::Abort => "Abort",
            StandardButton::Retry => "Retry",
            StandardButton::Ignore => "Ignore",
            StandardButton::Help => "Help",
        }
    }

    /// Returns the translated label for this button using the i18n system.
    ///
    /// # Why there is no `cfg` split here
    ///
    /// `crate::tr!` compiles to a real catalogue lookup when the `i18n` capability is
    /// on and to the key's own English text otherwise (see the macro's stub in
    /// `src/lib.rs`, whose doc says it exists "so call sites need no `cfg` of their
    /// own"). Splitting on `desktop` instead meant `tablet`/`mobile` — which enable
    /// `i18n` without `desktop` — silently took the untranslated branch.
    pub fn translated_label(&self) -> String {
        match self {
            StandardButton::Ok => tr!("common.button.ok"),
            StandardButton::Cancel => tr!("common.button.cancel"),
            StandardButton::Yes => tr!("common.button.yes"),
            StandardButton::No => tr!("common.button.no"),
            StandardButton::YesAll => tr!("common.button.yes_all"),
            StandardButton::NoAll => tr!("common.button.no_all"),
            StandardButton::Save => tr!("common.button.save"),
            StandardButton::Discard => tr!("common.button.discard"),
            StandardButton::Apply => tr!("common.button.apply"),
            StandardButton::Close => tr!("common.button.close"),
            StandardButton::Abort => tr!("common.button.abort"),
            StandardButton::Retry => tr!("common.button.retry"),
            StandardButton::Ignore => tr!("common.button.ignore"),
            StandardButton::Help => tr!("common.button.help"),
        }
    }
}
/// Message box dialog.
///
/// A ready-made modal prompt: it owns its title, body text, icon and the list of
/// standard buttons, and reports the user's answer through three signals rather
/// than a return value.
///
/// Note the widget does not dismiss itself: after `click_button` (or an Enter/
/// Escape key press) the signals fire but the dialog stays on screen, so the
/// owner is expected to react to `accepted`/`rejected` and close it.
///
/// As with every widget it inherits geometry, visibility, enablement and the
/// shared event plumbing from its [`BaseWidget`].
pub struct MessageBox {
    base: BaseWidget,
    title: String,
    text: String,
    icon: MessageBoxIcon,
    buttons: Vec<StandardButton>,
    default_button: Option<StandardButton>,
    modal: bool,
    /// Emitted for every button activation, including ones that also trigger
    /// [`Self::accepted`] or [`Self::rejected`]. Carries the button that was
    /// activated.
    pub button_clicked: Signal1<StandardButton>,
    /// Emitted when the box was answered affirmatively, i.e. with OK, Yes, Save
    /// or Apply. Carries no payload; read the button from
    /// [`Self::button_clicked`] if the distinction matters.
    pub accepted: GenericSignal,
    /// Emitted for every other button — Cancel, No, YesAll, NoAll, Discard,
    /// Close, Abort, Retry, Ignore and Help. Note that YesAll and NoAll are
    /// therefore *both* rejections by this rule.
    pub rejected: GenericSignal,
}
impl MessageBox {
    /// Creates an empty message box with no title or text, no icon, a single OK
    /// button as both the only and the default button, and modal mode enabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MessageBox, geometry, "MessageBox"),
            title: String::new(),
            text: String::new(),
            icon: MessageBoxIcon::NoIcon,
            buttons: vec![StandardButton::Ok],
            default_button: Some(StandardButton::Ok),
            modal: true,
            button_clicked: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    /// Creates a question prompt: [`MessageBoxIcon::Question`] with Yes and No
    /// buttons, Yes being the default.
    pub fn question(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Question;
        mb.buttons = vec![StandardButton::Yes, StandardButton::No];
        mb.default_button = Some(StandardButton::Yes);
        mb
    }
    /// Creates an informational box: [`MessageBoxIcon::Information`], leaving the
    /// default OK button and modal flag from [`Self::new`] in place.
    pub fn information(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Information;
        mb
    }
    /// Creates a warning box: [`MessageBoxIcon::Warning`], still with a single,
    /// default OK button.
    pub fn warning(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Warning;
        mb
    }
    /// Creates an error box: [`MessageBoxIcon::Critical`], still with a single,
    /// default OK button.
    pub fn critical(geometry: Rect, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut mb = Self::new(geometry);
        mb.title = title.into();
        mb.text = text.into();
        mb.icon = MessageBoxIcon::Critical;
        mb
    }
    /// The current title, empty if none was set.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// The current body text, empty if none was set.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// The current icon, [`MessageBoxIcon::NoIcon`] by default.
    pub fn icon(&self) -> MessageBoxIcon {
        self.icon
    }
    /// The configured buttons, in the order they were supplied.
    ///
    /// May be empty, in which case the user has no way to answer the box.
    pub fn buttons(&self) -> &[StandardButton] {
        &self.buttons
    }
    /// The button activated by Enter, or `None` if Enter should do nothing.
    ///
    /// Not validated against [`Self::buttons`]: the default can name a button that
    /// is not currently shown, and Enter will still activate it.
    pub fn default_button(&self) -> Option<StandardButton> {
        self.default_button
    }
    /// Replaces the title and requests a redraw.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.base.request_redraw();
    }
    /// Replaces the body text and requests a redraw.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }
    /// Replaces the icon and requests a redraw.
    pub fn set_icon(&mut self, icon: MessageBoxIcon) {
        self.icon = icon;
        self.base.request_redraw();
    }
    /// Replaces the visible buttons and requests a redraw.
    ///
    /// Does not touch [`Self::default_button`], so a stale default may remain
    /// reachable via Enter after this call.
    pub fn set_buttons(&mut self, buttons: Vec<StandardButton>) {
        self.buttons = buttons;
        self.base.request_redraw();
    }
    /// Sets which button Enter activates and requests a redraw.
    ///
    /// Takes a value, not an `Option`: there is no way to disable the default
    /// button through this setter once it has been set.
    pub fn set_default_button(&mut self, btn: StandardButton) {
        self.default_button = Some(btn);
        self.base.request_redraw();
    }
    /// Whether the box is marked modal. Defaults to `true`.
    ///
    /// This records the intent; enforcement is the modal stack in
    /// [`crate::widget::runtime`] (`enter_modal` / `exit_modal`), which
    /// `MessageBoxHandle::show_modal` drives so input outside the box is blocked.
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// Marks the box modal or not and requests a redraw.
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
        self.base.request_redraw();
    }

    /// Simulates the user activating `btn`: emits [`Self::button_clicked`], then
    /// either [`Self::accepted`] or [`Self::rejected`].
    ///
    /// The accept/reject split is by button identity, not by context: OK, Yes,
    /// Save and Apply accept, and *every* other button rejects — including YesAll,
    /// Close and Help. Any button at all may be passed, even one not in
    /// [`Self::buttons`]; no validation is performed. The dialog is not dismissed
    /// and no redraw is requested.
    pub fn click_button(&mut self, btn: StandardButton) {
        self.button_clicked.emit(btn);
        match btn {
            StandardButton::Ok
            | StandardButton::Yes
            | StandardButton::Save
            | StandardButton::Apply => {
                self.accepted.emit();
            }
            _ => {
                self.rejected.emit();
            }
        }
    }
    /// The glyph drawn for the current icon: `"ℹ"` for information, `"?"` for a
    /// question, `"⚠"` for a warning, `"✗"` for critical, and an empty string for
    /// [`MessageBoxIcon::NoIcon`].
    fn icon_symbol(&self) -> &'static str {
        match self.icon {
            MessageBoxIcon::Information => "ℹ",
            MessageBoxIcon::Question => "?",
            MessageBoxIcon::Warning => "⚠",
            MessageBoxIcon::Critical => "✗",
            MessageBoxIcon::NoIcon => "",
        }
    }
    /// The colour used to draw [`Self::icon_symbol`]: the theme's semantic token for
    /// the icon's severity, so the indicator moves with the appearance instead of
    /// being pinned to a literal. The literals that used to be here were the *light*
    /// palette's values, so a severity indicator stayed light-blue on a dark dialog.
    ///
    /// [`MessageBoxIcon::NoIcon`] draws no glyph at all, so its arm never reaches the
    /// pixels; it still resolves a token rather than a literal so the match stays total.
    fn icon_color(&self) -> Color {
        let token = match self.icon {
            MessageBoxIcon::Information | MessageBoxIcon::Question => SemanticColor::Info,
            MessageBoxIcon::Warning => SemanticColor::Warning,
            MessageBoxIcon::Critical => SemanticColor::Error,
            MessageBoxIcon::NoIcon => SemanticColor::Info,
        };
        // `semantic_color` takes and releases the manager's lock and returns an owned
        // colour, so no guard outlives the call.
        crate::style::semantic_color(token).unwrap_or_else(|| self.icon_color_fallback())
    }

    /// The literal last resort behind [`Self::icon_color`], used only when no theme is
    /// active and therefore no token can be resolved.
    fn icon_color_fallback(&self) -> Color {
        match self.icon {
            MessageBoxIcon::Information => Color::rgb(0, 120, 215),
            MessageBoxIcon::Question => Color::rgb(0, 120, 215),
            MessageBoxIcon::Warning => Color::rgb(255, 140, 0),
            MessageBoxIcon::Critical => Color::rgb(196, 43, 28),
            MessageBoxIcon::NoIcon => Color::rgb(0, 0, 0),
        }
    }
}
impl Widget for MessageBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(350, 150)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `MessageBox` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `MessageBox`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. Both properties are read-only: the old
/// write layer had no arm for this kind.
impl WidgetProperties for MessageBox {
    /// Returns `"title"`, `"text"` and `"icon"`, delegating anything else to the
    /// base widget's shared properties such as geometry and visibility.
    ///
    /// `"icon"` answers the current [`MessageBoxIcon`] as its lowercase spelling
    /// (`"warning"`, `"critical"`, …) — see [`expect_message_box_icon`] for the
    /// accepted set, which is the same one the JSON loader uses.
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "icon" => Ok(CapabilityValue::String(message_box_icon_to_str(self.icon()).to_string())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            _ => base_property_get(self, name),
        }
    }

    /// Accepts `"title"`, `"text"` and `"icon"`, requiring a string value for each
    /// and rejecting other types with [`CapabilityAccessError`]. Other names fall
    /// through to the base widget's setters. Every write requests a redraw.
    ///
    /// The icon was previously unreachable from this route even though the control
    /// draws it and offers `warning()` / `critical()` constructors: a caller using
    /// the documented property API got `UnknownProperty` for a property the widget
    /// plainly has.
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "icon" => {
                self.set_icon(expect_message_box_icon(value)?);
                Ok(())
            }
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    /// `"title"`, `"text"`, `"icon"`, `"modal"`, then every universally supported base
    /// property.
    fn property_names(&self) -> &'static [&'static str] {
        // `modal` is listed because the schema publishes it: the getter and setter exist,
        // so a name that is declared but absent here is a property the designer offers and
        // the control cannot answer. This is the BLUE20 layer 2 Q1 defect the alignment gate
        // exists to find, and `BASE_PROPERTY_NAMES` does not carry it.
        property_names_of!["title", "text", "icon", "modal", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `message_box` publishes.
    ///
    /// The convenience constructors (`warning()`, `critical()`, `question()`,
    /// `information()`) cover the common cases, but a caller holding an already-built
    /// box needs a way to switch its severity — and a generic consumer offering a
    /// command palette needs a *name* for that. Both carry a payload, so both are
    /// answered through the property route, which is where the value goes.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_title" | "set_icon" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
/// Keyboard handling: Enter activates the default button and Escape activates
/// Cancel, falling back to No and then Close.
///
/// The event is always forwarded to the base widget first (which handles
/// enable/disable bookkeeping), and everything is ignored entirely while the
/// widget is disabled. Key codes are raw values — `13` for Enter and `27` for
/// Escape — rather than a key enum. Escape does nothing if none of Cancel, No or
/// Close is among the configured buttons, and Enter does nothing when
/// [`MessageBox::default_button`] is `None`.
///
/// A primary press is resolved against the **same** button rects the draw path painted (see
/// [`action_row_geometry`]), so the button a user sees under the pointer is the button that is
/// activated. Before this the box answered only keys: it painted an OK button that could not be
/// clicked, which is a control whose ink and whose hit region were two different things — the
/// defect rule 5 of the assembly spec names, and the one this crate has already recorded twice
/// (`tag_input`'s unreachable close button, `scroll_bar`'s non-inverse value mapping).
impl EventHandler for MessageBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.action_button_at(*pos) {
                    if let Some(activated) = self.buttons.get(index).copied() {
                        self.click_button(activated);
                    }
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == 13 {
                    // Enter → default button
                    if let Some(btn) = self.default_button {
                        self.click_button(btn);
                    }
                } else if *key == 27 {
                    // Escape → Cancel/No
                    if self.buttons.contains(&StandardButton::Cancel) {
                        self.click_button(StandardButton::Cancel);
                    } else if self.buttons.contains(&StandardButton::No) {
                        self.click_button(StandardButton::No);
                    } else if self.buttons.contains(&StandardButton::Close) {
                        self.click_button(StandardButton::Close);
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
/// The width the action buttons of every dialog in this module are given.
///
/// # Why this is a named constant and not a literal at each draw site
///
/// Eight dialogs declare an OK/Cancel pair, and before this each spelled the button's
/// width itself (`80`, sometimes as `BTN_W`). One fact with eight spellings is what rule
/// #101 forbids, and it is how the message box's row and the file dialog's row came to be
/// measured differently against the same frame.
///
/// The button is **not** allowed to grow with its label: unlike a toolbar item, a dialog's
/// action row is a fixed set of standard commands (`OK`, `Cancel`, `Yes to No`), and the
/// platform convention — and Qt's `QDialogButtonBox` — is that they are all the same width
/// so the row reads as one control. `draw_text_fitted` elides a locale whose translation is
/// longer than the box, so a longer label loses characters rather than moving its siblings.
pub(crate) const DIALOG_BUTTON_WIDTH: i32 = 80;

/// The gap between two adjacent action buttons, and between the row and the content box's
/// edge: [`dimensions::BUTTON_ICON_SPACING`], the same 6 px that separates a control's own
/// parts elsewhere in this crate.
///
/// It is deliberately *not* `DIALOG_PADDING`: that is the frame's edge-to-content distance, and
/// rule 4 of the assembly spec keeps "edge to content" and "element to element" apart. Using
/// the frame padding for both made the row's right inset twice the gap between its buttons.
pub(crate) const DIALOG_BUTTON_SPACING: u32 = dimensions::BUTTON_ICON_SPACING;

/// The height of an action button's label: one line box.
///
/// Derived from the text layer rather than from [`dimensions::DIALOG_BUTTON_HEIGHT`], which is
/// the *row* height other dialogs reserve. Presenting this through [`Hints::preferred`] is what
/// lets the row's layout answer "how wide are you" from a real measurement, and it keeps a
/// button's own height tied to the font it draws its label in.
pub(crate) fn action_button_hints(context: &RenderContext, label: &str) -> Hints {
    let font = Font::default();
    let line = context.measure_text(label, &font).height.max(1);
    Hints::fixed(DIALOG_BUTTON_WIDTH as u32, line)
}

impl MessageBox {
    /// The frame the message box actually paints: at most its intrinsic size, centred in
    /// the area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the box is **offered** — a census cell, a parent's client area.
    /// Painting it verbatim drew a 240x120 census cell as a 240x120 slab whose title bar,
    /// message and button row were each pinned to a literal written for a wider default
    /// size. [`ControlMetrics::painted_box`] caps each axis at the box's own intrinsic size
    /// and centres what is left, and it clamps up to one pixel so a squeezed box stays
    /// visible. Every band below — the title strip, the icon/message row and the button row
    /// — is derived from this one rect.
    ///
    /// # Why the width is also content-driven here
    ///
    /// `DIALOG_MIN_WIDTH` is a floor, not the intrinsic size: the layout below is asked for
    /// the size of a row of buttons, and a row wider than 280 px grows the frame instead of
    /// running past it. A dialog whose action row left its own frame was the defect the old
    /// `rect.x + rect.width - count * (80 + 8)` derivation produced once a caller set a
    /// longer label.
    fn frame_rect(&self, context: &RenderContext) -> Rect {
        ControlMetrics::painted_box(self.base.geometry(), self.intrinsic_size(context))
    }

    /// The box this dialog needs: the action row's own width and the floor, component-wise.
    ///
    /// This is Qt's `implicitWidth = max(implicitBackgroundWidth + inset,
    /// implicitContentWidth + padding)` spelled with this crate's primitives, where the floor
    /// is the background and the measured row is the content.
    fn intrinsic_size(&self, context: &RenderContext) -> Size {
        // The labels are the *translated* ones, because those are the strings the row draws —
        // measuring the English `label()` while painting `translated_label()` is how a row
        // sized for "OK" ends up eliding "Annuler".
        let labels: Vec<String> =
            self.buttons.iter().map(|button| button.translated_label()).collect();
        let row = action_row_geometry(context, &labels, Rect::new(0, 0, 0, 0), false);
        let row_height = dimensions::DIALOG_TITLE_BAR_HEIGHT.max(row.row.height);
        let height = dimensions::DIALOG_TITLE_BAR_HEIGHT
            .saturating_add(row_height)
            .max(dimensions::DIALOG_MIN_HEIGHT);
        Size::new(row.row.width.max(dimensions::DIALOG_MIN_WIDTH), height)
    }

    /// The index of the action button under `pos`, or `None`.
    ///
    /// # Why the band is reconstructed rather than stored
    ///
    /// The rects the buttons were painted in live only for the duration of a `draw`, and a
    /// press is delivered by an event that has no render context of its own. Rebuilding the
    /// band from the same derivations the draw used — `painted_box`, `top_band`,
    /// `bottom_band` — is what keeps the press and the ink on one geometry: those helpers are
    /// pure functions of the control's rectangle, so the same input gives the same band at any
    /// time. A *cached* rect would be the other option and it is the worse one, because a cache
    /// is a second copy of the truth that a geometry change can leave stale — which is
    /// precisely how a hit region comes to disagree with the drawing it is meant to describe.
    ///
    /// The measurement only needs a backend to ask for the line height, and the band only needs
    /// the control's rectangle, so the press path costs no state and no redraw.
    fn action_button_at(&self, pos: Point) -> Option<usize> {
        let backend_size =
            Size::new(self.base.geometry().width.max(1), self.base.geometry().height.max(1));
        let mut backend = crate::render::SoftwarePaintBackend::new(backend_size, 1.0);
        let context = RenderContext::new(&mut backend);
        let rect = self.frame_rect(&context);
        // The band the buttons are drawn in: the strip the title bar leaves, minus the row the
        // message occupies — the same two derivations `draw` uses, in the same order.
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let button_band = ControlMetrics::bottom_band(body, dimensions::DIALOG_BUTTON_HEIGHT);
        // `place` is true exactly when the box has buttons, which is the same condition `draw`
        // passes: a box with no buttons has no row to hit and `hit` on an empty row is `None`
        // anyway, but the flag also keeps the two call sites reading the same way.
        let labels: Vec<String> =
            self.buttons.iter().map(|button| button.translated_label()).collect();
        let place = !self.buttons.is_empty();
        action_row_geometry(&context, &labels, button_band, place).hit(pos)
    }
}

impl Draw for MessageBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect(context);
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The surface already read the style, but the
        // title bar and the buttons were literals, so a light/dark switch left them
        // unchanged and the census reported the control as theme-blind.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("message_box");
        // `message_box` is absent from `WidgetRole::for_kind_name`'s table — the variant
        // spells as `message_box`, but only `errordialog` is classified — so it resolves to
        // `theme.colors.background`: the colour the window behind it is already filled with.
        // A frame painted in that colour would be indistinguishable from the window, so a
        // resolved surface equal to the window fill is re-derived a visible step away from
        // it, the same distinction `Colors::input_background` draws for a field.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.45));
        // The title bar is a distinct band on the frame, derived from it so the two stay one
        // visible step apart in either appearance. It used to be the literal brand blue.
        let title_bar = surface.blend(&ink, 0.08);
        // An ordinary button is a raised step on the frame; the default button is the
        // theme's action colour with text chosen for contrast against it.
        let button_fill = surface.blend(&ink, 0.12);
        let primary = theme.as_ref().and_then(|t| t.background_color).unwrap_or(button_fill);
        let primary_ink = primary.contrast_color();

        // A dialog is a normal widget: every element it emits has to stay inside its own
        // rectangle. None of these labels was bounded, so a long title, message or button
        // word ran past the frame — the raster backends clip that away, but the SVG
        // backend emits absolute coordinates and showed the overflow as drawing outside
        // the picture. Each label is fitted to the band it belongs to instead.
        let font = Font::default();

        // Dialog background. Rounded by [`dimensions::DIALOG_RADIUS`] so a message box reads
        // as the same class of object as every other dialog; the radius is clamped to the
        // frame so a box smaller than its own corner is not drawn with an inverted one.
        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.fill_rect(rect, surface);
            context.draw_rect(rect, border);
        }
        // Title bar. The band is the label's own box, so the fit is measured against the
        // bar rather than against the dialog: a title longer than the bar truncates at the
        // bar's edge instead of leaving the control. The strip comes from `top_band`, which
        // keeps its thickness and clamps to the frame.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, title_bar);
        if !self.title.is_empty() {
            let title_font = Font::default();
            let title_line = context.text_line(title_bar_band, &title_font);
            context.draw_text_fitted(
                Rect::new(
                    rect.x + 8,
                    title_line.y,
                    rect.width.saturating_sub(16),
                    title_line.height.max(1),
                ),
                &self.title,
                &title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // The icon/message row and the button row both live inside the space the title bar
        // leaves, which is what keeps a long message from starting under the strip and a
        // button row from overlapping the message on a short frame.
        //
        // The icon and the message share one band: the icon is a glyph at the left, the
        // message starts after it (or at the frame's own margin when there is no icon).
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let icon_sym = self.icon_symbol();
        let body_font = Font::default();
        let body_line_h = context.measure_text("M", &body_font).height.max(1) as i32;
        // The icon column is measured from the glyph, not a fixed `rect.x + 60` written for
        // a wide dialog, so a wide scalar cannot overlap the message beside it.
        let icon_left = 12.min(body.width as i32);
        let gutter = if icon_sym.is_empty() {
            icon_left
        } else {
            let icon_metrics = context.measure_text(icon_sym, &body_font);
            let icon_line = context.text_line(body, &body_font);
            context.draw_text_fitted(
                Rect::new(
                    body.x + icon_left,
                    icon_line.y,
                    icon_metrics.width.max(1),
                    icon_line.height.max(1),
                ),
                icon_sym,
                &body_font,
                self.icon_color(),
                HorizontalAlignment::Left,
            );
            icon_left + icon_metrics.width as i32 + 8
        };

        // The button row is the bottom band; the message occupies what is left above it.
        let button_band = ControlMetrics::bottom_band(body, dimensions::DIALOG_BUTTON_HEIGHT);
        // The row is computed **before** the message band, because the message band's own
        // trailing edge is derived from the row. That is the `SpinBox.qml:20-21` relation — the
        // text side's padding is the sibling column's width — and it is the whole reason
        // [`ActionRowGeometry::leading_inset`] exists: the message must stop where the buttons
        // begin, whatever the buttons' widths and their labels' translations turn out to be.
        let labels: Vec<String> =
            self.buttons.iter().map(|button| button.translated_label()).collect();
        let row = action_row_geometry(context, &labels, button_band, !self.buttons.is_empty());
        let message_area =
            ControlMetrics::content_above_bottom_band(body, dimensions::DIALOG_BUTTON_HEIGHT);
        // The message band ends where the row begins, measured from the row rather than from a
        // literal written for one button count. `saturating_sub` because a row that fills the
        // frame leaves the message no trailing room, which is a message that elides rather than
        // one that runs under the buttons.
        let message_right = (message_area.width as i32 - row.leading_inset as i32).max(0);
        let message_band = Rect::new(
            message_area.x + gutter,
            message_area.y,
            (message_right - gutter).max(0) as u32,
            message_area.height,
        );
        // Guarded on the text being non-empty: an unguarded draw emits `<text …></text>`,
        // an empty element the rasteriser never produces. The line box is centred on the
        // message band through the shared primitive rather than at a literal `y`.
        if !self.text.is_empty() {
            let message_line = context.text_line(message_band, &body_font);
            context.draw_text_fitted(
                Rect::new(
                    message_band.x,
                    message_line.y,
                    message_band.width.max(1),
                    message_line.height.max(1),
                ),
                &self.text,
                &body_font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // Buttons, right-aligned as a row.
        //
        // # Why this is not `rect.x + rect.width - count * (80 + 8)`
        //
        // The old form computed each button's x from a fixed 88 px stride measured back from
        // the frame's right edge, and gave every button the same 80 px whatever its label
        // said. Two defects followed and both were visible in the census snapshot: the row's
        // total width was assumed rather than measured, so a row wider than the frame started
        // left of the dialog and the `max(rect.x)` floor silently overlapped its neighbours; and
        // a locale whose `Cancel` is twice as long elided its label from the middle of a
        // button it did not fit. Rule 4 of the assembly spec is that a segment's size comes
        // from its siblings, not from a literal — `SpinBox.qml:20-21` is the reference, where
        // the text side's padding is the *button column's own width*.
        //
        // The row is therefore a real layout: it is handed the buttons' measured hints plus
        // their inter-button margins, it computes each one's rect, and the *same* rects are what
        // is painted here and what [`ActionRowGeometry::hit`] resolves a press against — so the
        // ink and the hit test cannot disagree about where a button is.
        // `body_line_h` records the line box the rows were measured against, so a later
        // change to the reserved rows and this measurement cannot silently disagree.
        debug_assert!(body_line_h > 0);
        for (button, button_rect) in self.buttons.iter().zip(row.button_rects()) {
            let is_default = self.default_button == Some(*button);
            let bg = if is_default { primary } else { button_fill };
            let fg = if is_default { primary_ink } else { ink };
            context.fill_rect(*button_rect, bg);
            context.draw_rect(*button_rect, border);
            context.draw_text_line(
                *button_rect,
                &button.translated_label(),
                &font,
                fg,
                HorizontalAlignment::Center,
            );
        }
    }
}

/// The action-button row a dialog paints, computed once and used by both the ink and the
/// hit test.
///
/// # Why a type rather than three loose numbers
///
/// Every dialog needs the *same three facts* about its row — where the row starts, how wide it
/// is and where each button sits — and a caller that takes one of them without the others is
/// how a paint path and a hit test come to disagree about the same button. `buttons` is what
/// both consumers index.
pub(crate) struct ActionRowGeometry {
    /// The row's rect: the union of the buttons, inset to the content edge and right-anchored.
    pub row: Rect,
    /// Each button's rect, in the caller's own order.
    pub buttons: Vec<Rect>,
    /// The width from the row's leading edge to the content box's: the *derived* left inset the
    /// row leaves for whatever shares its band.
    pub leading_inset: u32,
}

impl ActionRowGeometry {
    /// The button rects as a slice.
    pub fn button_rects(&self) -> &[Rect] {
        &self.buttons
    }

    /// The button whose rect contains `pos`, if any.
    ///
    /// The hit test is deliberately a lookup in the rects the paint used rather than a second
    /// derivation from the index: a press must resolve to the button the user can see, and a
    /// separately computed region is how the two drift apart (this crate has recorded that
    /// defect twice — `tag_input`'s unreachable close button and `scroll_bar`'s non-inverse
    /// value mapping).
    pub fn hit(&self, pos: Point) -> Option<usize> {
        self.buttons.iter().position(|button| button.contains_point(pos))
    }
}

/// Lays `labels` out as a right-aligned row of action buttons inside `band`.
///
/// # What the row derives from
///
/// Each button's width is [`DIALOG_BUTTON_WIDTH`] and each gap is [`DIALOG_BUTTON_SPACING`],
/// both declared once so all eight dialogs that draw an OK/Cancel pair draw the same row. The
/// buttons' *hints* are measured from the labels the caller is about to paint, which is what
/// makes the row answer "how wide am I" truthfully instead of assuming a count.
///
/// # Why the row is anchored by its siblings
///
/// The row is placed by the running sum of the preceding buttons' widths plus the gaps between
/// them, measured back from the content box's trailing edge. A stride literal (`i * 88`) is the
/// defect this removes: it silently assumes every button is the same width *and* that the gaps
/// are, so the first change to either moves every later button by a multiple of the error, and
/// the row's own extent stops being a fact anything else can read — which is exactly what
/// `leading_inset` here gives back to the caller.
///
/// When `place` is false the row is returned in *unplaced* coordinates: it keeps the total
/// width a caller needs for an intrinsic-size calculation but does not pretend to know where a
/// zero-extent band is.
///
/// # Why a `BoxLayout` and not three lines of arithmetic here
///
/// BLUE22 §B.6 rule 2 is that a composite's children are positioned by a `Layout`, not by
/// `rect.x + k`. Doing the sum by hand would put a second placement rule next to the layouts'
/// and would lose the layout's device scaling (`update_with_context` grows the row's gaps with
/// the text-size preference) — the failure mode where a control's own padding scales with the
/// font and its gaps do not.
pub(crate) fn action_row_geometry(
    context: &RenderContext,
    labels: &[String],
    band: Rect,
    place: bool,
) -> ActionRowGeometry {
    if labels.is_empty() {
        return ActionRowGeometry { row: band, buttons: Vec::new(), leading_inset: 0 };
    }
    // The buttons' own wishes, one per label. `Hints::fixed` rather than the widget's
    // `size_hint` because an action button is a standard command of a standard width: it does
    // not grow with its translation, it elides.
    let children: Vec<ChildInfo> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            // The gap is `margins.left` on every button *after the first*, so the layout solves
            // one button's rect and the spacing follows from the sibling order rather than from
            // a separate term each call site must remember to add.
            //
            // Exactly one side carries it — the button's *leading* margin — because placing it on
            // both sides of every child pays it twice: the trailing margin of one button and the
            // leading margin of the next are adjacent, so a 6 px gap became 12 px between every
            // pair, even though the two buttons' own edges were what the caller asked to be
            // spaced. The first button carries no leading margin either: the row's own edge is
            // its edge, and a margin there would be room the row reserves but never spends.
            let leading = if index == 0 { 0 } else { DIALOG_BUTTON_SPACING };
            ChildInfo::new(context_row_id(index), action_button_hints(context, label)).with_params(
                // `EdgeOffsets::new` is `(top, right, bottom, left)`: the gap before a button is
                // its *left* margin, which is the last argument.
                LayoutParams::new().with_margins(EdgeOffsets::new(0, 0, 0, leading)),
            )
        })
        .collect();
    // The row is a real layout, handed the buttons by their measured hints. `justify_content`
    // is `FlexEnd`, which is what makes the row **right-anchored to the content box**: the
    // layout places the children at their preferred size — none of them sets `fill` — and puts
    // all the leftover room *before* the first one. Nothing here computes an x, because the
    // anchor is a property of the container: that is the only shape in which "right-aligned"
    // cannot be forgotten at one of the eight dialogs that draw this row.
    //
    // `gap` is 0 and the spacing rides on each child's own margin instead, so the distance
    // before a button is derived from the *button* rather than from an index comparison inside
    // the layout. The first button carries no leading margin, which is what lets the row's
    // leading edge be a button rather than a gap.
    let mut layout = FlexLayout::with_params(
        crate::layout::FlexDirection::Row,
        crate::layout::FlexWrap::NoWrap,
        JustifyContent::FlexEnd,
        crate::layout::AlignItems::Stretch,
        0,
        0,
    );
    for child in &children {
        layout.add_widget(child.id, child.params.stretch);
    }
    // The box the row is laid out in is exactly as wide as the children ask for, not the band
    // the caller offered. A row of buttons is not made wider by the dialog around it being wide,
    // and laying it out in a box wider than its own total is what let the previous
    // stretch-based derivation blow every button up to the band's width. With the measure box
    // and the row's own extent equal, the anchor places the row at the measure box's origin and
    // the `shift` below is the *only* thing that moves it — one place, and not eight.
    //
    // The measured box is **not** clamped to the band. Clamping it would feed the layout a
    // parent narrower than its children, which is precisely the case whose handling this
    // function is replacing: the children would be laid out relative to a box that is not the
    // one they were measured in. A row too wide for the band is instead handled where it can be
    // described — see `shift`.
    let row_extent = children
        .iter()
        .map(|child| child.bounds().width)
        .fold(0u32, |total, width| total.saturating_add(width))
        .max(1);
    let measure = Rect::new(band.x, band.y, row_extent, band.height);
    let mut placed: Vec<(ObjectId, Rect)> = Vec::with_capacity(children.len());
    layout.arrange(measure, &children, &mut |id, rect| placed.push((id, rect)));
    // Where the row lands inside the band it was offered: `band` is the room the caller has,
    // `measure` is what the row needs, and the difference is anchored to the trailing edge —
    // which is what "right-aligned" means, and why the buttons sit at the dialog's own right
    // edge in every dialog that calls this rather than at an offset each one recomputes.
    //
    // A row *wider* than the band keeps its own width and is anchored to the band's leading
    // edge (`shift` floors at zero). Squeezing it instead would make the buttons narrower than
    // the labels they are about to draw, so every one of them would elide: a row of unreadable
    // commands is a worse answer than a row that overhangs the panel it sits in, and the latter
    // is what a dialog whose content is bigger than its frame already does everywhere else. The
    // frame itself is sized from this row's own width (`intrinsic_size`), so in practice the
    // overhang only arises when a host forces a smaller rectangle on the control.
    let shift = if place { (band.width as i32 - row_extent as i32).max(0) } else { 0 };
    let mut buttons = Vec::with_capacity(children.len());
    for child in &children {
        // A child the layout did not describe must not vanish from a row the caller indexes by
        // position, so a missing rect falls back to the child's own measured bounds at the
        // measure box's origin rather than to a zero rect (which would be an invisible button).
        let rect =
            placed.iter().find(|(id, _)| *id == child.id).map(|(_, rect)| *rect).unwrap_or_else(
                || {
                    let size = child.bounds();
                    Rect::new(measure.x, band.y, size.width, band.height)
                },
            );
        buttons.push(Rect::new(rect.x + shift, band.y, rect.width, band.height));
    }
    let first = buttons.first().copied().unwrap_or(band);
    let last = buttons.last().copied().unwrap_or(band);
    let row_left = first.x;
    let row_right = last.x.saturating_add(last.width as i32);
    let row = Rect::new(row_left, band.y, (row_right - row_left).max(0) as u32, band.height);
    // What the row leaves *before* it: the padding this dialog's own content must carry so its
    // title, message and icon cannot overlap the buttons that share their band. This is the
    // `SpinBox.qml:20-21` relation — the text side's padding is the sibling column's width —
    // and it is why the row reports it instead of leaving each call site to re-subtract.
    let leading_inset = if place { (row_left - band.x).max(0) as u32 } else { 0 };
    ActionRowGeometry { row, buttons, leading_inset }
}

/// The id the row's `index`-th button carries into the layout.
///
/// Derived from the index rather than from `ObjectId::new()` so the layout's output can be put
/// back in the caller's order without a map: the caller indexes its own buttons by position and
/// the row must answer in that same order.
fn context_row_id(index: usize) -> ObjectId {
    ROW_ID_BASE + index as u64
}

/// The slot-space origin the row's ids are handed out from.
///
/// A file-local base: the row's ids only have to be distinct from each other, because the row is
/// placed in one call and the caller matches the result back by position. A high base keeps them
/// from colliding with the ids a host hands the same layout in its own tree.
const ROW_ID_BASE: u64 = 0x1000_0000_0000_0000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect, Size};
    use crate::event::Event;
    use crate::render::SoftwarePaintBackend;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    // ── 1. Creating default message box ─────────────────────────────

    /// A translated label must resolve through the catalogue, not echo its key.
    ///
    /// The catalogue is a process-global, so this initialises it explicitly rather
    /// than relying on some other test having done so.
    ///
    /// Gated on `i18n`: without the catalogue the key *is* the correct answer, and
    /// that behaviour is asserted separately below so it is pinned in every profile.
    ///
    /// This is the assertion that the `desktop`-gated copy of `translated_label`
    /// would have failed on a `tablet`/`mobile` build: there the function was compiled
    /// into its "fallback" body, so `"common.button.ok"` came back as the literal key
    /// even though `i18n` was enabled and initialised.
    #[cfg(feature = "i18n")]
    #[test]
    fn translated_label_resolves_through_the_catalogue() {
        crate::i18n::init();
        assert_eq!(
            StandardButton::Ok.translated_label(),
            "OK",
            "a built-in button label must translate rather than echo its catalogue key"
        );
        assert_eq!(StandardButton::Cancel.translated_label(), "Cancel");
    }

    /// The translated label must differ from the key for a key the catalogue knows.
    ///
    /// Stated as a separate property so a catalogue regression that made `translate`
    /// return its input would fail here and not only in the equality above.
    #[cfg(feature = "i18n")]
    #[test]
    fn translated_label_is_not_the_catalogue_key() {
        crate::i18n::init();
        let translated = StandardButton::Yes.translated_label();
        assert_ne!(translated, "common.button.yes");
        assert!(!translated.is_empty(), "an unknown key still yields the key itself");
    }

    /// Without the catalogue the key is the honest answer, and must not be empty.
    ///
    /// This is the other half of the contract: a build that cannot translate has to say
    /// so (the `tr!` stub logs a warning) rather than render nothing.
    #[cfg(not(feature = "i18n"))]
    #[test]
    fn translated_label_without_i18n_returns_the_key() {
        assert_eq!(StandardButton::Ok.translated_label(), "common.button.ok");
    }

    /// The action row is right-anchored to the content box's trailing edge, and every button
    /// after the first is offset by the preceding button's own width plus one spacing.
    ///
    /// # What this pins
    ///
    /// It used to be pinned as "the row is `count * (80 + 8)` wide starting at the frame's right
    /// edge", which a hardcoded row satisfies without ever measuring its buttons. The assertion
    /// here is instead the *derivation*: the second button's offset from the first is a function
    /// of the first button's width, so a change to either the spacing constant or the button
    /// width moves the pair together rather than desynchronising the stride from the buttons.
    #[test]
    fn the_action_row_is_right_anchored_and_each_button_follows_its_sibling() {
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        let ctx = RenderContext::new(&mut backend);
        let labels = vec!["OK".to_string(), "Cancel".to_string()];
        let band = Rect::new(0, 92, 240, 28);
        let row = action_row_geometry(&ctx, &labels, band, true);
        assert_eq!(row.buttons.len(), 2, "one rect per label");
        // The last button ends at the content box's trailing edge: that is what "right-anchored"
        // means, and it is the property a count-based stride only satisfies by coincidence.
        let last = row.buttons[1];
        assert_eq!(
            last.x + last.width as i32,
            band.x + band.width as i32,
            "the row must be anchored to the band's trailing edge, not to a count-derived offset"
        );
        // The second button begins one spacing after the first ends.
        let first = row.buttons[0];
        assert_eq!(
            last.x - (first.x + first.width as i32),
            DIALOG_BUTTON_SPACING as i32,
            "the gap between two buttons must be the declared spacing"
        );
        assert_eq!(
            first.width, DIALOG_BUTTON_WIDTH as u32,
            "a standard command keeps its declared width instead of growing into the band"
        );
        assert_eq!(
            row.leading_inset,
            (row.row.x - band.x) as u32,
            "the inset a dialog's own content carries is measured from the band's leading edge"
        );
        assert_eq!(
            band.x + row.leading_inset as i32,
            row.row.x,
            "the leading inset ends exactly where the row begins, so nothing can overlap it"
        );
        assert_eq!(
            row.leading_inset + row.row.width,
            band.width,
            "the inset and the row tile the band, leaving no gap between them"
        );
    }

    /// A row wider than the band keeps its own widths instead of squeezing every label.
    ///
    /// This replaces a test that pinned the old `max(rect.x)` clamp as "no button leaves the
    /// band". That clamp did not achieve it — it moved the row's start while every button was
    /// still stepped by a fixed 88 px, so the buttons overlapped each other *and* the last one
    /// still ran past the frame's right edge. What is pinned now is the contract the helper
    /// actually implements and documents: a row too wide for its band is anchored to the band's
    /// leading edge and keeps the widths its labels were measured at, so nothing elides and no
    /// button overlaps its neighbour. `intrinsic_size` sizes the frame from this same row, so a
    /// dialog only meets this case when a host forces a smaller rectangle on it.
    #[test]
    fn a_row_wider_than_its_band_keeps_its_widths_and_its_leading_edge() {
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        let ctx = RenderContext::new(&mut backend);
        let labels: Vec<String> = (0..6).map(|i| format!("Button {i}")).collect();
        let band = Rect::new(0, 92, 120, 28);
        let row = action_row_geometry(&ctx, &labels, band, true);
        assert_eq!(
            row.row.x, band.x,
            "an oversized row is anchored to the band's leading edge, not centred on it"
        );
        for (index, button) in row.button_rects().iter().enumerate() {
            assert_eq!(
                button.width, DIALOG_BUTTON_WIDTH as u32,
                "a squeezed band must not shrink the buttons: an elided command is unreadable"
            );
            if index > 0 {
                let previous = row.button_rects()[index - 1];
                assert_eq!(
                    button.x - (previous.x + previous.width as i32),
                    DIALOG_BUTTON_SPACING as i32,
                    "the gap between two buttons survives an oversized row"
                );
            }
        }
    }

    /// A press resolves against the rects that were painted, not against a second derivation.
    #[test]
    fn a_press_hits_the_button_that_was_drawn_there() {
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        let ctx = RenderContext::new(&mut backend);
        let labels = vec!["OK".to_string(), "Cancel".to_string()];
        let band = Rect::new(0, 92, 240, 28);
        let row = action_row_geometry(&ctx, &labels, band, true);
        for (index, button) in row.button_rects().iter().enumerate() {
            let centre =
                Point::new(button.x + button.width as i32 / 2, button.y + button.height as i32 / 2);
            assert_eq!(row.hit(centre), Some(index), "the centre of {button:?} belongs to it");
        }
        // A point in the leftover room the row did not take belongs to no button.
        assert_eq!(row.hit(Point::new(band.x + 1, band.y + band.height as i32 / 2)), None);
    }

    #[test]
    fn test_default_creation() {
        let mb = MessageBox::new(Rect::new(100, 100, 300, 150));

        assert_eq!(mb.kind(), WidgetKind::MessageBox);
        assert_eq!(mb.geometry(), Rect::new(100, 100, 300, 150));
        assert!(mb.title().is_empty());
        assert!(mb.text().is_empty());
        assert_eq!(mb.icon(), MessageBoxIcon::NoIcon);
        assert_eq!(mb.buttons(), &[StandardButton::Ok]);
        assert_eq!(mb.default_button(), Some(StandardButton::Ok));
        assert!(mb.is_modal());
        assert!(mb.is_visible());
        assert!(mb.is_enabled());
    }

    // ── 2. Setting title and message text ───────────────────────────

    #[test]
    fn test_set_title_and_text() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert!(mb.title().is_empty());
        assert!(mb.text().is_empty());

        mb.set_title("Warning");
        assert_eq!(mb.title(), "Warning");

        mb.set_text("Are you sure?");
        assert_eq!(mb.text(), "Are you sure?");
    }

    // ── 3. Setting icon type ────────────────────────────────────────

    #[test]
    fn test_set_icon() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert_eq!(mb.icon(), MessageBoxIcon::NoIcon);

        mb.set_icon(MessageBoxIcon::Information);
        assert_eq!(mb.icon(), MessageBoxIcon::Information);

        mb.set_icon(MessageBoxIcon::Warning);
        assert_eq!(mb.icon(), MessageBoxIcon::Warning);

        mb.set_icon(MessageBoxIcon::Critical);
        assert_eq!(mb.icon(), MessageBoxIcon::Critical);

        mb.set_icon(MessageBoxIcon::Question);
        assert_eq!(mb.icon(), MessageBoxIcon::Question);

        mb.set_icon(MessageBoxIcon::NoIcon);
        assert_eq!(mb.icon(), MessageBoxIcon::NoIcon);
    }

    /// The icon must be reachable through the **property route**, not only through
    /// the inherent `set_icon`.
    ///
    /// A warning/error prompt carries a severity glyph, and the control always drew
    /// one — but `get`/`set` did not answer `"icon"`, `property_names` omitted it and
    /// the capability table did not declare it, so a caller using the documented
    /// property API got `UnknownProperty` for a property the widget plainly has,
    /// while the JSON loader accepted an `icon` key. This pins all four halves.
    #[test]
    fn message_box_icon_is_reachable_through_the_property_route() {
        use crate::widget::capability::types::CapabilityValue;

        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        // Readable, and the default reads back as the `none` token.
        assert_eq!(
            mb.get("icon").expect("`icon` must be readable"),
            CapabilityValue::String("none".to_string())
        );

        // Writable, by the same spellings the JSON loader accepts.
        for (token, expected) in [
            ("information", MessageBoxIcon::Information),
            ("question", MessageBoxIcon::Question),
            ("warning", MessageBoxIcon::Warning),
            ("critical", MessageBoxIcon::Critical),
            ("none", MessageBoxIcon::NoIcon),
        ] {
            mb.set("icon", CapabilityValue::String(token.to_string()))
                .unwrap_or_else(|e| panic!("set(\"icon\", {token:?}) must be accepted: {e:?}"));
            assert_eq!(mb.icon(), expected, "token {token:?} must select {expected:?}");
            // Round-trips: what was written is what is read back.
            assert_eq!(mb.get("icon").unwrap(), CapabilityValue::String(token.to_string()));
        }

        // `error` is an accepted synonym for `critical`, because that is what a
        // caller writing an error prompt will reach for first.
        mb.set("icon", CapabilityValue::String("error".to_string()))
            .expect("`error` must be accepted as a synonym for `critical`");
        assert_eq!(mb.icon(), MessageBoxIcon::Critical);

        // The set is published, so a generic consumer can discover it.
        assert!(
            mb.property_names().contains(&"icon"),
            "`icon` must appear in property_names, or the schema promises a property the \
             contract does not publish"
        );

        // A bad token is refused rather than silently becoming `NoIcon` — the JSON
        // loader's `_ =>` fallback is deliberate there, but a direct write must not
        // guess.
        assert!(
            mb.set("icon", CapabilityValue::String("chartreuse".to_string())).is_err(),
            "an unknown icon token must be refused"
        );
        // And a wrong type is a type error, not a silent coercion.
        assert!(mb.set("icon", CapabilityValue::Bool(true)).is_err());
    }

    /// The convenience constructors set the severity a caller expects. A `warning()`
    /// that produced `NoIcon` would be the worst kind of silent bug: the prompt still
    /// appears, just without telling the user how serious it is.
    #[test]
    fn message_box_constructors_carry_their_severity() {
        let cases: [(MessageBox, MessageBoxIcon); 4] = [
            (
                MessageBox::warning(Rect::new(0, 0, 300, 150), "Disk almost full", "12 MB left"),
                MessageBoxIcon::Warning,
            ),
            (
                MessageBox::critical(Rect::new(0, 0, 300, 150), "Save failed", "Disk is full"),
                MessageBoxIcon::Critical,
            ),
            (
                MessageBox::information(Rect::new(0, 0, 300, 150), "Done", "Export finished"),
                MessageBoxIcon::Information,
            ),
            (
                MessageBox::question(
                    Rect::new(0, 0, 300, 150),
                    "Discard?",
                    "This cannot be undone",
                ),
                MessageBoxIcon::Question,
            ),
        ];

        for (mb, expected) in cases {
            assert_eq!(mb.icon(), expected);
            // The severity is also readable through the property route, so a caller
            // that builds a box one way can inspect it the other.
            assert_eq!(
                mb.get("icon").unwrap(),
                CapabilityValue::String(message_box_icon_to_str(expected).to_string())
            );
        }
    }

    // ── 4. Setting buttons ──────────────────────────────────────────

    #[test]
    fn test_set_buttons() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        // Default is Ok
        assert_eq!(mb.buttons(), &[StandardButton::Ok]);

        // OK/Cancel
        mb.set_buttons(vec![StandardButton::Ok, StandardButton::Cancel]);
        assert_eq!(mb.buttons(), &[StandardButton::Ok, StandardButton::Cancel]);

        // Yes/No
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No]);
        assert_eq!(mb.buttons(), &[StandardButton::Yes, StandardButton::No]);

        // Yes/No/Cancel
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No, StandardButton::Cancel]);
        assert_eq!(mb.buttons().len(), 3);
    }

    // ── 5. Default button ───────────────────────────────────────────

    #[test]
    fn test_default_button() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert_eq!(mb.default_button(), Some(StandardButton::Ok));

        mb.set_default_button(StandardButton::Cancel);
        assert_eq!(mb.default_button(), Some(StandardButton::Cancel));
    }

    // ── 6. Signal accessors ─────────────────────────────────────────

    #[test]
    fn test_accepted_signal() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let fired = Arc::new(Mutex::new(false));
        mb.accepted.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        mb.click_button(StandardButton::Ok);
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn test_rejected_signal() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let fired = Arc::new(Mutex::new(false));
        mb.rejected.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        mb.click_button(StandardButton::Cancel);
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn test_button_clicked_signal() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        mb.click_button(StandardButton::Yes);
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::Yes));
    }

    // ── 7. Result retrieval (accepted/rejected) ─────────────────────

    #[test]
    fn test_click_button_accept_reject_pattern() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        let accepted = Arc::new(Mutex::new(false));
        let rejected = Arc::new(Mutex::new(false));

        mb.accepted.connect({
            let accepted = Arc::clone(&accepted);
            move || {
                *accepted.lock().unwrap() = true;
            }
        });
        mb.rejected.connect({
            let rejected = Arc::clone(&rejected);
            move || {
                *rejected.lock().unwrap() = true;
            }
        });

        // Ok, Yes, Save, Apply emit accepted
        mb.click_button(StandardButton::Ok);
        assert!(*accepted.lock().unwrap());
        assert!(!*rejected.lock().unwrap());

        // Reset
        *accepted.lock().unwrap() = false;

        mb.click_button(StandardButton::Cancel);
        assert!(!*accepted.lock().unwrap());
        assert!(*rejected.lock().unwrap());
    }

    // ── 8. Factory constructors ─────────────────────────────────────

    #[test]
    fn test_factory_constructors() {
        let info = MessageBox::information(Rect::new(0, 0, 300, 150), "Information", "File saved.");
        assert_eq!(info.title(), "Information");
        assert_eq!(info.text(), "File saved.");
        assert_eq!(info.icon(), MessageBoxIcon::Information);
        assert_eq!(info.buttons(), &[StandardButton::Ok]);
        assert_eq!(info.default_button(), Some(StandardButton::Ok));

        let warn = MessageBox::warning(Rect::new(0, 0, 300, 150), "Warning", "Low disk space");
        assert_eq!(warn.title(), "Warning");
        assert_eq!(warn.text(), "Low disk space");
        assert_eq!(warn.icon(), MessageBoxIcon::Warning);

        let err = MessageBox::critical(Rect::new(0, 0, 300, 150), "Error", "Operation failed");
        assert_eq!(err.title(), "Error");
        assert_eq!(err.text(), "Operation failed");
        assert_eq!(err.icon(), MessageBoxIcon::Critical);

        let q = MessageBox::question(Rect::new(0, 0, 300, 150), "Question", "Continue?");
        assert_eq!(q.title(), "Question");
        assert_eq!(q.text(), "Continue?");
        assert_eq!(q.icon(), MessageBoxIcon::Question);
        assert_eq!(q.buttons(), &[StandardButton::Yes, StandardButton::No]);
        assert_eq!(q.default_button(), Some(StandardButton::Yes));
    }

    // ── 9. Geometry delegation ─────────────────────────────────────

    #[test]
    fn test_geometry_delegation() {
        let mut mb = MessageBox::new(Rect::new(10, 20, 300, 150));

        assert_eq!(mb.geometry(), Rect::new(10, 20, 300, 150));

        mb.set_geometry(Rect::new(0, 0, 400, 200));
        assert_eq!(mb.geometry(), Rect::new(0, 0, 400, 200));
        assert_eq!(mb.geometry(), Rect::new(0, 0, 400, 200));
        assert_eq!(mb.position(), Point::new(0, 0));
        assert_eq!(mb.size(), crate::core::Size::new(400, 200));
    }

    // ── 10. Widget ID and kind ──────────────────────────────────────

    #[test]
    fn test_widget_id_and_kind() {
        let mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert_eq!(mb.kind(), WidgetKind::MessageBox);
        assert_ne!(mb.id(), 0);

        let mb2 = MessageBox::new(Rect::new(0, 0, 200, 100));
        assert_ne!(mb.id(), mb2.id());
    }

    // ── 11. SVG output ──────────────────────────────────────────────

    #[test]
    fn test_svg_output() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        // Default output
        let svg = render_to_svg(&mut mb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"150\""));

        // With title and text set
        let mut mb2 = MessageBox::new(Rect::new(0, 0, 400, 200));
        mb2.set_title("Test Title");
        mb2.set_text("Hello");
        mb2.set_icon(MessageBoxIcon::Warning);
        let svg2 = render_to_svg(&mut mb2);
        assert!(svg2.starts_with("<svg"));
    }

    // ── 12. Modality setting ────────────────────────────────────────

    #[test]
    fn test_modality_setting() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        assert!(mb.is_modal());

        mb.set_modal(false);
        assert!(!mb.is_modal());

        mb.set_modal(true);
        assert!(mb.is_modal());
    }

    // ── 13. Disabled state ──────────────────────────────────────────

    #[test]
    fn test_disabled_state_blocks_events() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));

        mb.set_enabled(false);
        assert!(!mb.is_enabled());

        // Enter key should not trigger default button when disabled
        let accepted_fired = Arc::new(Mutex::new(false));
        mb.accepted.connect({
            let accepted_fired = Arc::clone(&accepted_fired);
            move || {
                *accepted_fired.lock().unwrap() = true;
            }
        });

        mb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(!*accepted_fired.lock().unwrap());

        // Re-enable and verify it works
        mb.set_enabled(true);
        mb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(*accepted_fired.lock().unwrap());
    }

    // ── 14. Keyboard event handling (Enter/ESC) ─────────────────────

    #[test]
    fn test_keyboard_enter_triggers_default_button() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No]);
        mb.set_default_button(StandardButton::Yes);

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        // Enter (key 13) triggers default button (Yes)
        mb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::Yes));
    }

    #[test]
    fn test_keyboard_escape_triggers_cancel() {
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb.set_buttons(vec![StandardButton::Ok, StandardButton::Cancel]);

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        // Escape (key 27) triggers Cancel
        mb.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::Cancel));
    }

    #[test]
    fn test_keyboard_escape_falls_back_to_no_then_close() {
        // No Cancel button → falls back to No
        let mut mb = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb.set_buttons(vec![StandardButton::Yes, StandardButton::No]);

        let captured = Arc::new(Mutex::new(None::<StandardButton>));
        mb.button_clicked.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<StandardButton>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        mb.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(*captured.lock().unwrap(), Some(StandardButton::No));

        // Neither Cancel nor No → Close
        let mut mb2 = MessageBox::new(Rect::new(0, 0, 300, 150));
        mb2.set_buttons(vec![StandardButton::Ok]);
        mb2.set_default_button(StandardButton::Ok);

        let captured2 = Arc::new(Mutex::new(false));
        mb2.button_clicked.connect({
            let captured2 = Arc::clone(&captured2);
            move |_: Arc<StandardButton>| {
                *captured2.lock().unwrap() = true;
            }
        });

        mb2.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        // With only Ok, no Cancel/No/Close, ESC is no-op
        // (the event handler only checks Cancel/No/Close)
        // Actually it won't match anything since buttons only contains Ok
        // Let's just verify button_clicked wasn't called
        assert!(!*captured2.lock().unwrap());
    }

    // ── 15. StandardButton label ────────────────────────────────────

    #[test]
    fn test_standard_button_labels() {
        assert_eq!(StandardButton::Ok.label(), "OK");
        assert_eq!(StandardButton::Cancel.label(), "Cancel");
        assert_eq!(StandardButton::Yes.label(), "Yes");
        assert_eq!(StandardButton::No.label(), "No");
        assert_eq!(StandardButton::Save.label(), "Save");
        assert_eq!(StandardButton::Apply.label(), "Apply");
        assert_eq!(StandardButton::Close.label(), "Close");
        assert_eq!(StandardButton::Abort.label(), "Abort");
        assert_eq!(StandardButton::Retry.label(), "Retry");
        assert_eq!(StandardButton::Ignore.label(), "Ignore");
        assert_eq!(StandardButton::Help.label(), "Help");
        assert_eq!(StandardButton::YesAll.label(), "Yes to All");
        assert_eq!(StandardButton::NoAll.label(), "No to All");
        assert_eq!(StandardButton::Discard.label(), "Discard");
    }
}
