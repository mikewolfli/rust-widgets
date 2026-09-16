// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Custom-painted control backend: paints every control on a host surface.

use crate::compat::Mutex;
use crate::control_backend::types::CustomControlState;

/// Backend that paints every control on a surface it owns.
///
/// It holds no per-widget state of its own: control state belongs to the widget
/// object, reached through `widget::runtime`. What remains in
/// `CustomControlState` is host policy that a widget cannot express (see its
/// docs).
pub struct CustomPaintControlBackend {
    pub(crate) state: Mutex<CustomControlState>,
}

impl CustomPaintControlBackend {
    /// Creates the custom-painted control backend.
    pub fn new() -> Self {
        Self { state: Mutex::new(CustomControlState::default()) }
    }
}

crate::impl_default_via_new!(CustomPaintControlBackend);
