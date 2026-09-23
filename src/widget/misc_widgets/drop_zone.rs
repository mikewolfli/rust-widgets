// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DropZone widget — a named drop target for drag-and-drop payloads.
//!
//! A [`DropZone`] accepts [`crate::event::dnd::DragPayload`]s whose `type_id`
//! matches an accepted MIME type. It renders a dashed border and a highlight when
//! a compatible drag hovers over it, and emits a `payload_dropped` signal when a
//! payload is committed onto it. It implements [`crate::event::dnd::DropTarget`]
//! so it can be handed directly to the drag session without an adapter.

use crate::core::{Color, Point, Rect};
use crate::event::dnd::{DragPayload, DropEffect, DropTarget};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The feedback a drop zone is currently showing.
///
/// # Why five states and not a `hovered` bool
///
/// A drop target has to answer five different questions, and they are not degrees of one another:
///
/// * **Idle** — nothing is being dragged over it.
/// * **Hovering** — *something* is over it, but not something it accepts.
/// * **Accepted** — a payload of the right type is over it, and a drop would be taken.
/// * **Rejected** — a payload the zone refuses is over it, so the user should not let go.
/// * **Dropped** — the payload has just been committed (a transient confirmation).
///
/// Collapsing these to `hovered` is what made the control *mislead*: it lit up identically for a
/// payload it would take and one it would refuse, so a user learned about the refusal only after
/// releasing. The states are ordered by strength below, so "what does the pointer leave behind if it
/// moves here" has exactly one answer even when two conditions overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropZoneState {
    /// No drag is over the zone.
    #[default]
    Idle,
    /// A drag is over the zone but its type does not match.
    Hovering,
    /// A payload this zone accepts is over the zone.
    Accepted,
    /// A payload this zone refuses is over the zone.
    Rejected,
    /// A payload was just committed onto the zone.
    Dropped,
}

impl DropZoneState {
    /// The token this state publishes through the property contract.
    ///
    /// # Why the token and the state are one table
    ///
    /// The read direction and the write direction have to agree about spelling, or a caller driving
    /// the control from a document could not restore what the control reports. Deriving both from
    /// this one match is what stops them drifting.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Hovering => "hovering",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Dropped => "dropped",
        }
    }

    /// Parses the token [`Self::as_str`] publishes.
    pub fn from_token(token: &str) -> Option<Self> {
        match crate::widget::capability::coercion::normalize_key(token).as_str() {
            "idle" => Some(Self::Idle),
            "hovering" => Some(Self::Hovering),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "dropped" => Some(Self::Dropped),
            _ => None,
        }
    }

    /// Whether a drop would be taken right now.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Accepted | Self::Dropped)
    }
}

/// A named accept region for drag-and-drop payloads.
///
/// The zone accepts exactly one MIME type (its `accepted_type`); a payload of any
/// other type is refused. A compatible drag hovering over it lights the zone up,
/// and committing the drop emits [`DropZone::payload_dropped`] with the payload.
pub struct DropZone {
    base: BaseWidget,
    /// The MIME type this zone accepts, e.g. `"text/plain"` or `"card"`.
    accepted_type: String,
    /// Which of the five drag feedback states the zone is currently in.
    ///
    /// See [`DropZoneState`] for why this is not a `hovered` bool.
    state: DropZoneState,
    /// Emitted, with the accepted payload, when a drop is committed onto the zone.
    pub payload_dropped: Signal1<DragPayload>,
}

impl DropZone {
    /// Creates an empty drop zone that accepts the given MIME type.
    pub fn new(geometry: Rect, accepted_type: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DropZone, geometry, "DropZone"),
            accepted_type: accepted_type.into(),
            state: DropZoneState::Idle,
            payload_dropped: Signal1::new(),
        }
    }

    /// Returns the MIME type this zone accepts.
    pub fn accepted_type(&self) -> &str {
        &self.accepted_type
    }

    /// Sets the MIME type this zone accepts and clears any feedback state.
    ///
    /// The state is cleared because it was a statement about the *old* accepted type: leaving a
    /// `Rejected` up would claim the zone still refuses a payload it may now take.
    pub fn set_accepted_type(&mut self, accepted_type: impl Into<String>) {
        self.accepted_type = accepted_type.into();
        self.set_state(DropZoneState::Idle);
    }

    /// Returns the feedback state the zone is showing.
    pub fn state(&self) -> DropZoneState {
        self.state
    }

    /// Sets the feedback state, repainting only when it changed.
    ///
    /// This is the one writer, so the flag and the paint can never disagree about which state the
    /// zone is in — every transition below goes through it.
    pub fn set_state(&mut self, state: DropZoneState) {
        if self.state != state {
            self.state = state;
            self.base.request_redraw();
        }
    }

    /// Returns whether a compatible drag is currently hovering over the zone.
    ///
    /// True for [`DropZoneState::Accepted`] only. This is deliberately **not** "any drag is over it":
    /// the old bool meant that, and it is exactly what let a zone light up for a payload it would
    /// refuse.
    pub fn is_hovered(&self) -> bool {
        self.state == DropZoneState::Accepted
    }

    /// Reports a drag of `payload` entering or moving over the zone.
    ///
    /// # Why this takes the payload rather than a bool
    ///
    /// "A drag is over me" and "a drag I can take is over me" are different facts, and only the
    /// payload can tell them apart. A caller that only knows a pointer is inside should pass the
    /// payload it is carrying and let this decide, instead of presetting the answer.
    pub fn report_hover(&mut self, payload: &DragPayload) {
        self.set_state(if self.accepts(payload) {
            DropZoneState::Accepted
        } else {
            DropZoneState::Rejected
        });
    }

    /// Reports that the payload carried by the drag changed while still over the zone.
    ///
    /// Identical to [`Self::report_hover`]; it exists as a separate name because the two are different
    /// events at the call site and a reader of the drag session should not have to guess that the
    /// re-evaluation is the same operation.
    pub fn report_hover_changed(&mut self, payload: &DragPayload) {
        self.report_hover(payload);
    }

    /// Clears any pending feedback state, e.g. when a drag is cancelled or leaves.
    ///
    /// A `Dropped` confirmation is cleared too: it describes a commit that has already been reported
    /// through [`Self::payload_dropped`], so leaving it up would keep claiming a drop is happening.
    pub fn clear_hover(&mut self) {
        self.set_state(DropZoneState::Idle);
    }

    /// Returns whether `payload` is of a type this zone accepts.
    pub fn accepts(&self, payload: &DragPayload) -> bool {
        payload.is_type(&self.accepted_type)
    }
}

impl DropTarget for DropZone {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        self.accepts(payload)
    }

    fn on_drop(&mut self, payload: &DragPayload, _pos: Point) -> DropEffect {
        // `can_accept` already gated the type; a zone cannot refuse at commit time
        // because acceptance is purely a type match. The state becomes `Dropped` so the zone can show
        // the commit — the old code cleared the highlight, so a successful drop and a cancelled drag
        // left the zone looking identical.
        self.payload_dropped.emit(payload.clone());
        self.set_state(DropZoneState::Dropped);
        DropEffect::Copy
    }

    fn preview_rect(&self, _payload: &DragPayload, _pos: Point) -> Option<Rect> {
        Some(self.geometry())
    }
}

impl Widget for DropZone {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 120)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `DropZone`'s property contract.
///
/// `accepted_type` and `state` are readable/writable; the dropped *signal* is not a property and is
/// subscribed through the event bridge. `state` is writable because a caller driving a drag session
/// from a document or a test must be able to place the zone in the state it would be in, and
/// `drop_zone` otherwise has no way to be driven without a live pointer.
impl WidgetProperties for DropZone {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "accepted_type" => Ok(CapabilityValue::String(self.accepted_type().to_string())),
            "state" => Ok(CapabilityValue::String(self.state().as_str().to_string())),
            // Kept as a derived read so a consumer written against the old contract keeps working;
            // it is true only for `accepted`, which is what the old bool meant to mean.
            "hovered" => Ok(CapabilityValue::Bool(self.is_hovered())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "accepted_type" => {
                self.set_accepted_type(expect_string(value)?);
                Ok(())
            }
            "state" => {
                let token = expect_string(value)?;
                let state =
                    DropZoneState::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_state(state);
                Ok(())
            }
            // Derived from `state`, so writing it directly would be a second way to say the same
            // thing — and one of the two would be able to disagree.
            "hovered" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["accepted_type", "state", "hovered", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for DropZone {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // The drop itself is driven by `DropTarget` through the drag session; here
        // the zone only needs to clear its hover state when the pointer leaves.
        match event {
            Event::MouseLeave { .. } => self.clear_hover(),
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for DropZone {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the fills below
        // were previously hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("drop_zone");
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(36, 107, 201));
        // `drop_zone` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A well
        // painted in that colour would be byte-identical to the frame behind it, so the drop
        // target had no visible face at rest: `drop_zone.svg` carried `rgba(18,18,18)` twice and
        // only the dashed outline said a control was there. A resolved surface equal to the
        // window fill is therefore re-derived a visible step toward the ink, the same guard
        // `dial.rs` and `slider.rs` apply to their own faces. A drop zone has to show a well it
        // drops into, so the fallback steps *away* from the window rather than landing on it.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.10),
        };
        let outline = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.45));

        // Base fill: the theme's surface, lifted toward its ink while a drop is being taken so the
        // drop target reads as active, and *pressed away* from the ink when the payload would be
        // refused.
        //
        // # Why rejected is shaded rather than a third tint of the same blue
        //
        // The two live states must be distinguishable without relying on a hue the user may not
        // perceive, so they differ in **lightness and border weight** as well as in colour: a refused
        // drag shades the well *in* (receding), an accepted one lifts it *out* (inviting). A `Dropped`
        // confirmation is the strongest lift, so it still reads after the pointer has left.
        //
        // Rejected blends toward `BLACK` rather than toward the ink, because `Color::blend` clamps its
        // weight to `0.0..=1.0` — a negative weight is silently a no-op, which would have made
        // `Rejected` byte-identical to `Idle` and left the defect in place with new code around it.
        // Shading toward black is also the crate's established direction for "less prominent"
        // (`bottom_sheet`, the dialog chrome); a dark theme's own surface is dark, so the step is a
        // smaller absolute change there but still moves in the right direction.
        let fill = match self.state {
            DropZoneState::Idle => surface,
            DropZoneState::Hovering => surface.blend(&ink, 0.06),
            DropZoneState::Accepted => surface.blend(&ink, 0.12),
            DropZoneState::Dropped => surface.blend(&ink, 0.20),
            DropZoneState::Rejected => surface.blend(&Color::BLACK, 0.10),
        };
        context.fill_rect(rect, fill);

        // A dashed border drawn as a series of short strokes. The colour and the stroke width both
        // carry the state: an accepted drag draws the zone's own ink at full weight, a resisted one
        // draws the outline *dashed thinner* so it reads as "not here".
        let (border, stroke) = match self.state {
            DropZoneState::Idle | DropZoneState::Hovering => (outline, 1),
            DropZoneState::Accepted => (ink, 2),
            DropZoneState::Dropped => (ink, 2),
            DropZoneState::Rejected => (outline.blend(&ink, 0.25), 1),
        };
        let dash = 8u32;
        let mut x = rect.x;
        while x + dash as i32 <= rect.x + rect.width as i32 {
            context.draw_line_stroke(
                Point::new(x, rect.y),
                Point::new(x + dash as i32, rect.y),
                border,
                stroke,
            );
            context.draw_line_stroke(
                Point::new(x, rect.y + rect.height as i32 - 1),
                Point::new(x + dash as i32, rect.y + rect.height as i32 - 1),
                border,
                stroke,
            );
            x += (dash * 2) as i32;
        }
        let mut y = rect.y;
        while y + dash as i32 <= rect.y + rect.height as i32 {
            context.draw_line_stroke(
                Point::new(rect.x, y),
                Point::new(rect.x, y + dash as i32),
                border,
                stroke,
            );
            context.draw_line_stroke(
                Point::new(rect.x + rect.width as i32 - 1, y),
                Point::new(rect.x + rect.width as i32 - 1, y + dash as i32),
                border,
                stroke,
            );
            y += (dash * 2) as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> DropZone {
        DropZone::new(Rect::new(0, 0, 200, 120), "text/plain")
    }

    #[test]
    fn new_zone_records_its_accepted_type() {
        let z = zone();
        assert_eq!(z.accepted_type(), "text/plain");
        assert!(!z.is_hovered());
    }

    #[test]
    fn accepts_only_matching_type_id() {
        let z = zone();
        assert!(z.accepts(&DragPayload::new("text/plain", "doc1")));
        assert!(!z.accepts(&DragPayload::new("image/png", "img1")));
    }

    #[test]
    fn on_drop_emits_payload_and_confirms_the_drop() {
        let mut z = zone();
        let payload = DragPayload::new("text/plain", "doc1").with_label("doc1");

        let got = std::sync::Arc::new(std::sync::Mutex::new(None::<DragPayload>));
        let sink = got.clone();
        z.payload_dropped.connect(move |p| {
            *sink.lock().unwrap() = Some(p.as_ref().clone());
        });

        // A hover of an acceptable payload, then a drop through the `DropTarget` trait.
        z.report_hover(&payload);
        assert_eq!(z.state(), DropZoneState::Accepted);
        let effect = z.on_drop(&payload, Point::new(10, 10));

        assert_eq!(effect, DropEffect::Copy);
        // The zone now confirms the commit rather than going blank: a successful drop and a cancelled
        // drag must not leave the zone looking identical.
        assert_eq!(z.state(), DropZoneState::Dropped);
        assert!(z.state().is_active(), "a committed drop is still an active state");
        let recorded = got.lock().unwrap().clone();
        assert_eq!(recorded, Some(payload));
    }

    #[test]
    fn set_accepted_type_clears_the_state() {
        let mut z = zone();
        z.set_state(DropZoneState::Accepted);
        z.set_accepted_type("application/json");
        assert_eq!(
            z.state(),
            DropZoneState::Idle,
            "a state about the old accepted type must not survive the change"
        );
        assert_eq!(z.accepted_type(), "application/json");
    }

    #[test]
    fn preview_rect_is_the_zone_geometry() {
        let z = zone();
        let payload = DragPayload::new("text/plain", "doc");
        assert_eq!(z.preview_rect(&payload, Point::new(0, 0)), Some(Rect::new(0, 0, 200, 120)));
    }

    // ─── The five feedback states (F-6) ───

    /// A hovering payload the zone would **refuse** must not look like one it accepts.
    ///
    /// # The defect this pins
    ///
    /// The zone had a single `hovered` bool, set for *any* drag over it. It therefore lit up
    /// identically for a payload it would take and one it would refuse, so the user learned about the
    /// refusal only by releasing — the control actively misled instead of informing.
    #[test]
    fn a_refused_payload_and_an_accepted_one_are_different_states() {
        let mut z = zone();
        let accepted = DragPayload::new("text/plain", "doc1");
        let refused = DragPayload::new("image/png", "img1");

        assert_eq!(z.state(), DropZoneState::Idle, "a new zone is at rest");

        z.report_hover(&refused);
        assert_eq!(z.state(), DropZoneState::Rejected);
        assert!(!z.is_hovered(), "a refused payload is not an accepted hover");
        assert!(!z.state().is_active(), "and no drop would be taken");

        // The same drag session changing payload, without leaving the zone.
        z.report_hover_changed(&accepted);
        assert_eq!(z.state(), DropZoneState::Accepted);
        assert!(z.is_hovered());
        assert!(z.state().is_active(), "a drop would now be taken");

        // And back again, so the transition is not one-way.
        z.report_hover_changed(&refused);
        assert_eq!(z.state(), DropZoneState::Rejected);

        z.clear_hover();
        assert_eq!(z.state(), DropZoneState::Idle);
    }

    /// Every state must paint differently, or the state is a field nothing reads.
    ///
    /// Read off the rendered SVG by counting distinct outputs rather than by asserting a particular
    /// colour: the point is that the five states are *tellable apart*, not that one of them is a
    /// specific blue.
    #[test]
    fn every_feedback_state_paints_differently() {
        use crate::widget::svg::render_to_svg;

        let mut rendered = Vec::new();
        for state in [
            DropZoneState::Idle,
            DropZoneState::Hovering,
            DropZoneState::Accepted,
            DropZoneState::Rejected,
            DropZoneState::Dropped,
        ] {
            let mut z = zone();
            z.set_state(state);
            rendered.push((state, render_to_svg(&mut z)));
        }

        for i in 0..rendered.len() {
            for j in (i + 1)..rendered.len() {
                let (a_state, a) = &rendered[i];
                let (b_state, b) = &rendered[j];
                assert_ne!(
                    a, b,
                    "{a_state:?} and {b_state:?} rendered identically, so the state is invisible"
                );
            }
        }
    }

    /// The state is reachable and round-trips through the property contract, in both spellings.
    #[test]
    fn the_state_round_trips_through_the_property_api() {
        let mut z = zone();
        assert_eq!(z.get("state").unwrap().as_str(), Some("idle"));

        for token in ["hovering", "accepted", "rejected", "dropped", "idle"] {
            z.set("state", CapabilityValue::String(token.to_string())).unwrap();
            assert_eq!(z.get("state").unwrap().as_str(), Some(token), "{token} did not round-trip");
        }

        // The long/case variants are accepted, so a caller need not know the exact spelling.
        z.set("state", CapabilityValue::String("ACCEPTED".to_string())).unwrap();
        assert_eq!(z.state(), DropZoneState::Accepted);

        // An unknown token is refused rather than silently ignored.
        assert!(z.set("state", CapabilityValue::String("nonsense".to_string())).is_err());
        assert_eq!(z.state(), DropZoneState::Accepted, "a failed write must not change the state");
    }

    /// `hovered` stays readable as the derived "would this zone take a drop", and stays unwritable.
    ///
    /// It is kept because a consumer written against the old contract must keep working; it is
    /// *derived* because a second writer would be a second way to say the same thing.
    #[test]
    fn the_legacy_hovered_flag_is_derived_from_the_state() {
        let mut z = zone();
        assert_eq!(z.get("hovered").unwrap().as_bool(), Some(false));
        z.set("state", CapabilityValue::String("accepted".to_string())).unwrap();
        assert_eq!(z.get("hovered").unwrap().as_bool(), Some(true));
        z.set("state", CapabilityValue::String("rejected".to_string())).unwrap();
        assert_eq!(
            z.get("hovered").unwrap().as_bool(),
            Some(false),
            "a refused payload never counted as a hover"
        );
        assert!(z.set("hovered", CapabilityValue::Bool(true)).is_err());
    }
}
