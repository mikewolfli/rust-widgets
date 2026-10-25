// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Slider widget.
use core::fmt::Write as _;

use crate::compat::{Arc, String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Orientation, Point, Rect, Size};
use crate::event::{wheel::WheelScroll, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{
    expect_bool, expect_i32_saturating, expect_orientation, orientation_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, focus_ring_color, ControlMetrics, FocusRing, FOCUS_RING_WIDTH,
};
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Which direction a slider ↔ spin box link carries values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkMode {
    /// The slider drives the spin box. A spin-box edit stays local to the spin box, so a display
    /// field cannot silently move the control the user is dragging.
    #[default]
    OneWay,
    /// Either control drives the other. A value arriving from the spin box is rounded to the
    /// nearest integer on the slider side; the conversion rule is stated on
    /// [`Slider::bind_to_spin_box`].
    TwoWay,
}

/// The shared slot a spin box publishes its current link through.
///
/// `Rc` rather than `Arc`: the two widgets and the link all live on one GUI thread, and the
/// slot only exists so [`Slider::unbind`] can clear the peer side from a `&SpinBox` reference.
pub(crate) type NumericLinkSlot =
    crate::compat::Rc<crate::compat::RefCell<Option<Arc<NumericLink>>>>;

/// Converts an `f64` bound to the nearest representable `i32`, saturating rather than wrapping.
fn clamp_f64_to_i32(value: f64) -> i32 {
    if value.is_nan() {
        0
    } else if value >= i32::MAX as f64 {
        i32::MAX
    } else if value <= i32::MIN as f64 {
        i32::MIN
    } else {
        value as i32
    }
}

/// The mutable half of a link, under one lock so a read of `(mode, value)` sees one snapshot.
#[derive(Clone, Copy)]
struct NumericLinkState {
    mode: LinkMode,
    decimals: u32,
    spin_value: f64,
    spin_minimum: f64,
    spin_maximum: f64,
    slider_value: i32,
    /// Set while one side is propagating, so the peer's signal cannot start a second pass.
    syncing: bool,
    /// Cleared by [`Slider::unbind`]; a detached link is read for its last value but carries
    /// nothing in either direction.
    attached: bool,
}

/// The shared value a [`Slider`] and a spin box read when they are linked.
///
/// # Why the link owns the value rather than one widget pushing into the other
///
/// A `Signal` callback is `Send + Sync + 'static` and cannot borrow either control, so a push
/// model would need one of the two to poll a mailbox. Putting the value in a shared cell makes
/// both readers (`Slider::value`, spin-box `value`) and both writers agree by construction. The
/// signals are still emitted in both directions, so a host listening to either control hears a
/// change made on the other side.
pub(crate) struct NumericLink {
    state: crate::compat::Mutex<NumericLinkState>,
    /// A clone of the spin box's `value_changed`, so a slider-driven change reaches its
    /// subscribers.
    spin_changed: Signal1<i32>,
    /// A clone of the slider's `value_changed`, so a spin-box-driven change reaches its
    /// subscribers.
    slider_changed: Signal1<i32>,
    /// Disconnects the spin-box listener when the last link reference drops.
    scope: ConnectionScope,
}

impl NumericLink {
    fn new(
        mode: LinkMode,
        decimals: u32,
        spin_value: f64,
        spin_minimum: f64,
        spin_maximum: f64,
        slider_value: i32,
        spin_changed: Signal1<i32>,
        slider_changed: Signal1<i32>,
    ) -> Self {
        Self {
            state: crate::compat::Mutex::new(NumericLinkState {
                mode,
                decimals,
                spin_value,
                spin_minimum,
                spin_maximum,
                slider_value,
                syncing: false,
                attached: true,
            }),
            spin_changed,
            slider_changed,
            scope: ConnectionScope::new(),
        }
    }

    fn snapshot(&self) -> NumericLinkState {
        *crate::compat::lock(&self.state)
    }

    pub(crate) fn mode(&self) -> LinkMode {
        self.snapshot().mode
    }

    /// Whether values still cross this link in either direction.
    pub(crate) fn is_attached(&self) -> bool {
        self.snapshot().attached
    }

    /// Stops both directions while keeping the last value readable.
    pub(crate) fn detach(&self) {
        crate::compat::lock(&self.state).attached = false;
    }

    pub(crate) fn set_mode(&self, mode: LinkMode) {
        crate::compat::lock(&self.state).mode = mode;
    }

    pub(crate) fn set_decimals(&self, decimals: u32) {
        crate::compat::lock(&self.state).decimals = decimals;
    }

    pub(crate) fn set_bounds(&self, minimum: f64, maximum: f64, decimals: u32) {
        let mut state = crate::compat::lock(&self.state);
        state.spin_minimum = minimum;
        state.spin_maximum = maximum;
        state.decimals = decimals;
    }

    /// The spin box's `(minimum, maximum, decimals)` as of its last bounds write.
    pub(crate) fn bounds(&self) -> (f64, f64, u32) {
        let state = self.snapshot();
        (state.spin_minimum, state.spin_maximum, state.decimals)
    }

    pub(crate) fn spin_value(&self) -> f64 {
        self.snapshot().spin_value
    }

    fn slider_value(&self) -> i32 {
        self.snapshot().slider_value
    }

    /// Stores a spin-box-side change and returns the slider value it maps to.
    ///
    /// The caller (the spin box) emits its own `value_changed`; the listener installed by the
    /// slider then reads this state and, in [`LinkMode::TwoWay`], publishes the slider side.
    pub(crate) fn set_spin_value(&self, value: f64) {
        crate::compat::lock(&self.state).spin_value = value;
    }

    /// The spin box's value, rounded to the nearest integer for the slider's `i32` payload.
    ///
    /// The spin's precision is applied first so the number that crosses is the number the spin
    /// *shows*; the slider then cannot carry a fraction, so the last step is the nearest whole
    /// number. At `decimals == 0` this is the identity.
    fn spin_to_slider(&self) -> i32 {
        let state = self.snapshot();
        let rounded = crate::widget::input_widgets::spinbox::round_to_decimals(
            state.spin_value,
            state.decimals,
        );
        let integer = rounded.round();
        if integer >= i32::MAX as f64 {
            i32::MAX
        } else if integer <= i32::MIN as f64 {
            i32::MIN
        } else {
            integer as i32
        }
    }

    /// Adopts the spin box's value on the slider side without writing back to the spin box.
    ///
    /// This is the "read the peer" half used when the caller switches a link to `TwoWay` or
    /// synchronises the range: pushing the rounded value back through `set_value` would overwrite
    /// the spin box's decimal precision with the whole number the slider can represent.
    pub(crate) fn pull_from_spin(&self) -> i32 {
        let slider = self.spin_to_slider();
        self.set_slider_value(slider);
        slider
    }

    /// Writes the slider side without touching the spin box, emitting its signal when the value
    /// actually moved.
    pub(crate) fn set_slider_value(&self, value: i32) {
        let changed = {
            let mut state = crate::compat::lock(&self.state);
            if state.slider_value == value {
                false
            } else {
                state.slider_value = value;
                true
            }
        };
        if changed {
            self.slider_changed.emit(value);
        }
    }

    /// Applies a slider-driven value and publishes it on the spin box's signal.
    ///
    /// Returns the spin box's integer payload. The `syncing` flag is held across the emit, so the
    /// spin-box listener recognises this as a value it has already seen and does not push back.
    fn slider_drives_spin(&self, slider_value: i32) -> i32 {
        let payload = {
            let mut state = crate::compat::lock(&self.state);
            state.slider_value = slider_value;
            state.spin_value = f64::from(slider_value);
            state.syncing = true;
            state.spin_value as i32
        };
        self.spin_changed.emit(payload);
        crate::compat::lock(&self.state).syncing = false;
        payload
    }

    /// Handles a change published by the spin box's own `value_changed`.
    ///
    /// The value is read from the shared state rather than from the payload, because the payload
    /// is the rounded `i32` and a decimal spin box has already stored the exact `f64`.
    fn on_spin_changed(&self) {
        let state = self.snapshot();
        if state.syncing || !state.attached || state.mode != LinkMode::TwoWay {
            return;
        }
        let slider = self.spin_to_slider();
        {
            let mut state = crate::compat::lock(&self.state);
            if state.syncing {
                return;
            }
            if state.slider_value == slider {
                return;
            }
            state.slider_value = slider;
            state.syncing = true;
        }
        self.slider_changed.emit(slider);
        crate::compat::lock(&self.state).syncing = false;
    }
}
/// Slider widget.
pub struct Slider {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    orientation: Orientation,
    tick_position: TickPosition,
    tick_interval: i32,
    tracking: bool,
    slider_position: i32,
    mouse_pressed: bool,
    /// Whether the part of the groove already passed is highlighted.
    track_highlight: bool,
    /// Whether each tick is labelled with its numeric value.
    tick_labels: bool,
    /// Formatted tick labels and their metrics, rebuilt only when the tick configuration,
    /// the effective face or the device scale changes.
    tick_label_cache: TickLabelCache,
    /// Whether the hover/drag halo interpolates instead of stepping between two levels.
    animated_halo: bool,
    /// The halo's current strength, `0.0` at rest and a blend weight while hovered or dragged.
    ///
    /// Only read when [`Slider::set_animated_halo`] is on: with it off the draw pass keeps
    /// the discrete two-level behaviour, which is what makes the default picture unchanged.
    halo: PropertyDriver,
    /// The writing direction the value axis runs in.
    ///
    /// Defaults to left-to-right, so a slider that never asks for a direction behaves exactly as it
    /// did before the field existed. In a right-to-left interface the minimum belongs at the right
    /// edge, and without this the handle moved the opposite way from the value it reported — the one
    /// functional consequence of missing RTL support in this crate.
    direction: crate::core::TextDirection,
    /// Emitted with the new value on every value change, from user interaction
    /// or from a programmatic setter.
    pub value_changed: Signal1<i32>,
    /// Emitted with the new value when the value changes by dragging the handle,
    /// as opposed to clicking the groove or keyboard stepping. Allows a consumer
    /// to treat continuous manipulation differently (for example, defer expensive
    /// work until release).
    pub slider_moved: Signal1<i32>,
    /// Emitted when the handle is pressed. Carries no payload.
    pub slider_pressed: GenericSignal,
    /// Emitted when the handle is released. Paired with
    /// [`Slider::slider_pressed`], so the widget's own press tracking decides
    /// when it fires rather than the pointer's position.
    pub slider_released: GenericSignal,
    /// The value link to a spin box, while bound.
    link: Option<Arc<NumericLink>>,
    /// The spin box's link slot, kept so [`Slider::unbind`] clears the peer side too.
    link_slot: Option<NumericLinkSlot>,
    /// The reactive model value this control consumes through the property bridge.
    ///
    /// Present only after [`Slider::bind_value`]. The value is not pushed from `set_value`
    /// automatically: doing so would re-enter the runtime while the widget is already borrowed
    /// by the caller's setter. A host pushes with [`Slider::sync_to_binding`] after user
    /// interaction, which is the point at which the borrow has ended.
    binding: Option<crate::data_binding::Binding<CapabilityValue>>,
}
/// Tick mark position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickPosition {
    /// No tick marks
    #[default]
    NoTicks,
    /// Tick marks above (for horizontal) or left (for vertical)
    TicksAbove,
    /// Tick marks below (for horizontal) or right (for vertical)
    TicksBelow,
    /// Tick marks on both sides
    TicksBothSides,
}

/// One formatted tick label with the metrics `draw` positions it by.
struct CachedTickLabel {
    /// The label text, formatted once rather than per frame.
    text: String,
    /// Measured width, used to centre the label under a horizontal tick.
    width: u32,
    /// Measured height, used to centre the label beside a vertical tick.
    height: u32,
}

/// Tick labels and metrics for the current `(range, interval, face, scale)`.
#[derive(Default)]
struct TickLabelCache {
    minimum: i32,
    maximum: i32,
    interval: i32,
    /// The face the labels were measured with; `None` means the cache is empty.
    font: Option<Font>,
    dpi_scale_bits: u32,
    labels: Vec<CachedTickLabel>,
}

/// Edge length of the draggable handle, in logical pixels.
///
/// Named once because two places have to agree about it: `draw` centres the handle on the
/// position `value_to_pixel_pos` returns, and that function has to keep half a handle inside
/// each end. While the size was a literal in the draw path only, the travel range assumed a
/// zero-width handle and the minimum value's handle hung off the control.
///
/// It is *derived* from [`dimensions::SLIDER_THUMB_RADIUS`] rather than being a second
/// literal for the same object: the thumb is one diameter, and the arithmetic that keeps it
/// inside the control and the metric table that names it must not be able to disagree.
const SLIDER_SIZE: f32 = (dimensions::SLIDER_THUMB_RADIUS * 2) as f32;

/// How far the hover halo reaches past the handle's own edge.
///
/// Half a handle, so the halo reads as a ring around a disc rather than as a second, larger disc:
/// the handle stays the shape the census measures as the dominant colour.
const HALO_EXTRA_RADIUS: u32 = dimensions::SLIDER_THUMB_RADIUS / 2;

/// How far a **hovered** slider's halo is stepped toward the window fill.
///
/// The figure the major toolkits publish for "the pointer is over me" — Material's `hoverOpacity`
/// on a text field, the same 0.08 this crate's own state overrides use for a hovered fill
/// (`preset_states::HOVER_BLEND`), so the slider's halo and a button's hover are the same strength
/// of gesture rather than two arbitrary numbers.
const HOVER_HALO_WEIGHT: f32 = 0.08;

/// How far a **dragged** slider's halo is stepped toward the window fill.
///
/// Firmer than [`HOVER_HALO_WEIGHT`], because a drag is a committed gesture — the same relation
/// `preset_states::PRESSED_BLEND` (0.12) has to its hover.
const DRAG_HALO_WEIGHT: f32 = 0.12;

/// Formats a [`TickPosition`] as its published token.
///
/// Local rather than imported from `capability::access` (or `coercion`) for the
/// same reason the type itself lives here: `Slider` is available in every profile
/// while those modules' `TickPosition` helpers are gated to `full_widgets`.
pub const fn tick_position_to_str(tick_position: TickPosition) -> &'static str {
    match tick_position {
        TickPosition::NoTicks => "none",
        TickPosition::TicksAbove => "above",
        TickPosition::TicksBelow => "below",
        TickPosition::TicksBothSides => "both",
    }
}

/// Parses the token [`tick_position_to_str`] publishes, plus the spellings config
/// files have historically used.
///
/// The local inverse of [`tick_position_to_str`]; `coercion::expect_tick_position`
/// would do the same job but is `full_widgets`-gated, and a `Slider` must be able
/// to answer its own contract in every profile.
fn expect_tick_position(value: CapabilityValue) -> Result<TickPosition, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(text) => crate::widget::capability::coercion::normalize_key(&text),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "none" | "noticks" => Ok(TickPosition::NoTicks),
        "above" | "ticksabove" | "left" => Ok(TickPosition::TicksAbove),
        "below" | "ticksbelow" | "right" => Ok(TickPosition::TicksBelow),
        "both" | "ticksbothsides" => Ok(TickPosition::TicksBothSides),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}
impl Slider {
    /// Creates a slider with default range 0-100.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Slider, geometry, "Slider"),
            minimum: 0,
            maximum: 100,
            value: 0,
            single_step: 1,
            page_step: 10,
            orientation: Orientation::Horizontal,
            tick_position: TickPosition::NoTicks,
            tick_interval: 0,
            tracking: true,
            slider_position: 0,
            mouse_pressed: false,
            track_highlight: false,
            tick_labels: false,
            tick_label_cache: TickLabelCache::default(),
            animated_halo: false,
            halo: PropertyDriver::at(0.0, MotionSlot::Fast),
            direction: crate::core::TextDirection::default(),
            value_changed: Signal1::new(),
            slider_moved: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
            link: None,
            link_slot: None,
            binding: None,
        }
    }

    /// Binds this slider's `value` property to a reactive model value.
    ///
    /// The initial model value is applied immediately, and (when the slider is mounted) every
    /// later model change is written through the widget's own property contract. This is the
    /// control-side half of W-17: the bridge already existed, but no control consumed it.
    ///
    /// The reverse direction is [`Self::sync_to_binding`], called by the host after an
    /// interaction. The two directions are separate on purpose: the setter path already runs
    /// inside the widget borrow, so pushing there would re-enter the runtime.
    pub fn bind_value(&mut self, binding: &crate::data_binding::Binding<CapabilityValue>) {
        self.binding = Some(binding.clone());
        // The mounted path needs the widget registry, which a sized profile does not compile.
        // The fallback below works in every profile: it applies the initial value directly and
        // leaves later pushes to the host (or to a rebuild), which is the honest limit of a build
        // with no mounted-widget registry.
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.base.id()) {
                crate::data_binding::bind_widget_property(binding, "slider.value", id, "value");
                return;
            }
        }
        let _ = crate::widget::widget_property_set(self, "value", binding.get());
        binding.announce_synced("value");
    }

    /// Pushes this slider's current value into the bound model, reporting whether one is bound.
    pub fn sync_to_binding(&mut self) -> bool {
        let Some(binding) = self.binding.clone() else {
            return false;
        };
        binding.set(CapabilityValue::Int(i64::from(self.value())));
        binding.announce_synced("value");
        true
    }

    /// The reactive value this slider consumes, if it is bound.
    pub fn bound_value(&self) -> Option<&crate::data_binding::Binding<CapabilityValue>> {
        self.binding.as_ref()
    }

    /// Binds this slider to `spin` so the two present one numeric value.
    ///
    /// # What is bound
    ///
    /// The value, in the direction [`LinkMode`] selects (the default is [`LinkMode::OneWay`],
    /// slider → spin box), and the range through an explicit [`Self::sync_range`]. The link also
    /// makes `value_changed` fire on **both** controls for a change made on either side, because
    /// a host commonly subscribes to only one of the pair.
    ///
    /// # The conversion rule (i32 ↔ f64)
    ///
    /// The two controls publish different payloads — `Slider` is `i32`, `SpinBox` stores `f64` —
    /// so the link states the conversion once:
    ///
    /// * **slider → spin**: the `i32` widens exactly to `f64`; the spin box then rounds to its
    ///   own `decimals`, which is a no-op at the default `decimals == 0`.
    /// * **spin → slider**: the `f64` is first rounded to the spin box's `decimals` (so the number
    ///   that crosses is the number the spin box *shows*), then rounded to the nearest whole
    ///   number because the slider's payload cannot carry a fraction. With `decimals > 0` the
    ///   slider therefore tracks the nearest integer, and precision below one unit is not
    ///   representable on it.
    ///
    /// Binding replaces any previous link; call [`Self::unbind`] first if the old peer must be
    /// released explicitly. The re-entrancy guard lives in the shared link, so a value pushed by
    /// one side cannot be pushed straight back by the other.
    pub fn bind_to_spin_box(&mut self, spin: &crate::widget::SpinBox) {
        let decimals = spin.decimals();
        let current = spin.value_f64();
        let slider_value = self.value();
        let mode = self.link.as_ref().map_or(LinkMode::OneWay, |link| link.mode());
        let link = Arc::new(NumericLink::new(
            mode,
            decimals,
            current,
            spin.minimum_f64(),
            spin.maximum_f64(),
            slider_value,
            spin.value_changed.clone(),
            self.value_changed.clone(),
        ));

        // The spin box's own changes reach the slider only in `TwoWay`; `on_spin_changed` checks
        // the mode. The weak reference keeps the link from holding itself alive through the
        // signal it holds.
        let weak = Arc::downgrade(&link);
        link.spin_changed.connect_scoped(&link.scope, move |_value: Arc<i32>| {
            if let Some(link) = weak.upgrade() {
                link.on_spin_changed();
            }
        });

        self.link_slot = Some(spin.numeric_link_slot());
        let slot = spin.numeric_link_slot();
        if let Some(previous) = slot.borrow().as_ref() {
            // A second bind supersedes the first rather than leaving two live links on one spin
            // box.
            previous.detach();
        }
        *slot.borrow_mut() = Some(Arc::clone(&link));
        self.link = Some(Arc::clone(&link));
        // The two controls start from one value. The slider is the control being bound, so its
        // current value is the one written through; `slider_drives_spin` publishes it on the
        // spin box's signal with the re-entrancy guard held.
        link.slider_drives_spin(slider_value);
        self.value = slider_value;
        self.slider_position = slider_value;
        self.base.request_redraw();
    }

    /// Removes the link and stops it carrying values.
    ///
    /// The spin box keeps the link's last value as its own (clearing the slot would rewind it to
    /// the value it had when the pair was bound), but a detached link carries nothing: neither
    /// side's `value_changed` reaches the other after this call.
    pub fn unbind(&mut self) {
        if let Some(link) = &self.link {
            link.detach();
        }
        self.link = None;
    }

    /// Whether this slider is currently bound to a spin box.
    pub fn linked(&self) -> bool {
        self.link.is_some()
    }

    /// The direction values cross the current link in; [`LinkMode::OneWay`] when unbound.
    pub fn link_mode(&self) -> LinkMode {
        self.link.as_ref().map_or(LinkMode::OneWay, |link| link.mode())
    }

    /// Sets the direction values cross the current link in.
    pub fn set_link_mode(&mut self, mode: LinkMode) {
        if let Some(link) = &self.link {
            link.set_mode(mode);
            // Switching to `TwoWay` must adopt the peer's value now rather than at its next edit.
            if mode == LinkMode::TwoWay {
                let value = link.pull_from_spin();
                self.value = value;
                self.slider_position = value;
                self.base.request_redraw();
            }
        }
    }

    /// Copies the spin box's bounds, precision and step onto this slider.
    ///
    /// # Why the bounds are rounded outward
    ///
    /// The spin box may bound a decimal range (`0.5 .. 9.5`), which an `i32` slider cannot
    /// express. The floor is applied to the lower bound and the ceiling to the upper one, so the
    /// slider's reachable range never exceeds what the spin box would accept. The current value
    /// is re-clamped afterwards and, when linked, pushed back through the same conversion rule
    /// [`Self::bind_to_spin_box`] documents.
    pub fn sync_range(&mut self) {
        let Some(link) = &self.link else {
            return;
        };
        let (spin_minimum, spin_maximum, decimals) = link.bounds();
        // The floor is applied to the lower bound and the ceiling to the upper one, so the
        // slider's reachable range never exceeds what the spin box accepts.
        let minimum = clamp_f64_to_i32(spin_minimum.ceil());
        let maximum = clamp_f64_to_i32(spin_maximum.floor()).max(minimum);
        link.set_decimals(decimals);
        self.minimum = minimum;
        self.maximum = maximum;
        // Adopt the spin box's value on the slider side; pushing it back would replace a decimal
        // value with the whole number the slider can carry.
        let value = link.spin_to_slider().clamp(minimum, maximum);
        link.set_slider_value(value);
        self.value = value;
        self.slider_position = value;
        self.base.request_redraw();
    }
    /// Returns minimum value.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Sets minimum value.
    pub fn set_minimum(&mut self, minimum: i32) {
        let changed = self.minimum != minimum;
        self.minimum = minimum;
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
        self.set_value(self.value()); // Re-clamp
                                      // `draw` derives the tick marks from the bounds, so moving a bound moves ink. `set_value`
                                      // above only repaints when the *value* had to be re-clamped, which it may well not have —
                                      // so a bounds-only change was invisible.
        if changed {
            self.base.request_redraw();
        }
    }
    /// Returns maximum value.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Sets maximum value.
    pub fn set_maximum(&mut self, maximum: i32) {
        let changed = self.maximum != maximum;
        self.maximum = maximum;
        if self.minimum > self.maximum {
            self.minimum = self.maximum;
        }
        self.set_value(self.value()); // Re-clamp
        if changed {
            self.base.request_redraw();
        }
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, minimum: i32, maximum: i32) {
        let changed = self.minimum != minimum || self.maximum != maximum.max(minimum);
        self.minimum = minimum;
        self.maximum = maximum.max(minimum);
        self.set_value(self.value()); // Re-clamp
        if changed {
            self.base.request_redraw();
        }
    }
    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.link.as_ref().map_or(self.value, |link| link.slider_value())
    }
    /// Sets value, clamped to valid range.
    pub fn set_value(&mut self, value: i32) {
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        if self.value() == clamped {
            return;
        }
        // The link's shared value is written before the signal so a listener that reads
        // `value()` sees the value being announced rather than the previous one.
        if let Some(link) = &self.link {
            link.slider_drives_spin(clamped);
        }
        self.value = clamped;
        self.slider_position = clamped;
        self.value_changed.emit(self.value());
        self.base.request_redraw();
    }
    /// Returns single step value.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Sets single step value.
    pub fn set_single_step(&mut self, step: i32) {
        let step = step.max(1);
        if self.single_step == step {
            return;
        }
        self.single_step = step;
        self.base.request_redraw();
    }
    /// Returns page step value.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    /// Sets page step value.
    pub fn set_page_step(&mut self, step: i32) {
        let step = step.max(1);
        if self.page_step == step {
            return;
        }
        self.page_step = step;
        self.base.request_redraw();
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        self.base.request_redraw();
    }
    /// Returns tick position.
    pub fn tick_position(&self) -> TickPosition {
        self.tick_position
    }
    /// Sets tick position.
    pub fn set_tick_position(&mut self, position: TickPosition) {
        if self.tick_position == position {
            return;
        }
        self.tick_position = position;
        self.base.request_redraw();
    }
    /// Returns tick interval.
    pub fn tick_interval(&self) -> i32 {
        self.tick_interval
    }
    /// Sets tick interval.
    pub fn set_tick_interval(&mut self, interval: i32) {
        let interval = interval.max(0);
        if self.tick_interval == interval {
            return;
        }
        self.tick_interval = interval;
        self.base.request_redraw();
    }
    /// Returns whether tracking is enabled.
    pub fn tracking(&self) -> bool {
        self.tracking
    }
    /// Sets tracking state.
    pub fn set_tracking(&mut self, tracking: bool) {
        self.tracking = tracking;
    }

    /// Whether the passed part of the groove is highlighted.
    pub fn track_highlight(&self) -> bool {
        self.track_highlight
    }

    /// Enables the accent-coloured filled track from the start of the groove to the handle.
    ///
    /// The fill runs in **value order**, so a right-to-left slider fills from its right edge
    /// and a vertical slider from its bottom edge: the part painted is the part the value has
    /// already passed, in both writing directions.
    ///
    /// Defaults to `false`, which is the historical picture with no fill at all.
    pub fn set_track_highlight(&mut self, on: bool) {
        if self.track_highlight == on {
            return;
        }
        self.track_highlight = on;
        self.base.request_redraw();
    }

    /// Whether each tick is labelled with its numeric value.
    pub fn tick_labels(&self) -> bool {
        self.tick_labels
    }

    /// Enables the numeric label drawn with each tick mark.
    ///
    /// Labels are only drawn where tick marks are (`tick_position` is not
    /// [`TickPosition::NoTicks`] and `tick_interval` is positive), and the tick count is already
    /// capped, so this cannot become an unbounded text spray.
    ///
    /// Defaults to `false`, so an existing slider keeps its unlabelled ticks.
    pub fn set_tick_labels(&mut self, on: bool) {
        if self.tick_labels == on {
            return;
        }
        self.tick_labels = on;
        self.base.request_redraw();
    }

    /// Whether the hover/drag halo interpolates between levels.
    pub fn animated_halo(&self) -> bool {
        self.animated_halo
    }

    /// Enables interpolation of the hover/drag halo.
    ///
    /// With it on, the halo fades in and out over the theme's fast motion token as the pointer
    /// arrives, presses and leaves, driven by [`Slider::tick`]. With it off the halo keeps its
    /// historical two-level appearance, which is what keeps existing snapshots identical —
    /// which is also why this is not the default.
    ///
    /// Switching it on places the halo at the strength the current state calls for, so the
    /// setter itself does not make the control flash.
    pub fn set_animated_halo(&mut self, on: bool) {
        if self.animated_halo == on {
            return;
        }
        self.animated_halo = on;
        if on {
            let target = self.halo_target();
            self.halo.jump_to(target);
        }
        self.base.request_redraw();
    }

    /// The halo strength currently painted, as a blend weight toward the window fill.
    ///
    /// `0.0` at rest, and one of the two halo weights while hovered or dragged. With
    /// [`Slider::set_animated_halo`] on this is the *interpolated* value, which is the number
    /// an animation has to be sampled to see.
    pub fn halo_strength(&self) -> f32 {
        if !self.base.is_enabled() {
            0.0
        } else if self.animated_halo {
            self.halo.value()
        } else if self.base.is_pressed() {
            DRAG_HALO_WEIGHT
        } else if self.base.is_hovered() {
            HOVER_HALO_WEIGHT
        } else {
            0.0
        }
    }

    /// The halo strength the current interaction state calls for.
    fn halo_target(&self) -> f32 {
        if !self.base.is_enabled() {
            0.0
        } else if self.base.is_pressed() {
            DRAG_HALO_WEIGHT
        } else if self.base.is_hovered() {
            HOVER_HALO_WEIGHT
        } else {
            0.0
        }
    }

    /// Advances the animated halo by `delta_ms`.
    ///
    /// Returns `true` while the halo is still moving and `false` once it has settled. With
    /// [`Slider::set_animated_halo`] off this is a still picture and owes nothing.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animated_halo {
            return false;
        }
        let target = self.halo_target();
        self.halo.set_target(target);
        self.halo.tick(delta_ms)
    }

    /// Whether the animated halo owes frames.
    pub fn is_animating(&self) -> bool {
        self.animated_halo && (self.halo.value() - self.halo_target()).abs() > f32::EPSILON
    }
    /// Returns slider position.
    pub fn slider_position(&self) -> i32 {
        if self.link.is_some() {
            self.value()
        } else {
            self.slider_position
        }
    }
    /// Sets slider position (without emitting signals).
    pub fn set_slider_position(&mut self, position: i32) {
        let new_position = ordered_clamp_i32(position, self.minimum, self.maximum);
        if self.slider_position == new_position {
            return;
        }
        self.slider_position = new_position;
        if self.tracking {
            self.set_value(self.slider_position);
        }
        self.slider_moved.emit(self.slider_position);
    }
    /// Adds single step to value.
    pub fn trigger_action(&mut self, action: SliderAction) {
        // The step arithmetic saturates before `set_value` clamps it: `self.value +
        // self.single_step` on values near the `i32` extremes overflowed (debug abort,
        // release wrap) *before* the clamp could bound it, so the slider jumped to a
        // wrapped value instead of stopping at the bound.
        match action {
            SliderAction::SliderSingleStepAdd => {
                self.set_value(self.value().saturating_add(self.single_step));
            }
            SliderAction::SliderSingleStepSub => {
                self.set_value(self.value().saturating_sub(self.single_step));
            }
            SliderAction::SliderPageStepAdd => {
                self.set_value(self.value().saturating_add(self.page_step));
            }
            SliderAction::SliderPageStepSub => {
                self.set_value(self.value().saturating_sub(self.page_step));
            }
            SliderAction::SliderToMinimum => {
                self.set_value(self.minimum);
            }
            SliderAction::SliderToMaximum => {
                self.set_value(self.maximum);
            }
            SliderAction::SliderMove => {
                // Handled by mouse events
            }
        }
    }
    /// The tick count for the current range, capped at 100.
    ///
    /// # Why the arithmetic is `i64` (D09-CTRL-01)
    ///
    /// `((maximum - minimum) / tick_interval) + 1` in `i32` overflows for a legal `i32::MIN..=i32::MAX`
    /// range before the `.min(100)` cap can apply. Computing the span in `i64` keeps the division
    /// exact and the cap meaningful, and the tick *values* are derived through `tick_value` below so a
    /// tick index cannot itself overflow the `i32` domain.
    fn tick_count(&self) -> u32 {
        if self.tick_interval <= 0 {
            return 0;
        }
        let span = self.maximum as i64 - self.minimum as i64;
        let interval = self.tick_interval as i64;
        // `span >= 0` by the range invariant, so truncating division is a floor here.
        let total = span / interval + 1;
        total.clamp(0, 100) as u32
    }

    /// The value at tick `index`, saturating at `maximum`.
    ///
    /// The index is converted through `i64` first: `minimum + index * tick_interval` in `i32` could
    /// overflow at the domain's top for a coarse interval (D09-CTRL-01).
    fn tick_value(&self, index: u32) -> i32 {
        let value = self.minimum as i64 + index as i64 * self.tick_interval as i64;
        value.clamp(self.minimum as i64, self.maximum as i64) as i32
    }

    /// Formats and measures the tick labels, reusing the previous result while every
    /// input that determines it stays the same.
    ///
    /// The draw pass used to `write!` a fresh `String` and call `measure_text` for each
    /// tick on every frame, up to a hundred times while dragging. The labels depend only
    /// on the range, the interval, the effective face and the device scale, so they are
    /// built once per change and read-only afterwards.
    fn refresh_tick_labels(&mut self, context: &RenderContext) {
        let scale_bits = context.dpi_scale().to_bits();
        let unchanged = {
            let source = crate::widget::metrics::effective_font(self.style());
            self.tick_label_cache.font.as_ref() == Some(source)
                && self.tick_label_cache.dpi_scale_bits == scale_bits
                && self.tick_label_cache.minimum == self.minimum
                && self.tick_label_cache.maximum == self.maximum
                && self.tick_label_cache.interval == self.tick_interval
        };
        if unchanged {
            return;
        }
        // The face is cloned only on the frame that actually rebuilds the labels; a still
        // control reads the resolved font by reference.
        let font = crate::widget::metrics::effective_font(self.style()).clone();

        self.tick_label_cache.labels.clear();
        let count = self.tick_count();
        for index in 0..count {
            let value = self.tick_value(index);
            let mut label = String::new();
            let _ = write!(label, "{}", value);
            let metrics = context.measure_text(&label, &font);
            self.tick_label_cache.labels.push(CachedTickLabel {
                text: label,
                width: metrics.width,
                height: metrics.height,
            });
        }
        self.tick_label_cache.minimum = self.minimum;
        self.tick_label_cache.maximum = self.maximum;
        self.tick_label_cache.interval = self.tick_interval;
        self.tick_label_cache.font = Some(font.clone());
        self.tick_label_cache.dpi_scale_bits = scale_bits;
    }

    /// Returns value for a given pixel position.
    fn pixel_pos_to_value(&self, pos: f32) -> i32 {
        let rect = self.geometry();
        // The span and the relative offset are computed in `f64`, not `i32` (D09-CTRL-01): a legal
        // `i32::MIN..=i32::MAX` range has no `i32` difference, so `(maximum - minimum) as f32`
        // panicked in a checked build and wrapped to a wrong — even negative — span otherwise.
        // `f64` represents every `i32` exactly and their difference without loss.
        let range = self.maximum as f64 - self.minimum as f64;
        // The handle is `SLIDER_SIZE` wide and `value_to_pixel_pos` centres it on the position it
        // returns, so that function's travel stops half a handle short of each end. This one used the
        // **full** width, which made the two calculations non-inverses: the value 0 was drawn at
        // x = inset, and reading that same x back produced a mid-range value. In practice clicking
        // the handle did not select the value the handle was showing — and the error grew with the
        // zoom of the range, so it was not a rounding artefact. Sharing the inset makes the two
        // exact inverses, which the round-trip test pins.
        let inset = SLIDER_SIZE / 2.0;
        let relative = match self.orientation {
            Orientation::Horizontal => {
                let travel = (rect.width as f32 - inset * 2.0).max(0.0);
                if travel == 0.0 {
                    return self.minimum;
                }
                let left_fraction = ((pos - rect.x as f32 - inset) / travel).clamp(0.0, 1.0);
                // The pixel is measured from the left edge, but the value runs in reading order, so
                // the direction converts between the two frames. Without it an RTL slider reported
                // the *inverse* value of the position a user clicked — the defect this fixes.
                self.direction.left_fraction_to_begin_fraction(left_fraction)
            }
            Orientation::Vertical => {
                let travel = (rect.height as f32 - inset * 2.0).max(0.0);
                if travel == 0.0 {
                    return self.minimum;
                }
                // Vertical runs top-to-bottom in every direction: the block axis is not mirrored, so
                // the direction must not be applied here.
                1.0 - ((pos - rect.y as f32 - inset) / travel).clamp(0.0, 1.0)
            }
        };
        let value = self.minimum as f64 + range * relative as f64;
        // `round()` then a saturating cast: a value at the far end can round a hair past the bound,
        // and `f64 as i32` saturates rather than wrapping, so the answer stays inside `[minimum,
        // maximum]` even at the `i32` extremes.
        ordered_clamp_i32(value.round() as i32, self.minimum, self.maximum)
    }
    /// Returns pixel position for a given value.
    fn value_to_pixel_pos(&self, value: i32) -> f32 {
        let rect = self.geometry();
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        // The handle is `SLIDER_SIZE` wide and is *centred* on the position this returns, so
        // the travel range has to stop half a handle short of each end. Using the full width
        // put the minimum value's handle centre on `rect.x`, i.e. half of the handle outside
        // the control; the raster backend clipped it and the SVG snapshot showed it hanging
        // off the left edge, which is how the defect was found.
        let inset = SLIDER_SIZE / 2.0;
        let travel = match self.orientation {
            Orientation::Horizontal => (rect.width as f32 - inset * 2.0).max(0.0),
            Orientation::Vertical => (rect.height as f32 - inset * 2.0).max(0.0),
        };
        // Same `f64` reasoning as `pixel_pos_to_value` (D09-CTRL-01): the span and the
        // `clamped - minimum` offset overflow `i32` for a full-`i32` range.
        let range = self.maximum as f64 - self.minimum as f64;
        if range == 0.0 {
            return match self.orientation {
                Orientation::Horizontal => rect.x as f32 + inset,
                Orientation::Vertical => rect.y as f32 + rect.height as f32 / 2.0,
            };
        }
        let relative = (clamped as f64 - self.minimum as f64) / range;
        match self.orientation {
            Orientation::Horizontal => {
                // The value runs in reading order; the pixel is measured from the left edge. The
                // direction is the single conversion between the two, and it is the same one
                // `pixel_pos_to_value` applies in reverse — so a handle drawn at a position reads
                // back as the value that put it there.
                let left_fraction = self.direction.begin_fraction_to_left_fraction(relative as f32);
                rect.x as f32 + inset + travel * left_fraction
            }
            Orientation::Vertical => {
                rect.y as f32 + inset + travel * (1.0 - relative as f32) // Invert Y axis
            }
        }
    }

    /// Returns the direction the value axis runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// The x coordinate the handle is centred on for the current value, in the control's own
    /// geometry.
    ///
    /// Exposed because the handle's position is the slider's visible answer to "where is the
    /// value", and a host drawing an overlay (a value bubble, a custom groove) needs to agree with
    /// the slider about it rather than recompute the mapping and drift.
    pub fn handle_centre_x(&self) -> f32 {
        self.value_to_pixel_pos(self.value())
    }

    /// The y coordinate the handle is centred on for the current value, for a vertical slider.
    pub fn handle_centre_y(&self) -> f32 {
        self.value_to_pixel_pos(self.value())
    }

    /// The value a pointer at `x` selects, in the slider's own geometry.
    ///
    /// The inverse of [`Self::handle_centre_x`], so a host forwarding a drag and a host drawing the
    /// handle use one mapping between them.
    pub fn value_at_x(&self, x: f32) -> i32 {
        self.pixel_pos_to_value(x)
    }

    /// Sets the writing direction the value axis runs in.
    ///
    /// A right-to-left slider puts the minimum at the right edge, so dragging left raises the value.
    /// Set this at construction or when the locale changes; the redraw is requested here so a host
    /// does not have to remember to.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.base.request_redraw();
    }

    /// The handle's own square, in control coordinates.
    ///
    /// Derived from the same [`Self::value_to_pixel_pos`] mapping and [`ControlMetrics`]
    /// centring the paint path uses, so the keyboard focus ring frames the handle that is
    /// actually on screen rather than a second estimate of its position.
    fn handle_rect(&self) -> Rect {
        let radius = (SLIDER_SIZE / 2.0) as u32;
        let side = radius * 2;
        let disc = ControlMetrics::centered_disc(self.geometry(), side);
        let centre = self.value_to_pixel_pos(self.value()) as i32;
        match self.orientation {
            Orientation::Horizontal => Rect::new(centre - radius as i32, disc.y, side, side),
            Orientation::Vertical => Rect::new(disc.x, centre - radius as i32, side, side),
        }
    }
}
/// Slider actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderAction {
    /// Move slider by single step up/right
    SliderSingleStepAdd,
    /// Move slider by single step down/left
    SliderSingleStepSub,
    /// Move slider by page step up/right
    SliderPageStepAdd,
    /// Move slider by page step down/left
    SliderPageStepSub,
    /// Move slider to minimum
    SliderToMinimum,
    /// Move slider to maximum
    SliderToMaximum,
    /// Move slider to arbitrary position
    SliderMove,
}
// Implement Widget trait
impl Widget for Slider {
    /// Resolves the published event names this control emits to their signals.
    ///
    /// | published name | signal | payload |
    /// |---|---|---|
    /// | `value_changed` | `value_changed` | `i32` |
    /// | `slider_moved` | `slider_moved` | `i32` |
    /// | `slider_pressed` | `slider_pressed` | none |
    /// | `slider_released` | `slider_released` | none |
    ///
    /// `value_changed` and `slider_moved` carry the same type but mean different things: the first
    /// fires on every value change including a programmatic setter, the second only while the handle
    /// is dragged. A designer offers both because they are both published, and the payload type it
    /// shows is the same for each.
    ///
    /// `clicked` is deliberately **not** here: the control owns that signal on its base, but its
    /// capability does not publish the name, so an arm resolving it would be dead code that looks
    /// like support. `tools/check_event_signal_dyn.py` fails on either half of that mismatch.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |value| {
                    CapabilityValue::Int(*value as i64)
                }))
            }
            "slider_moved" => {
                Some(EventSignalRef::mapped("slider_moved", &self.slider_moved, |value| {
                    CapabilityValue::Int(*value as i64)
                }))
            }
            "slider_pressed" => Some(EventSignalRef::unit("slider_pressed", &self.slider_pressed)),
            "slider_released" => {
                Some(EventSignalRef::unit("slider_released", &self.slider_released))
            }
            _ => None,
        }
    }

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
        let extent = crate::widget::metrics::dimensions::BAR_DEFAULT_EXTENT;
        let thickness = crate::widget::metrics::dimensions::BAR_DEFAULT_THICKNESS;
        match self.orientation() {
            Orientation::Horizontal => Size::new(extent, thickness),
            Orientation::Vertical => Size::new(thickness, extent),
        }
    }

    /// One frame of the animated hover/drag halo.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Slider::tick(self, delta_ms)
    }

    /// The slider owes frames while its halo is still moving.
    fn is_animating(&self) -> bool {
        Slider::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Slider`'s property contract.
///
/// The control names its own properties here, reads and writes them against its
/// own fields, and forwards every name it does not recognise to the shared base
/// helpers. Semantics mirror the previous centralised dispatch exactly.
impl WidgetProperties for Slider {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "minimum" => Ok(CapabilityValue::Int(self.minimum() as i64)),
            "maximum" => Ok(CapabilityValue::Int(self.maximum() as i64)),
            "value" => Ok(CapabilityValue::Int(self.value() as i64)),
            "single_step" => Ok(CapabilityValue::Int(self.single_step() as i64)),
            "page_step" => Ok(CapabilityValue::Int(self.page_step() as i64)),
            "orientation" => {
                Ok(CapabilityValue::String(orientation_to_str(self.orientation()).to_string()))
            }
            "tick_position" => {
                Ok(CapabilityValue::String(tick_position_to_str(self.tick_position()).to_string()))
            }
            "tick_interval" => Ok(CapabilityValue::Int(self.tick_interval() as i64)),
            "tracking" => Ok(CapabilityValue::Bool(self.tracking())),
            "slider_position" => Ok(CapabilityValue::Int(self.slider_position() as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "minimum" => {
                self.set_minimum(expect_i32_saturating(value)?);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_i32_saturating(value)?);
                Ok(())
            }
            "value" => {
                self.set_value(expect_i32_saturating(value)?);
                Ok(())
            }
            "single_step" => {
                self.set_single_step(expect_i32_saturating(value)?);
                Ok(())
            }
            "page_step" => {
                self.set_page_step(expect_i32_saturating(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_orientation(value)?);
                Ok(())
            }
            "tick_position" => {
                self.set_tick_position(expect_tick_position(value)?);
                Ok(())
            }
            "tick_interval" => {
                self.set_tick_interval(expect_i32_saturating(value)?);
                Ok(())
            }
            "tracking" => {
                self.set_tracking(expect_bool(value)?);
                Ok(())
            }
            "slider_position" => {
                self.set_slider_position(expect_i32_saturating(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SLIDER_PROPERTIES`: this control's own names plus the four
        // shared ones that `base_property_get` answers.
        property_names_of![
            "minimum",
            "maximum",
            "value",
            "single_step",
            "page_step",
            "orientation",
            "tick_position",
            "tick_interval",
            "tracking",
            "slider_position",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `slider` publishes.
    ///
    /// All three assign state and need an argument a command carries none of:
    /// `set_range` the bounds, `set_value` the position and `set_slider_position` the
    /// derived handle coordinate. They are refused as
    /// [`CapabilityAccessError::OutOfRange`] so the caller is sent to the property
    /// route (`set("value", ..)`, `set("slider_position", ..)`) rather than to a
    /// different control, which is what `UnknownCommand` would say.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_range" | "set_value" | "set_slider_position" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Slider {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.mouse_pressed = true;
                self.slider_pressed.emit();
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                self.mouse_pressed = true;
                self.slider_pressed.emit();
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.mouse_pressed = false;
                self.slider_released.emit();
                if !self.tracking {
                    self.set_value(self.slider_position());
                }
            }
            // A press whose release lands outside the widget never reaches the arm
            // above: the runtime's hit-test returns `None` for a point outside every
            // control, so no `MouseRelease` is delivered. Without this arm the flag
            // stayed set and the *next* hover moved the handle with no button held —
            // the control behaved as if permanently dragging. Cancelling is the right
            // answer rather than committing, because the user left the control.
            Event::MouseLeave { .. } if self.mouse_pressed => {
                self.mouse_pressed = false;
                self.slider_released.emit();
                if !self.tracking {
                    self.set_value(self.slider_position());
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                self.mouse_pressed = false;
                self.slider_released.emit();
                if !self.tracking {
                    self.set_value(self.slider_position());
                }
            }
            Event::MouseMove { pos } if self.mouse_pressed => {
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            // A wheel notch is one single step. A positive delta scrolls "forward" — toward
            // the maximum — matching the direction a vertical wheel moves a page.
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y != 0 {
                    let step = (scroll.lines_y as i64) * (self.single_step as i64);
                    let next = (self.value() as i64 + step).clamp(i32::MIN as i64, i32::MAX as i64);
                    self.set_value(next as i32);
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchMove { pos, .. } if self.mouse_pressed => {
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            Event::KeyPress { key, modifiers: _ } => {
                // Arrow keys name a *screen* direction, but a slider's value runs in reading order.
                // The two agree in LTR and are opposite in RTL, so the step is expressed in reading
                // order and converted through the direction once, here — instead of each arm
                // knowing about direction and risking one of them being missed.
                let step =
                    |begin_step: i32| match self.direction.begin_step_to_left_step(begin_step) {
                        // Leftward on screen.
                        step if step < 0 => SliderAction::SliderSingleStepSub,
                        _ => SliderAction::SliderSingleStepAdd,
                    };
                match *key {
                    37 => {
                        // Left arrow (or up arrow for vertical)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepSub);
                        } else {
                            self.trigger_action(step(-1));
                        }
                    }
                    38 => {
                        // Up arrow (or right arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepAdd);
                        } else {
                            self.trigger_action(step(1));
                        }
                    }
                    39 => {
                        // Right arrow (or down arrow for vertical)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepAdd);
                        } else {
                            self.trigger_action(step(1));
                        }
                    }
                    40 => {
                        // Down arrow (or left arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepSub);
                        } else {
                            self.trigger_action(step(-1));
                        }
                    }
                    33 => {
                        // Page up
                        self.trigger_action(SliderAction::SliderPageStepSub);
                    }
                    34 => {
                        // Page down
                        self.trigger_action(SliderAction::SliderPageStepAdd);
                    }
                    36 => {
                        // Home
                        self.trigger_action(SliderAction::SliderToMinimum);
                    }
                    35 => {
                        // End
                        self.trigger_action(SliderAction::SliderToMaximum);
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Slider {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let slider_pos = self.value_to_pixel_pos(self.value());

        // Resolve the label face once, outside the tick loop, and refresh the cached
        // labels before the style is borrowed for the rest of the draw.
        if self.tick_labels && self.tick_position != TickPosition::NoTicks {
            self.refresh_tick_labels(context);
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. The groove read `style` already,
        // but the handle and the ticks were literals, so a light/dark switch left them unchanged
        // — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.style();
        // `slider` classifies as `WidgetRole::Accent`, whose resolved colours are the theme's
        // accent token and its contrasting ink. `primary` is read alongside it as the crate's
        // conventional value-indicator token, so the handle matches the rest of the library.
        let theme = crate::style::resolved_theme_style_shared("slider");
        // A stripped device build (`mini`/`embedded`) has no theme module, so there is no
        // manager to read. The literals below are the same ones the `None` arm uses, which
        // is what keeps the two profiles rendering alike rather than inventing a palette
        // for the profile that has none (principle #37).
        #[cfg(device_profile)]
        let (window_fill, foreground, primary, accent, muted) = match crate::style::palette_shared()
        {
            Some(colors) => (
                colors.background,
                colors.foreground,
                colors.primary,
                colors.accent,
                colors.secondary,
            ),
            None => (
                Color::rgb(240, 240, 240),
                Color::BLACK,
                Color::rgb(33, 150, 243),
                Color::rgb(255, 152, 0),
                Color::rgb(158, 158, 158),
            ),
        };
        #[cfg(not(device_profile))]
        let (window_fill, foreground, primary, accent, muted) = (
            Color::rgb(240, 240, 240),
            Color::BLACK,
            Color::rgb(33, 150, 243),
            Color::rgb(255, 152, 0),
            Color::rgb(158, 158, 158),
        );

        // The groove: a caller's own colour wins, then the theme's resolved background. A control
        // classified as `Surface` or `Accent` resolves to something that is not the window fill on
        // its own, but the filter keeps a window-coloured value from being painted as the track.
        let groove_color = style
            .background_color
            .filter(|resolved| *resolved != window_fill)
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .filter(|resolved| *resolved != window_fill)
            .unwrap_or_else(|| window_fill.blend(&foreground, 0.14));
        // The handle is the largest area the slider paints and the census measures it as the
        // dominant colour, so it is what must move with the appearance: it carries the theme's
        // primary (the value indicator) rather than a fixed blue. A caller's border colour, which
        // is what the old code used to outline the handle, is not reused for the fill — it stands
        // in for the handle's own outline below.
        let handle_color = primary;
        // Ticks are de-emphasised from the groove rather than being a fixed grey.
        let tick_color = groove_color.blend(&muted, 0.55);
        // The handle's outline, one visible step from the handle itself. `accent` is kept in the
        // read so the palette tuple stays uniform across the material controls.
        let handle_border = style
            .border_color
            .filter(|resolved| *resolved != handle_color)
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != handle_color)
            .unwrap_or_else(|| handle_color.blend(&accent, 0.40));
        // A disabled slider must *look* disabled: it used to paint the same track, fill, handle
        // and halo as an enabled one, and only refuse input. The states are damped toward the
        // window fill by the same weights a disabled control uses elsewhere, and the halo is
        // suppressed because a disabled control cannot be hovered or dragged.
        let is_enabled = self.base.is_enabled();
        let (groove_color, handle_color, tick_color, handle_border) = if is_enabled {
            (groove_color, handle_color, tick_color, handle_border)
        } else {
            (
                groove_color.blend(&window_fill, 0.50),
                handle_color.blend(&window_fill, 0.55),
                tick_color.blend(&window_fill, 0.50),
                handle_border.blend(&window_fill, 0.55),
            )
        };
        // ── The interaction halo (BLUE23 Appendix A.4 / M1) ──
        //
        // # The defect this removes
        //
        // The control maintained a private `mouse_pressed` flag that `draw` **never read** (BLUE21
        // AR6): a slider could be dragged with no visual confirmation that it was being dragged,
        // and hovering it did nothing at all. `BaseWidget` already keeps both facts — that is the
        // whole point of M1 — so this reads them rather than the field, and the field stays what
        // it is for (deciding whether a pointer move resizes the value).
        //
        // The weights are the two the major toolkits publish for exactly this: a hover is a faint
        // step, a drag a firmer one. Drawn as a disc **behind** the handle, so the halo cannot hide
        // the value indicator it is drawing attention to.
        let halo_weight = if !is_enabled {
            None
        } else if self.animated_halo {
            let weight = self.halo.value();
            if weight > f32::EPSILON {
                Some(weight)
            } else {
                None
            }
        } else if self.base.is_pressed() {
            Some(DRAG_HALO_WEIGHT)
        } else if self.base.is_hovered() {
            Some(HOVER_HALO_WEIGHT)
        } else {
            None
        };
        let halo_color = halo_weight.map(|weight| handle_color.blend(&window_fill, weight));
        // Draw groove (track)
        //
        // The groove and the handle are **fixed-size chrome centred in the area the control
        // was given**, not a scaling of it. Painting `rect` directly made a slider handed the
        // 240x120 census cell draw a 240x120 handle — `slider.svg` carried `<rect x="0"
        // y="0" width="16" height="120"/>`, a full-height slab sixteen pixels wide, which is
        // a picture of a slider-shaped rectangle rather than a slider. The two dimensions
        // come from the shared `dimensions` table ([`dimensions::SLIDER_TRACK_HEIGHT`],
        // [`dimensions::SLIDER_THUMB_RADIUS`]) rather than from local literals, so the drawn
        // handle and `SLIDER_SIZE` — which the value↔pixel mapping and therefore the
        // round-trip invariant depend on — cannot describe different controls.
        //
        // The *travel* still comes from `self.geometry()`: a 120 px-tall vertical slider has
        // 120 px of travel, and shrinking that to the thumb's diameter would silently change
        // the mapping `pixel_pos_to_value` inverts.
        let track = ControlMetrics::centered_band(rect, dimensions::SLIDER_TRACK_HEIGHT);
        let thumb = ControlMetrics::centered_disc(rect, SLIDER_SIZE as u32);
        // A value with no travel (a degenerate range) has no handle to place: the
        // `range == 0.0` arm of `value_to_pixel_pos` already answers the left/top end, so the
        // disc is drawn there and this branch is not taken with a zero-extent element.
        let handle_radius = (SLIDER_SIZE / 2.0) as u32;
        // The filled part of the groove, in the same accent the handle uses, damped alongside
        // the other chrome when the control is disabled. Only read when the caller asked for
        // the highlight, so the default picture still paints the groove alone.
        let track_fill = if is_enabled { accent } else { accent.blend(&window_fill, 0.50) };
        let horizontal_ltr = self.direction == crate::core::TextDirection::LeftToRight;
        match self.orientation {
            Orientation::Horizontal => {
                // Draw groove
                context.fill_rect(
                    Rect::from_f32(
                        track.x as f32,
                        track.y as f32,
                        track.width as f32,
                        track.height as f32,
                    ),
                    groove_color,
                );
                if self.track_highlight {
                    // Value order: left-to-right fills from the left edge, right-to-left from
                    // the right edge, so the filled part is the part already passed.
                    let (begin, end) = if horizontal_ltr {
                        (track.x as f32, slider_pos)
                    } else {
                        (slider_pos, track.x as f32 + track.width as f32)
                    };
                    let left = begin.min(end).max(track.x as f32);
                    let right = begin.max(end).min(track.x as f32 + track.width as f32);
                    if right > left {
                        context.fill_rect(
                            Rect::from_f32(left, track.y as f32, right - left, track.height as f32),
                            track_fill,
                        );
                    }
                }
                // Draw slider handle: a disc centred on the mapped position, on the control's
                // own middle line, so it reads as sitting *on* the groove at every height.
                let handle_centre =
                    Point::from_f32(slider_pos, thumb.y as f32 + thumb.height as f32 / 2.0);
                if let Some(halo) = halo_color {
                    context.fill_circle(handle_centre, handle_radius + HALO_EXTRA_RADIUS, halo);
                }
                context.fill_circle(handle_centre, handle_radius, handle_color);
                // Draw handle border
                let handle_ring =
                    style.border_color.filter(|c| *c != handle_color).unwrap_or(handle_border);
                context.draw_circle_stroke(handle_centre, handle_radius, handle_ring, 1);
                // Draw ticks if enabled (capped at 100 ticks max to avoid
                // performance issues with large ranges and small intervals).
                if self.tick_position != TickPosition::NoTicks && self.tick_interval > 0 {
                    let tick_height = dimensions::SLIDER_TRACK_HEIGHT * 2;
                    let tick_count = self.tick_count();
                    // Resolve the label face once rather than on every tick.
                    let tick_font = crate::widget::metrics::effective_font(style);
                    for i in 0..tick_count {
                        let value = self.tick_value(i);
                        let tick_x = self.value_to_pixel_pos(value);
                        if self.tick_position == TickPosition::TicksAbove
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(tick_x, track.y as f32),
                                Point::from_f32(tick_x, track.y as f32 + tick_height as f32),
                                tick_color,
                            );
                        }
                        if self.tick_position == TickPosition::TicksBelow
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(
                                    tick_x,
                                    track.y as f32 + track.height as f32 - tick_height as f32,
                                ),
                                Point::from_f32(tick_x, track.y as f32 + track.height as f32),
                                tick_color,
                            );
                        }
                        if self.tick_labels {
                            if let Some(label) = self.tick_label_cache.labels.get(i as usize) {
                                let label_x = tick_x as i32 - label.width as i32 / 2;
                                let label_y = track.y + track.height as i32 + 2;
                                context.draw_text(
                                    Point::new(label_x, label_y),
                                    label.text.as_str(),
                                    tick_font,
                                    tick_color,
                                    HorizontalAlignment::Left,
                                );
                            }
                        }
                    }
                }
            }
            Orientation::Vertical => {
                // Draw groove
                context.fill_rect(
                    Rect::from_f32(
                        track.x as f32,
                        track.y as f32,
                        track.width as f32,
                        track.height as f32,
                    ),
                    groove_color,
                );
                if self.track_highlight {
                    // A vertical slider's minimum is at the bottom, so the passed part runs
                    // from the handle down to the end of the thumb's travel. The fill is a
                    // band on the value axis, centred in the control, because that is the line
                    // the handle moves along.
                    let thickness = dimensions::SLIDER_TRACK_HEIGHT.min(rect.width);
                    let band_x = rect.x + (rect.width.saturating_sub(thickness) / 2) as i32;
                    let bottom =
                        (rect.y + rect.height as i32 - (SLIDER_SIZE / 2.0) as i32).max(track.y);
                    let top = (slider_pos as i32).max(track.y).min(bottom);
                    if bottom > top {
                        context.fill_rect(
                            Rect::from_f32(
                                band_x as f32,
                                top as f32,
                                thickness as f32,
                                (bottom - top) as f32,
                            ),
                            track_fill,
                        );
                    }
                }
                // Draw slider handle: mirrored on the block axis, again centred in the
                // control's own box rather than filling it.
                let handle_centre =
                    Point::from_f32(thumb.x as f32 + thumb.width as f32 / 2.0, slider_pos);
                if let Some(halo) = halo_color {
                    context.fill_circle(handle_centre, handle_radius + HALO_EXTRA_RADIUS, halo);
                }
                context.fill_circle(handle_centre, handle_radius, handle_color);
                // Draw handle border
                let handle_ring =
                    style.border_color.filter(|c| *c != handle_color).unwrap_or(handle_border);
                context.draw_circle_stroke(handle_centre, handle_radius, handle_ring, 1);
                // Draw ticks if enabled (capped at 100 ticks max to avoid
                // performance issues with large ranges and small intervals).
                if self.tick_position != TickPosition::NoTicks && self.tick_interval > 0 {
                    let tick_width = dimensions::SLIDER_TRACK_HEIGHT * 2;
                    let tick_count = self.tick_count();
                    // Resolve the label face once rather than on every tick.
                    let tick_font = crate::widget::metrics::effective_font(style);
                    for i in 0..tick_count {
                        let value = self.tick_value(i);
                        let tick_y = self.value_to_pixel_pos(value);
                        if self.tick_position == TickPosition::TicksAbove
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(track.x as f32, tick_y),
                                Point::from_f32(track.x as f32 + tick_width as f32, tick_y),
                                tick_color,
                            );
                        }
                        if self.tick_position == TickPosition::TicksBelow
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(
                                    track.x as f32 + track.width as f32 - tick_width as f32,
                                    tick_y,
                                ),
                                Point::from_f32(track.x as f32 + track.width as f32, tick_y),
                                tick_color,
                            );
                        }
                        if self.tick_labels {
                            if let Some(label) = self.tick_label_cache.labels.get(i as usize) {
                                let label_x = track.x + tick_width as i32 + 2;
                                let label_y = tick_y as i32 - label.height as i32 / 2;
                                context.draw_text(
                                    Point::new(label_x, label_y),
                                    label.text.as_str(),
                                    tick_font,
                                    tick_color,
                                    HorizontalAlignment::Left,
                                );
                            }
                        }
                    }
                }
            }
        }

        // ── Keyboard focus ring ──
        //
        // The handle is the part the arrow keys move, so the ring frames it rather than the
        // whole groove. Gated on the *reason* focus arrived: a pointer press focuses the
        // slider without painting a ring under the cursor, while Tab and Shortcut draw one.
        if self.base.draws_focus_ring() {
            let ring = FocusRing::for_control(self.handle_rect(), (SLIDER_SIZE / 2.0) as u32);
            if ring.is_drawable() {
                // The fallback is the handle's contrasting tone; an active theme overrides
                // it with its declared `outline` token.
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(handle_color.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Orientation, Rect, Size};
    use crate::event::Event;
    use crate::style::WidgetStyle;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Helper to create a default slider with a standard geometry.
    fn make_slider() -> Slider {
        Slider::new(Rect::new(0, 0, 200, 30))
    }

    /// `set` must clamp an out-of-range integer instead of wrapping it with `as`.
    ///
    /// Before the coercion helper, `set("minimum", Int(1 << 40))` narrowed through
    /// `as i32` to `0`, injecting a value unrelated to the request and potentially
    /// inverting `minimum <= maximum`. The saturating helper keeps the magnitude
    /// and the existing range invariant.
    #[test]
    fn set_clamps_an_out_of_range_integer_instead_of_wrapping() {
        let mut slider = make_slider();
        slider.set("minimum", CapabilityValue::Int(1 << 40)).expect("an integer is accepted");
        assert_eq!(slider.minimum(), i32::MAX);
        assert!(slider.minimum() <= slider.maximum());

        slider.set("value", CapabilityValue::Int(-(1 << 40))).expect("an integer is accepted");
        assert_eq!(slider.value(), slider.minimum());
    }

    // ── 0. Writing direction (BLUE21 P2-10) ──

    /// The default slider is left-to-right, so nothing about it changed.
    ///
    /// This is the assertion that makes the direction field safe to add: every existing caller
    /// keeps the exact behaviour it had, and a control only becomes direction-aware by asking.
    #[test]
    fn a_slider_defaults_to_left_to_right() {
        let s = make_slider();
        assert_eq!(s.direction(), crate::core::TextDirection::LeftToRight);
    }

    /// A right-to-left slider puts its minimum at the right edge and its maximum at the left.
    ///
    /// # The defect this pins
    ///
    /// Value and position were mapped by one identity, so in a right-to-left interface the handle
    /// moved the *opposite* way from the value it reported: dragging toward the right lowered the
    /// value. That is not a translation gap — the control was functionally wrong, which is why the
    /// plan singled RTL out as the one functional defect among the appearance findings.
    ///
    /// The assertion is deliberately about *opposition*, not about a specific pixel: what makes the
    /// control correct is that the two directions disagree about which end is the beginning.
    #[test]
    fn a_right_to_left_slider_maps_its_value_the_other_way_round() {
        let mut ltr = make_slider();
        ltr.set_range(0, 100);
        ltr.set_value(0);
        let ltr_min_x = ltr.handle_centre_x();
        ltr.set_value(100);
        let ltr_max_x = ltr.handle_centre_x();
        assert!(ltr_min_x < ltr_max_x, "left-to-right runs minimum\u{2192}maximum left to right");

        let mut rtl = make_slider();
        rtl.set_range(0, 100);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        rtl.set_value(0);
        let rtl_min_x = rtl.handle_centre_x();
        rtl.set_value(100);
        let rtl_max_x = rtl.handle_centre_x();
        assert!(rtl_min_x > rtl_max_x, "right-to-left runs minimum\u{2192}maximum right to left");

        // And the two are exact mirrors of each other, which is stronger than mere opposition:
        // an implementation that merely swapped the ends would still get the travel wrong.
        let width = 200.0;
        assert!(
            ((ltr_min_x + rtl_min_x) - width).abs() < 1.0,
            "the minimum sits at mirrored positions: {ltr_min_x} vs {rtl_min_x}"
        );
        assert!(
            ((ltr_max_x + rtl_max_x) - width).abs() < 1.0,
            "the maximum sits at mirrored positions: {ltr_max_x} vs {rtl_max_x}"
        );
    }

    /// The drawn handle and the value read from a position are inverses in both directions.
    ///
    /// This is the property that keeps a drag from drifting away from the pointer: `draw` places
    /// the handle with one conversion and the drag handler reads a value back with the other. If
    /// they disagreed, dragging under RTL would walk the handle out from under the pointer — a bug
    /// that only appears in one direction and would be very hard to attribute later.
    #[test]
    fn the_two_directions_round_trip_a_value_through_a_position() {
        for direction in
            [crate::core::TextDirection::LeftToRight, crate::core::TextDirection::RightToLeft]
        {
            let mut s = make_slider();
            s.set_range(0, 100);
            s.set_direction(direction);
            for value in [0, 1, 25, 50, 75, 99, 100] {
                s.set_value(value);
                let x = s.handle_centre_x();
                let read_back = s.value_at_x(x);
                assert!(
                    (read_back - value).abs() <= 1,
                    "{direction:?}: value {value} at x={x} read back as {read_back}"
                );
            }
        }
    }

    /// A horizontal right-to-left slider steps toward the maximum on the left arrow.
    ///
    /// The arrow keys name a screen direction, and the value runs in reading order; those coincide
    /// in LTR and are opposite in RTL. The defect the plan records is precisely that the arrow arms
    /// did not consult direction at all, so an RTL user pressing left moved the value the way an LTR
    /// user would.
    #[test]
    fn the_left_arrow_steps_toward_the_maximum_in_a_right_to_left_slider() {
        // Left-to-right: the left arrow lowers the value.
        let mut ltr = make_slider();
        ltr.set_range(0, 100);
        ltr.set_value(50);
        ltr.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(ltr.value(), 49, "left arrow decrements in a left-to-right slider");

        // Right-to-left: the left arrow *raises* it, because left is further along the line.
        let mut rtl = make_slider();
        rtl.set_range(0, 100);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        rtl.set_value(50);
        rtl.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(rtl.value(), 51, "left arrow increments in a right-to-left slider");

        // The right arrow is the mirror, so both directions are covered.
        rtl.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(rtl.value(), 50, "right arrow decrements in a right-to-left slider");
    }

    /// A vertical slider is not mirrored, because the block axis does not flip with direction.
    ///
    /// Vertical sliders run top-to-bottom in every writing direction; applying the direction to the
    /// vertical axis would be a second defect rather than a fix for the first.
    #[test]
    fn a_vertical_slider_ignores_the_writing_direction() {
        let mut ltr = Slider::new(Rect::new(0, 0, 30, 200));
        ltr.set_orientation(Orientation::Vertical);
        ltr.set_range(0, 100);
        ltr.set_value(0);
        let ltr_min_y = ltr.handle_centre_y();
        ltr.set_value(100);
        let ltr_max_y = ltr.handle_centre_y();

        let mut rtl = Slider::new(Rect::new(0, 0, 30, 200));
        rtl.set_orientation(Orientation::Vertical);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        rtl.set_range(0, 100);
        rtl.set_value(0);
        let rtl_min_y = rtl.handle_centre_y();
        rtl.set_value(100);
        let rtl_max_y = rtl.handle_centre_y();

        assert_eq!(ltr_min_y, rtl_min_y, "the vertical axis is not mirrored");
        assert_eq!(ltr_max_y, rtl_max_y, "the vertical axis is not mirrored");
    }

    // ── 1. Creation defaults ──

    #[test]
    fn default_range_is_0_to_100() {
        let s = make_slider();
        assert_eq!(s.minimum(), 0);
        assert_eq!(s.maximum(), 100);
    }

    #[test]
    fn default_value_is_0() {
        let s = make_slider();
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn default_orientation_horizontal() {
        let s = make_slider();
        assert_eq!(s.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn default_tracking_is_true() {
        let s = make_slider();
        assert!(s.tracking());
    }

    #[test]
    fn default_tick_position_is_no_ticks() {
        let s = make_slider();
        assert_eq!(s.tick_position(), TickPosition::NoTicks);
    }

    #[test]
    fn default_single_step_is_1() {
        let s = make_slider();
        assert_eq!(s.single_step(), 1);
    }

    #[test]
    fn default_page_step_is_10() {
        let s = make_slider();
        assert_eq!(s.page_step(), 10);
    }

    // ── 2. set_value / clamping ──

    #[test]
    fn set_value_normal() {
        let mut s = make_slider();
        s.set_value(42);
        assert_eq!(s.value(), 42);
    }

    #[test]
    fn set_value_clamps_below_minimum() {
        let mut s = make_slider();
        s.set_value(-10);
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn set_value_clamps_above_maximum() {
        let mut s = make_slider();
        s.set_value(200);
        assert_eq!(s.value(), 100);
    }

    #[test]
    fn set_value_same_value_does_not_emit() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_value(0);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn set_value_emits_on_change() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_value(55);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // ── 3. set_minimum / set_maximum / set_range ──

    #[test]
    fn set_minimum_adjusts_maximum_when_crossed() {
        let mut s = make_slider();
        s.set_minimum(150);
        assert_eq!(s.minimum(), 150);
        assert_eq!(s.maximum(), 150);
    }

    #[test]
    fn set_minimum_reclamps_value() {
        let mut s = make_slider();
        s.set_value(50);
        s.set_minimum(60);
        assert_eq!(s.value(), 60);
    }

    #[test]
    fn set_maximum_adjusts_minimum_when_crossed() {
        let mut s = make_slider();
        s.set_maximum(-20);
        assert_eq!(s.maximum(), -20);
        assert_eq!(s.minimum(), -20);
    }

    #[test]
    fn set_maximum_reclamps_value() {
        let mut s = make_slider();
        s.set_value(80);
        s.set_maximum(40);
        assert_eq!(s.value(), 40);
    }

    #[test]
    fn set_range_clamps_maximum_not_below_minimum() {
        let mut s = make_slider();
        s.set_range(10, 5);
        assert_eq!(s.minimum(), 10);
        assert_eq!(s.maximum(), 10);
    }

    #[test]
    fn set_range_reclamps_value() {
        let mut s = make_slider();
        s.set_value(50);
        s.set_range(0, 30);
        assert_eq!(s.value(), 30);
    }

    // ── 4. value_changed signal emission ──

    #[test]
    fn value_changed_emits_new_value() {
        let mut s = make_slider();
        let emitted = Arc::new(AtomicUsize::new(0));
        let e = Arc::clone(&emitted);
        s.value_changed.connect(move |v| {
            e.store(*v as usize, Ordering::SeqCst);
        });
        s.set_value(77);
        assert_eq!(emitted.load(Ordering::SeqCst), 77);
    }

    #[test]
    fn value_changed_not_emitted_on_noop() {
        let mut s = make_slider();
        s.set_value(33);
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_value(33);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    // ── 5. slider_moved, slider_pressed, slider_released signals ──

    #[test]
    fn slider_moved_emitted_on_set_slider_position() {
        let mut s = make_slider();
        let emitted = Arc::new(AtomicUsize::new(999));
        let e = Arc::clone(&emitted);
        s.slider_moved.connect(move |v| {
            e.store(*v as usize, Ordering::SeqCst);
        });
        s.set_slider_position(30);
        assert_eq!(emitted.load(Ordering::SeqCst), 30);
    }

    #[test]
    fn slider_pressed_emitted_on_mouse_press() {
        let mut s = make_slider();
        let pressed = Arc::new(AtomicUsize::new(0));
        let p = Arc::clone(&pressed);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.slider_pressed.connect(move || {
            p.fetch_add(1, Ordering::SeqCst);
        });
        s.handle_event(&Event::mouse_press(50, 15, 1));
        assert_eq!(pressed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn slider_released_emitted_on_mouse_release() {
        let mut s = make_slider();
        let released = Arc::new(AtomicUsize::new(0));
        let r = Arc::clone(&released);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.slider_released.connect(move || {
            r.fetch_add(1, Ordering::SeqCst);
        });
        s.handle_event(&Event::mouse_press(50, 15, 1));
        s.handle_event(&Event::mouse_release(50, 15, 1));
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn slider_moved_during_drag() {
        let mut s = make_slider();
        let moved = Arc::new(AtomicUsize::new(999));
        let m = Arc::clone(&moved);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.slider_moved.connect(move |v| {
            m.store(*v as usize, Ordering::SeqCst);
        });
        s.handle_event(&Event::mouse_press(10, 15, 1));
        s.handle_event(&Event::mouse_move(100, 15));
        // After a drag the slider_moved should have been emitted.
        assert_ne!(moved.load(Ordering::SeqCst), 999);
    }

    // ── 6. single_step / page_step set/get (floor at 1) ──

    #[test]
    fn single_step_floors_at_1() {
        let mut s = make_slider();
        s.set_single_step(0);
        assert_eq!(s.single_step(), 1);
        s.set_single_step(-5);
        assert_eq!(s.single_step(), 1);
    }

    #[test]
    fn single_step_normal() {
        let mut s = make_slider();
        s.set_single_step(5);
        assert_eq!(s.single_step(), 5);
    }

    #[test]
    fn page_step_floors_at_1() {
        let mut s = make_slider();
        s.set_page_step(0);
        assert_eq!(s.page_step(), 1);
        s.set_page_step(-3);
        assert_eq!(s.page_step(), 1);
    }

    #[test]
    fn page_step_normal() {
        let mut s = make_slider();
        s.set_page_step(15);
        assert_eq!(s.page_step(), 15);
    }

    // ── 7. orientation set/get ──

    #[test]
    fn set_orientation_vertical() {
        let mut s = make_slider();
        s.set_orientation(Orientation::Vertical);
        assert_eq!(s.orientation(), Orientation::Vertical);
    }

    #[test]
    fn set_orientation_horizontal() {
        let mut s = make_slider();
        s.set_orientation(Orientation::Vertical);
        s.set_orientation(Orientation::Horizontal);
        assert_eq!(s.orientation(), Orientation::Horizontal);
    }

    // ── 8. trigger_action for all 8 SliderAction variants ──

    #[test]
    fn trigger_action_single_step_add() {
        let mut s = make_slider();
        s.set_value(5);
        s.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(s.value(), 6);
    }

    #[test]
    fn trigger_action_single_step_sub() {
        let mut s = make_slider();
        s.set_value(5);
        s.trigger_action(SliderAction::SliderSingleStepSub);
        assert_eq!(s.value(), 4);
    }

    #[test]
    fn trigger_action_page_step_add() {
        let mut s = make_slider();
        s.set_value(5);
        s.trigger_action(SliderAction::SliderPageStepAdd);
        assert_eq!(s.value(), 15);
    }

    #[test]
    fn trigger_action_page_step_sub() {
        let mut s = make_slider();
        s.set_value(50);
        s.trigger_action(SliderAction::SliderPageStepSub);
        assert_eq!(s.value(), 40);
    }

    #[test]
    fn trigger_action_to_minimum() {
        let mut s = make_slider();
        s.set_value(80);
        s.trigger_action(SliderAction::SliderToMinimum);
        assert_eq!(s.value(), s.minimum());
    }

    #[test]
    fn trigger_action_to_maximum() {
        let mut s = make_slider();
        s.set_value(10);
        s.trigger_action(SliderAction::SliderToMaximum);
        assert_eq!(s.value(), s.maximum());
    }

    #[test]
    fn trigger_action_slider_move_no_op() {
        let mut s = make_slider();
        s.set_value(50);
        s.trigger_action(SliderAction::SliderMove);
        // SliderMove is a no-op handled by mouse events; value unchanged.
        assert_eq!(s.value(), 50);
    }

    #[test]
    fn trigger_action_single_step_sub_clamps_to_minimum() {
        let mut s = make_slider();
        s.set_value(0);
        s.trigger_action(SliderAction::SliderSingleStepSub);
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn trigger_action_single_step_add_clamps_to_maximum() {
        let mut s = make_slider();
        s.set_value(100);
        s.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(s.value(), 100);
    }

    #[test]
    fn trigger_action_page_step_sub_clamps_to_minimum() {
        let mut s = make_slider();
        s.set_value(3);
        s.trigger_action(SliderAction::SliderPageStepSub);
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn trigger_action_page_step_add_clamps_to_maximum() {
        let mut s = make_slider();
        s.set_value(95);
        s.trigger_action(SliderAction::SliderPageStepAdd);
        assert_eq!(s.value(), 100);
    }

    // ── 9. set_slider_position with tracking=true/false ──

    #[test]
    fn set_slider_position_with_tracking_emits_value_changed() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_slider_position(60);
        // tracking=true by default → set_value called → value_changed emitted
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(s.value(), 60);
    }

    #[test]
    fn set_slider_position_without_tracking_does_not_emit_value_changed() {
        let mut s = make_slider();
        s.set_tracking(false);
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_slider_position(60);
        // tracking=false → no set_value → no value_changed
        assert_eq!(count.load(Ordering::SeqCst), 0);
        // But value is NOT updated; only slider_position is set
        assert_eq!(s.value(), 0);
        assert_eq!(s.slider_position(), 60);
    }

    #[test]
    fn set_slider_position_clamps_to_range() {
        let mut s = make_slider();
        s.set_slider_position(999);
        assert_eq!(s.slider_position(), 100);
        s.set_slider_position(-50);
        assert_eq!(s.slider_position(), 0);
    }

    #[test]
    fn set_slider_position_without_tracking_applies_on_release() {
        let mut s = make_slider();
        s.set_tracking(false);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.handle_event(&Event::mouse_press(80, 15, 1));
        s.handle_event(&Event::mouse_release(80, 15, 1));
        // On release, if !tracking, set_value is called with slider_position.
        //
        // The expected value is `(80 - inset) / travel * 100` = `72 / 184 * 100` = 39.1 -> 39. It
        // used to be asserted as `80 / 200 * 100 = 40`, because the read direction divided by the
        // full width while the draw direction inset by half a handle. That made the control answer a
        // value the handle was not showing, and the two directions were not inverses; the round-trip
        // test above is what caught it, and this expectation moved with the fix rather than the fix
        // being shaped to keep a wrong number.
        assert_eq!(s.value(), 39);
    }

    // ── 10. Widget trait delegation ──

    #[test]
    fn widget_id_delegation() {
        let s = make_slider();
        // id is always non-zero for a fresh BaseWidget
        assert_ne!(s.id(), 0u64);
    }

    #[test]
    fn widget_kind_is_slider() {
        let s = make_slider();
        assert_eq!(s.kind(), WidgetKind::Slider);
    }

    #[test]
    fn widget_geometry_roundtrip() {
        let mut s = make_slider();
        let new_rect = Rect::new(10, 20, 300, 50);
        s.set_geometry(new_rect);
        assert_eq!(s.geometry(), new_rect);
    }

    #[test]
    fn widget_visibility() {
        let mut s = make_slider();
        assert!(s.is_visible());
        s.hide();
        assert!(!s.is_visible());
        s.show();
        assert!(s.is_visible());
    }

    #[test]
    fn widget_enabled() {
        let mut s = make_slider();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
        s.set_enabled(true);
        assert!(s.is_enabled());
    }

    #[test]
    fn widget_parent_roundtrip() {
        let mut s = make_slider();
        let parent = 42u64;
        s.set_parent(Some(parent));
        assert_eq!(s.parent(), Some(parent));
        s.set_parent(None);
        assert_eq!(s.parent(), None);
    }

    #[test]
    fn widget_children_delegation() {
        let mut s = make_slider();
        let child = 99u64;
        assert!(s.children().is_empty());
        s.add_child(child);
        assert_eq!(s.children().len(), 1);
        assert_eq!(s.children()[0], child);
        s.remove_child(child);
        assert!(s.children().is_empty());
    }

    #[test]
    fn widget_min_max_size() {
        let mut s = make_slider();
        assert_eq!(s.min_size(), None);
        assert_eq!(s.max_size(), None);
        let min = Size::new(50, 20);
        let max = Size::new(500, 100);
        s.set_min_size(Some(min));
        s.set_max_size(Some(max));
        assert_eq!(s.min_size(), Some(min));
        assert_eq!(s.max_size(), Some(max));
    }

    #[test]
    fn widget_tooltip_roundtrip() {
        let mut s = make_slider();
        s.set_tooltip("Volume".to_string());
        assert_eq!(s.tooltip(), "Volume");
    }

    #[test]
    fn widget_style_roundtrip() {
        let mut s = make_slider();
        let style =
            WidgetStyle { background_color: Some(Color::rgb(255, 0, 0)), ..WidgetStyle::default() };
        s.set_style(style.clone());
        assert_eq!(s.style().background_color, Some(Color::rgb(255, 0, 0)));
    }

    #[test]
    fn a_drag_that_leaves_the_control_stops_tracking_the_pointer() {
        // A press whose release lands outside the widget never delivers `MouseRelease`:
        // the runtime's hit-test answers `None` for a point outside every control. With
        // no `MouseLeave` arm `mouse_pressed` stayed set, so every later *hover* moved
        // the handle with no button held — the slider behaved as if permanently
        // dragging.
        let mut s = make_slider();
        s.set_range(0, 100);
        s.set_value(50);

        s.handle_event(&Event::MousePress { pos: Point::new(100, 15), button: 1, modifiers: 0 });
        // The pointer leaves without a release being delivered.
        s.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });

        let after_leave = s.value();
        // Hover moves must not change anything now: no button is held.
        for x in [20, 40, 60, 160, 190] {
            s.handle_event(&Event::MouseMove { pos: Point::new(x, 15) });
        }
        assert_eq!(
            s.value(),
            after_leave,
            "hovering after the pointer left must not move the handle"
        );
    }

    #[test]
    fn widget_signals_accessible() {
        let s = make_slider();
        // All signal accessors should return valid signal references without panicking.
        let _ = s.hover_signal();
        let _ = s.mouse_down_signal();
        let _ = s.mouse_up_signal();
        let _ = s.key_down_signal();
        let _ = s.key_up_signal();
        let _ = s.focus_gained_signal();
        let _ = s.focus_lost_signal();
        let _ = s.redraw_requested_signal();
        let _ = s.layout_requested_signal();
        let _ = s.connection_scope();
    }

    #[test]
    fn widget_signal_emission_hover() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.base.hover_signal().connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        // The base widget's handle_event emits hover_signal on MouseMove.
        s.base.handle_event(&Event::mouse_move(10, 5));
        // One emission from the event handler.
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // ── 12. Edge cases ─────────────────────────────────────────────────
    #[test]
    fn test_slider_negative_range() {
        // Set range with min > max — should clamp max to min
        let mut s = make_slider();
        s.set_range(50, -50);
        assert_eq!(s.minimum(), 50);
        assert_eq!(s.maximum(), 50, "max should be clamped to min when max < min");
        // Value should be reclamped to valid range
        assert_eq!(s.value(), 50);

        // Setting minimum above maximum directly — max follows
        let mut s2 = make_slider();
        s2.set_minimum(200);
        assert_eq!(s2.minimum(), 200);
        assert_eq!(s2.maximum(), 200, "max should be raised to match min");

        // Setting maximum below minimum directly — min follows
        let mut s3 = make_slider();
        s3.set_maximum(-100);
        assert_eq!(s3.maximum(), -100);
        assert_eq!(s3.minimum(), -100, "min should be lowered to match max");
    }

    #[test]
    fn test_slider_step_larger_than_range() {
        let mut s = make_slider();
        // Range is 0–100, set step to 200 (> range width)
        s.set_single_step(200);
        assert_eq!(s.single_step(), 200);

        // From value=5, step sub should clamp to minimum
        s.set_value(5);
        s.trigger_action(SliderAction::SliderSingleStepSub);
        // 5 - 200 = -195, clamped to 0
        assert_eq!(s.value(), 0, "step sub should clamp to minimum when step > range");

        // From value=50, step add should clamp to maximum
        s.set_value(50);
        s.trigger_action(SliderAction::SliderSingleStepAdd);
        // 50 + 200 = 250, clamped to 100
        assert_eq!(s.value(), 100, "step add should clamp to maximum when step > range");

        // Narrow range (0–1) with step=5
        let mut s2 = make_slider();
        s2.set_range(0, 1);
        s2.set_single_step(5);
        s2.trigger_action(SliderAction::SliderSingleStepSub);
        assert_eq!(s2.value(), 0);

        s2.set_value(1);
        s2.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(s2.value(), 1);
    }

    /// BLUE23 Appendix A.4 / M1: hovering or dragging a slider is **visible**, and a drag is the
    /// firmer of the two gestures.
    ///
    /// # The defect this pins
    ///
    /// The control maintained a private `mouse_pressed` that `draw` never read (BLUE21 AR6). A
    /// slider could be dragged with no confirmation it was being dragged, and hovering it did
    /// nothing at all. `BaseWidget` already keeps both facts, which is the whole point of M1.
    ///
    /// The observable is the halo's **colour**: it is a step from the handle toward the window
    /// fill, so at rest there is no halo at all, hovered it is a faint step, and dragged a firmer
    /// one. Asking for the colour rather than counting shapes keeps the assertion about what the
    /// user sees rather than about which SVG element the backend picked.
    #[test]
    #[cfg(device_profile)]
    fn hovering_and_dragging_step_the_halo_by_different_amounts() {
        // The halo is computed against the **active theme's** surface, and the theme registry is
        // process-wide, so this test has to serialise against every other test that switches the
        // appearance. Without the guard and the pin it read its backdrop from whichever theme
        // happened to be active mid-run: measured, it failed 5 runs out of 5 when another test in
        // the same binary had switched to dark, and passed when run alone. The assertion is about a
        // *relation between the two states*, so both colours and the fill must come from one theme.
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);

        let inside = crate::core::Point::new(100, 15);
        let mut s = make_slider();

        // At rest the handle is the only disc, so the halo colour is absent.
        assert_eq!(halo_colour(&mut s), None, "a resting slider has no halo");

        s.handle_event(&Event::MouseEnter { pos: inside });
        let hovered = halo_colour(&mut s).expect("a hovered slider shows a halo");

        s.handle_event(&Event::MousePress { pos: inside, button: 1, modifiers: 0 });
        let dragged = halo_colour(&mut s).expect("a dragging slider shows a halo");
        // A drag must be the **firmer** gesture, not merely a different one: the halo is a step
        // from the handle toward the window fill, so "firmer" is "closer to the fill". A bare
        // `assert_ne!` passed when the two weights were swapped — measured — because swapping them
        // still yields two different colours. Stating the direction is what pins the relation.
        let fill = window_fill_of(&mut s);
        let dist = |c: Color| {
            (c.r as i32 - fill.r as i32).abs()
                + (c.g as i32 - fill.g as i32).abs()
                + (c.b as i32 - fill.b as i32).abs()
        };
        assert!(
            dist(dragged) < dist(hovered),
            "a drag must step further toward the surface than a hover: dragged {dragged:?} is {} \
             away from {fill:?}, hovered {hovered:?} is {}",
            dist(dragged),
            dist(hovered)
        );

        // Releasing returns to the hover strength rather than dropping straight to nothing — the
        // pointer is still over the control, and that is still a state.
        s.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        assert_eq!(
            halo_colour(&mut s),
            Some(hovered),
            "releasing leaves the pointer hovering, so the hover halo returns"
        );

        s.handle_event(&Event::MouseLeave { pos: inside });
        assert_eq!(halo_colour(&mut s), None, "leaving must remove the halo, not latch it");
    }

    /// The window fill the halo is stepped *toward* — the control's own backdrop.
    ///
    /// Read from the first `fill="rgba(` in the document, which is the backdrop the exporter
    /// composites over. Named separately because "firmer" is only meaningful relative to it.
    ///
    /// Gated with its only caller: the halo test needs a theme, which a profile without a device
    /// profile does not have, so the test and its helpers are compiled out together.
    #[cfg(device_profile)]
    fn window_fill_of(s: &mut Slider) -> Color {
        let svg = crate::widget::svg::render_widget_to_svg(s, Rect::new(0, 0, 200, 30));
        let at = svg.find("fill=\"rgba(").expect("a backdrop") + "fill=\"rgba(".len();
        let end = svg[at..].find(')').expect("the backdrop's close") + at;
        let mut parts = svg[at..end].split(',');
        let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
        let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
        let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
        Color::rgb(r, g, b)
    }

    /// The halo's colour, or `None` when the control paints only its handle.
    ///
    /// # How the halo is told apart from the handle
    ///
    /// The document always holds two circles: the handle's **fill** (`r=10`, a colour) and the
    /// handle's **outline** (`fill="none"`). A halo is a third, and it is painted before the
    /// handle, so "more than one *filled* circle" is the test — the outline is filtered out by its
    /// `fill="none"` rather than by position, which would break the moment a tick mark moved.
    ///
    /// Returning `Option` rather than a count is what lets "no halo" and "a halo the same colour
    /// as the handle" be told apart: the second is a halo the user cannot see, which is the defect.
    ///
    /// Gated with its only caller; see [`window_fill_of`].
    #[cfg(device_profile)]
    fn halo_colour(s: &mut Slider) -> Option<Color> {
        let svg = crate::widget::svg::render_widget_to_svg(s, Rect::new(0, 0, 200, 30));
        let filled: crate::compat::Vec<&str> = svg
            .match_indices("<circle")
            .map(|(i, _)| &svg[i..])
            .filter(|c| c.contains("fill=\"rgba("))
            .collect();
        // One filled circle = the handle alone. Two = a halo, then the handle.
        let halo = filled.first().filter(|_| filled.len() > 1)?;
        let at = halo.find("fill=\"rgba(")? + "fill=\"rgba(".len();
        let end = halo[at..].find(')')? + at;
        let mut parts = halo[at..end].split(',');
        let r = parts.next()?.trim().parse().ok()?;
        let g = parts.next()?.trim().parse().ok()?;
        let b = parts.next()?.trim().parse().ok()?;
        Some(Color::rgb(r, g, b))
    }

    /// N-CH-02: a step action at the `i32` extreme saturates instead of overflowing.
    ///
    /// `set_value` clamps, but the old `self.value + self.single_step` overflowed *before* the clamp
    /// ran, so a slider already at `i32::MAX`/`i32::MIN` (or with a large step) wrapped to the
    /// opposite end instead of stopping at the bound.
    #[test]
    fn step_actions_saturate_at_the_domain_extremes() {
        let mut slider = make_slider();
        slider.set_range(i32::MIN, i32::MAX);
        slider.set_single_step(1);

        slider.set_value(i32::MAX);
        slider.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(slider.value(), i32::MAX, "stepping up at the maximum stays there");

        slider.set_value(i32::MIN);
        slider.trigger_action(SliderAction::SliderSingleStepSub);
        assert_eq!(slider.value(), i32::MIN, "stepping down at the minimum stays there");

        // A large positive step from a positive value saturates rather than wrapping.
        slider.set_single_step(i32::MAX);
        slider.set_value(1);
        slider.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(
            slider.value(),
            i32::MAX,
            "a step that would overflow must saturate at the maximum, not wrap negative"
        );
    }

    // ── D09-CTRL-01: the value↔pixel mappings and the tick count must not overflow `i32` ──

    /// A full-`i32` range must map value↔pixel without overflowing, in both orientations.
    ///
    /// # The defect this pins
    ///
    /// `pixel_pos_to_value` computed `(maximum - minimum) as f32` and `value_to_pixel_pos` computed
    /// `(clamped - minimum) as f32` in `i32`. For a legal `i32::MIN..=i32::MAX` range the span has no
    /// `i32` representation, so a checked build panicked and a wrapping one produced a wrong — even
    /// negative — span. Both now compute in `f64`, so this test reaching its assertions at all is the
    /// regression check for the panic, and the round trip pins the correct result.
    #[test]
    fn a_full_range_maps_between_value_and_pixel_in_both_orientations() {
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            let mut slider = make_slider();
            slider.set_orientation(orientation);
            slider.set_range(i32::MIN, i32::MAX);

            // Endpoints and midpoints map to finite pixels and stay inside the domain. The exact
            // `i32` value cannot survive a round trip over a 2^32-wide span sampled by ~180 pixels
            // (one pixel is ~12 million values), so the meaningful inverse is checked on the
            // **pixel**: a value mapped to a pixel must map back to a value on the same pixel.
            for value in [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX] {
                let pixel = slider.value_to_pixel_pos(value);
                assert!(
                    pixel.is_finite(),
                    "{orientation:?}: value {value} must map to a finite pixel"
                );
                let back = slider.pixel_pos_to_value(pixel);
                let recovered_pixel = slider.value_to_pixel_pos(back);
                assert!(
                    (recovered_pixel - pixel).abs() <= 1.0,
                    "{orientation:?}: value {value} -> pixel {pixel} -> value {back} -> pixel \
                     {recovered_pixel}, the pixel must be recovered"
                );
            }

            // The extremes land at the two ends of the travel, and the minimum is below the maximum
            // in pixel order (a wrapped span would invert this).
            let min_px = slider.value_to_pixel_pos(i32::MIN);
            let max_px = slider.value_to_pixel_pos(i32::MAX);
            assert!(min_px.is_finite() && max_px.is_finite());
            match orientation {
                Orientation::Horizontal => assert!(min_px < max_px),
                Orientation::Vertical => assert!(max_px < min_px, "the maximum sits on top"),
            }
        }
    }

    /// A full-range slider with ticks must not panic and must cap the drawn ticks at 100.
    ///
    /// The old tick count computed `(maximum - minimum) / tick_interval + 1` in `i32`, which
    /// overflowed for the full-`i32` span *before* the `.min(100)` cap could bound it. The cap is now
    /// applied to a span computed in `i64`.
    #[test]
    fn a_full_range_tick_count_is_capped_and_does_not_panic() {
        let mut slider = make_slider();
        slider.set_range(i32::MIN, i32::MAX);
        slider.set_tick_interval(1);
        assert_eq!(slider.tick_count(), 100, "the tick count is capped at 100");

        // A coarse interval over the full span is likewise well-defined and in range.
        slider.set_tick_interval(100_000_000);
        let count = slider.tick_count();
        assert!((1..=100).contains(&count), "a coarse interval yields 1..=100 ticks, got {count}");

        // The full-span ticks are monotonic: `tick_value` walks `minimum + index * interval` in
        // `i64` and clamps at `maximum`, so a wrapped/overflowing intermediate would invert the
        // order. This is the meaningful domain property; an `i32` value cannot be out of `i32`
        // range, so a `MIN..=MAX` check would be vacuous.
        slider.set_tick_interval(i32::MAX);
        let mut previous: Option<i32> = None;
        for index in 0..slider.tick_count() {
            let value = slider.tick_value(index);
            if let Some(previous) = previous {
                assert!(
                    value >= previous,
                    "tick {index} ({value}) must not precede tick {} ({previous})",
                    index - 1
                );
            }
            previous = Some(value);
        }
        // A zero or negative interval yields no ticks (rather than dividing by zero).
        slider.set_tick_interval(0);
        assert_eq!(slider.tick_count(), 0);
    }

    /// A full-range slider with ticks enabled must actually paint without panicking, in both
    /// orientations — the drawing path is the one the report is about.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn a_full_range_slider_paints_its_ticks_without_panicking() {
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            let mut slider = make_slider();
            slider.set_orientation(orientation);
            slider.set_range(i32::MIN, i32::MAX);
            slider.set_value(0);
            slider.set_tick_interval(1_000_000_000);
            slider.set_tick_position(TickPosition::TicksBothSides);
            let svg =
                crate::widget::svg::render_widget_to_svg(&mut slider, Rect::new(0, 0, 200, 30));
            assert!(svg.starts_with("<svg"), "{orientation:?}: the draw must produce a document");
        }
    }

    /// Keyboard focus paints the ring on the handle; a pointer press does not.
    ///
    /// The slider reported `WidgetState::Focused` for keyboard focus but painted no ring, so
    /// the state had no visible consumer. The ring is the only 2 px stroke the control emits,
    /// which makes its presence observable in the render.
    #[test]
    fn a_keyboard_focused_slider_paints_the_ring_and_pointer_focus_does_not() {
        // The ring's colour reads the process-wide theme; hold the guard so a concurrent
        // appearance switch cannot change this render mid-assertion.
        let _guard = crate::style::theme_test_guard();
        let mut keyboard = make_slider();
        keyboard.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        assert!(keyboard.base.draws_focus_ring(), "Tab focus must ask for a ring");
        let focused = crate::widget::svg::render_to_svg(&mut keyboard);
        assert!(
            focused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "keyboard focus must paint the ring around the handle: {focused}"
        );

        let mut pointer = make_slider();
        pointer.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Pointer });
        let unfocused = crate::widget::svg::render_to_svg(&mut pointer);
        assert!(
            !unfocused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "pointer focus must not paint a ring: {unfocused}"
        );
    }

    /// The ring follows the handle to both ends of the travel.
    #[test]
    fn the_slider_focus_ring_tracks_the_handle() {
        let mut slider = make_slider();
        slider.set_value(100);
        let at_maximum = slider.handle_rect();
        slider.set_value(0);
        let at_minimum = slider.handle_rect();
        assert!(
            at_maximum.x > at_minimum.x,
            "the ring must move with the value: {:?} vs {:?}",
            at_maximum,
            at_minimum
        );
        assert_eq!(at_maximum.y, at_minimum.y, "a horizontal slider's handle keeps one row");
    }

    /// The passed part of the groove is filled only when the caller asks for it.
    ///
    /// The highlight is opt-in so the historical groove — one flat bar — stays byte-identical.
    #[test]
    fn the_passed_groove_is_filled_only_when_asked() {
        let render = |slider: &mut Slider| -> String {
            crate::widget::svg::render_widget_to_svg(slider, Rect::new(0, 0, 200, 30))
        };
        let mut slider = make_slider();
        slider.set_value(50);
        assert!(!slider.track_highlight(), "no fill by default");
        let plain = render(&mut slider);

        slider.set_track_highlight(true);
        assert!(slider.track_highlight());
        let highlighted = render(&mut slider);
        assert_ne!(plain, highlighted, "the fill must reach the picture");

        // The fill is a second `<rect>` beyond the groove: the resting slider paints the
        // groove, the handle and the handle's outline, and nothing else rectangular.
        let rects = |svg: &str| svg.matches("<rect").count();
        assert!(
            rects(&highlighted) > rects(&plain),
            "the highlight is an extra filled rectangle: {plain} vs {highlighted}"
        );
    }

    /// The animated halo interpolates between levels instead of stepping.
    #[test]
    fn the_animated_halo_interpolates_then_settles() {
        let mut slider = make_slider();
        assert!(!slider.animated_halo(), "the discrete halo is the default");
        assert_eq!(slider.halo_strength(), 0.0);

        slider.set_animated_halo(true);
        assert!(slider.animated_halo());
        let inside = Point::new(100, 15);

        slider.handle_event(&Event::MouseEnter { pos: inside });
        assert!(slider.is_animating(), "arriving starts a halo transition");
        assert!(slider.tick(1), "the transition owes more frames");
        let partway = slider.halo_strength();
        assert!(
            partway > 0.0 && partway < HOVER_HALO_WEIGHT,
            "the halo passes through interior strengths: {partway}"
        );
        while slider.tick(1000) {}
        assert_eq!(slider.halo_strength(), HOVER_HALO_WEIGHT);
        assert!(!slider.is_animating(), "a settled halo owes no frames");

        // A press is the firmer gesture, and it eases up to that level rather than jumping.
        slider.handle_event(&Event::MousePress { pos: inside, button: 1, modifiers: 0 });
        assert!(slider.tick(1));
        let rising = slider.halo_strength();
        assert!(rising > HOVER_HALO_WEIGHT && rising < DRAG_HALO_WEIGHT);
        while slider.tick(1000) {}
        assert_eq!(slider.halo_strength(), DRAG_HALO_WEIGHT);
    }

    /// A wheel notch adjusts the value by one step and clamps at both ends.
    #[test]
    fn the_wheel_steps_the_value_by_one_step() {
        let mut slider = make_slider();
        slider.set_range(0, 10);
        slider.set_value(5);

        slider.handle_event(&Event::wheel(0, 1, 0));
        assert_eq!(slider.value(), 6);
        slider.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(slider.value(), 5);

        slider.set_single_step(3);
        slider.handle_event(&Event::wheel(0, 1, 0));
        assert_eq!(slider.value(), 8, "the wheel uses the configured step");

        slider.set_value(10);
        slider.handle_event(&Event::wheel(0, 1, 0));
        assert_eq!(slider.value(), 10, "the maximum clamps the wheel");
        slider.set_value(0);
        slider.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(slider.value(), 0, "the minimum clamps the wheel");
    }

    /// Tick labels put each major tick's value on the canvas, and only when asked.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn tick_labels_draw_the_numeric_value() {
        let mut slider = make_slider();
        slider.set_tick_position(TickPosition::TicksBelow);
        slider.set_tick_interval(25);
        assert!(!slider.tick_labels(), "labels are opt-in");

        let unlabelled =
            crate::widget::svg::render_widget_to_svg(&mut slider, Rect::new(0, 0, 200, 30));
        assert_eq!(crate::widget::svg::text_subpath_count(&unlabelled), 0);

        slider.set_tick_labels(true);
        let labelled =
            crate::widget::svg::render_widget_to_svg(&mut slider, Rect::new(0, 0, 200, 30));
        assert!(
            crate::widget::svg::text_subpath_count(&labelled) > 0,
            "each major tick must carry its value: {labelled}"
        );
    }

    // ── COMP-04: the slider ↔ spin box link ────────────────────────────────

    fn paired() -> (Slider, crate::widget::SpinBox) {
        let mut slider = Slider::new(Rect::new(0, 0, 200, 30));
        slider.set_range(0, 100);
        let mut spin = crate::widget::SpinBox::new(Rect::new(0, 0, 80, 24));
        spin.set_range(0, 100);
        (slider, spin)
    }

    /// The default direction is slider → spin; a spin-box edit stays local.
    #[test]
    fn a_one_way_link_drives_the_spin_box_only() {
        let (mut slider, mut spin) = paired();
        slider.bind_to_spin_box(&spin);
        assert!(slider.linked());
        assert_eq!(slider.link_mode(), LinkMode::OneWay);

        slider.set_value(40);
        assert_eq!(spin.value(), 40, "the slider drives the spin box");

        spin.set_value(70);
        assert_eq!(spin.value(), 70);
        assert_eq!(slider.value(), 40, "a one-way link does not carry spin edits back");
    }

    /// `TwoWay` carries both directions, including a typed spin-box value.
    #[test]
    fn a_two_way_link_carries_both_directions() {
        let (mut slider, mut spin) = paired();
        slider.bind_to_spin_box(&spin);
        slider.set_link_mode(LinkMode::TwoWay);

        slider.set_value(25);
        assert_eq!(spin.value(), 25);

        spin.set_value(75);
        assert_eq!(slider.value(), 75, "the spin box drives the slider too");
        assert_eq!(slider.link_mode(), LinkMode::TwoWay);
    }

    /// A decimal spin box crosses as the number it shows, then rounds for the i32 slider.
    #[test]
    fn a_decimal_spin_box_value_crosses_through_its_precision() {
        let (mut slider, mut spin) = paired();
        spin.set_decimals(1);
        spin.set_range_f64(0.0, 100.0);
        slider.bind_to_spin_box(&spin);
        slider.set_link_mode(LinkMode::TwoWay);

        spin.set_value_f64(2.4);
        assert_eq!(slider.value(), 2, "2.4 rounds down to the slider's grid");
        spin.set_value_f64(2.6);
        assert_eq!(slider.value(), 3, "2.6 rounds up");
        assert!((spin.value_f64() - 2.6).abs() < 1e-9, "the spin keeps its precision");
    }

    /// `unbind` stops the traffic in both directions.
    #[test]
    fn unbinding_stops_synchronisation() {
        let (mut slider, mut spin) = paired();
        slider.bind_to_spin_box(&spin);
        assert!(spin.is_linked());
        slider.set_value(30);
        assert_eq!(spin.value(), 30);

        slider.unbind();
        assert!(!slider.linked());
        assert!(!spin.is_linked(), "the peer side is cleared too");
        slider.set_value(60);
        assert_eq!(spin.value(), 30, "an unbound slider no longer drives the spin box");
        spin.set_value(10);
        assert_eq!(slider.value(), 60);
    }

    /// `sync_range` copies the spin box's bounds, rounded outward to whole numbers.
    #[test]
    fn sync_range_copies_the_spin_boxes_bounds() {
        let (mut slider, mut spin) = paired();
        spin.set_decimals(1);
        spin.set_range_f64(2.5, 9.5);
        slider.bind_to_spin_box(&spin);
        slider.sync_range();

        assert_eq!(slider.minimum(), 3, "a fractional lower bound is rounded up");
        assert_eq!(slider.maximum(), 9, "a fractional upper bound is rounded down");
        assert!(
            slider.value() >= 3 && slider.value() <= 9,
            "the adopted value must land in the copied range: {} not in 3..=9",
            slider.value()
        );
    }

    /// A two-way link emits each side's signal once per change and never loops.
    #[test]
    fn a_two_way_link_does_not_loop() {
        use crate::compat::Arc;
        use core::sync::atomic::{AtomicU32, Ordering};

        let (mut slider, mut spin) = paired();
        slider.bind_to_spin_box(&spin);
        slider.set_link_mode(LinkMode::TwoWay);

        let slider_hits = Arc::new(AtomicU32::new(0));
        let spin_hits = Arc::new(AtomicU32::new(0));
        let s = Arc::clone(&slider_hits);
        slider.value_changed.connect(move |_| {
            s.fetch_add(1, Ordering::SeqCst);
        });
        let p = Arc::clone(&spin_hits);
        spin.value_changed.connect(move |_| {
            p.fetch_add(1, Ordering::SeqCst);
        });

        slider.set_value(50);
        assert_eq!(slider_hits.load(Ordering::SeqCst), 1, "one slider change, one signal");
        assert_eq!(spin_hits.load(Ordering::SeqCst), 1, "and the spin box hears it once");
        assert_eq!(spin.value(), 50);

        spin.set_value(80);
        assert_eq!(spin_hits.load(Ordering::SeqCst), 2);
        assert_eq!(slider_hits.load(Ordering::SeqCst), 2);
        assert_eq!(slider.value(), 80);
    }
}
