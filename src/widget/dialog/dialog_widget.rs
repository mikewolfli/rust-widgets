// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dialog widget — a secondary, usually-modal window that hosts its own children.
//!
//! This is the generic dialog control: a titled frame that carries a single
//! content widget, exposes modal intent (enforced through [`crate::widget::runtime`]'s
//! modal stack), and announces accept/reject through dedicated signals. It is the
//! standard desktop dialog — distinct from `MessageBox` (a fixed message + buttons)
//! and `PopupWindow` (a chrome-only popup), which `WidgetKind::Dialog` was previously
//! aliased to.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The height of the dialog's title bar, in pixels.
///
/// Also the amount the content area is inset by when a title is present, so the two
/// cannot drift apart. It is [`dimensions::DIALOG_TITLE_BAR_HEIGHT`] rather than a local
/// `24`: every dialog in this module draws the same strip, and a second value in the same
/// module is how the eight of them acquired four different heights (rule #101).
const DIALOG_TITLE_BAR_HEIGHT: u32 = dimensions::DIALOG_TITLE_BAR_HEIGHT;

/// How small a revealing dialog is at the instant it appears: 0.92 of its settled size.
///
/// The value lives in [`super::REVEAL_MIN_SCALE`] so `Dialog` and `MessageBox` settle identically —
/// they are the same class of object and a caller must not be able to tell which reveal it is
/// watching. Named locally only because the draw and its test both read it.
const DIALOG_REVEAL_MIN_SCALE: f32 = super::REVEAL_MIN_SCALE;

/// A generic dial‑log window: a titled frame that owns one content widget.
///
/// The dialog is the secondary window a caller fills with its own form. Modality is a
/// stored intent that [`crate::widget::runtime::enter_modal`] / [`crate::widget::runtime::exit_modal`]
/// make real — `open` and `close` drive both the visible state and the modal stack.
pub struct Dialog {
    base: BaseWidget,
    /// Title shown in the dialog's own chrome.
    title: String,
    /// The single content widget the dialog hosts, if any.
    content_widget: Option<ObjectId>,
    /// Whether the dialog blocks interaction with its owner while open.
    modal: bool,
    /// How far the dialog has revealed, `0.0` hidden and `1.0` fully shown.
    ///
    /// # Why this is separate from the widget's own visible flag
    ///
    /// `show`/`hide` carry the *logical* state and the modal stack acts on it; they are answered the
    /// instant they change. This is what the draw path measures the scrim's strength and the frame's
    /// own placement with — so a dialog grows into place rather than appearing at its final size on
    /// one frame and vanishing on another. Same split, same reason, as `Switch`'s `checked`/`travel`;
    /// it starts at `0.0` because a freshly built dialog is hidden.
    reveal: PropertyDriver,
    /// Emitted by [`Dialog::accept`], when the dialog is accepted.
    pub accepted: GenericSignal,
    /// Emitted by [`Dialog::reject`], when the dialog is rejected.
    pub rejected: GenericSignal,
    /// Emitted by [`Dialog::open`].
    pub opened: GenericSignal,
    /// Emitted by [`Dialog::close`].
    pub closed: GenericSignal,
}

impl Dialog {
    /// Creates a titleless, non-modal dialog.
    pub fn new(geometry: Rect) -> Self {
        Self::with_title(String::new(), geometry)
    }

    /// Creates a dialog with a title.
    pub fn with_title(title: impl Into<String>, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dialog, geometry, "Dialog"),
            title: title.into(),
            content_widget: None,
            modal: true,
            // At rest at the hidden end: a freshly built dialog is not shown, so it must not animate
            // itself away on its first frame.
            reveal: PropertyDriver::at(0.0, MotionSlot::Normal),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            opened: GenericSignal::new(),
            closed: GenericSignal::new(),
        }
    }

    /// Returns the dialog's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the dialog's title and requests a redraw.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.base.request_redraw();
    }

    /// Returns the content widget id, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Sets the content widget this dialog hosts.
    ///
    /// Replaces any previous content child and records the parent/child link so the
    /// tree walk (and the modal subtree test) sees it.
    pub fn set_content_widget(&mut self, widget: Option<ObjectId>) {
        if let Some(old) = self.content_widget {
            self.base.remove_child(old);
        }
        self.content_widget = widget;
        if let Some(id) = widget {
            self.base.add_child(id);
            let _ = crate::widget::runtime::with_widget_mut(id, |child| {
                child.set_parent(Some(self.id()));
            });
        }
        self.base.request_redraw();
    }

    /// Returns whether the dialog is modal. Defaults to `true`.
    ///
    /// Records the intent; enforcement is the modal stack in [`crate::widget::runtime`]
    /// (`enter_modal` / `exit_modal`), which [`Dialog::open`] drives.
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// Sets the modality intent.
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }

    /// Shows the dialog, emits `opened`, and — when modal — enters the modal stack.
    pub fn open(&mut self) {
        self.show();
        // Aim the reveal; the frames that follow grow and settle the frame. A dialog that appears at
        // full size on the frame it is shown and vanishes on the frame it is hidden is the one
        // gesture a dialog is *for* being skipped, and `show`/`hide` only carry the logical state.
        self.reveal.set_target(1.0);
        if self.modal {
            let _ = crate::widget::runtime::enter_modal(self.id());
        }
        self.opened.emit();
    }

    /// Hides the dialog, pops it from the modal stack (if it was there), and emits
    /// `closed`.
    pub fn close(&mut self) {
        let _ = crate::widget::runtime::exit_modal(self.id());
        self.hide();
        self.reveal.set_target(0.0);
        self.closed.emit();
    }

    /// How far the dialog has revealed, `0.0` fully hidden and `1.0` fully shown.
    ///
    /// The *drawn* fraction: the scrim's strength and the frame's own offset are functions of it, so
    /// a test can assert the dialog appeared rather than teleported by sampling it per frame.
    pub fn reveal_progress(&self) -> f32 {
        self.reveal.value()
    }

    /// Advances the reveal by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    /// Whether the dialog is between two reveal states -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.reveal.is_moving()
    }

    /// Accepts the dialog: emits `accepted` then closes it.
    pub fn accept(&mut self) {
        self.accepted.emit();
        self.close();
    }

    /// Rejects the dialog: emits `rejected` then closes it.
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.close();
    }

    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered** — a census cell, a layout slot, a
    /// parent's whole client area. Painting it verbatim is what turned the 240x120 census
    /// cell into a 240x120 frame: a rectangle shaped like a dialog rather than a dialog.
    /// [`ControlMetrics::painted_box`] caps each axis at [`dimensions::DIALOG_MIN_WIDTH`] /
    /// [`dimensions::DIALOG_MIN_HEIGHT`] and centres what is left, and it clamps the result
    /// *up* to one pixel so a dialog squeezed to nothing is still visible. Every element
    /// below — the frame, the title bar and the title's line box — is derived from this one
    /// box, so the chrome and its label cannot be placed from different rectangles.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }

    /// The rectangle available to the content child.
    ///
    /// Insets the top by the title bar **only when a title is set**, so a titleless
    /// dialog gives its child the full rect.
    pub fn content_rect(&self) -> Rect {
        let rect = self.frame_rect();
        if self.title.is_empty() {
            return rect;
        }
        let inset = DIALOG_TITLE_BAR_HEIGHT.min(rect.height);
        Rect::new(rect.x, rect.y + inset as i32, rect.width, rect.height - inset)
    }
}

impl Widget for Dialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(
            crate::widget::metrics::dimensions::DIALOG_WIDGET_DEFAULT_WIDTH,
            dimensions::DIALOG_MIN_HEIGHT,
        )
    }

    // The reveal is the control's own animation; the trait spelling is what the frame bus reaches
    // through `&mut dyn Widget`, which is the only way the dialog actually appears.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Dialog::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        Dialog::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Dialog`'s property contract.
impl WidgetProperties for Dialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            "has_content" => Ok(CapabilityValue::Bool(self.content_widget().is_some())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "modal" => match value {
                CapabilityValue::Bool(v) => {
                    self.set_modal(v);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "has_content" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "modal", "has_content", BASE_PROPERTY_NAMES]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "accept" => {
                self.accepted.emit();
                Ok(())
            }
            "reject" => {
                self.rejected.emit();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Dialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button: 1, .. } => self.base.set_mouse_pressed(true),
            Event::MouseRelease { button: 1, .. } => self.base.set_mouse_pressed(false),
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for Dialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // A fully hidden dialog paints nothing at all: "reveal is zero" means the dialog is not on
        // screen, not that it is a very small dialog. Read once and used for everything below — the
        // scrim, the frame's scale — so no two of them can disagree about how far along it is.
        let reveal = self.reveal.value();
        if reveal <= 0.0 {
            return;
        }

        // ── The modal scrim ──
        //
        // A **modal** dialog dims what is behind it: that dimming is the entire visual difference
        // between "a dialog is up and the page is inert" and "a panel happened to be drawn on the
        // page". It was absent here while `bottom_sheet`/`modal_bottom_sheet` both had it, so the
        // one control whose whole purpose is modality was the one that did not show it. The
        // backdrop fades in with the reveal, so it is never fully dimmed while the frame is still
        // growing; one value drives both, so they cannot get out of step.
        if self.modal {
            super::draw_modal_scrim_scaled(context, self.geometry(), reveal);
        }

        // The min-scale floor below applies to a reveal that is *in progress*, so the first visible
        // frame already shows the dialog's shape rather than a speck.

        // The **frame**, not the control's rectangle: see `frame_rect`. A dialog offered a
        // 240x120 census cell paints its own centred box, and every band below — the title
        // strip, its separator and the title's line box — is taken from this one rect.
        let resting = self.frame_rect();
        if resting.width == 0 || resting.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen, because the frame, the title bar
        // and the title text were all hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("dialog");
        // The dialog is a `Surface`-role control, and `Surface` resolves to
        // `theme.colors.background` — the colour the window behind it is already filled
        // with. Painting the frame in it would make the dialog indistinguishable from the
        // window, so the surface is tinted one step toward the foreground: the same
        // distinction `Colors::input_background` makes for a field, applied to a panel.
        // The window fill is read as its own lock acquisition and copied out as a value, so
        // the guard is dropped before anything else touches the theme — the global
        // manager's mutex is not re-entrant.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let themed_surface = theme.as_ref().and_then(|t| t.background_color);
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        let ink = style.text_color.or(themed_ink).unwrap_or(Color::rgb(40, 40, 40));
        // The filter is on the **resolved** value, not only on the theme's: the active
        // theme is applied to every control before it is drawn, so `style.background_color`
        // already holds `Surface`'s window-fill colour and letting it through unfiltered
        // is exactly the invisible-panel defect this guards against.
        let surface = match style.background_color.or(themed_surface) {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border = style
            .border_color
            .or(themed_border)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.45));
        // The title bar is a distinct band on the frame, derived from it so the two stay
        // one visible step apart in either appearance.
        let title_bar = surface.blend(&ink, 0.08);

        // The frame **scales up** from its own centre as the reveal progresses, so the appear reads
        // as a growth rather than a fade-in of a static box. `MIN_SCALE` is the floor for a reveal
        // that is in progress: at reveal 1.0 the scale is exactly 1.0, so a settled dialog is
        // pixel-identical to the pre-animation one. This is the whole reason the resting snapshot
        // is unchanged by this feature.
        let scale = DIALOG_REVEAL_MIN_SCALE + (1.0 - DIALOG_REVEAL_MIN_SCALE) * reveal;
        let rect = super::scale_about_centre(resting, scale);
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.face(
                rect,
                surface,
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
            context.draw_rect(rect, border);
        }

        if self.title.is_empty() {
            return;
        }
        // The title strip is a **band** of the frame, not a sub-rectangle reconstructed
        // from the frame's top edge: `top_band` keeps the strip's thickness and clamps it
        // to the frame, so it cannot start above the frame or grow past it.
        let bar = ControlMetrics::top_band(rect, DIALOG_TITLE_BAR_HEIGHT);
        if bar.height == 0 {
            return;
        }
        context.fill_rect(bar, title_bar);
        context.draw_line(
            Point::new(bar.x, bar.y + bar.height as i32),
            Point::new(bar.x + bar.width as i32, bar.y + bar.height as i32),
            border,
        );
        // The title is centred **on the bar's own band** through the shared primitive: the
        // origin is the glyph's top edge, so `text_line` is what places it on the strip's
        // middle line. The old `rect.y + (bar - title_h) / 2` measured against a height the
        // strip did not have, and a bare literal `y` would reintroduce exactly that.
        let title_font = Font::default();
        let band = ControlMetrics::band_inset(bar, 0);
        let title_line = context.text_line(band, &title_font);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn dialog() -> Dialog {
        Dialog::with_title("Settings", Rect::new(0, 0, 400, 300))
    }

    #[test]
    fn dialog_creation_defaults() {
        let d = Dialog::new(Rect::new(0, 0, 400, 300));
        assert_eq!(d.kind(), WidgetKind::Dialog);
        assert!(d.title().is_empty());
        assert!(d.is_modal());
        assert!(d.content_widget().is_none());
    }

    #[test]
    fn dialog_title_and_modal_roundtrip() {
        let mut d = dialog();
        assert_eq!(d.title(), "Settings");
        d.set_title("Preferences");
        assert_eq!(d.title(), "Preferences");
        d.set_modal(false);
        assert!(!d.is_modal());
    }

    #[test]
    fn dialog_accept_and_reject_emit_their_signals() {
        let mut d = dialog();
        let accepted = Arc::new(AtomicUsize::new(0));
        let rejected = Arc::new(AtomicUsize::new(0));
        let a = accepted.clone();
        let r = rejected.clone();
        d.accepted.connect(move || {
            a.fetch_add(1, Ordering::SeqCst);
        });
        d.rejected.connect(move || {
            r.fetch_add(1, Ordering::SeqCst);
        });

        d.accept();
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        assert_eq!(rejected.load(Ordering::SeqCst), 0);

        d.reject();
        assert_eq!(rejected.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dialog_open_close_emit_lifecycle_signals() {
        let mut d = dialog();
        let opened = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let o = opened.clone();
        let c = closed.clone();
        d.opened.connect(move || {
            o.fetch_add(1, Ordering::SeqCst);
        });
        d.closed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        d.open();
        assert!(d.is_visible());
        assert_eq!(opened.load(Ordering::SeqCst), 1);

        d.close();
        assert!(!d.is_visible());
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dialog_content_rect_insets_only_when_titled() {
        let titleless = Dialog::new(Rect::new(0, 0, 160, 100));
        assert_eq!(titleless.content_rect(), Rect::new(0, 0, 160, 100));

        let titled = Dialog::with_title("T", Rect::new(0, 0, 160, 100));
        assert_eq!(
            titled.content_rect(),
            Rect::new(0, DIALOG_TITLE_BAR_HEIGHT as i32, 160, 100 - DIALOG_TITLE_BAR_HEIGHT)
        );
    }

    #[test]
    fn dialog_with_a_title_paints_chrome() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        // Both are opened and settled first: a dialog that has not been shown has not begun to
        // reveal, and paints nothing at all -- so an unopened pair would compare two empty
        // documents and the title difference would be invisible for the wrong reason.
        let mut titleless = Dialog::new(Rect::new(0, 0, 160, 100));
        let mut titled = Dialog::with_title("Details".to_string(), Rect::new(0, 0, 160, 100));
        for dialog in [&mut titleless, &mut titled] {
            dialog.open();
            while dialog.tick(1000) {}
        }
        let plain = crate::widget::svg::render_to_svg(&mut titleless);
        let decorated = crate::widget::svg::render_to_svg(&mut titled);
        assert_ne!(plain, decorated);
        // The title is `font8x8` glyph geometry rather than a `<text>` element, so the string is
        // not in the document and the check is on **where the ink landed**: a titled dialog gains
        // a run on the title strip, and a titleless one paints no text at all.
        let (left, top, right, bottom) = crate::widget::svg::text_ink_box(&decorated)
            .unwrap_or_else(|| panic!("a titled dialog must paint its title: {decorated}"));
        assert!(right > left, "the title laid down ink: {left}..{right}");
        assert!(
            (0..DIALOG_TITLE_BAR_HEIGHT as i32).contains(&top)
                && bottom <= DIALOG_TITLE_BAR_HEIGHT as i32,
            "the title must sit on the title strip, got {top}..{bottom}"
        );
        assert!(
            crate::widget::svg::text_ink_box(&plain).is_none(),
            "a titleless dialog paints no title ink: {plain}"
        );
    }

    /// A modal dialog dims the page behind it, and a non-modal one does not.
    ///
    /// # The defect this pins
    ///
    /// The dialog is the one control whose whole purpose is modality, and it was the only one of
    /// the modal family with **no scrim**: `bottom_sheet` and `modal_bottom_sheet` both dim the
    /// page, so a dialog and a sheet looked like different degrees of the same thing when they
    /// should have looked like the same thing. The assertion is the composited page: the scrim laid
    /// over the window fill must be **darker** than the window fill itself, which is what "the page
    /// is inert" looks like and what a scrim blended toward the ink would get backwards.
    #[test]
    fn a_modal_dialog_dims_the_page_behind_it() {
        use crate::style::LayerColor;

        let _theme_guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [crate::theme::AppearanceMode::Light, crate::theme::AppearanceMode::Dark]
        {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let page = crate::style::theme_manager()
                .current_theme()
                .expect("a preset is active")
                .colors
                .background;
            let scrim = crate::style::layer_color(LayerColor::Scrim)
                .expect("the preset defines a scrim role");
            let composited = page.blend(&scrim, scrim.a as f32 / 255.0);
            assert!(
                composited.luminance() < page.luminance(),
                "the {appearance:?} scrim must dim the page: {page:?} -> {composited:?}"
            );
        }

        // And it is the *modal* intent that paints it, not merely being a dialog: a non-modal one
        // leaves the page it sits beside untouched.
        //
        // Both are *opened* first, because a dialog that has not been shown has not begun to reveal
        // and paints nothing -- so comparing two hidden dialogs would compare two empty documents and
        // the modality difference would be invisible for the wrong reason.
        let mut modal = Dialog::with_title("Hi", Rect::new(0, 0, 200, 120));
        modal.set_modal(true);
        modal.open();
        while modal.tick(1000) {}
        let with_scrim = crate::widget::svg::render_to_svg(&mut modal);
        let mut modeless = Dialog::with_title("Hi", Rect::new(0, 0, 200, 120));
        modeless.set_modal(false);
        modeless.open();
        while modeless.tick(1000) {}
        let without_scrim = crate::widget::svg::render_to_svg(&mut modeless);
        assert_ne!(
            with_scrim, without_scrim,
            "modality must be visible, or the dialog lies about whether the page is inert"
        );
    }

    /// Opening a dialog reveals it rather than placing it there at full size.
    ///
    /// # The defect this pins
    ///
    /// `open`/`close` carried only the logical state, and the draw read it directly, so the frame
    /// appeared complete on the frame it was shown and vanished on the frame it was hidden. The
    /// assertions are in two halves, because a progress nothing reads is not a reveal: the model must
    /// take an interior value, and the *painted* frame must be smaller at that moment.
    #[test]
    fn opening_a_dialog_reveals_it() {
        use crate::widget::svg::render_to_svg;

        let mut dialog = Dialog::with_title("Hi", Rect::new(0, 0, 200, 120));
        assert_eq!(dialog.reveal_progress(), 0.0, "a fresh dialog is hidden");
        assert!(!dialog.is_animating(), "and owes no frames");

        dialog.open();
        assert!(dialog.is_visible(), "the logical state answers at once");
        assert!(dialog.is_animating(), "while the drawn frame owes frames");

        // The painted frame's width, read from the document: the frame is the widest rounded
        // rectangle the dialog paints.
        fn frame_width(svg: &str) -> u32 {
            svg.split("<rect ")
                .filter(|chunk| chunk.contains("rx=\""))
                .filter_map(|chunk| {
                    let w = chunk.split("width=\"").nth(1)?;
                    w.split('"').next()?.parse::<u32>().ok()
                })
                .max()
                .unwrap_or(0)
        }

        assert!(dialog.tick(60), "still growing after one step");
        let mid = dialog.reveal_progress();
        assert!(
            mid > 0.0 && mid < 1.0,
            "the dialog must pass through an interior reveal (got {mid})"
        );
        let mid_width = frame_width(&render_to_svg(&mut dialog));

        while dialog.tick(60) {}
        assert_eq!(dialog.reveal_progress(), 1.0, "and settle fully shown");
        let settled_width = frame_width(&render_to_svg(&mut dialog));

        assert!(
            mid_width > 0 && mid_width < settled_width,
            "a revealing frame must be smaller than a settled one: mid={mid_width} settled={settled_width}"
        );
    }
}
