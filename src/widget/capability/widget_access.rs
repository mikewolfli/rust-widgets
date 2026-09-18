// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! By-id control-state access for the crate-root convenience wrappers.
//!
//! # Why the wrappers cannot ask the platform
//!
//! `set_widget_text` / `set_widget_value` / `set_widget_checked` and their
//! siblings are the crate-root spelling of "change this control". They called
//! `platform::get_platform().set_widget_*`, which was right while the host owned
//! the controls. After BLUE15 the host owns a window and a drawing surface only
//! (rule #56), so those trait methods are honest defaults that report "no such
//! control" — the wrappers therefore returned `false` and changed nothing, while
//! the C ABI functions of the same name went through the control backend and did
//! work. Two paths, one of which silently did nothing.
//!
//! The live path is the control's **own property contract** — where BLUE15 rule
//! #67 puts control state. These helpers are the thin adapter: resolve the
//! property on the widget the id addresses, and report failure rather than
//! pretending success.
//!
//! # Why the names are probed instead of looked up by kind
//!
//! Controls publish the same concept under different spellings: a slider has
//! `minimum`/`maximum`, a combo box has `current_index`, a list box has
//! `current_row`, a text entry has `placeholder_text`. A central "kind → name"
//! table is exactly the centralised match that rule #67 removed, so a wrapper
//! that holds only an id probes the spellings the concept is documented under and
//! fails when none is accepted. This is the same technique the control backend
//! already uses for label names, and it is pinned by
//! `tests/widget_accessor_routing_test.rs`, which drives each accessor against a
//! real control and reads the property back.
//!
//! # Profiles without a property registry
//!
//! `widgets_unstripped` gates the registry itself (`capability::access`), so in
//! `mini`/`embedded` there is no property to write and no widget that carries one.
//! Every helper then answers "no", which is the same truthful absence the rest of
//! the stripped profiles report, and keeps this module compiled everywhere the
//! wrappers are.

//! Every helper is compiled in every profile so the wrappers that call them are
//! too, but `mini` compiles neither the wrappers (`not(alloc_frugal)`) nor a
//! property registry — so nothing calls them there, legitimately.
#![cfg_attr(alloc_frugal, allow(dead_code))]

#[cfg(widgets_unstripped)]
use super::access::{read_widget_property_by_id, write_widget_property_by_id};
use super::types::CapabilityValue;
use crate::compat::String;
use crate::core::ObjectId;

/// Writes `value` under the first of `names` the control accepts.
pub(crate) fn write_first(id: ObjectId, names: &[&str], value: &CapabilityValue) -> bool {
    #[cfg(widgets_unstripped)]
    {
        names.iter().any(|name| write_widget_property_by_id(id, name, value.clone()).is_ok())
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (id, names, value);
        false
    }
}

/// Reads the first of `names` the control answers.
///
/// A stored `Null` is a real answer (an unset index, an absent group), not a
/// miss, so it is returned rather than skipped in favour of a later spelling.
pub(crate) fn read_first(id: ObjectId, names: &[&str]) -> Option<CapabilityValue> {
    #[cfg(widgets_unstripped)]
    {
        names.iter().find_map(|name| read_widget_property_by_id(id, name).ok())
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (id, names);
        None
    }
}

/// Writes a number, negotiating the control's own numeric representation.
///
/// Controls disagree on how a number is stored: a slider's range is `i64`, a
/// combo box's index is `u64`, and a chart axis may be `f64`. The extractors in
/// `coercion` do not cross the integer/float boundary, so an integral value is
/// offered as an integer first — that spelling satisfies all three storage kinds —
/// and only as a float when it is genuinely fractional. A fractional value sent to
/// an integer-typed property is refused, which is the honest answer: the control
/// cannot hold it.
pub(crate) fn write_number(id: ObjectId, names: &[&str], value: f64) -> bool {
    let integral = value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value <= i64::MAX as f64;
    if integral && write_first(id, names, &CapabilityValue::Int(value as i64)) {
        return true;
    }
    write_first(id, names, &CapabilityValue::Float(value))
}

/// Reads a number from the first of `names` that holds one.
pub(crate) fn read_number(id: ObjectId, names: &[&str]) -> Option<f64> {
    match read_first(id, names)? {
        CapabilityValue::Float(v) => Some(v),
        CapabilityValue::Int(v) => Some(v as f64),
        CapabilityValue::UInt(v) => Some(v as f64),
        _ => None,
    }
}

/// Reads a count/index from the first of `names` that holds one.
///
/// `Null` maps to `None`: the control is saying "nothing is selected", which is
/// not the same as an index of zero.
pub(crate) fn read_index(id: ObjectId, names: &[&str]) -> Option<usize> {
    match read_first(id, names)? {
        CapabilityValue::UInt(v) => usize::try_from(v).ok(),
        CapabilityValue::Int(v) if v >= 0 => usize::try_from(v as u64).ok(),
        _ => None,
    }
}

/// Reads a boolean from the first of `names` that holds one.
pub(crate) fn read_flag(id: ObjectId, names: &[&str]) -> Option<bool> {
    match read_first(id, names)? {
        CapabilityValue::Bool(v) => Some(v),
        _ => None,
    }
}

/// Reads a string from the first of `names` that holds one.
pub(crate) fn read_text(id: ObjectId, names: &[&str]) -> Option<String> {
    match read_first(id, names)? {
        CapabilityValue::String(v) => Some(v),
        _ => None,
    }
}
