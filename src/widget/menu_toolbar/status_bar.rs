// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Status bar widget.
//!
//! # The two messages are two columns of one assembled row
//!
//! BLUE22 §B.8 lists this control's defect as "multiple segments by hand, the last position
//! hard-coded". The strip really is a row of segments — a transient message on the leading side and
//! a permanent one on the trailing side — so the segment boxes are handed to a [`FlexLayout`]: the
//! message column fills, and the layout puts the remainder before it, which is what
//! `justify_content = FlexEnd` means. Nothing here computes the permanent message's `x`.
//!
//! The grip keeps its own derived box (it is a corner affordance, not a row segment), and its
//! reserve is still read from that box — but the reserve is now one of the columns the layout is
//! given rather than a subtraction from the message's width.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::layout::{
    AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent, LayoutParams,
};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::composite::CompositeBuilder;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetFactory, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Status bar widget — shows status messages and permanent widgets.
///
/// Holds two independent strings: a transient `message` on the left and a
/// `permanent_message` on the right. "Permanent" describes the intended role,
/// not the implementation — both fields are plain stored strings and nothing
/// clears either on a timer. See [`StatusBar::show_message`].
///
pub struct StatusBar {
    base: BaseWidget,
    message: String,
    permanent_message: String,
    size_grip_enabled: bool,
    /// Emitted with the new message text on every message change, including the
    /// empty string emitted by [`StatusBar::clear_message`]. Changing the
    /// permanent message does **not** emit it.
    pub message_changed: Signal1<String>,
}
impl StatusBar {
    /// Creates an empty status bar with the size grip enabled.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is 400x24.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StatusBar, geometry, "StatusBar"),
            message: String::new(),
            permanent_message: String::new(),
            size_grip_enabled: true,
            message_changed: Signal1::new(),
        }
    }
    /// Returns the transient message, or `""` when none is shown.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Returns the permanent message, or `""` when none is set.
    pub fn permanent_message(&self) -> &str {
        &self.permanent_message
    }
    /// Returns whether the resize grip is drawn. Defaults to `true`.
    ///
    /// Note that `StatusBar::draw` does not currently render a grip, so this
    /// flag has no visual effect yet.
    pub fn size_grip_enabled(&self) -> bool {
        self.size_grip_enabled
    }
    /// Show a temporary status message (timeout_ms is informational; actual timeout managed externally).
    /// Shows a transient message for `_timeout_ms` milliseconds.
    ///
    /// The widget does not schedule clearing, so a message stays until
    /// [`StatusBar::clear_message`] or another `show_message` call. The caller
    /// owns the timeout. Emits `message_changed` but does not itself request a
    /// redraw.
    ///
    /// # Disabled contract
    ///
    /// A status message is user-visible text; a disabled status bar does not show it, so
    /// announcing the change would report a transition the user never saw. The message is
    /// still stored (the host owns the data) and the suppression is queryable through
    /// [`StatusBar::message_changed_suppression_reason`].
    pub fn show_message(&mut self, message: impl Into<String>, _timeout_ms: u64) {
        self.message = message.into();
        if !self.base.is_enabled() {
            // See `message_changed_suppression_reason`.
            return;
        }
        self.message_changed.emit(self.message.clone());
    }
    /// Clears the transient message and emits `message_changed` with an empty
    /// string. The permanent message is untouched.
    ///
    /// Gated by `enabled` for the same reason as [`StatusBar::show_message`].
    pub fn clear_message(&mut self) {
        self.message.clear();
        if !self.base.is_enabled() {
            // See `message_changed_suppression_reason`.
            return;
        }
        self.message_changed.emit(String::new());
    }

    /// Reports why the next message change would **not** emit `message_changed`, or `None`
    /// when the signal will fire.
    ///
    /// Without this, a host that wrote a message and observed no signal could not tell
    /// "the status bar is disabled" from "the message was already what I wrote".
    pub fn message_changed_suppression_reason(&self) -> Option<&'static str> {
        if !self.base.is_enabled() {
            return Some(
                "message_changed suppressed: the status bar is disabled, so the message is \
                 not user-visible",
            );
        }
        None
    }
    /// Replaces the permanent message and requests a redraw.
    ///
    /// Unlike the transient message this is not reported through
    /// `message_changed`, so a listener relying on that signal will miss it.
    pub fn set_permanent_message(&mut self, msg: impl Into<String>) {
        self.permanent_message = msg.into();
        self.base.request_redraw();
    }
    /// Enables or disables the resize grip flag and requests a redraw. See
    /// [`StatusBar::size_grip_enabled`] — currently has no visual effect.
    pub fn set_size_grip_enabled(&mut self, enabled: bool) {
        self.size_grip_enabled = enabled;
        self.base.request_redraw();
    }
    /// The size grip's own box, or `None` when the grip is off.
    ///
    /// # Why the grip is a box and not a `- 14` literal
    ///
    /// The grip was drawn from `rect.x + width - 14` with three lines spanning 12 px, while the
    /// permanent message reserved `20` for it — two unrelated numbers for one object, so the
    /// measure of "how much room does the grip need" and the measure of "how much room does the
    /// grip use" could not be kept in agreement by inspection. The reserve and the drawing now
    /// read the same box.
    ///
    /// `STATUS_GRIP_SIZE` is a named constant because it is also the answer to "how far from the
    /// strip's corner does the grip sit", which is what the reserve is derived from.
    fn size_grip_rect(&self, band: Rect) -> Option<Rect> {
        if !self.size_grip_enabled {
            return None;
        }
        let size = dimensions::STATUS_GRIP_SIZE.min(band.width).min(band.height);
        // Inset from the corner by the strip's own padding, so the grip's distance from the edge
        // is the same fact as the message's distance from the edge.
        let inset = dimensions::STATUS_BAR_PADDING_H;
        Some(Rect::new(
            band.x + band.width.saturating_sub(size + inset) as i32,
            band.y + band.height.saturating_sub(size + inset) as i32,
            size,
            size,
        ))
    }

    /// The width the permanent message must leave for the grip: 0 when there is no grip.
    ///
    /// Derived from the grip's own box (plus its leading gap), so a wider grip narrows the
    /// message rather than the two overlapping. It is handed to the row as the trailing column's
    /// own width, so the message column's `fill` cannot absorb it.
    fn grip_reserve(&self, band: Rect) -> u32 {
        match self.size_grip_rect(band) {
            Some(grip) => {
                { band.x + band.width as i32 - grip.x + dimensions::STATUS_BAR_PADDING_H as i32 }
                    .max(0) as u32
            }
            None => 0,
        }
    }

    /// The transient message's box and the permanent message's box, placed by the layout.
    ///
    /// # Why the segments are assembled rather than subtracted
    ///
    /// The permanent message used to be drawn at `rect.x + 6` with a width of
    /// `rect.width - reserved` and `HorizontalAlignment::Right` — a right-anchored label expressed
    /// as "start at the leading edge and be as wide as everything except the grip", which is a
    /// different statement from "sit against the trailing edge". The two agree only while nothing
    /// else is on the strip; the moment the message needs a box of its own, or the grip's reserve
    /// changes, the spelling has to be re-derived. Handing two columns to a `FlexEnd` row makes the
    /// anchor a property of the layout, which is how it cannot be forgotten at a call site.
    ///
    /// Both columns declare the strip's own padding as a leading margin, so each message keeps its
    /// distance from the edge it is anchored to — the padding is the same fact for both.
    fn segment_boxes(&self, band: Rect) -> (Rect, Rect) {
        let factory = WidgetFactory::new_with_defaults();
        let reserve = self.grip_reserve(band);
        let mut row = CompositeBuilder::new(
            Box::new(FlexLayout::with_params(
                FlexDirection::Row,
                FlexWrap::NoWrap,
                JustifyContent::FlexEnd,
                AlignItems::Stretch,
                0,
                0,
            )),
            EdgeOffsets::all(0),
            Size::new(0, 0),
        );
        // The transient message: it fills whatever is left, so a long one is fitted rather than
        // running under the permanent message.
        let message = row.add_sized(
            &factory,
            "label",
            &self.message,
            Size::new(dimensions::STATUS_BAR_PADDING_H, band.height),
            LayoutParams::filled().with_margins(EdgeOffsets::new(
                0,
                0,
                0,
                dimensions::STATUS_BAR_PADDING_H,
            )),
        );
        debug_assert!(message.is_some(), "a status segment is a core control");
        // The permanent message: its own preferred width is the grip's reserve, so the column is at
        // least as wide as the corner it must clear — the `reserved` subtraction, expressed as the
        // column's size instead of as shaved-off width.
        let permanent = row.add_sized(
            &factory,
            "label",
            &self.permanent_message,
            Size::new(reserve.max(dimensions::STATUS_BAR_PADDING_H), band.height),
            LayoutParams::new().with_margins(EdgeOffsets::new(
                0,
                0,
                0,
                dimensions::STATUS_BAR_PADDING_H,
            )),
        );
        debug_assert!(permanent.is_some(), "a status segment is a core control");

        let mut placed: Vec<Rect> = Vec::with_capacity(2);
        row.arrange(band, &mut |_, rect| placed.push(rect));
        match (placed.first(), placed.get(1)) {
            (Some(message), Some(permanent)) => (*message, *permanent),
            // `debug_assert!` above makes this unreachable in a debug build; degrading to empty
            // boxes keeps every drawing call inside the strip rather than at a stale rectangle.
            _ => (
                Rect::new(band.x, band.y, 0, band.height),
                Rect::new(band.x + band.width as i32, band.y, 0, band.height),
            ),
        }
    }
}

impl Widget for StatusBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for StatusBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" => Ok(CapabilityValue::String(self.message().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" => {
                // Timeout 0 means "until replaced", which is what a property
                // write means: the caller wants this text to stay put, not to
                // expire on a timer it never asked for.
                self.show_message(expect_string(value)?, 0);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["message", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for StatusBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for StatusBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        // Background
        //
        // From the style, not a literal. This painted `Color::rgb(240, 240, 240)` and so
        // stayed light in a dark theme — a light band across the bottom of a dark window.
        // The literal survives only as the fallback for a style with no colour set.
        let band = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
        context.fill_rect(rect, band);
        context.draw_line(
            Point::new(rect.x, rect.y),
            Point::new(rect.x + rect.width as i32, rect.y),
            style.border_color.unwrap_or_else(|| band.contrast_color().with_alpha(60)),
        );

        let font = Font::default();

        // Glue the strip's ink to the strip's own fill, rather than to a literal: the fill is
        // what the theme resolved, so the ink follows it into either appearance.
        let ink = style.text_color.unwrap_or_else(|| band.contrast_color());

        // The two segments, placed by the row: the message fills and the permanent message is
        // anchored to the trailing edge by `justify_content = FlexEnd`. One assembly for both, so
        // neither can be positioned against the other's assumption.
        let (message_box, permanent_box) = self.segment_boxes(rect);
        // Temporary message (left side).
        if !self.message.is_empty() {
            context.draw_text_fitted(
                context.text_line(message_box, &font),
                &self.message,
                &font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // Permanent message (right side, before the size grip).
        if !self.permanent_message.is_empty() {
            // Muted relative to the main message. The old form blended the ink *toward the
            // band*, which on a dark appearance pulled light text 40% of the way toward a dark
            // band — i.e. it lowered the contrast it was meant to preserve, and the light-mode
            // fallback was a hardcoded `rgb(80,80,80)`, so the two branches disagreed about
            // which appearance they were describing. Blending toward the *band* by a smaller
            // amount, then asserting a legible ratio, is the same visual intent without the
            // direction error.
            let muted = ink.blend(&band, 0.25).legible_on(band, 4.5);
            // The box is the column's own interior, panned to the right within it: the column is at
            // least as wide as the grip's reserve, and the text is anchored to the column's
            // trailing edge rather than to the strip's.
            context.draw_text_fitted(
                context.text_line(permanent_box, &font),
                &self.permanent_message,
                &font,
                muted,
                HorizontalAlignment::Right,
            );
        }
        // Size grip (bottom-right corner). Drawn inside the box the reserve above was derived
        // from, so the message stops exactly where the grip begins.
        if let Some(grip) = self.size_grip_rect(rect) {
            let grip_ink =
                style.border_color.unwrap_or_else(|| band.contrast_color().with_alpha(120));
            // Three diagonals across the grip's own box, so the ink scales with the box rather
            // than with three separate `i * 4` and `+ 12` literals.
            let step = (grip.width / 3).max(1) as i32;
            for i in 0..3 {
                let offset = i * step;
                context.draw_line(
                    Point::new(grip.x + offset, grip.y + grip.height as i32 - 1),
                    Point::new(grip.x + grip.width as i32 - 1, grip.y + offset),
                    grip_ink,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    /// The grip's drawn box and the room reserved for it are one derivation.
    ///
    /// They were two unrelated numbers: the grip was drawn from `rect.x + width - 14` with three
    /// lines spanning 12 px, while the permanent message reserved `20` for it. A wider grip, or
    /// a strip narrow enough for the difference to show, put the message on top of the grip.
    #[test]
    fn the_size_grip_and_its_reserve_are_one_derivation() {
        let band = Rect::new(0, 0, 240, 24);
        let mut bar = StatusBar::new(band);

        // A fresh status bar shows a grip; turning it off reserves nothing at all, which is the
        // half the previous code could not express (it reserved `4` for a grip it never drew).
        bar.set_size_grip_enabled(false);
        assert!(bar.size_grip_rect(band).is_none(), "a disabled grip has no box");
        assert_eq!(bar.grip_reserve(band), 0, "and nothing is reserved for it");

        bar.set_size_grip_enabled(true);
        let grip = bar.size_grip_rect(band).expect("the grip is enabled");
        let reserve = bar.grip_reserve(band);

        // The grip is inside the strip, and the reserve reaches from the strip's trailing edge
        // to the grip's own leading edge (plus the strip's padding as the gap).
        assert!(grip.x >= band.x, "the grip must not leave the strip: {grip:?}");
        assert!(
            grip.x + grip.width as i32 <= band.x + band.width as i32,
            "and must not run past it: {grip:?}"
        );
        assert_eq!(
            reserve,
            (band.x + band.width as i32 - grip.x) as u32 + dimensions::STATUS_BAR_PADDING_H,
            "the message's reserve is measured from the grip's own box"
        );
        // The message's box therefore stops at or before the grip's leading edge.
        let message_right = band.x + band.width as i32 - reserve as i32;
        assert!(
            message_right <= grip.x,
            "a right-aligned message must not be painted under the grip:              message ends at {message_right}, grip starts at {}",
            grip.x
        );
    }

    /// The grip scales with the shared size rather than a private set of literals.
    #[test]
    fn the_grip_uses_the_shared_size() {
        let band = Rect::new(0, 0, 240, 24);
        let mut bar = StatusBar::new(band);
        bar.set_size_grip_enabled(true);
        let grip = bar.size_grip_rect(band).expect("enabled");
        assert_eq!(grip.width, dimensions::STATUS_GRIP_SIZE);
        assert_eq!(grip.height, dimensions::STATUS_GRIP_SIZE);
        assert_eq!(
            band.x + band.width as i32 - (grip.x + grip.width as i32),
            dimensions::STATUS_BAR_PADDING_H as i32,
            "the grip keeps the strip's own padding from the corner"
        );
    }

    #[test]
    fn statusbar_creation_defaults() {
        let sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.message().is_empty());
        assert!(sb.permanent_message().is_empty());
        assert!(sb.size_grip_enabled());
    }

    #[test]
    fn statusbar_show_message() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.message().is_empty());
        sb.show_message("Ready", 3000);
        assert_eq!(sb.message(), "Ready");
    }

    #[test]
    fn statusbar_clear_message() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        sb.show_message("Busy", 5000);
        assert_eq!(sb.message(), "Busy");
        sb.clear_message();
        assert!(sb.message().is_empty());
    }

    #[test]
    fn statusbar_permanent_message() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.permanent_message().is_empty());
        sb.set_permanent_message("Line: 1  Col: 1");
        assert_eq!(sb.permanent_message(), "Line: 1  Col: 1");
        sb.set_permanent_message("");
        assert!(sb.permanent_message().is_empty());
    }

    #[test]
    fn statusbar_size_grip() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.size_grip_enabled());
        sb.set_size_grip_enabled(false);
        assert!(!sb.size_grip_enabled());
        sb.set_size_grip_enabled(true);
        assert!(sb.size_grip_enabled());
    }

    #[test]
    fn statusbar_geometry_delegation() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        sb.set_geometry(Rect::new(0, 700, 800, 24));
        assert_eq!(sb.geometry(), Rect::new(0, 700, 800, 24));
    }

    #[test]
    fn statusbar_visibility() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.is_visible());
        sb.hide();
        assert!(!sb.is_visible());
        sb.show();
        assert!(sb.is_visible());
    }

    #[test]
    fn statusbar_signal_accessors() {
        let sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        let _ = &sb.message_changed;
    }

    #[test]
    fn statusbar_id_kind() {
        let sb_a = StatusBar::new(Rect::new(0, 0, 800, 24));
        let sb_b = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert_ne!(sb_a.id(), sb_b.id());
        assert_eq!(sb_a.kind(), WidgetKind::StatusBar);
        assert_eq!(sb_b.kind(), WidgetKind::StatusBar);
    }

    #[test]
    fn statusbar_draw_produces_svg_output() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        sb.show_message("Ready", 3000);
        let svg = crate::widget::svg::render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
    }

    /// The permanent message is anchored to the strip's trailing edge, clear of the grip.
    ///
    /// # What this pins
    ///
    /// BLUE22 §F.2.2 records this control's defect as "the last segment's position hard-coded".
    /// The old form drew the permanent message from `rect.x + 6` with a width of
    /// `rect.width - reserved` and right alignment — a *right-anchored* label expressed as "start at
    /// the leading edge and be wide" — so the anchor was width arithmetic rather than a statement
    /// about the edge. The row now expresses it as `justify_content = FlexEnd`.
    ///
    /// The assertion is the relation that matters: the segment reaches the strip's trailing edge,
    /// and the grip sits inside the room it reserved. The segment is *wider* than the grip's own
    /// reserve — its own box width is `max(reserve, padding) + padding`, because §B.6 rule 4 makes
    /// the reserve a size the segment carries rather than a gap between two boxes — so the test
    /// states the containment rather than an equality that would encode the layout's arithmetic.
    #[test]
    fn the_permanent_segment_reaches_the_trailing_edge_and_clears_the_grip() {
        let strip = Rect::new(0, 0, 400, 24);
        let mut sb = StatusBar::new(strip);
        sb.set_size_grip_enabled(true);
        sb.set_permanent_message("Line: 1");

        let (_, permanent) = sb.segment_boxes(strip);
        let grip = sb.size_grip_rect(strip).expect("the grip is enabled");
        assert_eq!(
            permanent.x + permanent.width as i32,
            strip.x + strip.width as i32,
            "the permanent segment must reach the strip's trailing edge: {permanent:?}"
        );
        assert!(
            grip.x >= permanent.x,
            "and the grip must still be inside it, so the message clears the corner: \
             segment {permanent:?}, grip {grip:?}"
        );
        assert!(
            permanent.width >= sb.grip_reserve(strip),
            "the segment carries at least the grip's reserve, so the message cannot reach the \
             corner: segment width {}, reserve {}",
            permanent.width,
            sb.grip_reserve(strip)
        );
    }

    /// The transient message fills the room the permanent one and the grip leave.
    #[test]
    fn the_transient_segment_yields_to_the_permanent_one_and_the_grip() {
        let strip = Rect::new(0, 0, 400, 24);
        let mut sb = StatusBar::new(strip);
        sb.show_message("Ready", 0);
        sb.set_permanent_message("Line: 1");

        let (message, permanent) = sb.segment_boxes(strip);
        assert_eq!(
            message.x,
            dimensions::STATUS_BAR_PADDING_H as i32,
            "the message starts at the strip's own padding: {message:?}"
        );
        assert!(
            message.x + message.width as i32 <= permanent.x,
            "the two segments must not overlap: message {message:?}, permanent {permanent:?}"
        );
        assert_eq!(
            permanent.x + permanent.width as i32,
            strip.x + strip.width as i32,
            "and the permanent one keeps the trailing edge"
        );
    }

    /// Turning the grip off lets the permanent message use the room the grip reserved.
    ///
    /// The reserve is derived from the grip's own box, so a strip with no grip reserves nothing
    /// rather than keeping a constant's worth of empty space.
    #[test]
    fn without_a_grip_the_permanent_segment_reaches_the_corner() {
        let strip = Rect::new(0, 0, 400, 24);
        let mut sb = StatusBar::new(strip);
        sb.set_size_grip_enabled(false);
        sb.set_permanent_message("Line: 1");
        assert_eq!(sb.grip_reserve(strip), 0, "no grip reserves no room");
        let (_, permanent) = sb.segment_boxes(strip);
        assert_eq!(
            permanent.x + permanent.width as i32,
            strip.x + strip.width as i32,
            "the segment now reaches the strip's trailing edge: {permanent:?}"
        );
    }

    // ── Enabled contract (BLUE19 T-5 follow-up) ───────────────────────────

    #[test]
    fn statusbar_disabled_does_not_announce_a_message_the_user_cannot_see() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let seen = Arc::new(AtomicUsize::new(0));
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        {
            let seen = seen.clone();
            sb.message_changed.connect(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            });
        }

        sb.show_message("Ready", 0);
        assert_eq!(seen.load(Ordering::SeqCst), 1, "the enabled path must reach a listener");
        assert_eq!(sb.message(), "Ready");

        sb.set_enabled(false);
        sb.show_message("Hidden", 0);
        assert_eq!(sb.message(), "Hidden", "the message is still stored; the host owns the data");
        assert_eq!(
            seen.load(Ordering::SeqCst),
            1,
            "a disabled status bar must not announce text the user cannot see"
        );
        assert!(sb.message_changed_suppression_reason().is_some());

        sb.set_enabled(true);
        assert!(sb.message_changed_suppression_reason().is_none());
        sb.clear_message();
        assert_eq!(seen.load(Ordering::SeqCst), 2, "re-enabling restores the signal");
        assert_eq!(sb.message(), "");
    }
}
