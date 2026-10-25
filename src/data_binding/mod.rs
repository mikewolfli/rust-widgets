// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reactive data binding system — Model → View automatic synchronization.
//!
//! Provides `Binding<T>` for single values, `ObservableList<T>` for collections,
//! and `Computed<T>` for derived values, enabling MVVM-style reactive UIs.
//!
//! # Architecture
//!
//! The system follows a "source of truth + observers" pattern:
//!
//! - **`Binding<T>`**: A single reactive value. When the value changes via
//!   `set`, all registered listeners are notified. Supports
//!   two-way binding via `bind_to`.
//! - **`ObservableList<T>`**: An observable collection that fires
//!   notifications on structural mutations (push, pop, insert, remove, clear).
//! - **`Computed<T>`**: A derived value computed from a closure. Marked dirty
//!   via `invalidate` and lazily recomputed on
//!   `get`.
//! - **Traits**: `BindingListener` and `BoxedListener` define the listener
//!   contract. `FnListener` wraps a closure as a listener.
//! - **Macros**: [`binding!`](crate::binding!) and [`computed!`](crate::computed!)
//!   provide convenient construction syntax.
//!
//! # Example
//!
//! ```rust
//! use rust_widgets::data_binding::{Binding, FnListener};
//!
//! let mut name = Binding::new("World".to_string());
//! let mut greeting = String::new();
//!
//! name.subscribe("log", Box::new(FnListener::new(|key, _op| {
//!     println!("[{}] name changed!", key);
//! })));
//!
//! name.set("Rust".to_string());
//! assert_eq!(name.get(), "Rust");
//! ```
//!
//! # Reachability
//!
//! **State:** Reserved: a declarative data-binding layer with no production consumer; no example and no binding uses it. Removal condition: when the declarative JSON path adopts it, or when it is confirmed redundant with the property contract.

mod binding;
mod computed;
mod list;
mod macros;
mod traits;

pub use binding::*;
pub use computed::*;
pub use list::*;

pub use traits::*;

/// A mutable target that reads and writes one named property.
///
/// # Why this exists
///
/// [`Binding`] could notify listeners, but nothing in the widget layer listened: the bridge from
/// a model value to a control's property was missing, so the module had no consumer. This trait is
/// the smallest shape that bridge needs — a name and a `CapabilityValue` in both directions —
/// and a widget type implements it by delegating to its own property contract. It is deliberately
/// not tied to `Widget`, so a headless model target (a test double, a serialized form) can use the
/// same binding path.
pub trait PropertySink: Send {
    /// Writes `value` to `property`.
    fn set_property(
        &mut self,
        property: &str,
        value: crate::widget::capability::CapabilityValue,
    ) -> Result<(), crate::widget::capability::CapabilityAccessError>;

    /// Reads `property`'s current value.
    fn get_property(
        &self,
        property: &str,
    ) -> Result<
        crate::widget::capability::CapabilityValue,
        crate::widget::capability::CapabilityAccessError,
    >;
}

/// A [`PropertySink`] backed by a mounted widget id.
///
/// This is the adapter a control-agnostic caller uses: it resolves the id through the runtime and
/// delegates to the control's own property contract, so the same bridge drives a slider, a line
/// edit, or a third-party control without naming any of them.
#[cfg(not(alloc_frugal))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetPropertySink {
    /// The mounted widget whose property is read and written.
    pub widget_id: crate::core::ObjectId,
}

#[cfg(not(alloc_frugal))]
impl WidgetPropertySink {
    /// Creates a sink for `widget_id`.
    pub fn new(widget_id: crate::core::ObjectId) -> Self {
        Self { widget_id }
    }
}

#[cfg(not(alloc_frugal))]
impl PropertySink for WidgetPropertySink {
    fn set_property(
        &mut self,
        property: &str,
        value: crate::widget::capability::CapabilityValue,
    ) -> Result<(), crate::widget::capability::CapabilityAccessError> {
        crate::widget::runtime::with_widget_mut(self.widget_id, |widget| {
            crate::widget::widget_property_set(widget, property, value)
        })
        .unwrap_or(Err(crate::widget::capability::CapabilityAccessError::UnknownWidget))
    }

    fn get_property(
        &self,
        property: &str,
    ) -> Result<
        crate::widget::capability::CapabilityValue,
        crate::widget::capability::CapabilityAccessError,
    > {
        crate::widget::runtime::with_widget(self.widget_id, |widget| {
            crate::widget::widget_property_get(widget, property)
        })
        .unwrap_or(Err(crate::widget::capability::CapabilityAccessError::UnknownWidget))
    }
}

/// Binds one binding to one property of a **mounted** widget.
///
/// The observer writes every new value through the widget's own property contract, so a slider or
/// line edit consumes the model without either module naming the other. The initial value is
/// pushed immediately; otherwise a control would show its constructor default until the model next
/// changed. The reverse direction is [`pull_widget_property`], which a control calls when its own
/// value changes.
///
/// Only built where the runtime exists: `mini`/`embedded` has no mounted-widget registry, which is
/// the same reason it has no data-binding consumer.
#[cfg(not(alloc_frugal))]
pub fn bind_widget_property(
    binding: &Binding<crate::widget::capability::CapabilityValue>,
    key: &str,
    widget_id: crate::core::ObjectId,
    property: &str,
) {
    let property = crate::compat::String::from(property);
    let observed_property = property.clone();
    let synced = binding.clone();
    binding.observe(key, move |value| {
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            crate::widget::widget_property_set(widget, &observed_property, value.clone())
        });
        if matches!(applied, Some(Ok(()))) {
            synced.announce_synced(&observed_property);
        }
    });
    let initial = binding.get();
    let _ = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        crate::widget::widget_property_set(widget, &property, initial)
    });
    binding.announce_synced(&property);
}

/// Copies one mounted widget property back into the binding.
///
/// Returns `false` when the widget is not mounted or does not publish the property; a silent
/// `true` would make a typo look like a successful model update.
#[cfg(not(alloc_frugal))]
pub fn pull_widget_property(
    binding: &Binding<crate::widget::capability::CapabilityValue>,
    widget_id: crate::core::ObjectId,
    property: &str,
) -> bool {
    let value = crate::widget::runtime::with_widget(widget_id, |widget| {
        crate::widget::widget_property_get(widget, property)
    });
    match value {
        Some(Ok(value)) => {
            binding.set(value);
            binding.announce_synced(property);
            true
        }
        _ => false,
    }
}
