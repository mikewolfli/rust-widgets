// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

/// A scalar value stored in reflective object metadata.
///
/// # Not the same as `widget::view_widgets::properties_panel::PropertyValue`
///
/// Both are called `PropertyValue` but model different things, so they are kept
/// separate (principle #49):
///
/// * this one — the four **scalar** kinds an object property can hold, with no
///   presentation semantics;
/// * `properties_panel::PropertyValue` — how a property is **edited and
///   drawn** in a panel (`Color`, `Choice { options }`, …), which is a UI concern
///   and meaningless in object metadata.
///
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Boolean scalar value.
    Bool(bool),
    /// Signed integer scalar value.
    Int(i64),
    /// Floating-point scalar value.
    Float(f64),
    /// UTF-8 string scalar value.
    String(String),
}
