// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Switch/Toggle widget — a modern on/off binary state control.
//!
//! The Switch widget presents a sliding toggle that represents a boolean state,
//! similar to iOS UISwitch or Android Switch material widget. It supports
//! on/off toggling, animated transitions, and accessibility role mapping.

use crate::core::{Color, Rect, Size};
// Gated exactly like the test module that uses it: `#[cfg(test)]` alone is true in a build where
// `full_widgets` is off, and the import would then be unused (a warning, which this crate's
// profile checks treat as a defect).
use crate::event::key_codes;
#[cfg(all(test, full_widgets))]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    active_profile, focus_ring_color, profile_metrics, ControlMetrics, ControlMetricsProfile,
    FocusRing, SwitchGeometry as SwitchShape, FOCUS_HALO_BLUR_RADIUS, FOCUS_RING_WIDTH,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A named switch preset.
///
/// # Why a preset and not a second control
///
/// Every one of these is the same *control* — one boolean, one gesture, one animation — drawn
/// at different proportions. The exact shape lives in the metrics layer
/// ([`crate::widget::metrics::SwitchGeometry`]); this enum is the caller-facing name for the
/// three presets the library ships, so a host can say "draw this switch the macOS way"
/// without assembling a shape by hand.
///
/// [`Material`](Self::Material) is the default and is pixel-identical to the switch this
/// control has always drawn, including following the active metrics profile's track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwitchGeometry {
    /// Material's `52x32` track with a 14 px thumb radius.
    #[default]
    Material,
    /// iOS's `51x31` track with a 13 px thumb radius and a press stretch.
    Cupertino,
    /// macOS's `40x22` track with a 9 px thumb radius and a narrow inset.
    MacOS,
}

/// macOS's drawn shape: a `40x22` track with an 18 px thumb and a 2 px inset.
const MACOS_SWITCH_SHAPE: SwitchShape =
    SwitchShape { track: Size::new(40, 22), thumb_radius: 9, thumb_inset: 2, press_stretch: 0 };

impl SwitchGeometry {
    /// The token this preset is spelled with.
    pub const fn token(self) -> &'static str {
        match self {
            SwitchGeometry::Material => "material",
            SwitchGeometry::Cupertino => "cupertino",
            SwitchGeometry::MacOS => "macos",
        }
    }

    /// Parses a preset token published by [`SwitchGeometry::token`].
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "material" => Some(SwitchGeometry::Material),
            "cupertino" => Some(SwitchGeometry::Cupertino),
            "macos" => Some(SwitchGeometry::MacOS),
            _ => None,
        }
    }

    /// The exact drawn shape this preset selects.
    const fn shape(self) -> SwitchShape {
        match self {
            SwitchGeometry::Material => SwitchShape::MATERIAL,
            SwitchGeometry::Cupertino => SwitchShape::CUPERTINO,
            SwitchGeometry::MacOS => MACOS_SWITCH_SHAPE,
        }
    }

    /// The preset that names `shape`, or `Material` for a caller-supplied custom shape.
    ///
    /// The enum names presets rather than arbitrary shapes, so a shape built field by field
    /// has no token; [`Switch::drawn_shape`] remains the exact answer for that case.
    fn of(shape: SwitchShape) -> Self {
        if shape == SwitchShape::CUPERTINO {
            SwitchGeometry::Cupertino
        } else if shape == MACOS_SWITCH_SHAPE {
            SwitchGeometry::MacOS
        } else {
            SwitchGeometry::Material
        }
    }
}

impl From<SwitchGeometry> for SwitchShape {
    fn from(preset: SwitchGeometry) -> Self {
        preset.shape()
    }
}

/// The thumb's travel geometry, derived once for the paint path and the drag mapping.
struct ThumbTravel {
    track_rect: Rect,
    thumb_size: u32,
    inset: u32,
    drawn_width: u32,
    span: u32,
}

/// A press on the thumb that may become a horizontal drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThumbDrag {
    /// The pointer's x at the press, so a drag can be told from a click.
    origin_x: i32,
    /// Whether the pointer has moved far enough to count as a drag.
    moved: bool,
}

/// How far the pointer must travel before a press counts as a drag rather than a click.
///
/// Two pixels: enough to absorb the jitter of a click, small enough that a deliberate drag
/// is recognised on its first noticeable movement.
const SWITCH_DRAG_THRESHOLD: i32 = 2;

/// Switch/Toggle widget for binary on/off state selection.
pub struct Switch {
    base: BaseWidget,
    /// The **logical** state: the answer `is_checked` gives, changed by the user's action
    /// the instant it happens. It is not what the thumb is drawn from — see `travel`.
    checked: bool,
    /// `true` between a pointer press that hit this control and the release that ends
    /// it. The release is what commits the toggle, so a press that began elsewhere and
    /// merely *ends* over the switch must not flip it — see `handle_event`.
    ///
    /// Distinct from [`BaseWidget::is_pressed`], which is the paint flag the base keeps
    /// for every control: this one is the *commit guard* the toggle needs, and it means
    /// "a press landed on me and has not been released yet" rather than "paint pressed".
    pressed: bool,
    /// The **drawn** state: how far the thumb has travelled, `0.0` at the off end and
    /// `1.0` at the on end.
    /// # Why this is not `checked`
    ///
    /// `checked` is the logical state, and a toggle must be answered immediately: a
    /// caller that reads `is_checked` after a click has to see the new value the moment
    /// the click happened. Drawing the thumb from `checked` would therefore *also* move
    /// the thumb the instant the click arrived, with no transition — and a control whose
    /// motion has to be observable cannot put its presentation into the value the caller
    /// reads. The logical position and the drawn position are deliberately distinct,
    /// as a slider's own `position` (logical) versus its
    /// `visualPosition` are; this field is the crate's
    /// drawn position. `travel = 0` renders exactly the old off-end appearance, so a
    /// snapshot taken without a `tick` is unchanged.
    travel: PropertyDriver,
    /// The shape the track and thumb are drawn in.
    ///
    /// Defaults to Material's `52x32`; `cupertino_switch` is this same control carrying
    /// `SwitchShape::CUPERTINO`. One control, one gesture, one animation — and the two
    /// presets cannot drift on the parts that are not about size, because there is only
    /// one implementation of those parts.
    geometry: SwitchShape,
    /// The press that may become a horizontal thumb drag.
    ///
    /// `Some` while a press that landed on the thumb is held. A press elsewhere on the control
    /// leaves this `None`, which is what keeps the historical "press toggles" gesture intact.
    drag: Option<ThumbDrag>,
    /// The ON track's colour, when the caller wants to name it rather than take the
    /// theme's accent.
    ///
    /// # Why this is separate from `background_color`
    ///
    /// `style.background_color` is the **off** track (the chrome the control sits on);
    /// writing the active colour there paints an off switch in the on colour, which is
    /// exactly the bug `cupertino_switch` had — its iOS green landed on the resting track.
    /// The two ends are two facts and now have two fields.
    on_track: Option<Color>,
    /// The thumb's explicitly named colour, when the caller wants one.
    ///
    /// The resting thumb is derived from the theme's foreground (mixed most of the way to white),
    /// which is right for the default look but leaves a theme-tuned switch with no way to name
    /// the knob. `None` keeps that derivation.
    thumb: Option<Color>,
    /// Emitted when the checked state changes.
    pub toggled: Signal1<bool>,
}

impl Switch {
    /// Creates a new Switch widget with the given geometry.
    /// Initial state is unchecked (off).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Switch, geometry, "Switch"),
            checked: false,
            pressed: false,
            // A toggle travelling across its track is a *larger* movement than a direct
            // reaction to the pointer, which is what the theme's `slow` token describes.
            // Starting at rest (0.0) keeps a freshly built switch at the off end instead
            // of fading *out* on its first frame.
            travel: PropertyDriver::at(0.0, MotionSlot::Slow),
            geometry: SwitchShape::MATERIAL,
            drag: None,
            on_track: None,
            thumb: None,
            toggled: Signal1::new(),
        }
    }

    /// Returns whether the switch is in the checked (on) state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked state. Emits `toggled` signal if the state actually changes.
    ///
    /// # Why this does **not** declare that the control drives its own frames
    ///
    /// `checked` decides which end of the track the thumb travels toward, so the change is
    /// paint-worthy — and the tempting move is to say so here, since a control that repaints itself
    /// tells the frame bus to leave it alone. This one must not: the *drawn* state is `travel`,
    /// `travel` only advances in [`Self::tick`], and [`Self::tick`] belongs to whoever owns the
    /// control — the frame loop when it is mounted, [`crate::widget::draw_bridge::draw_of`] when the
    /// host owns it. Declaring self-driving here would take the owned case away from the only
    /// driver it has, leaving the thumb frozen at whichever end it was on when the caller toggled
    /// it.
    ///
    /// Asking for a repaint is not what makes the motion visible either, which is the second
    /// reason there is nothing to do here:
    ///
    /// * **mounted** — [`crate::widget::runtime::tick_animations`] reports `true` while any control
    ///   is in flight, and the frame loop paints again because of that;
    /// * **host-owned** — the paint path advances the control and reports the result through
    ///   [`crate::widget::runtime::animation_bus_needs_another_frame`], the same answer by a
    ///   different route.
    ///
    /// So the honest declaration is the one already in place: a `Switch` does not drive its own
    /// frames. Everything that must re-read `checked` — the capability layer, the JSON loader, the
    /// signal a host connected — is reached by the signal below.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            // Before the signal: a handler reading the style back must see the state it is
            // being told about, not the previous one. `apply_active_theme` runs once, at
            // creation, so without this the control keeps the fill of the state it was built
            // in — see [`crate::style::reapply_active_theme_state`].
            //
            // This does **not** make the track change colour on its own: `draw` blends
            // `off_track` toward `on_track` by `travel.value()`, and `travel` only advances in
            // `tick`, so the colour still belongs to the frame loop. What this fixes is the
            // style, which is what a caller reads and what a later re-theme merges onto.
            crate::style::reapply_active_theme_state(self);
            self.toggled.emit(checked);
        }
    }

    /// Toggles the checked state.
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }

    /// `true` while a pointer press that hit this control is still held.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Returns whether this switch is the keyboard's current target, regardless of
    /// whether a ring is drawn for it.
    pub fn is_focused(&self) -> bool {
        self.base.focus_reason().is_some()
    }

    /// Returns whether a focus ring should be painted right now.
    ///
    /// The single question every draw site asks — [`BaseWidget::draws_focus_ring`] —
    /// evaluated in one place so a control cannot accidentally implement "has focus"
    /// as "draw the ring".
    pub fn visual_focus(&self) -> bool {
        self.base.draws_focus_ring()
    }

    /// Returns whether the pointer is currently over this switch.
    ///
    /// Hover is recorded by [`BaseWidget`] from [`crate::event::Event::MouseEnter`] /
    /// [`crate::event::Event::MouseLeave`], which the widget runtime synthesises as
    /// the pointer moves between controls (no platform backend produces them).
    pub fn is_hovered(&self) -> bool {
        self.base.is_hovered()
    }

    /// Sets the hovered flag.
    ///
    /// # Why this is public
    ///
    /// The `MouseEnter`/`MouseLeave` arms set the flag from real pointer events, but a
    /// host driving a control from its own input layer — a touch backend that has no
    /// hover concept, a test, or a designer previewing a state — needs a way to say
    /// "show me this control hovered". Without it the hovered appearance was reachable
    /// only by synthesising an event.
    ///
    /// Setting it also requests a redraw, because the flag changes what is painted.
    pub fn set_hovered(&mut self, hovered: bool) {
        // Delegate to the trait implementation, which writes the flag **and** re-resolves the
        // theme for `switch:hover`; this control's implementation submits a narrow repaint.
        // See `Button::set_hovered` for why an inherent method must delegate through the trait
        // object rather than writing the field itself.
        <dyn Widget>::set_hovered(self, hovered);
    }

    /// How far the thumb has travelled, `0.0` at the off end and `1.0` at the on end.
    ///
    /// This is the *drawn* position, not the logical state: it is what the draw site
    /// reads to place the thumb and to blend the track colour, so the two cannot slide
    /// out of step with each other.
    pub fn travel_progress(&self) -> f32 {
        self.travel.value()
    }

    /// The shape this switch's track and thumb are drawn in.
    ///
    /// Named `drawn_shape` rather than `geometry` because `Widget::geometry()` already
    /// answers a different question (the control's rectangle), and a same-named accessor
    /// would silently shadow it at every call site.
    pub fn drawn_shape(&self) -> SwitchShape {
        self.geometry
    }

    /// The named preset this switch is drawn with.
    ///
    /// Named `switch_geometry` rather than `geometry` for the same reason
    /// [`Switch::drawn_shape`] is not called `shape`: `Widget::geometry()` answers the
    /// control's rectangle and must not be shadowed. A caller that supplied a custom shape
    /// through [`Switch::set_geometry`] has no named preset and is reported as
    /// [`SwitchGeometry::Material`]; [`Switch::drawn_shape`] is the exact answer for that case.
    pub fn switch_geometry(&self) -> SwitchGeometry {
        SwitchGeometry::of(self.geometry)
    }

    /// Selects a named preset for the track and thumb.
    ///
    /// `Material` stores the default shape, which is the one the active metrics profile may
    /// resize; `Cupertino` and `MacOS` store their own fixed proportions and are therefore
    /// not profile-resized. The default is `Material`, so a switch that never calls this is
    /// pixel-identical to the control this library has always drawn.
    ///
    /// This is the preset route; [`Switch::set_geometry`] remains the exact-shape route for a
    /// caller that built its own proportions.
    pub fn set_switch_geometry(&mut self, preset: SwitchGeometry) {
        let shape = preset.shape();
        if self.geometry == shape {
            return;
        }
        self.geometry = shape;
        self.base.request_redraw();
        self.base.request_layout();
    }

    /// The shape the layout and paint paths actually use.
    ///
    /// # Why the stored preset is not overwritten
    ///
    /// `geometry` records the shape the **caller** chose, and `drawn_shape` reports that choice.
    /// A switch still on the default preset, [`SwitchShape::MATERIAL`], is the one the metrics
    /// profile may resize: a Macos-profile theme draws macOS's `38x22` track, while Material's
    /// profile answers the same `52x32` the control always drew. An explicitly chosen preset — the
    /// iOS switch's `51x31` — is a platform shape in its own right and is left alone. Comparing
    /// the whole preset rather than the track alone also preserves a caller's custom thumb or
    /// stretch even when its track happens to match the default width and height.
    fn effective_geometry(&self) -> SwitchShape {
        if self.geometry != SwitchShape::MATERIAL {
            return self.geometry;
        }
        SwitchShape { track: profile_metrics(active_profile()).switch_track, ..self.geometry }
    }

    /// Replaces the drawn shape.
    ///
    /// # What this is for
    ///
    /// `cupertino_switch` uses it to carry `SwitchShape::CUPERTINO`. It is public
    /// rather than crate-private because a host that wants an iOS-shaped switch under
    /// its own name — or a third preset — should not have to reimplement the gesture and
    /// the animation to get it.
    ///
    /// It accepts either a named preset ([`SwitchGeometry`], converted to its shape) or an
    /// exact [`SwitchShape`], so a caller that wants one of the library's presets can spell it
    /// `set_geometry(SwitchGeometry::MacOS)` and a caller with custom proportions can pass the
    /// shape itself.
    ///
    /// It also reports the change as paint-worthy: the thumb's rectangle and the
    /// control's `size_hint` both read this field, so a silent assignment would leave a
    /// laid-out switch painted at the old size.
    pub fn set_geometry<G: Into<SwitchShape>>(&mut self, geometry: G) {
        let geometry = geometry.into();
        if self.geometry == geometry {
            return;
        }
        self.geometry = geometry;
        self.base.request_redraw();
        self.base.request_layout();
    }

    /// The ON track's explicitly named colour, if the caller named one.
    pub fn on_track_color(&self) -> Option<Color> {
        self.on_track
    }

    /// Names the ON track's colour, overriding the theme's accent.
    ///
    /// `None` restores the theme-resolved behaviour. This is the *only* way to say
    /// "this switch's active end is this colour": `set_style` with a `background_color`
    /// sets the **off** track, because that field is the control's chrome.
    pub fn set_on_track_color(&mut self, color: Option<Color>) {
        if self.on_track == color {
            return;
        }
        self.on_track = color;
        self.base.request_redraw();
    }

    /// The thumb's explicitly named colour, if the caller named one.
    pub fn thumb_color(&self) -> Option<Color> {
        self.thumb
    }

    /// Names the thumb's colour, or restores the theme-derived one with `None`.
    pub fn set_thumb_color(&mut self, color: Option<Color>) {
        if self.thumb == color {
            return;
        }
        self.thumb = color;
        self.base.request_redraw();
    }

    /// The thumb's rectangle for the current travel, or `None` when the track is too
    /// small to hold a disc.
    ///
    /// # Why this is an accessor and not an inline expression in `draw`
    ///
    /// The thumb's position is the value an animation is *for*: the only way to assert
    /// "the thumb slid rather than jumped" is to sample this at successive frames. Keeping
    /// the derivation here means the geometry test and the draw path cannot disagree about
    /// where the thumb is — the same "one derivation" rule the rest of the crate follows.
    ///
    /// `None` for a degenerate track rather than a zero-size rectangle: a switch laid out
    /// smaller than its own track has no thumb to point at, and the caller must be able to
    /// tell that from a thumb at the origin.
    pub fn thumb_rect(&self, rect: Rect) -> Option<Rect> {
        let travel_geometry = self.thumb_travel(rect)?;
        // A disabled switch shows its logical state at rest; an enabled one follows the
        // travel, which is what lets the thumb be seen crossing.
        let travel = if self.base.is_enabled() {
            self.travel.value()
        } else if self.checked {
            1.0
        } else {
            0.0
        };
        // A resting thumb is a disc; a held one stretches sideways into a capsule where
        // the shape asks for it. The stretch is applied to the *span* the thumb travels
        // over as well as to the thumb itself, so a stretched thumb still stops at the
        // track's inner edge instead of overhanging it.
        let travel_x = travel_geometry.inset as i32 + (travel_geometry.span as f32 * travel) as i32;
        // The centre travels; the rectangle grows around it. Placing the *left edge* at
        // the travel offset would make a held thumb jump left by its own stretch, which
        // reads as the control twitching on touch-down rather than responding.
        let centre_x =
            travel_geometry.track_rect.x + travel_x + travel_geometry.thumb_size as i32 / 2;
        let thumb_x = centre_x - travel_geometry.drawn_width as i32 / 2;
        let thumb_y = travel_geometry.track_rect.y + travel_geometry.inset as i32;
        Some(Rect::new(thumb_x, thumb_y, travel_geometry.drawn_width, travel_geometry.thumb_size))
    }

    /// The track and the span the thumb travels over, derived once.
    ///
    /// Both [`Switch::thumb_rect`] and the drag mapping read this, so the grip on screen and
    /// the travel the pointer addresses are the same geometry rather than two estimates.
    fn thumb_travel(&self, rect: Rect) -> Option<ThumbTravel> {
        let shape = self.effective_geometry();
        let track_rect = ControlMetrics::center_in(rect, shape.track);
        let thumb_size =
            shape.thumb_size().min(track_rect.height.saturating_sub(shape.thumb_inset * 2));
        if thumb_size == 0 {
            return None;
        }
        let held = self.base.is_pressed() && shape.press_stretch > 0;
        let stretch = if held { shape.press_stretch } else { 0 };
        let drawn_width = thumb_size + stretch * 2;
        let span = track_rect
            .width
            .saturating_sub(drawn_width + shape.thumb_inset * 2)
            .max(thumb_size.saturating_sub(drawn_width));
        Some(ThumbTravel { track_rect, thumb_size, inset: shape.thumb_inset, drawn_width, span })
    }

    /// The travel fraction a pointer at `x` selects, clamped to `0.0..=1.0`.
    ///
    /// The inverse of the placement [`Switch::thumb_rect`] performs: the same origin and the
    /// same span, so a dragged thumb sits exactly under the pointer that is moving it.
    fn travel_for_x(&self, rect: Rect, x: i32) -> f32 {
        let Some(travel_geometry) = self.thumb_travel(rect) else {
            return 0.0;
        };
        if travel_geometry.span == 0 {
            return 0.0;
        }
        let origin = travel_geometry.track_rect.x
            + travel_geometry.inset as i32
            + travel_geometry.thumb_size as i32 / 2;
        ((x - origin) as f32 / travel_geometry.span as f32).clamp(0.0, 1.0)
    }

    /// Advances the thumb's travel by `delta_ms` and reports whether another frame is
    /// needed.
    ///
    /// # The contract this implements
    ///
    /// Return `true` while there is still movement and `false` once the thumb has
    /// settled, so a host can stop scheduling frames for a control that is not moving. A
    /// `tick` that always returned `true` would keep the whole application repainting
    /// forever, which is why the boolean is part of the signature rather than something
    /// the caller infers.
    ///
    /// # Why the target is recomputed here
    ///
    /// The target comes from the control's own logical state on every tick, so a toggle
    /// that arrived without a `tick` in between is picked up rather than missed — and a
    /// reversal mid-flight re-aims the travel from where it currently is instead of
    /// restarting from the far end.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        // While the pointer is dragging the thumb, it — not `checked` — owns the travel. A
        // tick that re-aimed at the logical state here would pull the thumb out from under
        // the hand holding it.
        if self.drag.is_some() {
            return false;
        }
        // One place states which end `checked` means, so the tick and the "am I moving?" query
        // cannot disagree about it (`Switch::travel_target`).
        self.travel.set_target(self.travel_target());
        self.travel.tick(delta_ms)
    }

    /// The travel the logical `checked` flag calls for.
    ///
    /// Split out because two readers answer questions about the same fact -- the tick aims at
    /// it and `is_animating` compares against it -- and a `if self.checked { 1.0 } else { 0.0 }`
    /// written twice is exactly the kind of copy that drifts.
    fn travel_target(&self) -> f32 {
        if self.checked {
            1.0
        } else {
            0.0
        }
    }

    /// Submits one animated frame as damage bounded by the painted box plus the
    /// focus-ring and halo margin.
    ///
    /// `bounds` is `Some` only when the caller already has the rectangle; interaction
    /// paths pass `None` and let the runtime derive the box.
    ///
    /// A control that is unmounted falls back to [`BaseWidget::request_redraw`], which
    /// keeps the redraw signal contract for host-owned controls. An interaction frame in
    /// [`crate::widget::runtime::RepaintMode::Full`] still routes through
    /// [`crate::widget::runtime::request_repaint_rect`], whose documented fallback repaints
    /// the whole control.
    fn request_frame_repaint(&self, bounds: Option<Rect>) {
        let _ = bounds;
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.id()) {
                let narrow = crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full;
                // Interaction paths may take the whole-control fallback in `Full`;
                // `bounds` lets a caller that already has the rectangle skip the lookup.
                if narrow || bounds.is_none() {
                    let damage = bounds
                        .or_else(|| crate::widget::runtime::painted_box_with_damage_margin(id))
                        .unwrap_or_else(|| self.damage_bounds());
                    crate::widget::runtime::request_repaint_rect(id, damage);
                    // The replaced `request_redraw` emitted this signal; keep it.
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// This control's own rectangle grown by
    /// [`crate::widget::metrics::DAMAGE_MARGIN`].
    ///
    /// Used when the runtime cannot read the mounted geometry back, which is the case
    /// while an interaction event holds the control out of the registry. Deriving the
    /// rectangle from `Widget::geometry` yields the same answer the runtime returns for
    /// the mounted control.
    #[cfg(not(alloc_frugal))]
    fn damage_bounds(&self) -> Rect {
        let rect = self.geometry();
        let margin = crate::widget::metrics::DAMAGE_MARGIN;
        let offset = margin as i32;
        Rect::new(
            rect.x.saturating_sub(offset),
            rect.y.saturating_sub(offset),
            rect.width.saturating_add(margin).saturating_add(margin),
            rect.height.saturating_add(margin).saturating_add(margin),
        )
    }
}

impl Widget for Switch {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    /// Sets hover and repaints only the area this control can change.
    ///
    /// The flag write and theme re-resolution match the trait default; the repaint is
    /// the narrow one, so a hover does not invalidate the whole control.
    fn set_hovered(&mut self, hovered: bool) {
        if self.base.is_hovered() == hovered {
            return;
        }
        self.base.set_hovered(hovered);
        self.set_state_theme_hook();
        self.request_frame_repaint(None);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // The hint describes the control's own floor rather than the drawn track: the track is
        // `self.effective_geometry().track` — the caller's preset, or the active profile's track
        // for a switch on the default — but a switch that can show a focus ring needs room for the
        // ring's inset on both sides. The track's width alone made the hint *narrower* than the
        // width at which the ring is drawable, so `size_hint` described a control whose focus
        // state could not be rendered. The height stays the track's own; the width is the track
        // plus one focus ring either side. Both come from the shared table, so this hint and the
        // control's geometry are one fact.
        let hint = crate::widget::metrics::switch_hint_size(
            self.effective_geometry().track,
            FOCUS_RING_WIDTH,
        );
        debug_assert!(
            hint.height >= crate::widget::metrics::dimensions::SLIDER_TRACK_HEIGHT,
            "a switch's hint must read the shared metric system"
        );
        hint
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Reports `Checked` for an on switch, so the preset's `switch:checked` override is reachable.
    /// The same defect and the same fix as `CheckBox::widget_state` — the trait default knows the
    /// four primitive flags and none of them means "latched", so the key was declared and
    /// unreachable. This one is the most visible of the four: a switch's whole meaning is its
    /// position.
    fn widget_state(&self) -> crate::style::WidgetState {
        use crate::style::WidgetState;
        if !self.base.is_enabled() {
            return WidgetState::Disabled;
        }
        if self.checked {
            return WidgetState::Checked;
        }
        if self.base.is_pressed() {
            WidgetState::Pressed
        } else if self.base.is_hovered() {
            WidgetState::Hover
        } else if self.base.draws_focus_ring() {
            WidgetState::Focused
        } else {
            WidgetState::Normal
        }
    }

    // `Switch::tick` owns the thumb travel; the trait spelling is what the animation bus
    // reaches through `&mut dyn Widget`, which is the only way the sliding actually happens.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Switch::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.drag.is_none() && (self.travel.value() - self.travel_target()).abs() > f32::EPSILON
    }
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
            "toggled" => Some(EventSignalRef::mapped("toggled", &self.toggled, |v| {
                CapabilityValue::Bool(*v)
            })),
            _ => None,
        }
    }
}

/// `Switch`'s property contract.
impl WidgetProperties for Switch {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            // A named ON colour is optional, so "not set" is `Null` rather than a colour the
            // control would paint.
            "on_track_color" => Ok(match self.on_track_color() {
                Some(color) => CapabilityValue::Color(color),
                None => CapabilityValue::Null,
            }),
            "switch_geometry" => Ok(CapabilityValue::String(self.switch_geometry().token().into())),
            "thumb_color" => Ok(match self.thumb_color() {
                Some(color) => CapabilityValue::Color(color),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            "on_track_color" => {
                // `Null` restores the theme's accent; a hex string is accepted because the JSON
                // and C surfaces carry colours as text.
                let color = match value {
                    CapabilityValue::Null => None,
                    CapabilityValue::Color(color) => Some(color),
                    CapabilityValue::String(raw) => match crate::core::Color::parse_hex(&raw) {
                        Some(color) => Some(color),
                        None => return Err(CapabilityAccessError::TypeMismatch),
                    },
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_on_track_color(color);
                Ok(())
            }
            "switch_geometry" => {
                let token = expect_string(value)?;
                let preset =
                    SwitchGeometry::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_switch_geometry(preset);
                Ok(())
            }
            "thumb_color" => {
                // `Null` restores the theme-derived thumb; a hex string is accepted because the
                // JSON and C surfaces carry colours as text.
                let color = match value {
                    CapabilityValue::Null => None,
                    CapabilityValue::Color(color) => Some(color),
                    CapabilityValue::String(raw) => match crate::core::Color::parse_hex(&raw) {
                        Some(color) => Some(color),
                        None => return Err(CapabilityAccessError::TypeMismatch),
                    },
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_thumb_color(color);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SWITCH_PROPERTIES`; the two presentation knobs are published because both
        // have public setters and neither is reachable through `checked`.
        property_names_of![
            "checked",
            "on_track_color",
            "switch_geometry",
            "thumb_color",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `switch` publishes.
    ///
    /// `toggle` is the control's zero-argument action and flips the latch, emitting
    /// `toggled` on the transition exactly as a pointer activation does.
    /// `set_checked` assigns the same state but needs the boolean, so it is refused as
    /// [`CapabilityAccessError::OutOfRange`] — use `set("checked", ..)` — rather than
    /// reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_checked" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Switch {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let style = self.style();

        // Track geometry
        //
        // The track is a **fixed-size** stadium centred inside `rect`, not a scaling of
        // it. Deriving the track from the geometry made the *control* the track: a
        // 240x120 census cell drew a 240x120 stadium with a 116x116 thumb, which is a
        // picture of a switch-shaped rectangle rather than a switch. The geometry is the
        // widget's *occupancy* — its hit area and its layout slot — and the drawn chrome
        // is a separate, fixed proportion of it. That is the same split a Material switch
        // makes (a 52x32 track, a 14px thumb radius), and it is why `size_hint`
        // describes the control without the drawn shape depending on the layout engine's
        // answer.
        //
        // The values come from the shared `dimensions` table rather than being three
        // independent literals, and the track itself follows the active metrics profile when
        // the caller left the shape on its default. As literals they were `52`/`32`/`2` here
        // and `50`/`28` in `size_hint` — four numbers describing one control, with nothing tying
        // them together, so the hint and the ink could (and did) describe different switches.
        // `ControlMetrics::center_in` is the shared derivation for "fixed-size chrome
        // centred in the area I was given", and it clamps *down* rather than up: a control
        // laid out smaller than the nominal track must not paint outside the rectangle it
        // was given, since nothing clips a widget at this layer.
        let shape = self.effective_geometry();
        let track_rect = ControlMetrics::center_in(rect, shape.track);
        // The track's corner radius is half its height: a stadium. The thumb's own size and
        // position come from `Switch::thumb_rect`, so the two derivations cannot disagree.
        let track_height = track_rect.height;

        // Draw track
        //
        // The track colour resolves **caller first, then the semantic token, then the
        // literal**. The rung that used to sit here — the theme's *resolved* background —
        // was dead code: `switch` classifies as `WidgetRole::Choice`, and `role_colors` in
        // `src/theme/manager.rs` writes `Some(input_background())` into that role's
        // background unconditionally. So `themed_background` was always `Some`, and the
        // accent rung below it (the `Success` token, which is what makes an ON switch
        // *green*) could never be reached: every switch drew the theme's field grey, and
        // `switch.svg` showed a grey track for both the ON and the OFF state.
        //
        // `theme_derived` is what separates the two things that `background_color` was
        // conflating. When the theme authored the style the value belongs to the theme,
        // not the caller, and the control is free to substitute a more meaningful token;
        // when the caller set a colour it must win. Same rule, and the same mechanism, as
        // `banner.rs`.
        let theme = crate::style::resolved_theme_style_shared("switch");
        let caller_background = if style.theme_derived { None } else { style.background_color };
        // A stripped device build has no theme module, so the semantic token cannot be read
        // and the literal below is the only rung. `#[cfg]` on the binding rather than on the
        // call keeps the binding's type identical in both profiles.
        #[cfg(device_profile)]
        let themed_accent = crate::style::semantic_color(crate::style::SemanticColor::Success);
        #[cfg(not(device_profile))]
        let themed_accent: Option<Color> = None;
        // The ON end also honours the theme's **declared** `switch:checked` background, which is
        // a different fact from the switch's own `Success`-token accent: `Success` is "this is on",
        // `switch:checked` is "a theme author asked for this fill when on". The declared layer wins
        // when present, and `Success` stays the fallback — the same ladder `radiobutton.rs` uses.
        // Previously the resolved `switch:checked` value was computed and then dropped, so a theme
        // that changed it saw nothing move.
        #[cfg(device_profile)]
        let declared_checked = crate::style::resolved_theme_style_for_state_shared(
            "switch",
            crate::style::WidgetState::Checked,
        )
        .and_then(|resolved| resolved.background_color);
        #[cfg(not(device_profile))]
        let declared_checked: Option<Color> = None;
        // The OFF track is *chrome*, so it descends from the theme's own resolved
        // background — the field grey `Choice` resolves to — stepping one shade toward the
        // ink when that colour would be the window's own fill. A track painted in the
        // window colour is invisible against the surface the switch sits on.
        #[cfg(device_profile)]
        let themed_track = caller_background
            .or_else(|| theme.as_ref().and_then(|resolved| resolved.background_color))
            .map(|background| {
                // One palette read for the pair; this used to take the global theme lock twice
                // for every switch, every frame.
                let palette = crate::style::palette_shared();
                let window = palette.as_ref().map(|colors| colors.background);
                if window == Some(background) {
                    let ink =
                        palette.as_ref().map(|colors| colors.foreground).unwrap_or(Color::BLACK);
                    background.blend(&ink, 0.14)
                } else {
                    background
                }
            });
        #[cfg(not(device_profile))]
        let themed_track = caller_background;

        let off_track =
            caller_background.or(themed_track).unwrap_or(Color::rgba(180, 180, 180, 200));
        // The ON end resolves **caller-named colour first**, then the theme's declared
        // `switch:checked` fill, then the semantic accent. The literal is iOS's own on-tint, which
        // is also what a stripped build falls back to.
        let on_track = self
            .on_track
            .or(declared_checked)
            .or(themed_accent)
            .unwrap_or(Color::rgba(52, 199, 89, 200)); // iOS green
                                                       // The track's colour is a function of the *travel*, not of two discrete states.
                                                       // Blending the two endpoint colours by `travel.value()` is what makes the track
                                                       // change colour on the same frame as the thumb moves: a track that switched colour
                                                       // on `checked` while the thumb was still crossing would read as two separate
                                                       // actions, which is exactly the bug this control's `travel` exists to remove.
        let travel = if is_enabled {
            self.travel.value()
        } else {
            // A disabled switch shows its *logical* state at rest; there is no motion to
            // follow, and leaving it at 0 would draw a grey track for a switch that is on.
            if self.checked {
                1.0
            } else {
                0.0
            }
        };
        let track_color = if !is_enabled {
            off_track.blend(&Color::rgba(200, 200, 200, 160), 1.0)
        } else {
            off_track.blend(&on_track, travel)
        };
        context.fill_rounded_rect(track_rect, track_height / 2, track_color);

        // Draw thumb
        //
        // The thumb's x is derived from `travel.value()` and **not** from `checked`.
        // Reading `checked` here is precisely the defect the transition exists to fix: the
        // thumb would jump to the far end on the same frame the logical state changed, so
        // the animation could never be seen and a "travelling" toggle was really a
        // two-state image. `travel` is the crate's `visualPosition`.
        let thumb_rect = match self.thumb_rect(rect) {
            Some(thumb) => thumb,
            None => return,
        };
        // The corner radius follows the thumb's own width, so a track too small for the
        // nominal disc still draws a disc rather than a square: the size is read from the
        // rectangle just derived, not recomputed here where it could disagree.
        let thumb_radius = thumb_rect.width / 2;

        let thumb_color = if !is_enabled {
            Color::rgba(240, 240, 240, 200)
        } else if let Some(explicit) = self.thumb {
            // A named thumb colour short-circuits the theme derivation.
            explicit
        } else {
            // The thumb reads the theme's foreground (a light thumb on a dark
            // surface, a dark-ish thumb never — the thumb is always the lighter of
            // the pair), falling back to white only when no theme is active.
            theme
                .as_ref()
                .and_then(|resolved| resolved.text_color)
                .map(|text| {
                    // Keep the thumb light: mix most of the way to white so it stays
                    // the highlight against the coloured track.
                    text.blend(&Color::WHITE, 0.85)
                })
                .unwrap_or(Color::WHITE)
        };
        // A hovered switch answers the pointer before it is pressed, so the thumb steps
        // toward the ink: without it a switch that is under the cursor looks exactly like
        // one that is not, which is the affordance the control was missing.
        let thumb_color = if is_enabled && self.base.is_hovered() {
            thumb_color.blend(&track_color.contrast_color(), 0.10)
        } else {
            thumb_color
        };
        context.fill_rounded_rect(thumb_rect, thumb_radius, thumb_color);

        // Draw thumb shadow/border
        let thumb_border_color = style.border_color.unwrap_or(Color::rgba(0, 0, 0, 30));
        context.draw_rounded_rect_stroke(thumb_rect, thumb_radius, thumb_border_color, 1);

        // ── Focus ring ──
        //
        // Drawn strictly inside the control's rectangle (see `ControlMetrics::focus_ring_rect`)
        // and only when the *reason* focus arrived warrants it: a pointer press focuses without
        // drawing a ring, Tab and Shortcut draw one. The reason is recorded by [`BaseWidget`] for
        // every control, which is why the answer cannot differ between controls.
        //
        // The macOS metrics profile draws AppKit's outer halo instead; all other profiles keep
        // the inset stroke and its track-contrast colour.
        if self.visual_focus() {
            if active_profile() == ControlMetricsProfile::Macos {
                let halo = FocusRing::halo_for_control(rect, shape.thumb_radius);
                if halo.is_drawable() {
                    context.draw_box_shadow(
                        halo.rect,
                        focus_ring_color(track_color.contrast_color()),
                        0,
                        0,
                        FOCUS_HALO_BLUR_RADIUS,
                        1,
                    );
                }
            } else {
                let ring = FocusRing::for_control(rect, shape.thumb_radius);
                if ring.is_drawable() {
                    // The ring's colour is the contrast of the track it sits beside, so it reads
                    // on both the on and the off state rather than on one of them.
                    context.draw_rounded_rect_stroke(
                        ring.rect,
                        ring.radius,
                        focus_ring_color(track_color.contrast_color()),
                        FOCUS_RING_WIDTH,
                    );
                }
            }
        }
    }
}

impl EventHandler for Switch {
    /// Toggles on a completed activation, not on a bare release.
    ///
    /// # Why the press arm is load-bearing
    ///
    /// This handler used to be `Event::MouseRelease => self.toggle()` with the position
    /// discarded and no press tracking. Any release the host routed here flipped the
    /// switch — including a drag that began outside it and merely ended on top, and
    /// (since the control never takes pointer capture) any release a host delivered
    /// without a preceding press inside the control.
    ///
    /// [`Button`](crate::widget::base_widgets::button) and
    /// [`CheckBox`](crate::widget::base_widgets::checkbox) both arm on
    /// `MousePress`, which is the pattern the rest of the crate's binary controls
    /// follow; a switch is the same interaction and now behaves identically. Touch,
    /// `Tap` and the space bar are accepted for the same reason they are there:
    /// the control is otherwise unreachable from a touch host or a keyboard.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        let enabled = self.base.is_enabled();
        match event {
            Event::MousePress { pos, button: 1, .. } if enabled => {
                // Arm only for a press that actually lands on the control; a press
                // outside must not leave the latch armed for a later release.
                let pressed = self.base.contains_point_with_touch_expansion(*pos);
                if pressed != self.pressed {
                    self.pressed = pressed;
                    self.request_frame_repaint(None);
                }
                // A press that landed on the **thumb** may become a horizontal drag. A press
                // anywhere else keeps the historical click-to-toggle gesture.
                self.drag = if pressed {
                    match self.thumb_rect(self.geometry()) {
                        Some(thumb) if thumb.contains(*pos) => {
                            Some(ThumbDrag { origin_x: pos.x, moved: false })
                        }
                        _ => None,
                    }
                } else {
                    None
                };
            }
            Event::MouseMove { pos } if self.drag.is_some() => {
                let rect = self.geometry();
                if let Some(drag) = self.drag.as_mut() {
                    if (pos.x - drag.origin_x).abs() >= SWITCH_DRAG_THRESHOLD {
                        drag.moved = true;
                    }
                }
                // The thumb follows the pointer through the **same travel driver** the
                // animation uses. Jumping — rather than aiming and waiting for a tick — is
                // what makes it sit under the finger instead of trailing it.
                let travel = self.travel_for_x(rect, pos.x);
                self.travel.jump_to(travel);
                self.request_frame_repaint(None);
            }
            Event::MouseRelease { pos, button: 1 } if self.pressed => {
                self.pressed = false;
                let drag = self.drag.take();
                match drag {
                    // A real drag commits the side the thumb was released on: crossing the
                    // track's midpoint is what turns the switch on (or back off).
                    Some(drag) if drag.moved => {
                        self.set_checked(self.travel.value() >= 0.5);
                    }
                    // A press that never moved is still a click, which toggles.
                    _ => {
                        if self.base.contains_point_with_touch_expansion(*pos) {
                            self.toggle();
                        }
                    }
                }
                self.request_frame_repaint(None);
            }
            Event::MouseRelease { button: 1, .. } => {
                self.pressed = false;
                self.drag = None;
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if enabled => {
                let pressed = self.base.contains_point_with_touch_expansion(*pos);
                if pressed != self.pressed {
                    self.pressed = pressed;
                    self.request_frame_repaint(None);
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { pos, .. } if self.pressed => {
                self.pressed = false;
                if self.base.contains_point_with_touch_expansion(*pos) {
                    self.toggle();
                }
                self.request_frame_repaint(None);
            }
            // D09-EVT-02: a withdrawn contact clears the latch without toggling. The
            // platform may normalise a `TouchCancel` to a `TouchEnd`; the internal cancel
            // event reroutes it so a cancelled gesture never flips the switch.
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.pressed = false;
                self.request_frame_repaint(None);
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } if enabled => {
                self.toggle();
            }
            Event::KeyPress { key, .. } if *key == key_codes::SPACE && enabled => {
                self.toggle();
            }
            // Focus entry carries the *reason*, and the reason is what decides whether a
            // focus ring is painted; storing only a bool would make a click and a Tab
            // indistinguishable and the ring would appear on click, which reads as a stuck
            // highlight under the cursor. The base records it for every control.
            Event::FocusGained { .. } => {
                self.base.request_redraw();
            }
            // Focus loss abandons a held press, so the latch cannot survive a
            // window switch and fire on an unrelated later release — and it also drops
            // the ring, because the control is no longer the keyboard's target.
            Event::FocusLost => {
                self.pressed = false;
                self.drag = None;
                self.base.request_redraw();
            }
            Event::MouseEnter { .. } => {
                self.set_hovered(true);
                // The base records hover before this arm runs, so the setter above is a
                // no-op for a pointer move. Submit the hover frame here explicitly.
                self.request_frame_repaint(None);
            }
            Event::MouseLeave { .. } => {
                // A release delivered outside the control never reaches this widget, so a
                // departed pointer abandons a drag rather than leaving the thumb stranded.
                if self.drag.is_some() {
                    self.drag = None;
                    self.pressed = false;
                    self.travel.jump_to(self.travel_target());
                }
                self.set_hovered(false);
                self.request_frame_repaint(None);
            }
            _ => {}
        }
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::metrics::dimensions;
    use std::sync::{Arc, Mutex};

    #[test]
    fn switch_default_is_unchecked() {
        let sw = Switch::new(Rect::new(0, 0, 60, 30));
        assert!(!sw.is_checked());
        assert_eq!(sw.kind(), WidgetKind::Switch);
    }

    #[test]
    fn switch_set_checked_emits_signal() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let captured = Arc::new(Mutex::new(None));
        sw.toggled.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<bool>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        sw.set_checked(true);
        assert!(sw.is_checked());
        assert_eq!(*captured.lock().unwrap(), Some(true));
    }

    #[test]
    fn switch_toggle_flips_state() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        assert!(!sw.is_checked());
        sw.toggle();
        assert!(sw.is_checked());
        sw.toggle();
        assert!(!sw.is_checked());
    }

    /// A-2: the theme's declared `switch:checked` fill reaches the ON track.
    ///
    /// # The defect this pins
    ///
    /// `preset_states.rs` declares `switch:checked { background: primary, … }`, and
    /// `resolve_style_for_state` resolved it — then the resolved value was dropped
    /// (`caller_background = if theme_derived { None }`), so a theme that changed the key saw
    /// nothing move. The `Success` accent is a *separate* fact ("this switch is on"), and the
    /// declared fill is the theme's opinion of that state; both must be reachable, the declared
    /// one winning.
    ///
    /// # Why the assertion is on the rendered SVG
    ///
    /// Reading `resolved_theme_style_for_state` back would assert the resolver, not the control.
    /// The test registers a theme whose `switch:checked` fill is a distinctive colour, paints an
    /// ON switch, and requires that colour to appear in the emitted picture — the fill a user
    /// would see. Under the old drop, the track was the `Success` accent and the custom colour
    /// never reached a pixel.
    #[test]
    fn the_declared_checked_fill_reaches_the_on_track() {
        let _guard = crate::theme::theme_test_guard();
        let bounds = Rect::new(0, 0, 60, 30);

        // A switch at rest in the ON state: the travel has settled, so the track is the ON fill
        // and the paint is deterministic.
        let mut sw = Switch::new(bounds);
        sw.set_checked(true);
        while sw.is_animating() {
            let _ = Widget::tick(&mut sw, 100);
        }
        assert_eq!(sw.widget_state(), crate::style::WidgetState::Checked);

        let marker = Color::rgba(1, 2, 3, 255);
        {
            let mut manager = crate::theme::global_theme_manager();
            assert!(manager.set_theme("default"));
            let mut theme = crate::theme::Theme::default();
            theme.overrides.styles.insert(
                "switch:checked".to_string(),
                crate::theme::ThemeStyleToken {
                    background: Some(marker),
                    ..crate::theme::ThemeStyleToken::default()
                },
            );
            manager.register_theme(theme);
            assert!(manager.set_theme("default"));
        }

        let svg = crate::widget::svg::render_widget_to_svg(&mut sw, bounds);
        let expected =
            format!("rgba({},{},{},{:.2})", marker.r, marker.g, marker.b, marker.a as f32 / 255.0);
        assert!(
            svg.contains(&expected),
            "the theme's declared `switch:checked` fill {expected} must reach the ON track, but \
             the rendered SVG does not contain it"
        );
    }

    /// A completed pointer activation toggles the switch.
    ///
    /// The test used to dispatch a bare `MouseRelease`, which meant it asserted the
    /// *defect*: the release position was discarded and no press was required, so any
    /// release the host routed here flipped the control. The intent — "a pointer
    /// activation toggles" — is unchanged; only the interaction it performs is now
    /// the real one. The negative cases live in
    /// `switch_release_without_press_does_not_toggle` and
    /// `switch_press_outside_then_release_inside_does_not_toggle`.
    #[test]
    fn switch_mouse_press_toggles() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let p = Point::new(10, 10);
        sw.handle_event(&Event::MousePress { pos: p, button: 1, modifiers: 0 });
        sw.handle_event(&Event::MouseRelease { pos: p, button: 1 });
        assert!(sw.is_checked());
    }

    /// A release with no qualifying press must not toggle.
    ///
    /// Reproduces the reported defect: the old handler toggled on *any* release, so a
    /// bare `MouseRelease` — including one at a position far outside the geometry —
    /// flipped the switch.
    #[test]
    fn switch_release_without_press_does_not_toggle() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked(), "a release with no press must not toggle");
        // The original reproduction: a release nowhere near the control.
        sw.handle_event(&Event::MouseRelease { pos: Point::new(5000, 5000), button: 1 });
        assert!(!sw.is_checked(), "a release far outside the geometry must not toggle");
    }

    /// A press outside the control must not arm the latch for a later release.
    #[test]
    fn switch_press_outside_then_release_inside_does_not_toggle() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MousePress {
            pos: Point::new(5000, 5000),
            button: 1,
            modifiers: 0,
        });
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked(), "a drag that began outside must not commit");
    }

    /// Pressing inside and releasing outside cancels, as it does for `Button`.
    #[test]
    fn switch_press_inside_then_release_outside_cancels() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        sw.handle_event(&Event::MouseRelease { pos: Point::new(500, 500), button: 1 });
        assert!(!sw.is_checked());
        // The latch must also be clear, so the *next* stray release does not fire.
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked(), "the cancelled press must not stay armed");
    }

    /// Losing focus abandons a held press.
    #[test]
    fn switch_focus_loss_cancels_a_held_press() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(sw.is_pressed());
        sw.handle_event(&Event::FocusLost);
        assert!(!sw.is_pressed());
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked());
    }

    /// The space bar toggles, matching `CheckBox`.
    #[test]
    fn switch_space_bar_toggles() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert!(sw.is_checked());
    }

    /// D09-EVT-02: a touch cancel clears the latch without toggling.
    ///
    /// A `TouchCancel` normalised to a `TouchEnd` used to commit the toggle. The internal
    /// cancel event must clear the pressed latch and leave the state unchanged.
    #[cfg(feature = "touch")]
    #[test]
    fn switch_touch_cancel_does_not_toggle() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let p = Point::new(10, 10);
        // Arm via a touch begin inside the control.
        sw.handle_event(&Event::TouchBegin { pos: p, touch_id: 1 });
        assert!(!sw.is_checked());

        let cancel = crate::event::translator::touch_cancel(p, 1);
        sw.handle_event(&cancel);
        assert!(!sw.is_checked(), "a cancelled touch must not toggle the switch");

        // The latch is clear: a following end must not commit either.
        sw.handle_event(&Event::TouchEnd { pos: p, touch_id: 1 });
        assert!(!sw.is_checked(), "a cancelled press must not stay armed");
    }

    /// D09-EVT-02: a normal touch end still toggles — the cancel path does not regress it.
    #[cfg(feature = "touch")]
    #[test]
    fn switch_normal_touch_end_still_toggles() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let p = Point::new(10, 10);
        sw.handle_event(&Event::TouchBegin { pos: p, touch_id: 1 });
        sw.handle_event(&Event::TouchEnd { pos: p, touch_id: 1 });
        assert!(sw.is_checked(), "a completed touch still toggles");
    }

    #[test]
    fn switch_disabled_blocks_events() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.set_enabled(false);
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(!sw.is_checked());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn switch_svg_output() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let svg = crate::widget::svg::render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
    }

    /// BLUE23 §3.3, judgement 3 — the thumb's geometry is distinct across frames and moves
    /// monotonically. This is the plan's shared "three-frame" criterion: an animation cannot
    /// be proved with one frame, only with the *difference* between frames.
    #[test]
    fn the_travel_takes_the_thumb_across_distinct_positions() {
        let rect = crate::widget::census::CENSUS_RECT;
        let mut sw = Switch::new(rect);
        let at_rest = sw.thumb_rect(rect).expect("censused switch has a thumb").x;

        sw.set_checked(true);
        // Sample before any tick, then after two even steps of the travel.
        let before = sw.thumb_rect(rect).expect("thumb").x;
        assert_eq!(before, at_rest, "a state change alone must not teleport the thumb");

        assert!(sw.tick(80), "the travel is still running after one step");
        let mid = sw.thumb_rect(rect).expect("thumb").x;

        // Drain the rest of the travel so the end position is sampled at rest.
        let mut guard = 0;
        while sw.tick(80) {
            guard += 1;
            assert!(guard < 100, "the travel must settle");
        }
        let end = sw.thumb_rect(rect).expect("thumb").x;

        assert_ne!(mid, before, "the first frame must move the thumb");
        assert_ne!(end, mid, "the last frame must reach a new position");
        assert!(
            before < mid && mid < end,
            "the thumb must advance monotonically: {before}/{mid}/{end}"
        );
    }

    /// The logical state and the drawn state are **two different facts**.
    ///
    /// `checked` is what a caller reads after a click and must change the instant the
    /// click lands; the thumb's position is a presentation that lags behind it. Drawing
    /// the thumb from `checked` is the defect this pins against: the thumb jumped to the
    /// far end on the same frame as the state change, so the travel could never be seen
    /// and the "animation" was really a two-state image.
    #[test]
    fn the_logical_state_changes_before_the_thumb_arrives() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        assert_eq!(sw.travel_progress(), 0.0, "a fresh switch is at the off end");

        sw.toggle();

        // The logical answer is immediate…
        assert!(sw.is_checked(), "the caller must see the new state at once");
        // …while the thumb has not moved yet, because no frame has been delivered.
        assert_eq!(
            sw.travel_progress(),
            0.0,
            "the thumb must not teleport to the far end on the frame the state changed"
        );
    }

    /// Ticking moves the thumb toward the logical state and then **settles**.
    ///
    /// The boolean return is the contract a host schedules frames from: `true` while
    /// there is movement, `false` once there is none. A `tick` that always reported work
    /// would keep the whole application repainting forever, so the settle case is half of
    /// what this pins. A single long frame must arrive exactly, not approach forever.
    #[test]
    fn ticking_moves_the_thumb_toward_the_logical_state_and_settles() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        sw.toggle();

        // A short frame leaves the travel in flight and reports more work owed.
        assert!(sw.tick(30), "a toggle must start a transition");
        let partway = sw.travel_progress();
        assert!(partway > 0.0 && partway < 1.0, "the thumb is mid-travel: {partway}");

        // Ticking well past the theme's slow tempo lands it exactly…
        while sw.tick(1000) {}
        assert_eq!(sw.travel_progress(), 1.0, "the travel reaches its target exactly");
        // …and then reports that there is nothing left to do.
        assert!(!sw.tick(1000), "a settled switch owes no more frames");

        // The reverse journey settles the same way.
        sw.toggle();
        while sw.tick(1000) {}
        assert_eq!(sw.travel_progress(), 0.0);
        assert!(!sw.tick(1000));
    }

    /// A freshly built switch must be at the **off end** of its travel, not the on end.
    ///
    /// A transition that started at the interactive end of its range would make every new
    /// switch fade *out* on its first frame — and, since the OFF track is drawn from the
    /// travel, it would flash the ON colour before settling grey.
    #[test]
    fn a_freshly_built_switch_is_at_the_off_end_of_its_travel() {
        let sw = Switch::new(crate::widget::census::CENSUS_RECT);
        assert!(!sw.is_checked());
        assert_eq!(sw.travel_progress(), 0.0);
    }

    /// A reversal mid-flight **re-aims from where the thumb is**, it does not restart.
    ///
    /// The target is recomputed from the control's own state on every tick, so toggling
    /// back while the thumb is still crossing turns it around from its current position.
    /// Re-aiming is what makes a quick on-off read as one movement; restarting from the
    /// far end would make the same gesture read as two fades.
    #[test]
    fn a_transition_in_flight_re_aims_rather_than_restarting() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        sw.toggle();
        sw.tick(50);
        let forward = sw.travel_progress();
        assert!(forward > 0.0 && forward < 1.0, "the toggle is in flight: {forward}");

        // Reverse while the thumb is still travelling toward the on end.
        sw.toggle();
        assert!(!sw.is_checked());
        // The reverse journey must start from where the thumb is, so the first frame of it
        // is still *above* zero rather than having snapped back to the off end.
        sw.tick(1);
        let reversing = sw.travel_progress();
        assert!(
            reversing < forward,
            "the thumb turns around instead of continuing: {reversing} < {forward}"
        );
        assert!(reversing > 0.0, "the reversal starts from the current position: {reversing}");

        // And it settles back at the off end rather than oscillating.
        while sw.tick(1000) {}
        assert_eq!(sw.travel_progress(), 0.0);
    }

    /// Visual focus depends on the **reason** focus arrived, not merely on having it.
    ///
    /// A pointer press focuses the control — it is the keyboard's target from then on —
    /// but drawing a ring under the cursor reads as a stuck highlight, so only the
    /// keyboard reasons draw one. This is the standard rule for visual focus, and the
    /// reason the control stores a
    /// `FocusReason` rather than a bare bool.
    #[test]
    fn switch_visual_focus_depends_on_the_reason() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);

        sw.handle_event(&Event::FocusGained { reason: FocusReason::Pointer });
        assert!(sw.is_focused(), "the control is the keyboard target");
        assert!(!sw.visual_focus(), "but the user is on the pointer, so no ring is drawn");

        for reason in [FocusReason::Tab, FocusReason::BackTab, FocusReason::Shortcut] {
            let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
            sw.handle_event(&Event::FocusGained { reason });
            assert!(sw.is_focused());
            assert!(sw.visual_focus(), "{reason:?} must draw the ring");
        }

        sw.handle_event(&Event::FocusLost);
        assert!(!sw.is_focused());
        assert!(!sw.visual_focus());
    }

    /// Hover is tracked from the pointer entering and leaving the control.
    ///
    /// The control had no hover state at all, so a switch under the cursor looked
    /// identical to one that was not — the affordance was simply missing. Both directions
    /// are pinned: entering sets the flag and leaving clears it.
    #[test]
    fn switch_tracks_hover() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        assert!(!sw.is_hovered(), "a fresh switch is not hovered");

        sw.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(sw.is_hovered());

        sw.handle_event(&Event::MouseLeave { pos: Point::new(10, 10) });
        assert!(!sw.is_hovered());

        // The host-facing setter is the same fact, for a backend with no hover event.
        sw.set_hovered(true);
        assert!(sw.is_hovered());
    }

    /// An on switch reports `Checked`, so the preset's `switch:checked` key is reachable.
    ///
    /// The same defect `CheckBox::widget_state` documents: the trait default knows the four
    /// primitive flags and none of them means "latched", so the key resolved to `Normal` and the
    /// accent fill the preset asks for was never painted. A switch is the most visible case — its
    /// whole meaning is the position it reports.
    #[test]
    fn widget_state_reports_checked_for_an_on_switch() {
        use crate::style::WidgetState;
        let mut sw = Switch::new(crate::core::Rect::new(0, 0, 60, 30));
        assert_eq!(sw.widget_state(), WidgetState::Normal);

        sw.set_checked(true);
        assert_eq!(sw.widget_state(), WidgetState::Checked);

        sw.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert_eq!(sw.widget_state(), WidgetState::Checked, "the latch outranks a hover");

        sw.set_enabled(false);
        assert_eq!(sw.widget_state(), WidgetState::Disabled, "disabled outranks the latch");
    }

    /// The Material profile keeps the switch's `52x32` track and the hint derived from it.
    ///
    /// An explicitly chosen shape — the iOS preset — is a caller decision and must survive the
    /// Material profile unchanged, so that is asserted in the same test.
    #[test]
    fn the_material_profile_keeps_the_switch_track() {
        let _theme_guard = crate::style::theme_test_guard();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        let rect = crate::widget::census::CENSUS_RECT;
        let sw = Switch::new(rect);
        let hint = sw.size_hint();
        let expected =
            crate::widget::metrics::switch_hint_size(dimensions::SWITCH_TRACK, FOCUS_RING_WIDTH);
        assert_eq!(hint, expected, "the old track and its per-side ring room");
        assert_eq!(hint.height, dimensions::SWITCH_TRACK.height);

        let thumb = sw.thumb_rect(rect).expect("a roomy switch has a thumb");
        assert!(thumb.height <= dimensions::SWITCH_TRACK.height, "the thumb fits the track");

        let mut ios = Switch::new(rect);
        ios.set_geometry(SwitchShape::CUPERTINO);
        assert_eq!(
            ios.size_hint().height,
            dimensions::CUPERTINO_SWITCH_TRACK.height,
            "an explicitly chosen preset is not resized by the Material profile"
        );
    }

    /// A Macos-profile theme draws macOS's `38x22` track instead of Material's `52x32`.
    ///
    /// The hint and the thumb are read through the same effective geometry, so the reported
    /// floor and the drawn chrome cannot disagree about which track is in use.
    #[cfg(device_profile)]
    #[test]
    fn a_macos_profile_theme_uses_the_compact_switch_track() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut theme = crate::theme::Theme::default();
        theme.name = "p2-6-switch-macos".to_string();
        theme.metrics_profile = ControlMetricsProfile::Macos;
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            assert!(manager.set_theme("p2-6-switch-macos"));
        }

        let rect = crate::widget::census::CENSUS_RECT;
        let sw = Switch::new(rect);
        let hint = sw.size_hint();
        let thumb = sw.thumb_rect(rect).expect("the compact track still holds a thumb");
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        assert_eq!(hint.height, 22, "NSSwitch is 22 px tall: {hint:?}");
        assert_eq!(hint.width, 38 + 2 * FOCUS_RING_WIDTH, "38 px plus the ring room: {hint:?}");
        assert!(thumb.height <= 22, "the thumb is clamped to the compact track: {thumb:?}");
        assert_eq!(
            sw.drawn_shape(),
            SwitchShape::MATERIAL,
            "the profile changes the drawn track, not the preset the caller stored"
        );
    }

    /// The Cupertino profile carries the iOS track, matching `SwitchGeometry::CUPERTINO`.
    #[cfg(device_profile)]
    #[test]
    fn the_cupertino_profile_uses_the_ios_switch_track() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut theme = crate::theme::Theme::default();
        theme.name = "p2-6-switch-cupertino".to_string();
        theme.metrics_profile = ControlMetricsProfile::Cupertino;
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            assert!(manager.set_theme("p2-6-switch-cupertino"));
        }

        let rect = crate::widget::census::CENSUS_RECT;
        let hint = Switch::new(rect).size_hint();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        assert_eq!(hint.height, dimensions::CUPERTINO_SWITCH_TRACK.height);
        assert_eq!(hint.width, dimensions::CUPERTINO_SWITCH_TRACK.width + 2 * FOCUS_RING_WIDTH);
    }

    /// A hover frame submits damage bounded by the painted box and the margin.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_hover_frame_submits_the_painted_box_with_the_damage_margin() {
        use crate::widget::runtime;

        let id = runtime::register(crate::compat::Box::new(Switch::new(Rect::new(4, 8, 52, 32))))
            .expect("registry");
        assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Dirty));

        assert!(runtime::dispatch_event(id, &Event::mouse_enter(1, 1)));

        let expected = runtime::painted_box_with_damage_margin(id).expect("painted box");
        assert_eq!(
            runtime::dirty_rects(id),
            vec![expected],
            "a hover must damage the painted box plus the margin"
        );
        runtime::unregister(id);
    }

    /// A preset names one of the three shipped shapes and `MacOS` changes the proportions.
    #[test]
    fn switch_geometry_presets_select_their_own_proportions() {
        let rect = crate::widget::census::CENSUS_RECT;
        let mut sw = Switch::new(rect);
        assert_eq!(sw.switch_geometry(), SwitchGeometry::Material, "Material is the default");
        assert_eq!(SwitchGeometry::Material.token(), "material");
        assert_eq!(SwitchGeometry::Cupertino.token(), "cupertino");
        assert_eq!(SwitchGeometry::MacOS.token(), "macos");
        assert_eq!(SwitchGeometry::from_token("macos"), Some(SwitchGeometry::MacOS));
        assert_eq!(SwitchGeometry::from_token("unknown"), None);

        sw.set_switch_geometry(SwitchGeometry::MacOS);
        assert_eq!(sw.switch_geometry(), SwitchGeometry::MacOS);
        assert_eq!(sw.drawn_shape().track, Size::new(40, 22), "macOS is 40x22 pt");
        assert_eq!(sw.size_hint().height, 22);
        let thumb = sw.thumb_rect(rect).expect("the compact track holds a thumb");
        assert!(thumb.height <= 22, "the thumb is clamped to the macOS track: {thumb:?}");

        sw.set_switch_geometry(SwitchGeometry::Cupertino);
        assert_eq!(sw.switch_geometry(), SwitchGeometry::Cupertino);
        assert_eq!(sw.drawn_shape(), SwitchShape::CUPERTINO);

        // Setting the same preset again is a no-op.
        sw.set_switch_geometry(SwitchGeometry::Cupertino);
        assert_eq!(sw.switch_geometry(), SwitchGeometry::Cupertino);

        // The exact-shape setter accepts a named preset too, so both spellings select the
        // same proportions.
        sw.set_geometry(SwitchGeometry::MacOS);
        assert_eq!(sw.switch_geometry(), SwitchGeometry::MacOS);
        assert_eq!(sw.drawn_shape().track, Size::new(40, 22));
    }

    /// Dragging the thumb past the midpoint commits the state the thumb was released on.
    ///
    /// The press arms a drag only when it lands on the thumb; the thumb follows the pointer
    /// through the travel driver, and a release without movement still toggles.
    #[test]
    fn dragging_the_thumb_past_the_midpoint_toggles_on_release() {
        let rect = crate::widget::census::CENSUS_RECT;
        let mut sw = Switch::new(rect);
        let thumb = sw.thumb_rect(rect).expect("a roomy switch has a thumb");
        let grab = Point::new(thumb.x + thumb.width as i32 / 2, thumb.y + thumb.height as i32 / 2);

        sw.handle_event(&Event::MousePress { pos: grab, button: 1, modifiers: 0 });
        assert!(!sw.is_checked(), "a press alone does not commit");

        // Drag to the far right, past the midpoint, and release.
        let shape = sw.drawn_shape();
        let track = ControlMetrics::center_in(rect, shape.track);
        let far_right = track.x + track.width as i32 - shape.thumb_inset as i32;
        sw.handle_event(&Event::MouseMove { pos: Point::new(far_right, grab.y) });
        assert!(sw.travel_progress() > 0.5, "the thumb follows the pointer across");
        sw.handle_event(&Event::MouseRelease { pos: Point::new(far_right, grab.y), button: 1 });
        assert!(sw.is_checked(), "releasing past the midpoint turns the switch on");

        // And the reverse journey turns it back off.
        let thumb = sw.thumb_rect(rect).expect("thumb");
        let grab = Point::new(thumb.x + thumb.width as i32 / 2, thumb.y + thumb.height as i32 / 2);
        sw.handle_event(&Event::MousePress { pos: grab, button: 1, modifiers: 0 });
        let far_left = track.x + shape.thumb_inset as i32;
        sw.handle_event(&Event::MouseMove { pos: Point::new(far_left, grab.y) });
        sw.handle_event(&Event::MouseRelease { pos: Point::new(far_left, grab.y), button: 1 });
        assert!(!sw.is_checked(), "releasing past the midpoint turns it back off");
    }

    /// A press that does not move is still a click, even though the press landed on the thumb.
    #[test]
    fn pressing_the_thumb_without_dragging_still_toggles() {
        let rect = crate::widget::census::CENSUS_RECT;
        let mut sw = Switch::new(rect);
        let thumb = sw.thumb_rect(rect).expect("thumb");
        let at = Point::new(thumb.x + thumb.width as i32 / 2, thumb.y + thumb.height as i32 / 2);

        sw.handle_event(&Event::MousePress { pos: at, button: 1, modifiers: 0 });
        sw.handle_event(&Event::MouseRelease { pos: at, button: 1 });
        assert!(sw.is_checked(), "a click on the thumb keeps the press-toggles gesture");
    }

    /// The thumb colour has its own accessor and property route; `None` keeps the theme mix.
    #[test]
    fn the_thumb_colour_can_be_named_and_cleared() {
        let mut sw = Switch::new(Rect::new(0, 0, 52, 32));
        assert_eq!(sw.thumb_color(), None);
        sw.set_thumb_color(Some(Color::rgb(10, 20, 30)));
        assert_eq!(sw.thumb_color(), Some(Color::rgb(10, 20, 30)));
        assert_eq!(sw.get("thumb_color").unwrap(), CapabilityValue::Color(Color::rgb(10, 20, 30)));

        sw.set("thumb_color", CapabilityValue::Null).unwrap();
        assert_eq!(sw.thumb_color(), None, "Null restores the theme-derived thumb");
        assert!(sw.set("thumb_color", CapabilityValue::Bool(true)).is_err());
    }
}
