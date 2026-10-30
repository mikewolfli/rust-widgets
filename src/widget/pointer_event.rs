// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A pointer gesture as one event: the legacy payload-free signal **plus** the typed payload.
//!
//! # Why both signals are kept
//!
//! Several drawing surfaces used to declare `pub mouse_pressed: GenericSignal`, so existing callers
//! connect zero-argument slots (the documented example `canvas.mouse_pressed.connect_scoped(..)`
//! does exactly that). The published event, however, must carry *where* and *which button* — a
//! press without a position cannot start a drag, which is why the host had to mirror the pointer
//! position from `mouse_moved` before this type existed. Rather than delete the old signal, this
//! type keeps it and emits the typed one alongside it: old subscribers still fire, and a dynamic
//! subscriber receives the payload the capability schema declares.
//!
//! [`PointerEvent`] derefs to the legacy [`GenericSignal`], so every existing method call
//! (`connect`, `connect_scoped`, `disconnect`, `slot_count`, …) keeps compiling unchanged.
//!
//! # One type, one wire form
//!
//! Every control that publishes a pointer gesture resolves it through [`pointer_capability_value`],
//! so `"mouse_pressed"` means the same thing — `["x,y", button]` — on every surface (principle
//! #54: one representation per semantic, no per-control copies that can drift).

use crate::compat::{format, Vec};
use crate::core::Point;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::CapabilityValue;
use core::ops::Deref;

/// One pointer event that carries both the legacy payload-free signal and the typed payload.
#[derive(Clone, Default)]
pub struct PointerEvent {
    /// The payload-free compatibility signal.
    unit: GenericSignal,
    /// The typed signal the published event resolves to.
    payload: Signal1<(Point, u32)>,
}

impl PointerEvent {
    /// Creates an event pair with no subscribers.
    pub fn new() -> Self {
        Self { unit: GenericSignal::new(), payload: Signal1::new() }
    }

    /// The typed signal carrying `(position, button)`.
    pub fn payload_signal(&self) -> &Signal1<(Point, u32)> {
        &self.payload
    }

    /// Emits the event: the legacy signal first, then the typed one.
    pub fn emit_at(&self, position: Point, button: u32) {
        self.unit.emit();
        self.payload.emit((position, button));
    }
}

impl Deref for PointerEvent {
    type Target = GenericSignal;

    fn deref(&self) -> &Self::Target {
        &self.unit
    }
}

/// The published wire form of one pointer gesture.
///
/// `["x,y", button]`: the position in the same `"x,y"` spelling the property side uses for a point
/// (there is no point kind in `PropertyValueKind`), and the button as an unsigned count. A tuple
/// rather than a single string because the two components are independent facts, and a subscriber
/// that only needs the button should not have to split text to get it.
pub fn pointer_capability_value(value: &(Point, u32)) -> CapabilityValue {
    let (position, button) = value;
    CapabilityValue::Tuple(Vec::from([
        CapabilityValue::String(format!("{},{}", position.x, position.y)),
        CapabilityValue::UInt(*button as u64),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Arc, Mutex, String};

    /// The typed signal must deliver the position and button the emitter supplied.
    #[test]
    fn the_typed_signal_carries_position_and_button() {
        let event = PointerEvent::new();
        let seen = Arc::new(Mutex::new(None));
        let recorder = Arc::clone(&seen);
        event.payload_signal().connect(move |value: Arc<(Point, u32)>| {
            *lock(&recorder) = Some(*value);
        });

        event.emit_at(Point::new(12, 34), 1);

        let value = lock(&seen).expect("the typed signal must have fired");
        assert_eq!((value.0.x, value.0.y, value.1), (12, 34, 1));
    }

    /// The legacy unit signal must still fire, which is what keeps old subscribers working.
    #[test]
    fn the_legacy_unit_signal_still_fires() {
        let event = PointerEvent::new();
        let calls = Arc::new(Mutex::new(0usize));
        let counter = Arc::clone(&calls);
        event.connect(move || *lock(&counter) += 1);

        event.emit_at(Point::new(1, 2), 0);

        assert_eq!(*lock(&calls), 1, "a zero-argument subscriber must still be called");
    }

    /// The wire form must be `["x,y", button]` — the shape the capability table declares.
    #[test]
    fn the_wire_form_is_position_then_button() {
        let value = pointer_capability_value(&(Point::new(7, -2), 2));
        match value {
            CapabilityValue::Tuple(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0], CapabilityValue::String(String::from("7,-2")));
                assert_eq!(parts[1], CapabilityValue::UInt(2));
            }
            other => panic!("expected a tuple payload, got {other:?}"),
        }
    }
}
