// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Message box dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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
    /// This is a flag the surrounding dialog machinery reads; the widget itself
    /// performs no input blocking, so a modal box is only actually modal if the
    /// host honours the flag.
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
    /// The colour used to draw [`Self::icon_symbol`]: blue for information and
    /// questions, orange for warnings, red for critical, and black (unused) for
    /// [`MessageBoxIcon::NoIcon`].
    fn icon_color(&self) -> Color {
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
    /// Returns `"title"` and `"text"` as strings (never `None` — the fields simply
    /// default to empty), delegating anything else to the base widget's shared
    /// properties such as geometry and visibility.
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            _ => base_property_get(self, name),
        }
    }

    /// Accepts `"title"` and `"text"` and requires a string value for each,
    /// rejecting other types with [`CapabilityAccessError`]. Other names fall
    /// through to the base widget's setters. Both writes request a redraw.
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
            _ => base_property_set(self, name, value),
        }
    }

    /// `"title"`, `"text"`, then every universally supported base property.
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "text", BASE_PROPERTY_NAMES]
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
impl EventHandler for MessageBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
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
    }
}
impl Draw for MessageBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // Dialog background
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(160, 160, 160),
        );
        // Title bar
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, 28u32), Color::rgb(0, 120, 215));
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &self.title,
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        // Icon
        let icon_sym = self.icon_symbol();
        if !icon_sym.is_empty() {
            context.draw_text(
                Point::new(rect.x + 20, rect.y + 60),
                icon_sym,
                &Font::default(),
                self.icon_color(),
                HorizontalAlignment::Left,
            );
        }
        // Message text
        let text_x = if self.icon == MessageBoxIcon::NoIcon { rect.x + 12 } else { rect.x + 60 };
        context.draw_text(
            Point::new(text_x, rect.y + 60),
            &self.text,
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        // Buttons
        let btn_h = 28f32;
        let btn_w = 80f32;
        let btn_y = rect.y as f32 + rect.height as f32 - btn_h - 12.0;
        let total_btn_w = self.buttons.len() as f32 * (btn_w + 8.0);
        let mut btn_x = rect.x as f32 + rect.width as f32 - total_btn_w;
        for btn in &self.buttons {
            let is_default = self.default_button == Some(*btn);
            let bg = if is_default { Color::rgb(0, 120, 215) } else { Color::rgb(225, 225, 225) };
            let fg = if is_default { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) };
            context.fill_rect(Rect::from_f32(btn_x, btn_y, btn_w, btn_h), bg);
            context
                .draw_rect(Rect::from_f32(btn_x, btn_y, btn_w, btn_h), Color::rgb(100, 100, 100));
            context.draw_text(
                Point::from_f32(btn_x + btn_w / 2.0, btn_y + btn_h / 2.0),
                &btn.translated_label(),
                &Font::default(),
                fg,
                HorizontalAlignment::Left,
            );
            btn_x += btn_w + 8.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::event::Event;
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
