// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dialog widget — a secondary, usually-modal window that hosts its own children.
//!
//! This is the generic `QDialog`-style control: a titled frame that carries a single
//! content widget, exposes modal intent (enforced through [`crate::widget::runtime`]'s
//! modal stack), and announces accept/reject through dedicated signals. It is the
//! standard desktop dialog — distinct from `MessageBox` (a fixed message + buttons)
//! and `PopupWindow` (a chrome-only popup), which `WidgetKind::Dialog` was previously
//! aliased to.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
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
        self.closed.emit();
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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(320, 240)
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
        // The **frame**, not the control's rectangle: see `frame_rect`. A dialog offered a
        // 240x120 census cell paints its own centred box, and every band below — the title
        // strip, its separator and the title's line box — is taken from this one rect.
        let rect = self.frame_rect();
        if rect.width == 0 || rect.height == 0 {
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

        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.fill_rect(rect, surface);
            context.draw_rect(rect, border);
        }

        if self.title.is_empty() {
            return;
        }
        // The title strip is a **band** of the frame, not a sub-rectangle reconstructed
        // from the frame's top edge: `top_band` keeps the strip's thickness and clamps it
        // to the frame, so it cannot start above the frame or grow past it.
        let bar = ControlMetrics::top_band(rect, DIALOG_TITLE_BAR_HEIGHT);
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
        let mut titleless = Dialog::new(Rect::new(0, 0, 160, 100));
        let mut titled = Dialog::with_title("Details".to_string(), Rect::new(0, 0, 160, 100));
        let plain = crate::widget::svg::render_to_svg(&mut titleless);
        let decorated = crate::widget::svg::render_to_svg(&mut titled);
        assert_ne!(plain, decorated);
        assert!(decorated.contains("Details"));
    }
}
