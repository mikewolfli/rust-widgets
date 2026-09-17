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

/// The properties every control has through its [`crate::widget::BaseWidget`].
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
/// `property_names()` names the properties directly, through the
/// [`crate::property_names_of`] macro, and appends [`BASE_PROPERTY_NAMES`] for the shared
/// four.
///
/// # Why not derive them from the schema table
///
/// The schema table (`properties_*.in.rs`) and this list are two statements of the
/// same fact, and an earlier draft of this doc-comment proposed deriving one from
/// the other with a `schema_names(BUTTON_PROPERTIES)` helper. `property_names` must
/// return a `'static` slice, and no `const fn` can project a slice of
/// `PropertySchema` structs into a slice of `&str`, so that helper cannot exist
/// without allocating on every call.
///
/// The two lists are therefore kept in step by test instead:
/// `schema_and_contract_publish_the_same_names` fails when they disagree. That is
/// the arrangement actually in force — this comment previously described a function
/// that was never written, which is worse than no comment at all.
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
///     fn property_names(&self) -> &'static [&'static str] {
///         property_names_of!["text", "pressed", "default", BASE_PROPERTY_NAMES]
///     }
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
    /// — see [`crate::property_names_of`] for the const-compatible way to compose the two.
    fn property_names(&self) -> &'static [&'static str];

    /// The accepted spellings for an `Enum` property, or an empty slice.
    ///
    /// # Why this has a default
    ///
    /// Most controls publish no enum property, and many that do rely on the
    /// `impl_widget_property_hooks!` macro's own `*_PROPERTIES` table rather than writing
    /// this by hand. The default answers "no fixed set of values", which is correct for
    /// a non-enum property and is also the safe answer for an enum whose author has not
    /// listed tokens yet — it does not claim a set the control would then reject.
    ///
    /// # What an override must guarantee
    ///
    /// The tokens must be exactly the spellings `set` accepts. `widget_property_tokens`
    /// is the public reader; `capability::properties_tests` writes each returned token
    /// back through `set` and fails if any is refused, so a list that drifts from the
    /// control's parser is caught rather than shipped.
    fn property_tokens(&self, _name: &str) -> &'static [&'static str] {
        &[]
    }
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

/// Returns the accepted spellings for an `Enum` property, or an empty slice.
///
/// # Why a caller needs this
///
/// An enum property is written as one of a fixed set of tokens (`"single"`,
/// `"multiple"`, `"ascending"` …), but those tokens were only discoverable by reading
/// the control's source. A caller building a property editor or validating user input
/// had to hard-code its own copy of the list, and nothing failed when the control's
/// parser changed — the copy just silently went stale.
///
/// The answer comes from the control's own [`PropertySchema`], so it cannot drift from
/// what `set` accepts without the control's declaration changing too.
///
/// # Empty is a real answer
///
/// An empty slice means "this property declares no fixed set of values". That covers
/// both a non-enum property and an enum whose author has not listed its tokens yet. It
/// is not an error: most properties are not enums.
///
/// # Which schema
///
/// Looked up through the same registry path as the other reflection entry points, so a
/// control in a profile without the capability registry answers empty rather than
/// failing.
///
/// [`PropertySchema`]: crate::widget::capability::types::PropertySchema
pub fn widget_property_tokens(widget: &dyn Widget, name: &str) -> &'static [&'static str] {
    widget.properties_dyn().map_or(&[], |props| props.property_tokens(name))
}

/// Appends one item to a control that holds a list of strings.
///
/// # Why this is a downcast and not a property write
///
/// The `item_count` property is deliberately **read-only** on every control that
/// publishes it (`list_box`, `combo_box`, `list_view`, …): a count is a
/// consequence of the items, not a settable value, and letting a caller write it
/// would desynchronise it from the actual collection. So the only honest way to
/// grow a collection is the control's own method, which is what this dispatches
/// to.
///
/// Returns `false` when the control is not one that holds items. That is a real
/// "no", not a silent success: a caller adding to a `Button` should be told.
pub fn append_widget_list_item(widget: &mut dyn Widget, item: String) -> bool {
    use crate::widget::capability::coercion::widget_as_mut;
    if let Some(list) = widget_as_mut::<crate::widget::ListBox>(widget) {
        list.add_item(item);
        return true;
    }
    if let Some(combo) = widget_as_mut::<crate::widget::ComboBox>(widget) {
        combo.add_item(item);
        return true;
    }
    false
}

/// Removes every item from a control that holds a list of strings.
///
/// Returns `false` when the control does not hold items. See
/// [`append_widget_list_item`] for why this is not a property write.
pub fn clear_widget_list_items(widget: &mut dyn Widget) -> bool {
    use crate::widget::capability::coercion::widget_as_mut;
    if let Some(list) = widget_as_mut::<crate::widget::ListBox>(widget) {
        list.clear();
        return true;
    }
    if let Some(combo) = widget_as_mut::<crate::widget::ComboBox>(widget) {
        combo.clear();
        return true;
    }
    false
}

/// Returns how many items a control holds, or `0` when it holds none or does not
/// hold items.
pub fn widget_list_item_count(widget: &dyn Widget) -> usize {
    use crate::widget::capability::coercion::widget_as;
    if let Some(list) = widget_as::<crate::widget::ListBox>(widget) {
        return list.count();
    }
    if let Some(combo) = widget_as::<crate::widget::ComboBox>(widget) {
        return combo.count();
    }
    0
}

/// Reads one item's text out of a control that holds a list of strings.
///
/// # The gap this closes
///
/// `item_count` was the only collection fact readable through the property surface.
/// A caller could `add` items, count them and clear them, but could never read back
/// what it had added — so a control's contents were write-only across the whole
/// declarative API, and a test asserting "the items are what I set" was impossible to
/// write without downcasting to the concrete type.
///
/// Returns `None` in three cases, all of which are honestly "no value": the control
/// does not hold items, `index` is past the last item, or the item at `index` holds no
/// text. A caller that needs to tell them apart asks [`widget_list_item_count`] first.
pub fn widget_list_item(widget: &dyn Widget, index: usize) -> Option<String> {
    use crate::widget::capability::coercion::widget_as;
    if let Some(list) = widget_as::<crate::widget::ListBox>(widget) {
        return list.item(index).map(str::to_string);
    }
    if let Some(combo) = widget_as::<crate::widget::ComboBox>(widget) {
        return combo.item(index).map(str::to_string);
    }
    None
}

/// The contract path, with no fallback.
///
/// # Why there is only one path now
///
/// This used to try the control's own `WidgetProperties` impl and, on
/// `UnsupportedOnWidget`, delegate to a centralised nine-category probe over the
/// property tables (BLUE15 Phase C-1). That second path duplicated the contract
/// for every control that had migrated — two places answering "what properties does
/// this control have", which is exactly the drift the contract exists to prevent.
///
/// Every registered control now implements `WidgetProperties`, and a test asserts
/// it, so nothing can reach the fallback. It has been deleted rather than left as
/// dead code, and this function is consequently a plain forward.
pub fn read_widget_property_by_name(
    widget: &dyn Widget,
    name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    widget_property_get(widget, name)
}

/// Write-side counterpart to [`read_widget_property_by_name`].
pub fn write_widget_property_by_name(
    widget: &mut dyn Widget,
    name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    widget_property_set(widget, name, value)
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
