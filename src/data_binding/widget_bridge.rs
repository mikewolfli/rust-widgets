// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The model-to-widget property bridge.
//!
//! This is the part of `data_binding` that names a mounted widget: a property sink, the
//! one-way/two-way/validated binders and their pull-back channel. It lives in its own file so
//! `mod.rs` stays declarations and re-exports (principle #8); every item keeps its historical
//! `data_binding::` path through the re-export.

#[cfg(not(alloc_frugal))]
use super::Binding;

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
/// The initial push is announced as synced only when it actually reached the control. If the id is
/// not mounted or the control refuses the property, the failure is logged and the binding is left
/// out of the synced state — announcing success for a write that did not happen would promise
/// agreement between a model and a control that disagree (the same contract as
/// [`crate::data_binding::bind_chart_series`]).
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
    let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        crate::widget::widget_property_set(widget, &property, initial)
    });
    match applied {
        Some(Ok(())) => binding.announce_synced(&property),
        Some(Err(error)) => log::warn!(
            "bind_widget_property: initial push of `{}` to widget {} was refused ({}); the \
             binding is not announced as synced",
            property,
            widget_id,
            error
        ),
        None => log::warn!(
            "bind_widget_property: initial push of `{}` skipped because widget {} is not mounted; \
             the binding is not announced as synced",
            property,
            widget_id
        ),
    }
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

/// Binds one model value to a mounted widget property in **both** directions (BIND-010).
///
/// # What the two directions are
///
/// * model → control: [`bind_widget_property`]'s observer writes every new value through the
///   control's property contract;
/// * control → model: the control's value trigger pulls the property back (the reverse channel
///   [`pull_widget_property`] provides, wired here instead of left to the host).
///
/// # Why an echo guard
///
/// A push makes the control emit its value trigger, and the pull that follows would set the binding
/// again — each direction running forever. The guard is the same atomic re-arm [`Binding::bind_to`]
/// uses: the side that is applying a change marks the pair busy, so the event caused by that change
/// is ignored rather than re-propagated. The flag is cleared by the RAII [`EchoGuard`], not by a
/// trailing `store(false)`, so a panic in a property setter or in a `binding_synced` observer
/// cannot latch it and silently disable both directions for the rest of the binding's life.
///
/// The initial push follows [`bind_widget_property`]: it is announced as synced only when the
/// control actually accepted it.
#[cfg(all(not(alloc_frugal), full_widgets))]
pub fn bind_widget_property_two_way<H>(
    binding: &Binding<crate::widget::capability::CapabilityValue>,
    key: &str,
    widget: &H,
    widget_id: crate::core::ObjectId,
    property: &str,
) where
    H: crate::app::WidgetHandle,
{
    use crate::compat::Arc;
    use core::sync::atomic::{AtomicBool, Ordering};

    let syncing = Arc::new(AtomicBool::new(false));
    let property = crate::compat::String::from(property);

    // model → control
    let observed_property = property.clone();
    let linked = binding.clone();
    let push_guard = Arc::clone(&syncing);
    binding.observe(key, move |value| {
        if push_guard.swap(true, Ordering::SeqCst) {
            // The control's own change is being pulled into the model; do not push it back.
            return;
        }
        let _echo = EchoGuard { flag: &push_guard };
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            crate::widget::widget_property_set(widget, &observed_property, value.clone())
        });
        if matches!(applied, Some(Ok(()))) {
            linked.announce_synced(&observed_property);
        }
    });

    // control → model
    let pulled_property = property.clone();
    let pulled = binding.clone();
    let pull_guard = Arc::clone(&syncing);
    widget.on_value_changed(move |_text: &str| {
        if pull_guard.swap(true, Ordering::SeqCst) {
            return;
        }
        let _echo = EchoGuard { flag: &pull_guard };
        let _ = pull_widget_property(&pulled, widget_id, &pulled_property);
    });

    // The initial value is published like the one-way bridge, so the control never shows its
    // constructor default until the model next changes.
    let initial = binding.get();
    let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        crate::widget::widget_property_set(widget, &property, initial)
    });
    match applied {
        Some(Ok(())) => binding.announce_synced(&property),
        Some(Err(error)) => log::warn!(
            "bind_widget_property_two_way: initial push of `{}` to widget {} was refused ({}); \
             the binding is not announced as synced",
            property,
            widget_id,
            error
        ),
        None => log::warn!(
            "bind_widget_property_two_way: initial push of `{}` skipped because widget {} is not \
             mounted; the binding is not announced as synced",
            property,
            widget_id
        ),
    }
}

/// Clears the two-way echo flag when dropped.
///
/// The push and pull directions share one flag: the side applying a change raises it so the event
/// that change causes is ignored instead of being propagated back. Clearing it with a trailing
/// `store(false)` skips the reset whenever the callback panics — a property setter or a
/// `binding_synced` observer can both unwind — and a latched flag makes every later change look
/// like its own echo, silently stopping the binding in **both** directions. `Drop` runs on the
/// normal path and while unwinding. Mirrors `data_binding::binding::SyncingGuard`.
#[cfg(all(not(alloc_frugal), full_widgets))]
struct EchoGuard<'a> {
    flag: &'a core::sync::atomic::AtomicBool,
}

#[cfg(all(not(alloc_frugal), full_widgets))]
impl Drop for EchoGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, core::sync::atomic::Ordering::SeqCst);
    }
}

/// Binds a model value to a widget property through a validator (BIND-010).
///
/// # Why a hook rather than a caller-side check
///
/// A model can hold a value the control's schema refuses (a range limit, an empty required field).
/// Writing it directly would leave the control and the model disagreeing with no record of why. The
/// validator runs before every write — including the initial one — and a refusal is reported through
/// [`Binding::validation_failed_signal`] instead of reaching the control. A write the control itself
/// refuses (or an id nothing is mounted at) is logged and is not announced as synced, matching the
/// unvalidated bridge.
#[cfg(not(alloc_frugal))]
pub fn bind_widget_property_validated<V>(
    binding: &Binding<crate::widget::capability::CapabilityValue>,
    key: &str,
    widget_id: crate::core::ObjectId,
    property: &str,
    validator: V,
) where
    V: crate::data_binding::Validator<crate::widget::capability::CapabilityValue> + 'static,
{
    let property = crate::compat::String::from(property);
    let observed_property = property.clone();

    // Validate the initial value before the validator moves into the observer.
    match validator.validate(binding.get()) {
        Ok(value) => {
            let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
                crate::widget::widget_property_set(widget, &property, value)
            });
            match applied {
                Some(Ok(())) => binding.announce_synced(&property),
                Some(Err(error)) => log::warn!(
                    "bind_widget_property_validated: initial push of `{}` to widget {} was \
                     refused ({}); the binding is not announced as synced",
                    property,
                    widget_id,
                    error
                ),
                None => log::warn!(
                    "bind_widget_property_validated: initial push of `{}` skipped because widget \
                     {} is not mounted; the binding is not announced as synced",
                    property,
                    widget_id
                ),
            }
        }
        Err(error) => {
            binding.validation_failed_signal().emit(error.message);
        }
    }

    let linked = binding.clone();
    binding.observe(key, move |value| match validator.validate(value.clone()) {
        Ok(value) => {
            let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
                crate::widget::widget_property_set(widget, &observed_property, value)
            });
            if matches!(applied, Some(Ok(()))) {
                linked.announce_synced(&observed_property);
            }
        }
        Err(error) => {
            linked.validation_failed_signal().emit(error.message);
        }
    });
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;

    struct NonEmptyText;

    impl crate::data_binding::Validator<crate::widget::capability::CapabilityValue> for NonEmptyText {
        fn validate(
            &self,
            value: crate::widget::capability::CapabilityValue,
        ) -> Result<crate::widget::capability::CapabilityValue, crate::data_binding::ValidationError>
        {
            match &value {
                crate::widget::capability::CapabilityValue::String(text) if text.is_empty() => {
                    Err(crate::data_binding::ValidationError::new("text must not be empty"))
                }
                _ => Ok(value),
            }
        }
    }

    fn mount_button(text: &str) -> crate::core::ObjectId {
        crate::widget::runtime::register(crate::compat::Box::new(crate::widget::Button::new(
            text.to_string(),
            crate::core::Rect::new(0, 0, 80, 24),
        )))
        .expect("a button mounts")
    }

    fn capability_text(text: &str) -> crate::widget::capability::CapabilityValue {
        crate::widget::capability::CapabilityValue::String(crate::compat::String::from(text))
    }

    fn widget_text(id: crate::core::ObjectId) -> crate::widget::capability::CapabilityValue {
        crate::widget::runtime::with_widget(id, |widget| {
            crate::widget::widget_property_get(widget, "text")
        })
        .expect("the widget is mounted")
        .expect("the control publishes `text`")
    }

    /// BIND-010: a model change reaches the control and a control change returns to the model.
    #[test]
    fn a_two_way_property_binding_syncs_both_directions() {
        let id = mount_button("alpha");
        let binding = Binding::new(capability_text("alpha"));
        let handle = crate::app::ButtonHandle::from_raw(id);
        bind_widget_property_two_way(&binding, "text", &handle, id, "text");

        // model → control
        binding.set(capability_text("beta"));
        assert_eq!(widget_text(id), capability_text("beta"));

        // control → model: write the control's own property, then deliver the value trigger the
        // control would emit for it. The pull reads the live property, so the model follows.
        let _ = crate::widget::runtime::with_widget_mut(id, |widget| {
            crate::widget::widget_property_set(widget, "text", capability_text("gamma"))
        });
        crate::app::dispatch_trigger(id, crate::platform::WidgetTriggerKind::ValueChanged);
        assert_eq!(binding.get(), capability_text("gamma"), "the control change reaches the model");

        crate::app::remove_callbacks(id);
        crate::widget::runtime::unregister(id);
    }

    /// BIND-010: a value the validator refuses is reported and never reaches the control.
    #[test]
    fn a_validated_property_binding_refuses_a_bad_value() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        let id = mount_button("keep");
        let binding = Binding::new(capability_text(""));
        let failures = crate::compat::Arc::new(AtomicUsize::new(0));
        let counter = crate::compat::Arc::clone(&failures);
        binding.validation_failed_signal().connect_ref(move |_message| {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        bind_widget_property_validated(&binding, "text", id, "text", NonEmptyText);
        assert_eq!(failures.load(Ordering::SeqCst), 1, "the invalid initial value is reported");
        assert_eq!(widget_text(id), capability_text("keep"), "a refused value must not apply");

        binding.set(capability_text("valid"));
        assert_eq!(widget_text(id), capability_text("valid"));

        crate::app::remove_callbacks(id);
        crate::widget::runtime::unregister(id);
    }

    /// The RAII echo guard clears the flag while unwinding, so a panic cannot latch it.
    ///
    /// Mirrors `data_binding::binding`'s `syncing_guard_clears_flag_on_panic`: the bare
    /// `store(false)` the two-way bridge used before skipped the reset whenever propagation
    /// panicked, leaving the pair permanently disarmed.
    #[test]
    fn the_echo_guard_clears_the_flag_on_panic() {
        use core::sync::atomic::{AtomicBool, Ordering};

        let flag = AtomicBool::new(false);
        assert!(!flag.swap(true, Ordering::SeqCst), "the flag starts unset");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _echo = EchoGuard { flag: &flag };
            panic!("boom inside the guarded section");
        }));

        assert!(result.is_err(), "the probe must panic");
        assert!(
            !flag.load(Ordering::SeqCst),
            "the guard must clear the echo flag while unwinding, otherwise both directions die"
        );
    }

    /// A panic during propagation must not stop the two-way binding from updating afterwards.
    ///
    /// The `binding_synced` announcement runs at the tail of the push, inside the guarded section,
    /// so unwinding from a host observer hits exactly the path the old trailing `store(false)`
    /// missed. With the flag latched, a later pull would look like the bridge's own echo and a
    /// later push would be dropped; both directions are exercised here.
    #[test]
    fn a_panicking_propagation_does_not_latch_the_two_way_binding() {
        use core::sync::atomic::{AtomicBool, Ordering};

        let id = mount_button("alpha");
        let binding = Binding::new(capability_text("alpha"));
        let handle = crate::app::ButtonHandle::from_raw(id);
        bind_widget_property_two_way(&binding, "text", &handle, id, "text");

        let armed = crate::compat::Arc::new(AtomicBool::new(true));
        let panic_once = crate::compat::Arc::clone(&armed);
        binding.binding_synced_signal().connect_ref(move |_property| {
            if panic_once.swap(false, Ordering::SeqCst) {
                panic!("deliberate unwind from a synced announcement");
            }
        });

        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            binding.set(capability_text("beta"));
        }));
        assert!(caught.is_err(), "the announcement must unwind through the push");
        assert_eq!(
            widget_text(id),
            capability_text("beta"),
            "the push applied before the announcement unwound"
        );

        let _ = crate::widget::runtime::with_widget_mut(id, |widget| {
            crate::widget::widget_property_set(widget, "text", capability_text("gamma"))
        });
        crate::app::dispatch_trigger(id, crate::platform::WidgetTriggerKind::ValueChanged);
        assert_eq!(
            binding.get(),
            capability_text("gamma"),
            "control → model still propagates after the panic"
        );

        binding.set(capability_text("delta"));
        assert_eq!(
            widget_text(id),
            capability_text("delta"),
            "model → control still propagates after the panic"
        );

        crate::app::remove_callbacks(id);
        crate::widget::runtime::unregister(id);
    }

    /// A push that did not reach a control must not be announced as a synchronization.
    ///
    /// The contract documented at `data_binding::chart::bind_chart_series` is that a failed
    /// initial write reports failure instead of promising updates that cannot arrive; the same
    /// applies to both widget bridge forms, which used to call `announce_synced` unconditionally.
    #[test]
    fn a_failed_initial_push_is_not_announced_as_synced() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        // No widget is ever registered under this id.
        let unmounted = u64::MAX;
        let announcements = crate::compat::Arc::new(AtomicUsize::new(0));

        let one_way = Binding::new(capability_text("alpha"));
        let counter = crate::compat::Arc::clone(&announcements);
        one_way.binding_synced_signal().connect_ref(move |_property| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        bind_widget_property(&one_way, "text", unmounted, "text");
        assert_eq!(
            announcements.load(Ordering::SeqCst),
            0,
            "a one-way initial push that did not apply must not claim the binding is synced"
        );

        let two_way = Binding::new(capability_text("alpha"));
        let handle = crate::app::ButtonHandle::from_raw(unmounted);
        bind_widget_property_two_way(&two_way, "text", &handle, unmounted, "text");
        assert_eq!(
            announcements.load(Ordering::SeqCst),
            0,
            "a two-way initial push that did not apply must not claim the binding is synced"
        );

        let validated = Binding::new(capability_text("alpha"));
        let counter = crate::compat::Arc::clone(&announcements);
        validated.binding_synced_signal().connect_ref(move |_property| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        bind_widget_property_validated(&validated, "text", unmounted, "text", NonEmptyText);
        assert_eq!(
            announcements.load(Ordering::SeqCst),
            0,
            "a validated initial push that did not apply must not claim the binding is synced"
        );

        // Positive control: a mounted target is announced exactly once, so the zeros above are
        // not the vacuous result of a signal that never fires.
        let id = mount_button("alpha");
        let mounted = Binding::new(capability_text("alpha"));
        let counter = crate::compat::Arc::clone(&announcements);
        mounted.binding_synced_signal().connect_ref(move |_property| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        bind_widget_property(&mounted, "text", id, "text");
        assert_eq!(
            announcements.load(Ordering::SeqCst),
            1,
            "a successful initial push is announced"
        );

        crate::app::remove_callbacks(unmounted);
        crate::app::remove_callbacks(id);
        crate::widget::runtime::unregister(id);
    }
}
