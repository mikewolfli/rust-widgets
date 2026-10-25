// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DropZone widget — a named drop target for drag-and-drop payloads.
//!
//! A [`DropZone`] accepts [`crate::event::dnd::DragPayload`]s whose `type_id`
//! matches an accepted MIME type. It renders a dashed border and a highlight when
//! a compatible drag hovers over it, and emits a `payload_dropped` signal when a
//! payload is committed onto it. It implements [`crate::event::dnd::DropTarget`]
//! so it can be handed directly to the drag session without an adapter.

use crate::compat::format;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::dnd::{DragPayload, DropEffect, DropTarget};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_string_list};
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

/// How the drop zone emphasises an accepted drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropZoneHoverStyle {
    /// Lift the whole well toward the ink (the historical behaviour).
    #[default]
    Highlight,
    /// Keep the fill and strengthen only the outline.
    Outline,
    /// A subtle tint, for a zone that must not dominate the surface.
    Tint,
}

impl DropZoneHoverStyle {
    /// The token this style publishes.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Highlight => "highlight",
            Self::Outline => "outline",
            Self::Tint => "tint",
        }
    }

    /// Parses [`Self::as_str`].
    pub fn from_token(token: &str) -> Option<Self> {
        match crate::widget::capability::coercion::normalize_key(token).as_str() {
            "highlight" => Some(Self::Highlight),
            "outline" => Some(Self::Outline),
            "tint" => Some(Self::Tint),
            _ => None,
        }
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
    /// Additional accepted MIME types, beyond [`Self::accepted_type`].
    accepted_types: Vec<String>,
    /// Whether multiple dropped payloads accumulate instead of replacing one another.
    multi_file: bool,
    /// How an accepted drag is emphasised.
    hover_style: DropZoneHoverStyle,
    /// Whether the count of dropped files is painted as a badge.
    show_count_badge: bool,
    /// File names collected by the accepted drops.
    dropped_files: Vec<String>,
    /// Which of the five drag feedback states the zone is currently in.
    ///
    /// See [`DropZoneState`] for why this is not a `hovered` bool.
    state: DropZoneState,
    /// Emitted with the file-name list when a drag enters the zone.
    pub zone_entered: Signal1<Vec<String>>,
    /// Emitted when a drag leaves the zone without dropping.
    pub zone_exited: Signal1<Vec<String>>,
    /// Emitted, with the accepted file-name list, when a drop is committed.
    pub dropped: Signal1<Vec<String>>,
    /// Emitted when a payload is refused at commit time.
    pub drop_rejected: GenericSignal,
    /// Emitted, with the accepted payload, when a drop is committed onto the zone.
    pub payload_dropped: Signal1<DragPayload>,
}

impl DropZone {
    /// Creates an empty drop zone that accepts the given MIME type.
    pub fn new(geometry: Rect, accepted_type: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DropZone, geometry, "DropZone"),
            accepted_type: accepted_type.into(),
            accepted_types: Vec::new(),
            multi_file: false,
            hover_style: DropZoneHoverStyle::Highlight,
            show_count_badge: false,
            dropped_files: Vec::new(),
            state: DropZoneState::Idle,
            zone_entered: Signal1::new(),
            zone_exited: Signal1::new(),
            dropped: Signal1::new(),
            drop_rejected: GenericSignal::new(),
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
        // One implementation, reachable both as an inherent method and through `dyn DropTarget`.
        // The explicit qualification selects the trait's method, not this inherent one, so the
        // call cannot recurse.
        <Self as DropTarget>::report_hover(self, payload);
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
        self.accepts_type(&payload.type_id)
    }

    /// Whether a MIME type is accepted by this zone.
    pub fn accepts_type(&self, type_id: &str) -> bool {
        type_id == self.accepted_type || self.accepted_types.iter().any(|item| item == type_id)
    }

    /// The additional accepted MIME types.
    pub fn accepted_types(&self) -> &[String] {
        &self.accepted_types
    }

    /// Replaces the additional accepted MIME types.
    pub fn set_accepted_types(&mut self, types: Vec<String>) {
        if self.accepted_types == types {
            return;
        }
        self.accepted_types = types;
        self.set_state(DropZoneState::Idle);
    }

    /// Whether multiple drops accumulate.
    pub fn multi_file(&self) -> bool {
        self.multi_file
    }

    /// Enables or disables accumulation across drops.
    pub fn set_multi_file(&mut self, multi: bool) {
        if self.multi_file == multi {
            return;
        }
        self.multi_file = multi;
        if !multi {
            self.dropped_files.truncate(1);
        }
        self.base.request_redraw();
    }

    /// The hover emphasis style.
    pub fn hover_style(&self) -> DropZoneHoverStyle {
        self.hover_style
    }

    /// Sets the hover emphasis style.
    pub fn set_hover_style(&mut self, style: DropZoneHoverStyle) {
        if self.hover_style == style {
            return;
        }
        self.hover_style = style;
        self.base.request_redraw();
    }

    /// Whether the dropped-count badge is painted.
    pub fn show_count_badge(&self) -> bool {
        self.show_count_badge
    }

    /// Shows or hides the dropped-count badge.
    pub fn set_show_count_badge(&mut self, show: bool) {
        if self.show_count_badge == show {
            return;
        }
        self.show_count_badge = show;
        self.base.request_redraw();
    }

    /// The file names collected by accepted drops.
    pub fn dropped_files(&self) -> &[String] {
        &self.dropped_files
    }

    /// The file names a payload contributes.
    fn payload_files(payload: &DragPayload) -> Vec<String> {
        let name = if payload.label.is_empty() { &payload.item_id } else { &payload.label };
        if name.is_empty() {
            Vec::new()
        } else {
            vec![name.clone()]
        }
    }
}

impl DropTarget for DropZone {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        self.accepts(payload)
    }

    fn on_drop(&mut self, payload: &DragPayload, _pos: Point) -> DropEffect {
        if !self.can_accept(payload) {
            self.drop_rejected.emit();
            return DropEffect::None;
        }
        if self.multi_file {
            self.dropped_files.extend(Self::payload_files(payload));
        } else {
            self.dropped_files = Self::payload_files(payload);
        }
        self.dropped.emit(self.dropped_files.clone());
        self.payload_dropped.emit(payload.clone());
        self.set_state(DropZoneState::Dropped);
        DropEffect::Copy
    }

    fn preview_rect(&self, _payload: &DragPayload, _pos: Point) -> Option<Rect> {
        Some(self.geometry())
    }

    /// Turns the runtime's hover report into the zone's own feedback state.
    ///
    /// The inherent [`DropZone::report_hover`] is the same operation; this impl is what
    /// lets the coordinator call it through `dyn DropTarget` without downcasting to the
    /// concrete control.
    fn report_hover(&mut self, payload: &DragPayload) {
        let next =
            if self.accepts(payload) { DropZoneState::Accepted } else { DropZoneState::Rejected };
        if self.state == DropZoneState::Idle {
            self.zone_entered.emit(Self::payload_files(payload));
        }
        self.set_state(next);
    }

    /// Clears the highlight when the drag leaves without dropping.
    fn report_leave(&mut self) {
        if self.state != DropZoneState::Idle {
            self.zone_exited.emit(Vec::new());
        }
        self.clear_hover();
    }
}

impl Widget for DropZone {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 120)
    }
    /// Opts the zone into the runtime's drag coordinator as a drop target.
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        Some(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "payload_dropped" => {
                Some(EventSignalRef::mapped("payload_dropped", &self.payload_dropped, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            // The declared shape is `ListScalar` of `String`, so each file is delivered as its own
            // `CapabilityValue::String`; a debug spelling would be one unparseable scalar.
            "zone_entered" => Some(EventSignalRef::mapped(
                "zone_entered",
                &self.zone_entered,
                |values: &Vec<String>| {
                    CapabilityValue::Tuple(
                        values.iter().map(|value| CapabilityValue::String(value.clone())).collect(),
                    )
                },
            )),
            "zone_exited" => Some(EventSignalRef::mapped(
                "zone_exited",
                &self.zone_exited,
                |values: &Vec<String>| {
                    CapabilityValue::Tuple(
                        values.iter().map(|value| CapabilityValue::String(value.clone())).collect(),
                    )
                },
            )),
            "dropped" => {
                Some(EventSignalRef::mapped("dropped", &self.dropped, |values: &Vec<String>| {
                    CapabilityValue::Tuple(
                        values.iter().map(|value| CapabilityValue::String(value.clone())).collect(),
                    )
                }))
            }
            "drop_rejected" => Some(EventSignalRef::unit("drop_rejected", &self.drop_rejected)),
            _ => None,
        }
    }
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
            "accepted_types" => Ok(CapabilityValue::Tuple(
                self.accepted_types
                    .iter()
                    .map(|item| CapabilityValue::String(item.clone()))
                    .collect(),
            )),
            "multi_file" => Ok(CapabilityValue::Bool(self.multi_file)),
            "hover_style" => Ok(CapabilityValue::String(self.hover_style.as_str().to_string())),
            "show_count_badge" => Ok(CapabilityValue::Bool(self.show_count_badge)),
            "dropped_count" => Ok(CapabilityValue::UInt(self.dropped_files.len() as u64)),
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
            "accepted_types" => {
                self.set_accepted_types(expect_string_list(value)?);
                Ok(())
            }
            "multi_file" => {
                self.set_multi_file(expect_bool(value)?);
                Ok(())
            }
            "hover_style" => {
                let token = expect_string(value)?;
                let style = DropZoneHoverStyle::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_hover_style(style);
                Ok(())
            }
            "show_count_badge" => {
                self.set_show_count_badge(expect_bool(value)?);
                Ok(())
            }
            "dropped_count" => Err(CapabilityAccessError::ReadOnlyProperty),
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
        property_names_of![
            "accepted_type",
            "accepted_types",
            "multi_file",
            "hover_style",
            "show_count_badge",
            "dropped_count",
            "state",
            "hovered",
            BASE_PROPERTY_NAMES
        ]
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
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("drop_zone");
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
            DropZoneState::Accepted => match self.hover_style {
                DropZoneHoverStyle::Highlight => surface.blend(&ink, 0.12),
                DropZoneHoverStyle::Outline => surface,
                DropZoneHoverStyle::Tint => surface.blend(&ink, 0.04),
            },
            DropZoneState::Dropped => surface.blend(&ink, 0.20),
            DropZoneState::Rejected => surface.blend(&Color::BLACK, 0.10),
        };
        context.face_with_gradient(
            rect,
            fill,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // A dashed border drawn as a series of short strokes. The colour and the stroke width both
        // carry the state: an accepted drag draws the zone's own ink at full weight, a resisted one
        // draws the outline *dashed thinner* so it reads as "not here".
        let (border, stroke) = match self.state {
            DropZoneState::Idle | DropZoneState::Hovering => (outline, 1),
            DropZoneState::Accepted => match self.hover_style {
                DropZoneHoverStyle::Outline => (ink, 3),
                DropZoneHoverStyle::Highlight | DropZoneHoverStyle::Tint => (ink, 2),
            },
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

        // A dashed outline says "a control is here" but not what it wants. The accepted type is
        // the one fact a zone holds, so it is painted in the middle as the cue — without it the
        // census picture was an empty well, and a user had to guess whether a drag would be
        // accepted. The band is inset past the dashes so the text never sits on the outline.
        let font = crate::widget::metrics::effective_font(style);
        if rect.width > 40 && rect.height > 24 {
            let mut hint = String::new();
            let _ = core::fmt::Write::write_fmt(
                &mut hint,
                format_args!("Drop {} here", self.accepted_type),
            );
            let inner = Rect::new(rect.x + 10, rect.y, rect.width.saturating_sub(20), rect.height);
            context.draw_text_fitted_cow(
                inner,
                &hint,
                font,
                ink.blend(&fill, 0.30),
                HorizontalAlignment::Center,
            );
        }

        // The dropped-count badge: a small disc at the top-right whose number is the number of
        // file names the zone is holding.
        if self.show_count_badge && !self.dropped_files.is_empty() {
            let radius = 9u32;
            let center = Point::new(
                rect.x + rect.width as i32 - radius as i32 - 2,
                rect.y + radius as i32 + 2,
            );
            context.fill_circle(center, radius, ink);
            let mut count = String::new();
            let _ = core::fmt::Write::write_fmt(
                &mut count,
                format_args!("{}", self.dropped_files.len()),
            );
            let badge_font = crate::core::Font::bold("Arial", 9.0);
            let band = Rect::new(
                center.x - radius as i32,
                center.y - radius as i32,
                radius * 2,
                radius * 2,
            );
            context.draw_text_fitted_cow(
                band,
                &count,
                &badge_font,
                ink.contrast_color(),
                HorizontalAlignment::Center,
            );
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
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
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

    #[test]
    fn zone_lifecycle_signals_and_accepted_types_are_forwarded() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut z = zone();
        z.set_accepted_types(vec!["image/png".to_string()]);
        assert!(z.accepts_type("image/png"));
        z.set_multi_file(true);
        z.set_show_count_badge(true);
        z.set_hover_style(DropZoneHoverStyle::Outline);

        let entered = Arc::new(AtomicUsize::new(0));
        let exited = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let rejected = Arc::new(AtomicUsize::new(0));
        let (e, x, d, r) = (
            Arc::clone(&entered),
            Arc::clone(&exited),
            Arc::clone(&dropped),
            Arc::clone(&rejected),
        );
        z.zone_entered.connect(move |_| {
            e.fetch_add(1, Ordering::SeqCst);
        });
        z.zone_exited.connect(move |_| {
            x.fetch_add(1, Ordering::SeqCst);
        });
        z.dropped.connect(move |_| {
            d.fetch_add(1, Ordering::SeqCst);
        });
        z.drop_rejected.connect(move || {
            r.fetch_add(1, Ordering::SeqCst);
        });

        let good = DragPayload::new("image/png", "a.png");
        z.report_hover(&good);
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        z.on_drop(&good, Point::new(0, 0));
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(z.dropped_files(), &["a.png".to_string()]);
        z.report_leave();
        assert_eq!(exited.load(Ordering::SeqCst), 1);

        let bad = DragPayload::new("application/x-nope", "b.txt");
        z.on_drop(&bad, Point::new(0, 0));
        assert_eq!(rejected.load(Ordering::SeqCst), 1);
        assert!(z.event_signal_dyn("zone_entered").is_some());
        assert!(z.event_signal_dyn("dropped").is_some());
    }
}
