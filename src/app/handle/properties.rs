// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The one route to a mounted control's properties.
//!
//! Split out of `handle/types.rs` when the handle façade crossed the 2000-line structure limit
//! (principle #9). The functions here are the whole property access surface every handle family
//! shares, so a second way to reach a control's own contract cannot grow in one family unnoticed.

use crate::core::ObjectId;

/// The outcome of writing one property on a mounted control.
///
/// "Did not write" is not one case. A control that does not publish the property, or a value of
/// the wrong carrier, **refused** the write: the control still holds the old value, so a handle's
/// read cache must keep the old value too. An id with no control behind it has nobody to ask —
/// handles deliberately keep serving the last request as an offline read cache (HB-08) — so that
/// case must still record it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropertyWrite {
    /// The control accepted the value.
    Applied,
    /// The control is mounted but refused the value (unpublished name, wrong carrier).
    Refused,
    /// No control is mounted at this id.
    Unmounted,
}

impl PropertyWrite {
    /// Whether the control accepted the value.
    pub(crate) fn is_applied(self) -> bool {
        matches!(self, PropertyWrite::Applied)
    }

    /// Whether a handle's in-process read cache should record the requested value.
    ///
    /// `Applied` agrees with the control. `Unmounted` is the documented offline fallback: with no
    /// control to answer, the handle still reports what was asked for. A `Refused` write is the one
    /// case the cache must **not** absorb, or the getter would claim a value the control rejected.
    pub(crate) fn should_mirror(self) -> bool {
        !matches!(self, PropertyWrite::Refused)
    }
}

/// Writes one property on the control the id addresses, and reports whether it took.
///
/// # Why every handle setter must go through here
///
/// The handles used to write an in-process mirror and then call
/// [`Platform::set_widget_value`](crate::platform::Platform::set_widget_value) and its
/// siblings — a surface that exists for **host-native** controls. This library creates no
/// native controls: `App`-created widgets are mounted into `widget::runtime` by
/// `control_backend::custom`, so the platform holds no record for their ids and every one of
/// those calls answered `false`. The result was discarded, the mirror was written anyway, and
/// the getters read only the mirror — measured, with a line edit created through a
/// `WindowHandle`:
///
/// ```text
/// platform.set_widget_placeholder(id, "hint") -> false
/// handle.placeholder()                        -> "hint"      <- what the getter reported
/// widget read_property(placeholder_text)      -> String("")  <- what the control held
/// ```
///
/// So a host could set a slider's value, a field's placeholder or a check box's state and the
/// control never received it, while the read-back said it had. Two mechanisms for one fact
/// (only one of which reaches the control), and a mirror that drifted into a wrong answer.
///
/// This helper is the single mechanism. It is the same one `write_spinbox_affix` already used
/// for the spin box's affixes, generalised: `WidgetFactory::write_property` dispatches on the
/// control's own [`WidgetProperties`](crate::widget::capability::WidgetProperties) contract, so
/// a control that does not publish `name` **refuses** the write rather than accepting it into
/// a store nothing reads.
///
/// Returns `true` when the control took the value; `false` when it is mounted but rejected the
/// name/carrier, or when the id has no mounted widget. Use [`write_control_property_outcome`]
/// when the caller also needs to know *why* it did not land.
pub(crate) fn write_control_property(
    id: ObjectId,
    name: &str,
    value: crate::widget::capability::CapabilityValue,
) -> bool {
    write_control_property_outcome(id, name, value).is_applied()
}

/// Writes one property and reports why it did or did not apply.
///
/// [`write_control_property`] answers the common "did it land" question. A handle that also keeps
/// an in-process read cache needs the distinction: a **refused** write must not be cached (the
/// control still holds the old value), while an id with no mounted control keeps the offline
/// fallback the handle family documents (HB-08). See [`PropertyWrite::should_mirror`].
pub(crate) fn write_control_property_outcome(
    id: ObjectId,
    name: &str,
    value: crate::widget::capability::CapabilityValue,
) -> PropertyWrite {
    let written = crate::widget::runtime::with_widget_mut(id, |widget| {
        crate::widget::capability::WidgetFactory::shared().write_property(widget, name, value)
    });
    match written {
        Some(Ok(())) => PropertyWrite::Applied,
        Some(Err(error)) => {
            log::warn!("[handle] `{name}` was refused for id={id}: {error:?}");
            PropertyWrite::Refused
        }
        None => {
            log::warn!("[handle] `{name}` ignored for id={id}: the id is not a mounted widget");
            PropertyWrite::Unmounted
        }
    }
}

/// Records the outcome of a two-property write whose caller has no error channel.
///
/// Several setters (`set_range`, `set_content_size`, `set_scroll_position`) write a pair and
/// return `()`. They used to compose the two results as `let _ = a && b;`, which both discarded
/// the fact and read as a placeholder expression (project rule: no pseudo-expressions). This
/// keeps the call site meaningful: the individual refusal is already logged at `warn` by
/// [`write_control_property`], and this adds the operation that issued it.
pub(crate) fn note_property_pair(operation: &str, first: bool, second: bool) {
    if !(first && second) {
        log::debug!(
            "[handle] {operation}: one of the two property writes did not reach the control \
             (first={first}, second={second})"
        );
    }
}

/// Reads one property off the control the id addresses.
///
/// `None` means "this control does not publish that name, or the id is not mounted" — the
/// same two cases, which a caller distinguishes by whether it expects the control to have the
/// property at all. See [`write_control_property`] for why the platform is not asked.
pub(crate) fn read_control_property(
    id: ObjectId,
    name: &str,
) -> Option<crate::widget::capability::CapabilityValue> {
    crate::widget::runtime::with_widget_mut(id, |widget| {
        crate::widget::capability::WidgetFactory::shared().read_property(widget, name).ok()
    })
    .flatten()
}

/// Extracts a `f64` from a numeric capability value, whichever carrier it arrived in.
pub(crate) fn capability_as_f64(value: &crate::widget::capability::CapabilityValue) -> Option<f64> {
    use crate::widget::capability::CapabilityValue as V;
    match value {
        V::Int(v) => Some(*v as f64),
        V::UInt(v) => Some(*v as f64),
        V::Float(v) => Some(*v),
        _ => None,
    }
}

/// The carrier `single_step` expects, for the value given.
///
/// The property is declared `Int` on the slider, dial and scroll bar and `Number` on the spin
/// box, so an integral step is written as the integer carrier the declaration asks for. A
/// fractional step keeps the float carrier; a control that cannot represent it **refuses**
/// (the funnel logs it) rather than silently rounding the step the caller asked for.
pub(crate) fn step_carrier(step: f64) -> crate::widget::capability::CapabilityValue {
    if step.fract() == 0.0 && step >= i64::MIN as f64 && step <= i64::MAX as f64 {
        crate::widget::capability::CapabilityValue::Int(step as i64)
    } else {
        crate::widget::capability::CapabilityValue::Float(step)
    }
}
