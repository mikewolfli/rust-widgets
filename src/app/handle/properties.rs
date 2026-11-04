// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The one route to a mounted control's properties.
//!
//! Split out of `handle/types.rs` when the handle façade crossed the 2000-line structure limit
//! (principle #9). The functions here are the whole property access surface every handle family
//! shares, so a second way to reach a control's own contract cannot grow in one family unnoticed.

use crate::core::ObjectId;

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
/// Returns `false` when the id is not a mounted widget, or when the control refused the write
/// (an unpublished name, or a value of the wrong kind). The refusal is also logged, because a
/// silent `false` is the defect this replaced.
pub(crate) fn write_control_property(
    id: ObjectId,
    name: &str,
    value: crate::widget::capability::CapabilityValue,
) -> bool {
    let written = crate::widget::runtime::with_widget_mut(id, |widget| {
        crate::widget::capability::WidgetFactory::shared().write_property(widget, name, value)
    });
    match written {
        Some(Ok(())) => true,
        Some(Err(error)) => {
            log::warn!("[handle] `{name}` was refused for id={id}: {error:?}");
            false
        }
        None => {
            log::warn!("[handle] `{name}` ignored for id={id}: the id is not a mounted widget");
            false
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
            "[handle] {operation}: one of the two property writes was refused \
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
