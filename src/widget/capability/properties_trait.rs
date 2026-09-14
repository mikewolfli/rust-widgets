// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One property contract, implemented by the control that owns the property.
//!
//! # Why this replaces the centralised dispatch
//!
//! Property access used to be a `match widget.kind()` tower split across 18
//! `include!`-ed files (`access_read_*.in.rs` / `access_write_*.in.rs`), organised
//! by *category* rather than by control. Adding a control meant editing seven
//! places — the `WidgetKind` variant, `properties*.in.rs`, the read and write
//! access files, `coercion.rs`, `constructors.rs` and `registration.rs` — and
//! missing any one of them produced a control that silently answered "no such
//! property". Fourteen controls had already fallen into that gap (eleven
//! `WebEngine*` types plus `Chip`, `CupertinoSwitch`, `Frame`, `GridTable`,
//! `MenuItem`), and a read cost nine sequential category probes before giving up.
//!
//! With this trait a control answers for itself: its properties live in its own
//! file, next to the fields they read. Adding one touches **one** file, and the
//! compiler enforces that the contract is met rather than trusting the author to
//! remember seven edits.
//!
//! # Common properties are inherited, not repeated
//!
//! `enabled` / `visible` / `tooltip` / `geometry` mean the same thing for every
//! control and live in [`BaseWidget`](crate::widget::base::BaseWidget). Writing
//! them 167 times is how a "unified" property layer drifts, so
//! [`base_property_get`] and [`base_property_set`] implement them once and a
//! control forwards its unmatched names there:
//!
//! ```ignore
//! impl WidgetProperties for Button {
//!     fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
//!         match name {
//!             "text" => Ok(CapabilityValue::String(self.text().to_string())),
//!             _ => base_property_get(self, name), // ← shared fallback
//!         }
//!     }
//!     // …
//! }
//! ```

use crate::core::Rect;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::widget_trait::Widget;

/// The properties every control has through its [`BaseWidget`].
///
/// Exposed as a constant so a control's `property_names()` can concatenate rather
/// than retype them, and so tests and schema generation read one list.
pub const BASE_PROPERTY_NAMES: &[&str] = &["enabled", "visible", "tooltip", "geometry"];

/// Reads a property shared by every control.
///
/// A control calls this from the `_` arm of its own `get`, so the shared four are
/// implemented exactly once. Returns [`CapabilityAccessError::UnknownProperty`]
/// when the name is not shared, which lets the caller propagate it unchanged.
pub fn base_property_get(
    widget: &dyn Widget,
    name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match name {
        "enabled" => Ok(CapabilityValue::Bool(widget.is_enabled())),
        "visible" => Ok(CapabilityValue::Bool(widget.is_visible())),
        "tooltip" => Ok(CapabilityValue::String(widget.tooltip().to_string())),
        "geometry" => Ok(geometry_to_value(widget.geometry())),
        _ => Err(CapabilityAccessError::UnknownProperty),
    }
}

/// Writes a property shared by every control.
///
/// The counterpart to [`base_property_get`]; a control forwards its `_` arm here.
pub fn base_property_set(
    widget: &mut dyn Widget,
    name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match name {
        "enabled" => match value {
            CapabilityValue::Bool(enabled) => {
                widget.set_enabled(enabled);
                Ok(())
            }
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        "visible" => match value {
            CapabilityValue::Bool(visible) => {
                widget.set_visible(visible);
                Ok(())
            }
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        "tooltip" => match value {
            CapabilityValue::String(text) => {
                widget.set_tooltip(text);
                Ok(())
            }
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        // Geometry is deliberately read-only through this contract: a control's
        // rectangle is owned by the layout that placed it, and writing it here
        // would silently fight the layout on the next pass. Callers that mean to
        // move a control use `widget::runtime::set_geometry`.
        "geometry" => Err(CapabilityAccessError::ReadOnlyProperty),
        _ => Err(CapabilityAccessError::UnknownProperty),
    }
}

/// Encodes a rectangle as the tuple-shaped string the capability layer publishes.
///
/// Matches the format the previous centralised reader produced, so callers and
/// stored schema continue to parse it.
pub fn geometry_to_value(geometry: Rect) -> CapabilityValue {
    CapabilityValue::String(format!(
        "{},{},{},{}",
        geometry.x, geometry.y, geometry.width, geometry.height
    ))
}

/// The property contract for one control.
///
/// A control implements this in its own file and forwards unmatched names to
/// [`base_property_get`] / [`base_property_set`]. There is deliberately **no**
/// blanket `impl<T: Widget> WidgetProperties for T`: it would overlap every
/// concrete impl below (Rust coherence), which is why the shared four live in
/// plain functions instead of a default method.
///
/// # Where the name list comes from
///
/// `property_names()` returns the control's existing `*_PROPERTIES` schema
/// (declared in `properties_*.in.rs`), through [`schema_names`]. The schema already
/// records every property, its kind and whether it is writable — it is what the
/// factory validates against — so deriving the names from it means the two cannot
/// disagree. Declaring a parallel `&[&str]` list would be a second source of truth
/// for the same fact, which is the duplication this trait exists to remove.
///
/// # How a control declares the contract
///
/// ```ignore
/// impl WidgetProperties for Button {
///     fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
///         match name {
///             "text" => Ok(CapabilityValue::String(self.text().to_string())),
///             _ => base_property_get(self, name),
///         }
///     }
///     fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
///         match name {
///             "text" => { /* … */ Ok(()) }
///             _ => base_property_set(self, name, value),
///         }
///     }
///     fn property_names(&self) -> &'static [&'static str] { schema_names(BUTTON_PROPERTIES) }
/// }
/// ```
pub trait WidgetProperties {
    /// Reads a property by its stable lower-case name.
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError>;

    /// Writes a property by its stable lower-case name.
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError>;

    /// Names this control exposes, **including** the ones it inherits.
    ///
    /// The single source for schema generation, documentation and tests, so a
    /// property cannot be readable yet absent from the published list.
    ///
    /// # Why the shared four appear here too
    ///
    /// `enabled` / `visible` / `tooltip` / `geometry` are part of *this* control's
    /// published contract even though they are implemented once in
    /// [`base_property_get`]. A consumer that walks `property_names()` to build a
    /// property editor, a schema or a docs table must see them, and the pre-existing
    /// `*_PROPERTIES` tables listed them for exactly that reason. Omitting them would
    /// make a control look as if `enabled` did not exist.
    ///
    /// Controls therefore return `property_names_of!["own", .., BASE_PROPERTY_NAMES..]`
    /// — see [`property_names_of`] for the const-compatible way to compose the two.
    fn property_names(&self) -> &'static [&'static str];
}

/// Returns a `&'static [&'static str]` naming the given properties.
///
/// # Why a macro instead of a function
///
/// `WidgetProperties::property_names` must return a `'static` slice. Building one
/// from the existing `*_PROPERTIES` schema at run time would allocate (or leak) on
/// every call, and a `const fn` cannot project a slice of structs into a slice of
/// `&str`. Naming the properties once here keeps the declaration `const` and
/// allocation-free.
///
/// The shared four are appended via `BASE_PROPERTY_NAMES`, which carries its own
/// marks, so the macro accepts it as the last item:
///
/// ```ignore
/// fn property_names(&self) -> &'static [&'static str] {
///     property_names_of!["text", "pressed", "default", BASE_PROPERTY_NAMES]
/// }
/// ```
///
/// # Keeping it honest
///
/// The names must match what the control's `get` actually answers; the
/// `declared_names_are_all_readable` test in this module fails otherwise, so a
/// property cannot be published and unreadable at the same time.
#[macro_export]
macro_rules! property_names_of {
    ($($name:literal),* $(,)?) => {
        &[$($name),*]
    };
    ($($name:literal),* $(,)? BASE_PROPERTY_NAMES) => {
        &[$($name,)* "enabled", "visible", "tooltip", "geometry"]
    };
}

/// Maps the marker idents `yes` / `no` to a `bool`.
///
/// Used by the `properties_*.in.rs` schema tables so a long declaration reads as
/// prose rather than a column of bare `true`s.
#[macro_export]
macro_rules! bool_marker {
    (yes) => {
        true
    };
    (no) => {
        false
    };
    ($other:ident) => {
        compile_error!("use `yes` or `no` for the readable/writable marker")
    };
}

/// Emits the `dyn Widget` hooks that reach a control's [`WidgetProperties`] impl.
///
/// Invoke **inside** the control's `impl Widget for X` block, next to
/// [`crate::impl_draw_bridge!`]:
///
/// ```ignore
/// impl Widget for Button {
///     fn base(&self) -> &BaseWidget { &self.base }
///     fn base_mut(&mut self) -> &mut BaseWidget { &mut self.base }
///
///     impl_draw_bridge!();
///     impl_widget_property_hooks!();
/// }
/// ```
///
/// `Some(self)` is total here for the same reason as the drawing bridge: the type
/// is concrete and its `WidgetProperties` impl is checked by the compiler.
///
/// This is the property-layer analogue of [`crate::impl_draw_bridge!`], and it
/// exists for the same reason: `&mut dyn Widget` cannot select a concrete impl
/// without a downcast, so the override has to be generated where the concrete type
/// is still visible.
#[macro_export]
macro_rules! impl_widget_property_hooks {
    () => {
        fn properties_dyn(
            &self,
        ) -> Option<&dyn $crate::widget::capability::properties_trait::WidgetProperties> {
            Some(self)
        }

        fn properties_dyn_mut(
            &mut self,
        ) -> Option<&mut dyn $crate::widget::capability::properties_trait::WidgetProperties> {
            Some(self)
        }
    };
}

/// Convenience forwarding for a `&dyn Widget`, used by the reflection entry
/// points that still hold a trait object.
///
/// A widget that wants this behaviour implements [`WidgetProperties`] on its
/// concrete type; this helper downcasts and calls it, returning
/// [`CapabilityAccessError::UnsupportedOnWidget`] when the concrete type has no
/// impl yet. That is the honest answer, and it keeps the migration incremental:
/// a control without an impl behaves exactly as before rather than panicking.
pub fn widget_property_get(
    widget: &dyn Widget,
    name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    widget
        .properties_dyn()
        .map_or(Err(CapabilityAccessError::UnsupportedOnWidget), |props| props.get(name))
}

/// Write-side counterpart to [`widget_property_get`].
pub fn widget_property_set(
    widget: &mut dyn Widget,
    name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.properties_dyn_mut() {
        Some(props) => props.set(name, value),
        None => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}

/// Returns a control's declared property names, or an empty slice when it has no
/// [`WidgetProperties`] impl.
///
/// `None` and an empty slice mean different things and both are useful: `None` is
/// "this control has not migrated yet", `&[]` is "this control declares, on
/// purpose, that it has no properties" — the honest answer for the `WebEngine*`
/// types, which expose no widget properties at all.
pub fn widget_property_names(widget: &dyn Widget) -> Option<&'static [&'static str]> {
    widget.properties_dyn().map(WidgetProperties::property_names)
}

/// The compatibility probe, resolved at the call site rather than imported: it
/// exists only under `full_widgets`, while this dispatcher exists wherever the
/// contract does. A stripped profile has no property tables to probe, so the
/// fallback reports [`CapabilityAccessError::UnsupportedOnWidget`] there, which is
/// the same answer the tables themselves would give.
#[cfg(full_widgets)]
fn legacy_read(widget: &dyn Widget, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
    super::access::read_widget_property_legacy(widget, name)
}

#[cfg(not(full_widgets))]
fn legacy_read(
    _widget: &dyn Widget,
    _name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    Err(CapabilityAccessError::UnsupportedOnWidget)
}

#[cfg(full_widgets)]
fn legacy_write(
    widget: &mut dyn Widget,
    name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    super::access::write_widget_property_legacy(widget, name, value)
}

#[cfg(not(full_widgets))]
fn legacy_write(
    _widget: &mut dyn Widget,
    _name: &str,
    _value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    Err(CapabilityAccessError::UnsupportedOnWidget)
}

/// Reads a control's property by name, contract first and legacy table second.
///
/// # Why two paths
///
/// [`widget_property_get`] is the contract path: it asks the control's own
/// `WidgetProperties` impl, which is the single source of truth for what the
/// control exposes. The legacy probe is the fallback for controls whose readers
/// have not moved onto the contract yet (BLUE15 Phase C-1); it keeps the published
/// `WidgetFactory::read_property` behaviour intact instead of returning
/// [`CapabilityAccessError::UnsupportedOnWidget`] for a property the factory
/// advertises as readable.
///
/// # What falls through
///
/// **Only** `UnsupportedOnWidget` — the one answer that means "this control has no
/// contract at all", which is exactly the case the fallback exists for. Every
/// other error is the contract's *decision* and is returned unchanged:
///
/// * `UnknownProperty` — the control has a contract and does not declare this
///   name. Delegating here would let a category arm invent a property the control
///   deliberately does not expose.
/// * `ReadOnlyProperty` / `TypeMismatch` — the control refused the request, and an
///   older table must not overrule that.
pub fn read_widget_property_by_name(
    widget: &dyn Widget,
    name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget_property_get(widget, name) {
        Err(CapabilityAccessError::UnsupportedOnWidget) => legacy_read(widget, name),
        answer => answer,
    }
}

/// Write-side counterpart to [`read_widget_property_by_name`], with the same
/// fall-through rule: only "no contract at all" delegates.
pub fn write_widget_property_by_name(
    widget: &mut dyn Widget,
    name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget_property_set(widget, name, value.clone()) {
        Err(CapabilityAccessError::UnsupportedOnWidget) => legacy_write(widget, name, value),
        answer => answer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::widget::base::BaseWidget;
    use crate::widget::WidgetKind;

    /// A minimal widget used to pin the shared-property contract in isolation.
    struct Probe {
        base: BaseWidget,
    }

    impl crate::widget::widget_trait::Widget for Probe {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }

        // Without these the type would implement `WidgetProperties` yet be
        // unreachable through `dyn Widget`, which is exactly the failure mode the
        // dispatcher tests below exist to catch.
        crate::impl_widget_property_hooks!();
    }

    impl crate::event::EventHandler for Probe {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }

    impl WidgetProperties for Probe {
        fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
            base_property_get(self, name)
        }
        fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
            base_property_set(self, name, value)
        }
        fn property_names(&self) -> &'static [&'static str] {
            BASE_PROPERTY_NAMES
        }
    }

    fn probe() -> Probe {
        Probe { base: BaseWidget::new(WidgetKind::Panel, Rect::new(1, 2, 30, 40), "probe") }
    }

    /// Every shared property must round-trip, so forwarding to the base helpers
    /// is not merely present but correct.
    #[test]
    fn base_properties_round_trip() {
        let mut widget = probe();

        assert_eq!(widget.get("enabled"), Ok(CapabilityValue::Bool(true)));
        widget.set("enabled", CapabilityValue::Bool(false)).expect("writable");
        assert_eq!(widget.get("enabled"), Ok(CapabilityValue::Bool(false)));

        widget.set("visible", CapabilityValue::Bool(false)).expect("writable");
        assert_eq!(widget.get("visible"), Ok(CapabilityValue::Bool(false)));

        widget.set("tooltip", CapabilityValue::String("tip".into())).expect("writable");
        assert_eq!(widget.get("tooltip"), Ok(CapabilityValue::String("tip".into())));
    }

    /// A wrong-typed write must be rejected rather than silently coerced.
    #[test]
    fn base_property_set_rejects_type_mismatch() {
        let mut widget = probe();
        assert_eq!(
            widget.set("enabled", CapabilityValue::String("yes".into())),
            Err(CapabilityAccessError::TypeMismatch)
        );
    }

    /// Geometry must be readable and honestly read-only, not writable.
    #[test]
    fn geometry_is_readable_but_read_only() {
        let mut widget = probe();
        assert_eq!(widget.get("geometry"), Ok(CapabilityValue::String("1,2,30,40".into())));
        assert_eq!(
            widget.set("geometry", CapabilityValue::String("0,0,1,1".into())),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    /// An unknown name must be reported as such, not as "not supported here",
    /// so a caller can tell a typo from a control that has not migrated.
    #[test]
    fn unknown_names_are_distinguished() {
        let widget = probe();
        assert_eq!(widget.get("nope"), Err(CapabilityAccessError::UnknownProperty));
    }

    /// A property the contract rejects definitively must not be handed to the
    /// legacy tables.
    ///
    /// `ReadOnlyProperty` is a decision, not a gap: if the fallback saw it, a
    /// control could declare a property read-only and still have an older category
    /// arm write it (BLUE15 Phase C-1).
    #[test]
    fn definitive_contract_answers_are_not_delegated() {
        let mut widget = probe();
        assert_eq!(
            write_widget_property_by_name(
                &mut widget,
                "geometry",
                CapabilityValue::String("0,0,1,1".into()),
            ),
            Err(CapabilityAccessError::ReadOnlyProperty),
        );
    }

    /// The dispatcher must reach the contract through `dyn Widget`, not only on
    /// the concrete type.
    ///
    /// This pins the whole point of the reflection hooks: a control can implement
    /// `WidgetProperties` and still be invisible if its `impl Widget` forgets them.
    /// An unknown name then answers from the contract (`UnknownProperty`) instead
    /// of falling through to "this control has no contract at all".
    #[test]
    fn the_dispatcher_reaches_the_contract_through_dyn_widget() {
        let mut widget = probe();
        let dynamic: &mut dyn crate::widget::Widget = &mut widget;
        assert_eq!(
            write_widget_property_by_name(dynamic, "not_a_property", CapabilityValue::Bool(true)),
            Err(CapabilityAccessError::UnknownProperty),
        );
        let dynamic: &dyn crate::widget::Widget = &widget;
        assert_eq!(
            read_widget_property_by_name(dynamic, "not_a_property"),
            Err(CapabilityAccessError::UnknownProperty),
        );
        assert_eq!(
            read_widget_property_by_name(dynamic, "enabled"),
            Ok(CapabilityValue::Bool(true))
        );
    }

    /// The published name list must match what `get` actually answers, so schema
    /// generation and reflection cannot disagree.
    #[test]
    fn declared_names_are_all_readable() {
        let widget = probe();
        for name in widget.property_names() {
            assert!(
                widget.get(name).is_ok(),
                "property_names() declares {name:?} but get() rejects it"
            );
        }
    }
}
