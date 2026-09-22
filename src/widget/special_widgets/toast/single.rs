// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Toast` — one transient notification, without the queue.

use super::item::ToastLevel;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The horizontal padding of a toast's message from the accent stripe: 12.
///
/// The message starts clear of the stripe, which is 4 px wide; 12 is the stripe plus the same
/// 8 px gutter the rest of the crate uses between an indicator and its label.
const TOAST_PADDING_H: i32 = 12;

/// The trailing inset of a toast's close affordance from the bar's right edge: 6.
///
/// Deliberately its own value rather than [`TOAST_PADDING_H`]: a dismiss glyph reads as
/// attached to the toast's edge, where a line of text needs the gutter. Keeping the two apart
/// is what stops a change to the message padding from moving the button a user aims at.
const TOAST_CLOSE_INSET: i32 = 6;

/// A toast's close affordance: a 14x14 glyph box.
const TOAST_CLOSE_SIZE: u32 = 14;
use crate::{impl_widget_property_hooks, property_names_of};

/// A single transient notification message.
///
/// # Why this is not `ToastStack`
///
/// `ToastStack` is a *container*: it queues items, lays them out as rows, and
/// tracks a selection among them. The common case — show one message — does not
/// need any of that, and forcing it through the stack made callers construct an
/// item, own a stack, and push into it just to display one line.
///
/// This control owns one message and one level, and emits `dismissed` by itself,
/// so a host can mount it, connect the signal, and forget it. It deliberately does
/// not schedule its own expiry: a UI library has no clock it can trust, and a
/// control that silently disappears on a timer is untestable. The host owns the
/// timer and calls [`Self::dismiss`], which is what makes the ttl data rather than
/// behaviour.
pub struct Toast {
    base: BaseWidget,
    message: String,
    level: ToastLevel,
    ttl_ms: u32,
    dismissible: bool,
    /// Emitted when the toast is dismissed, with its message.
    pub dismissed: Signal1<String>,
}

impl Toast {
    /// Creates a toast showing `message` at [`ToastLevel::Info`].
    pub fn new(geometry: Rect, message: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Toast, geometry, "Toast"),
            message: message.into(),
            level: ToastLevel::Info,
            ttl_ms: 3000,
            dismissible: true,
            dismissed: Signal1::new(),
        }
    }

    /// Returns the message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets the message.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.base.request_redraw();
    }

    /// Returns the severity level.
    pub fn level(&self) -> ToastLevel {
        self.level
    }

    /// Sets the severity level.
    pub fn set_level(&mut self, level: ToastLevel) {
        self.level = level;
        self.base.request_redraw();
    }

    /// Returns the host-owned time-to-live hint in milliseconds.
    pub fn ttl_ms(&self) -> u32 {
        self.ttl_ms
    }

    /// Sets the time-to-live hint. Values below 100 ms are raised to it, matching
    /// `ToastItem::new`: a toast shorter than a frame is not a duration.
    pub fn set_ttl_ms(&mut self, ttl_ms: u32) {
        self.ttl_ms = ttl_ms.max(100);
    }

    /// Whether the close affordance is shown.
    pub fn is_dismissible(&self) -> bool {
        self.dismissible
    }

    /// Shows or hides the close affordance.
    pub fn set_dismissible(&mut self, dismissible: bool) {
        self.dismissible = dismissible;
        self.base.request_redraw();
    }

    /// Dismisses the toast, emitting `dismissed`.
    ///
    /// Emitting is unconditional rather than guarded on `dismissible`: that flag
    /// controls whether the *user* is offered a close button, not whether the
    /// program may end the toast. A non-dismissible toast still expires.
    pub fn dismiss(&mut self) {
        self.dismissed.emit(self.message.clone());
    }

    /// The rectangle of the close affordance, when it is shown.
    fn close_rect(&self) -> Option<Rect> {
        if !self.dismissible {
            return None;
        }
        // Placed from the toast's own band, so the dismiss control sits on the bar the message
        // is painted in rather than half way down a 120 px canvas that the toast does not fill.
        let band = self.band();
        let size = TOAST_CLOSE_SIZE.min(band.height);
        Some(Rect::new(
            band.x + band.width as i32 - size as i32 - TOAST_CLOSE_INSET,
            band.y + (band.height.saturating_sub(size) / 2) as i32,
            size,
            size,
        ))
    }

    /// The bar the toast actually paints: full width, `dimensions::TOAST_HEIGHT` tall,
    /// centred in the rectangle it was given.
    ///
    /// # Why the toast is not the rectangle
    ///
    /// A toast is a floating notification bar of one fixed height. Taking `rect.height` made a
    /// 240x120 census cell a 120 px-tall toast whose severity stripe ran the full canvas as a
    /// 4x120 band and whose message sat on the canvas's middle line — a card shaped like a toast
    /// rather than a toast. The band is the single derivation the stripe, the message and the
    /// close affordance all read.
    fn band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TOAST_HEIGHT)
    }

    /// Whether `pos` is over the close affordance.
    fn is_over_close(&self, pos: Point) -> bool {
        self.close_rect().is_some_and(|close| {
            pos.x >= close.x
                && pos.x < close.x + close.width as i32
                && pos.y >= close.y
                && pos.y < close.y + close.height as i32
        })
    }
}

impl Widget for Toast {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// One text row plus padding, matching a toast's shape in both the Material and
    /// the desktop conventions.
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 48)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Toast {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" | "text" => Ok(CapabilityValue::String(self.message.clone())),
            "level" => Ok(CapabilityValue::String(toast_level_token(self.level).to_string())),
            "ttl_ms" => Ok(CapabilityValue::UInt(self.ttl_ms as u64)),
            "dismissible" => Ok(CapabilityValue::Bool(self.dismissible)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" | "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_message(text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "level" => match value {
                CapabilityValue::String(token) => {
                    let level =
                        parse_toast_level(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                    self.set_level(level);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "ttl_ms" => match value {
                CapabilityValue::UInt(ms) => {
                    self.set_ttl_ms(u32::try_from(ms).unwrap_or(u32::MAX));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "dismissible" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_dismissible(flag);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["message", "level", "ttl_ms", "dismissible", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `toast` publishes.
    ///
    /// `dismiss` maps onto the widget's real `dismiss` and emits `dismissed`
    /// with the current message. `set_message` and `set_level` carry the text
    /// and the severity token respectively, so a bare invocation is reported as
    /// needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "dismiss" => {
                self.dismiss();
                Ok(())
            }
            "set_message" | "set_level" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Toast {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } if self.is_over_close(*pos) => {
                self.dismiss();
            }
            Event::KeyPress { key: 27, modifiers: _ } => {
                self.dismiss();
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Toast {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("toast");
        // `toast` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The toast's own fill is therefore a step toward
        // the foreground, so it reads as a raised surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(240, 245, 253));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.15));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let background = resolved.blend(&text_color, 0.08);
        // The severity stripe is a *state* indicator, so it reads the theme's
        // semantic tokens rather than a literal colour per level.
        let accent = crate::style::semantic_color(match self.level {
            ToastLevel::Info => crate::style::SemanticColor::Info,
            ToastLevel::Success => crate::style::SemanticColor::Success,
            ToastLevel::Warning => crate::style::SemanticColor::Warning,
            ToastLevel::Error => crate::style::SemanticColor::Error,
        })
        .map(|token| token.blend(&background, 0.15))
        .unwrap_or_else(|| background.blend(&text_color, 0.6));
        // The close affordance is secondary chrome, so it is a tint of the resolved
        // foreground rather than a second literal.
        let close_color = text_color.blend(&background, 0.4);

        // ── The bar actually painted ──
        //
        // The toast's chrome is one notification bar, not the whole area it was handed; the
        // severity stripe runs that bar's height and the message centres on it.
        let band = self.band();
        context.fill_rect(band, background);
        context.draw_rect(band, border);
        // A severity stripe rather than a badge: at toast height there is no room
        // for a square, and a stripe reads at any width.
        context.fill_rect(
            Rect::new(band.x, band.y, dimensions::TOAST_ACCENT_WIDTH, band.height),
            accent,
        );

        let text_x = band.x + TOAST_PADDING_H;
        // Centred through the shared primitive. The previous origin carried a hand-tuned
        // `(height + 12) / 2`, i.e. half a line box added to the band's middle — the same
        // half-line error as `height / 2`, only in the opposite shape. The close button on
        // this same toast already sat at the true centre, so the message and its own dismiss
        // control disagreed about the row they shared.
        let font = Font::default();
        let line = context.text_line(band, &font);
        // Bounded to end before the close button, so a long message cannot run under it.
        let text_width = self
            .close_rect()
            .map(|close| (close.x - text_x - 4).max(0) as u32)
            .unwrap_or_else(|| band.width.saturating_sub(TOAST_PADDING_H as u32));
        if !self.message.is_empty() && text_width > 0 {
            context.draw_text_fitted(
                Rect { x: text_x, y: line.y, width: text_width, height: line.height },
                &self.message,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        if let Some(close) = self.close_rect() {
            context.draw_line(
                Point::new(close.x + 3, close.y + 3),
                Point::new(close.x + close.width as i32 - 3, close.y + close.height as i32 - 3),
                close_color,
            );
            context.draw_line(
                Point::new(close.x + close.width as i32 - 3, close.y + 3),
                Point::new(close.x + 3, close.y + close.height as i32 - 3),
                close_color,
            );
        }
    }
}

/// Token spelling of a [`ToastLevel`], as the property contract publishes it.
fn toast_level_token(level: ToastLevel) -> &'static str {
    match level {
        ToastLevel::Info => "info",
        ToastLevel::Success => "success",
        ToastLevel::Warning => "warning",
        ToastLevel::Error => "error",
    }
}

/// Inverse of [`toast_level_token`]; `None` for an unrecognised token.
fn parse_toast_level(token: &str) -> Option<ToastLevel> {
    match token {
        "info" => Some(ToastLevel::Info),
        "success" => Some(ToastLevel::Success),
        "warning" => Some(ToastLevel::Warning),
        "error" => Some(ToastLevel::Error),
        _ => None,
    }
}
